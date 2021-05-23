use bevy::{prelude::*, wgpu::RenderStage};

pub mod camera;
pub(crate) mod nodes;
pub(crate) mod render_hook_systems;
pub(crate) mod xr_render_graph;

pub(crate) use render_hook_systems::*;
pub(crate) use xr_render_graph::*;

pub struct OpenXRWgpuPlugin;

impl Plugin for OpenXRWgpuPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_startup_system(add_xr_render_graph.system())
            .add_system_to_stage(
                RenderStage::Draw,
                pre_render_system.exclusive_system(), // FIXME there should maybe be some ImmediatelyBeforeRender system
            )
            .add_system_to_stage(
                RenderStage::PostRender,
                post_render_system.exclusive_system(), // FIXME there should maybe be some ImmediatelyAfterPost system
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                camera::system::openxr_camera_system.system(),
            );
    }
}
