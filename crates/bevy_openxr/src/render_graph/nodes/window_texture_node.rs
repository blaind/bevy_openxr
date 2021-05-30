use bevy::ecs::world::World;
use bevy::render::{
    render_graph::{Node, ResourceSlotInfo, ResourceSlots, WindowTextureNode},
    renderer::{RenderContext, RenderResourceId, RenderResourceType},
    texture::TextureDescriptor,
};
use bevy_openxr_core::event::XRViewSurfaceCreated;
use bevy_openxr_core::XRConfigurationState;
use std::borrow::Cow;

/// MAIN_SAMPLED_COLOR_ATTACHMENT node in OpenXR implementation, used instead of `WindowTextureNode`
/// otherwise matches `WindowTextureNode`, except the descriptor.size (`Extent3d`) is set from XR viewport events
pub struct XRWindowTextureNode {
    descriptor: TextureDescriptor,
    last_view_surface: Option<XRViewSurfaceCreated>,
}

impl XRWindowTextureNode {
    pub fn new(descriptor: TextureDescriptor) -> Self {
        XRWindowTextureNode {
            descriptor,
            last_view_surface: None,
        }
    }
}

impl Node for XRWindowTextureNode {
    fn output(&self) -> &[ResourceSlotInfo] {
        static OUTPUT: &[ResourceSlotInfo] = &[ResourceSlotInfo {
            name: Cow::Borrowed(WindowTextureNode::OUT_TEXTURE),
            resource_type: RenderResourceType::Texture,
        }];
        OUTPUT
    }

    fn update(
        &mut self,
        world: &World,
        render_context: &mut dyn RenderContext,
        _input: &ResourceSlots,
        output: &mut ResourceSlots,
    ) {
        const WINDOW_TEXTURE: usize = 0;

        // TODO performance use Change detection? (takes ~10 microseconds now, not too bad)
        let render_state = world.get_resource::<XRConfigurationState>().unwrap(); // can't be an event, as this doesn't run when event is sent

        if render_state.last_view_surface != self.last_view_surface {
            if let Some(last_view_surface) = &render_state.last_view_surface {
                // Configure texture size. This usually happens only at the start of openxr session
                let render_resource_context = render_context.resources_mut();
                if let Some(RenderResourceId::Texture(old_texture)) = output.get(WINDOW_TEXTURE) {
                    render_resource_context.remove_texture(old_texture);
                }

                self.descriptor.size.width = last_view_surface.width;
                self.descriptor.size.height = last_view_surface.height;

                // using GL multiview, two eyes - FIXME: eventually set the depth based on view count from event data
                self.descriptor.size.depth_or_array_layers = 2;

                let texture_resource = render_resource_context.create_texture(self.descriptor);
                output.set(WINDOW_TEXTURE, RenderResourceId::Texture(texture_resource));

                self.last_view_surface = Some(last_view_surface.clone());
            }
        }
    }
}
