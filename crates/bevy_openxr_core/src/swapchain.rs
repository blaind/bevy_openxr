use bevy::math::{Quat, Vec3};
use bevy::prelude::error;
use bevy::transform::components::Transform;
use bevy::utils::tracing::{debug, warn};
use openxr::{Time, View};
use std::{fmt::Debug, num::NonZeroU32, sync::Arc};
use wgpu::OpenXRHandles;

use crate::{
    hand_tracking::{HandPoseState, HandTrackers},
    OpenXRStruct, XRState,
};

pub struct XRSwapchain {
    /// OpenXR internal swapchain handle
    sc_handle: openxr::Swapchain<openxr::Vulkan>,

    /// Swapchain Framebuffers. `XRSwapchainNode` will take ownership of the color buffer
    buffers: Vec<Framebuffer>,

    /// Swapchain resolution
    resolution: wgpu::Extent3d,

    /// Swapchain view configuration type
    view_configuration_type: openxr::ViewConfigurationType,

    /// Desired environment blend mode
    environment_blend_mode: openxr::EnvironmentBlendMode,

    /// Rendering and prediction information for the next frame
    next_frame_state: Option<openxr::FrameState>,

    /// TODO: move this away, doesn't belong here
    hand_trackers: Option<HandTrackers>,

    waited: bool,
}

const VIEW_COUNT: u32 = 2; // FIXME get from settings

impl XRSwapchain {
    pub fn new(device: Arc<wgpu::Device>, openxr_struct: &mut OpenXRStruct) -> Self {
        let views = openxr_struct
            .instance
            .enumerate_view_configuration_views(
                openxr_struct.handles.system,
                openxr_struct.options.view_type,
            )
            .unwrap();

        assert_eq!(views.len(), VIEW_COUNT as usize);
        assert_eq!(views[0], views[1]);

        println!("Enumerated OpenXR views: {:#?}", views);

        let resolution = wgpu::Extent3d {
            width: views[0].recommended_image_rect_width,
            height: views[0].recommended_image_rect_height,
            depth_or_array_layers: 1,
        };

        let swapchain_formats = openxr_struct
            .handles
            .session
            .enumerate_swapchain_formats()
            .unwrap();

        let vk_swapchain_formats = swapchain_formats
            .iter()
            .map(|f| ash::vk::Format::from_raw(*f as i32))
            .collect::<Vec<_>>();

        let vk_wgpu_formats = vk_swapchain_formats
            .iter()
            .map(|&vk_format| {
                let hal_format: Option<gfx_hal::format::Format> = map_vk_format(vk_format);
                let wgpu_format = match hal_format {
                    Some(hal_format) => map_texture_format(hal_format),
                    None => None,
                };

                (vk_format, hal_format, wgpu_format)
            })
            .collect::<Vec<_>>();

        println!("OpenXR supported swapchain formats:");
        for (idx, (vk, hal, wgpu)) in vk_wgpu_formats.iter().enumerate() {
            println!(
                "   idx={}, vk={:?} gfx_hal={:?} wgpu={:?}",
                idx, vk, hal, wgpu
            );
        }

        let format = vk_wgpu_formats
            .iter()
            .enumerate()
            .find(|(_, (_, hal, wgpu))| hal.is_some() && wgpu.is_some())
            .map(|(idx, (vk, hal, wgpu))| (idx, vk, hal.unwrap(), wgpu.unwrap()));

        let (format_idx, vk_format, _hal_format, format) = match format {
            Some(f) => f,
            None => {
                panic!(
                    "OpenXR did not have any supported swapchain formats available. Can not continue"
                );
            }
        };

        println!(
            "Selected swapchain format: idx={} vk={:?} wgpu={:?}",
            format_idx, vk_format, format
        );

        let handle = openxr_struct
            .handles
            .session
            .create_swapchain(&openxr::SwapchainCreateInfo {
                create_flags: openxr::SwapchainCreateFlags::EMPTY,
                usage_flags: openxr::SwapchainUsageFlags::COLOR_ATTACHMENT,
                format: vk_format.as_raw() as _,
                sample_count: 1,
                width: resolution.width,
                height: resolution.height,
                face_count: 1,
                array_size: VIEW_COUNT,
                mip_count: 1,
            })
            .unwrap();

        let environment_blend_mode = openxr_struct
            .instance
            .enumerate_environment_blend_modes(
                openxr_struct.handles.system,
                openxr_struct.options.view_type,
            )
            .unwrap()[0];

        let images = handle.enumerate_images().unwrap();

        let buffers = images
            .into_iter()
            .map(|color_image| {
                // FIXME keep in sync with above usage_flags
                let texture = device.create_openxr_texture_from_raw_image(
                    &wgpu::TextureDescriptor {
                        size: wgpu::Extent3d {
                            width: resolution.width,
                            height: resolution.height,
                            depth_or_array_layers: 2,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
                        label: None,
                    },
                    color_image,
                );

                let color = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: Some(format),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: NonZeroU32::new(1),
                    base_array_layer: 0,
                    array_layer_count: NonZeroU32::new(2),
                });

                Framebuffer {
                    texture,
                    texture_view: Some(color),
                }
            })
            .collect();

