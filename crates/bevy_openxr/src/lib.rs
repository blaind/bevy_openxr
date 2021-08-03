use bevy::app::{App, Plugin, ScheduleRunnerPlugin, ScheduleRunnerSettings};
use bevy::ecs::prelude::*;

pub mod prelude {
    pub use crate::{
        render_graph::camera::{camera::XRCameraBundle, projection::XRProjection},
        HandPoseEvent, OpenXRPlugin, OpenXRSettings,
    };

    pub use openxr::HandJointLocations;
}

use bevy::utils::tracing::warn;
use bevy::wgpu::{WgpuBackend, WgpuOptions};
use bevy::window::{CreateWindow, Window, WindowId, Windows};
use openxr::HandJointLocations;

mod error;
mod hand_tracking;
mod platform;

mod render_graph;

pub use hand_tracking::*;
pub use render_graph::OpenXRWgpuPlugin;

#[derive(Default)]
pub struct OpenXRPlugin;

#[derive(Debug)]
pub struct OpenXRSettings {}

impl Default for OpenXRSettings {
    fn default() -> Self {
        OpenXRSettings {}
    }
}

impl Plugin for OpenXRPlugin {
    fn build(&self, app: &mut App) {
        {
            let settings = app.world.insert_resource(OpenXRSettings::default());

            println!("Settings: {:?}", settings);
        };

        // must be initialized at startup, so that bevy_wgpu has access
        platform::initialize_openxr();

        let mut wgpu_options = app
            .world
            .get_resource::<WgpuOptions>()
            .cloned()
            .unwrap_or_else(WgpuOptions::default);

        // force to Vulkan
        wgpu_options.backend = WgpuBackend::Vulkan;
        warn!("Set WgpuBackend to WgpuBackend::Vulkan (only one supported for OpenXR currently)");

        app
            // FIXME should handposeevent be conditional based on options
            .insert_resource(wgpu_options)
            .insert_resource(ScheduleRunnerSettings::run_loop(
                std::time::Duration::from_micros(0),
            ))
            .add_plugin(ScheduleRunnerPlugin::default())
            .add_event::<HandPoseEvent>()
            .add_system(handle_create_window_events.system());
    }
}

pub struct HandPoseEvent {
    pub left: Option<HandJointLocations>,
    pub right: Option<HandJointLocations>,
}

impl std::fmt::Debug for HandPoseEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(left: {}, right: {})",
            self.left.is_some(),
            self.right.is_some()
        )
    }
}

fn handle_create_window_events(
    mut windows: ResMut<Windows>,
    mut create_window_events: EventReader<CreateWindow>,
    // mut window_created_events: EventWriter<WindowCreated>,
) {
    for _create_window_event in create_window_events.iter() {
        if let None = windows.get_primary() {
            windows.add(Window::new(
                WindowId::primary(),
                &Default::default(),
                896,
                1008,
                1.,
                None,
            ));
        }

        /*
        window_created_events.send(WindowCreated {
            id: create_window_event.id,
        });
         */
    }
}
