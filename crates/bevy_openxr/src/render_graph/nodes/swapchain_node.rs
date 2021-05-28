use std::borrow::Cow;

use bevy::{
    ecs::world::World,
    render::{
        render_graph::{Node, ResourceSlotInfo, ResourceSlots},
        renderer::{RenderContext, RenderResourceId, RenderResourceType},
    },
};

use bevy_openxr_core::XRConfigurationState;

/// Like `WindowSwapChainNode`, but for XR implementation
/// XR implementation initializes the underlying textures at the startup, and after that
/// this node will swap the textures based on texture id retrieved from XR swapchain
#[derive(Default)]
pub struct XRSwapchainNode {
    resource_ids: Option<Vec<RenderResourceId>>,
}

impl XRSwapchainNode {
    pub const OUT_TEXTURE: &'static str = "texture";

    pub fn new() -> Self {
        XRSwapchainNode::default()
    }
}

impl Node for XRSwapchainNode {
    fn output(&self) -> &[ResourceSlotInfo] {
        static OUTPUT: &[ResourceSlotInfo] = &[ResourceSlotInfo {
            name: Cow::Borrowed(XRSwapchainNode::OUT_TEXTURE),
            resource_type: RenderResourceType::Texture,
        }];
        OUTPUT
    }

    fn update(
        &mut self,
        world: &World,
        _render_context: &mut dyn RenderContext,
        _input: &ResourceSlots,
        output: &mut ResourceSlots,
    ) {
        const WINDOW_TEXTURE: usize = 0;
        let xr_configuration_state = world.get_resource::<XRConfigurationState>().unwrap();

        let render_state = world.get_resource::<XRConfigurationState>().unwrap();

        let resource_ids = match &self.resource_ids {
            Some(resource_ids) => resource_ids,
            None => {
                if let Some(texture_view_ids) = &render_state.texture_view_ids {
                    self.resource_ids = Some(
                        texture_view_ids
                            .iter()
                            .map(|id| RenderResourceId::Texture(*id))
                            .collect(),
                    );
                    self.resource_ids.as_ref().unwrap()
                } else {
                    return;
                }
            }
        };

        // get next texture by id
        let render_resource_id = resource_ids
            .get(xr_configuration_state.next_swap_chain_index)
            .unwrap();

        // set output to desired resource id
        output.set(WINDOW_TEXTURE, render_resource_id.clone());
    }
}