        let hand_trackers = if openxr_struct.options.hand_trackers {
            // FIXME check feature
            Some(HandTrackers::new(&openxr_struct.handles.session).unwrap())
        } else {
            None
        };

        XRSwapchain {
            sc_handle: handle,
            buffers,
            resolution,
            view_configuration_type: openxr_struct.options.view_type,
            environment_blend_mode,
            next_frame_state: None,
            hand_trackers,
            waited: false,
        }
    }

    /// Return the next swapchain image index to render into
    /// FIXME: currently waits for compositor to release image for rendering, this might cause delays in bevy system
    ///        (e.g. should wait somewhere else - but how to use handle there)
    pub fn get_next_swapchain_image_index(&mut self) -> usize {
        let image_index = self.sc_handle.acquire_image().unwrap();
        self.sc_handle
            .wait_image(openxr::Duration::INFINITE)
            .unwrap();
        self.waited = true;
        image_index as usize
    }

    /// Prepares the device for rendering. Called before each frame is rendered
    pub fn prepare_update(&mut self, handles: &mut OpenXRHandles) -> XRState {
        // Check that previous frame was rendered
        if let Some(_) = self.next_frame_state {
            debug!("Called prepare_update() even though it was called already");
            return XRState::Running; // <-- FIXME might change state, should keep it in memory somewhere
        }

        let frame_state = match handles.frame_waiter.wait() {
            Ok(fs) => fs,
            Err(_) => {
                // FIXME handle this better
                return XRState::Paused;
            }
        };

        // 'Indicate that graphics device work is beginning'
        handles.frame_stream.begin().unwrap();

        if !frame_state.should_render {
            // if false, "the application should avoid heavy GPU work where possible" (openxr spec)
            handles
                .frame_stream
                .end(
                    frame_state.predicted_display_time,
                    self.environment_blend_mode,
                    &[],
                )
                .unwrap();

            return XRState::Paused;
        }

        // All ok for rendering
        self.next_frame_state = Some(frame_state);
        return XRState::Running;
    }

    /// TODO: move this away, doesn't belong here
    pub fn get_hand_positions(&mut self, handles: &mut OpenXRHandles) -> Option<HandPoseState> {
        let frame_state = match self.next_frame_state {
            Some(fs) => fs,
            None => return None,
        };

        let ht = match &self.hand_trackers {
            Some(ht) => ht,
            None => return None,
        };

        let hand_l = handles
            .space
            .locate_hand_joints(&ht.tracker_l, frame_state.predicted_display_time)
            .unwrap();
        let hand_r = handles
            .space
            .locate_hand_joints(&ht.tracker_r, frame_state.predicted_display_time)
            .unwrap();

        let hand_pose_state = HandPoseState {
            left: hand_l,
            right: hand_r,
        };

        Some(hand_pose_state)
    }

    pub fn get_view_positions(&mut self, handles: &mut OpenXRHandles) -> Option<Vec<Transform>> {
        if let None = self.next_frame_state {
            return None;
        }

        let frame_state = self.next_frame_state.as_ref().unwrap();

        // FIXME views acquisition should probably occur somewhere else - timing problem?
        let (_, views) = handles
            .session
            .locate_views(
                self.view_configuration_type,
                frame_state.predicted_display_time,
                &handles.space,
            )
            .unwrap();

        //println!("VIEWS: {:#?}", views);

        let transforms = views
            .iter()
            .map(|view| {
                let pos = &view.pose.position;
                let ori = &view.pose.orientation;
                let mut transform = Transform::from_translation(Vec3::new(pos.x, pos.y, pos.z));
                transform.rotation = Quat::from_xyzw(ori.x, ori.y, ori.z, ori.w);
                transform
            })
            .collect();

        //println!("TRANSFORMS: {:#?}", transforms);
        Some(transforms)
    }

    /// Finalizes the swapchain update - will tell openxr that GPU has rendered to textures
    pub fn finalize_update(&mut self, handles: &mut OpenXRHandles) {
        // Take the next frame state
        let next_frame_state = match self.next_frame_state.take() {
            Some(nfst) => nfst,
            None => {
                warn!("NO NEXT FRAME");
                return;
            }
        };

        if !self.waited {
            return;
        }

        // "Release the oldest acquired image"
        self.sc_handle.release_image().unwrap();
        self.waited = false;

        // FIXME views acquisition should probably occur somewhere else - timing problem?
        // FIXME is there a problem now, if the rendering uses different camera positions than what's used at openxr?
        // "When rendering, this should be called as late as possible before the GPU accesses it to"
        let (_, views) = handles
            .session
            .locate_views(
                self.view_configuration_type,
                next_frame_state.predicted_display_time,
                &handles.space,
            )
            .unwrap();

        // Tell OpenXR what to present for this frame
        // Because we're using GL_EXT_multiview, same rect for both eyes
        let rect = openxr::Rect2Di {
            offset: openxr::Offset2Di { x: 0, y: 0 },
            extent: openxr::Extent2Di {
                width: self.resolution.width as _,
                height: self.resolution.height as _,
            },
        };

        // Construct views
        // TODO: for performance (no-vec allocations), use `SmallVec`?
        let views = views
            .iter()
            .enumerate()
            .map(|(idx, view)| {
                openxr::CompositionLayerProjectionView::new()
                    .pose(view.pose)
                    .fov(view.fov)
                    .sub_image(
                        openxr::SwapchainSubImage::new()
                            .swapchain(&self.sc_handle)
                            .image_array_index(idx as u32)
                            .image_rect(rect),
                    )
            })
            .collect::<Vec<_>>();

        handles
            .frame_stream
            .end(
                next_frame_state.predicted_display_time,
                self.environment_blend_mode,
                &[&openxr::CompositionLayerProjection::new()
                    .space(&handles.space)
                    .views(&views)],
            )
            .unwrap();
    }

    /// Should be called only once by `XRSwapchainNode`
    pub fn take_texture_views(&mut self) -> Vec<wgpu::TextureView> {
        self.buffers
            .iter_mut()
            .map(|buf| buf.texture_view.take().unwrap())
            .collect()
    }

    pub fn get_resolution(&self) -> (u32, u32) {
        (self.resolution.width, self.resolution.height)
    }

    pub fn get_views(&self, handles: &mut OpenXRHandles) -> Vec<View> {
        let (_, views) = handles
            .session
            .locate_views(
                self.view_configuration_type,
                Time::from_nanos(1), // FIXME time must be non-zero, is this okay?
                &handles.space,
            )
            .unwrap();

        views
    }
}

