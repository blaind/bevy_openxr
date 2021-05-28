use bevy::{prelude::*, render::renderer::TextureId};
use bevy_openxr_core::{event::XRState, XRConfigurationState, XRDevice};

pub(crate) fn pre_render_system(
    mut xr_device: ResMut<XRDevice>,
    wgpu_handles: ResMut<bevy::wgpu::WgpuRendererHandles>,
    mut wgpu_render_state: ResMut<bevy::wgpu::WgpuRenderState>,
    mut xr_configuration_state: ResMut<XRConfigurationState>,
) {
    let (state, texture_views) = xr_device.prepare_update(&wgpu_handles.device);

    let should_render = if let XRState::Running = state {
        true
    } else {
        false
    };

    if let Some(texture_views) = texture_views {
        wgpu_render_state.add_textures = texture_views
            .into_iter()
            .map(|texture_view| bevy::wgpu::TextureView {
                id: TextureId::new(),
                texture_view,
            })
            .collect();

        // FIXME: move this to event (but can't use in bevy_wgpu since must be writable event)
        xr_configuration_state.texture_view_ids = Some(
            wgpu_render_state
                .add_textures
                .iter()
                .map(|tv| tv.id)
                .collect(),
        );
    }

    if should_render {
        xr_configuration_state.next_swap_chain_index = xr_device
            .get_swapchain_mut()
            .unwrap()
            .get_next_swapchain_image_index();
    }

    wgpu_render_state.should_render = should_render;
}

pub(crate) fn post_render_system(mut xr_device: ResMut<XRDevice>) {
    xr_device.finalize_update();
}