impl Debug for XRSwapchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XRSwapchain[]")
    }
}

/// Per view framebuffer, that will contain an underlying texture and a texture view (taken away by bevy render graph)
/// where the contents should be rendered
struct Framebuffer {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    texture_view: Option<wgpu::TextureView>,
}

// TODO: this is based on gfx_backend_vulkan/conv.rs, can it be used directly?
pub fn map_vk_format(vk_format: ash::vk::Format) -> Option<gfx_hal::format::Format> {
    if (vk_format.as_raw() as usize) < gfx_hal::format::NUM_FORMATS
        && vk_format != ash::vk::Format::UNDEFINED
    {
        Some(unsafe { std::mem::transmute(vk_format) })
    } else {
        None
    }
}

// TODO: this is just a reverse map based on wgpu/wgpu-core/src/conv.rs: map_texture_format (from wgpu to hal)
// maybe pull request to wgpu to abstract away?
pub(crate) fn map_texture_format(
    hal_format: gfx_hal::format::Format,
) -> Option<wgpu::TextureFormat> {
    use gfx_hal::format::Format as H;
    use wgpu::TextureFormat as Tf;
    Some(match hal_format {
        // Normal 8 bit formats
        H::R8Unorm => Tf::R8Unorm,
        H::R8Snorm => Tf::R8Snorm,
        H::R8Uint => Tf::R8Uint,
        H::R8Sint => Tf::R8Sint,

        // Normal 16 bit formats
        H::R16Uint => Tf::R16Uint,
        H::R16Sint => Tf::R16Sint,
        H::R16Sfloat => Tf::R16Float,
        H::Rg8Unorm => Tf::Rg8Unorm,
        H::Rg8Snorm => Tf::Rg8Snorm,
        H::Rg8Uint => Tf::Rg8Uint,
        H::Rg8Sint => Tf::Rg8Sint,

        // Normal 32 bit formats
        H::R32Uint => Tf::R32Uint,
        H::R32Sint => Tf::R32Sint,
        H::R32Sfloat => Tf::R32Float,
        H::Rg16Uint => Tf::Rg16Uint,
        H::Rg16Sint => Tf::Rg16Sint,
        H::Rg16Sfloat => Tf::Rg16Float,
        H::Rgba8Unorm => Tf::Rgba8Unorm,
        H::Rgba8Srgb => Tf::Rgba8UnormSrgb,
        H::Rgba8Snorm => Tf::Rgba8Snorm,
        H::Rgba8Uint => Tf::Rgba8Uint,
        H::Rgba8Sint => Tf::Rgba8Sint,
        H::Bgra8Unorm => Tf::Bgra8Unorm,
        H::Bgra8Srgb => Tf::Bgra8UnormSrgb,

        // Packed 32 bit formats
        H::A2r10g10b10Unorm => Tf::Rgb10a2Unorm,
        H::B10g11r11Ufloat => Tf::Rg11b10Float,

        // Normal 64 bit formats
        H::Rg32Uint => Tf::Rg32Uint,
        H::Rg32Sint => Tf::Rg32Sint,
        H::Rg32Sfloat => Tf::Rg32Float,
        H::Rgba16Uint => Tf::Rgba16Uint,
        H::Rgba16Sint => Tf::Rgba16Sint,
        H::Rgba16Sfloat => Tf::Rgba16Float,

        // Normal 128 bit formats
        H::Rgba32Uint => Tf::Rgba32Uint,
        H::Rgba32Sint => Tf::Rgba32Sint,
        H::Rgba32Sfloat => Tf::Rgba32Float,

        // Depth and stencil formats
        H::D32Sfloat => Tf::Depth32Float,

        // FIXME: check that these are really allright
        H::X8D24Unorm => Tf::Depth24Plus,
        //H::D32Sfloat => Tf::Depth24Plus, // double, above also
        H::D24UnormS8Uint => Tf::Depth24Plus,
        H::D32SfloatS8Uint => Tf::Depth24Plus,
        /* original wgpu->hal conversion
        Tf::Depth24Plus => {
            if private_features.texture_d24 {
                H::X8D24Unorm
            } else {
                H::D32Sfloat
            }
        }
        Tf::Depth24PlusStencil8 => {
            if private_features.texture_d24_s8 {
                H::D24UnormS8Uint
            } else {
                H::D32SfloatS8Uint
            }
        }
         */
        // BCn compressed formats
        H::Bc1RgbaUnorm => Tf::Bc1RgbaUnorm,
        H::Bc1RgbaSrgb => Tf::Bc1RgbaUnormSrgb,
        H::Bc2Unorm => Tf::Bc2RgbaUnorm,
        H::Bc2Srgb => Tf::Bc2RgbaUnormSrgb,
        H::Bc3Unorm => Tf::Bc3RgbaUnorm,
        H::Bc3Srgb => Tf::Bc3RgbaUnormSrgb,
        H::Bc4Unorm => Tf::Bc4RUnorm,
        H::Bc4Snorm => Tf::Bc4RSnorm,
        H::Bc5Unorm => Tf::Bc5RgUnorm,
        H::Bc5Snorm => Tf::Bc5RgSnorm,
        H::Bc6hSfloat => Tf::Bc6hRgbSfloat,
        H::Bc6hUfloat => Tf::Bc6hRgbUfloat,
        H::Bc7Unorm => Tf::Bc7RgbaUnorm,
        H::Bc7Srgb => Tf::Bc7RgbaUnormSrgb,

        // ETC compressed formats
        H::Etc2R8g8b8Unorm => Tf::Etc2RgbUnorm,
        H::Etc2R8g8b8Srgb => Tf::Etc2RgbUnormSrgb,
        H::Etc2R8g8b8a1Unorm => Tf::Etc2RgbA1Unorm,
        H::Etc2R8g8b8a1Srgb => Tf::Etc2RgbA1UnormSrgb,
        H::Etc2R8g8b8a8Unorm => Tf::Etc2RgbA8Unorm,
        //H::Etc2R8g8b8a8Unorm => Tf::Etc2RgbA8UnormSrgb, FIXME ok?
        H::EacR11Unorm => Tf::EacRUnorm,
        H::EacR11Snorm => Tf::EacRSnorm,
        H::EacR11g11Unorm => Tf::EtcRgUnorm,
        H::EacR11g11Snorm => Tf::EtcRgSnorm,

        // ASTC compressed formats
        //H::Astc4x4Srgb => Tf::Astc4x4RgbaUnorm, FIXME ok?
        H::Astc4x4Srgb => Tf::Astc4x4RgbaUnormSrgb,
        H::Astc5x4Unorm => Tf::Astc5x4RgbaUnorm,
        H::Astc5x4Srgb => Tf::Astc5x4RgbaUnormSrgb,
        H::Astc5x5Unorm => Tf::Astc5x5RgbaUnorm,
        H::Astc5x5Srgb => Tf::Astc5x5RgbaUnormSrgb,
        H::Astc6x5Unorm => Tf::Astc6x5RgbaUnorm,
        H::Astc6x5Srgb => Tf::Astc6x5RgbaUnormSrgb,
        H::Astc6x6Unorm => Tf::Astc6x6RgbaUnorm,
        H::Astc6x6Srgb => Tf::Astc6x6RgbaUnormSrgb,
        H::Astc8x5Unorm => Tf::Astc8x5RgbaUnorm,
        H::Astc8x5Srgb => Tf::Astc8x5RgbaUnormSrgb,
        H::Astc8x6Unorm => Tf::Astc8x6RgbaUnorm,
        H::Astc8x6Srgb => Tf::Astc8x6RgbaUnormSrgb,
        H::Astc10x5Unorm => Tf::Astc10x5RgbaUnorm,
        H::Astc10x5Srgb => Tf::Astc10x5RgbaUnormSrgb,
        H::Astc10x6Unorm => Tf::Astc10x6RgbaUnorm,
        H::Astc10x6Srgb => Tf::Astc10x6RgbaUnormSrgb,
        H::Astc8x8Unorm => Tf::Astc8x8RgbaUnorm,
        H::Astc8x8Srgb => Tf::Astc8x8RgbaUnormSrgb,
        H::Astc10x8Unorm => Tf::Astc10x8RgbaUnorm,
        H::Astc10x8Srgb => Tf::Astc10x8RgbaUnormSrgb,
        H::Astc10x10Unorm => Tf::Astc10x10RgbaUnorm,
        H::Astc10x10Srgb => Tf::Astc10x10RgbaUnormSrgb,
        H::Astc12x10Unorm => Tf::Astc12x10RgbaUnorm,
        H::Astc12x10Srgb => Tf::Astc12x10RgbaUnormSrgb,
        H::Astc12x12Unorm => Tf::Astc12x12RgbaUnorm,
        H::Astc12x12Srgb => Tf::Astc12x12RgbaUnormSrgb,
        _ => {
            error!("Could not map hal format {:?} to wgpu format", hal_format);
            return None;
        }
    })
}
