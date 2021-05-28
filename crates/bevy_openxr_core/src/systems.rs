use bevy::app::{EventWriter, Events};
use bevy::ecs::system::ResMut;

use crate::XRConfigurationState;
use crate::{
    event::{XRCameraTransformsUpdated, XREvent, XRState, XRViewSurfaceCreated, XRViewsCreated},
    hand_tracking::HandPoseState,
    XRDevice,
};

pub(crate) fn openxr_event_system(
    mut openxr: ResMut<XRDevice>,
    mut hand_pose: ResMut<HandPoseState>,
    mut state_events: ResMut<Events<XRState>>,
    mut configuration_state: ResMut<XRConfigurationState>,

    mut view_surface_created_sender: EventWriter<XRViewSurfaceCreated>,
    mut views_created_sender: EventWriter<XRViewsCreated>,
    mut camera_transforms_updated: EventWriter<XRCameraTransformsUpdated>,
) {
    // TODO add this drain -system as pre-render and post-render system?
    for event in openxr.drain_events() {
        match event {
            XREvent::ViewSurfaceCreated(view_created) => {
                configuration_state.last_view_surface = Some(view_created.clone());
                view_surface_created_sender.send(view_created);
            }
            XREvent::ViewsCreated(views) => views_created_sender.send(views),
        }
    }

    // This should be before all other events
    match openxr.inner.handle_openxr_events() {
        None => (),
        Some(changed_state) => {
            // FIXME handle XRState::Exiting
            state_events.send(changed_state);
        }
    }

    // FIXME: this should happen just before bevy render graph and / or wgpu render?
    openxr.touch_update();

    // FIXME this should be in before-other-systems system? so that all systems can use hand pose data...
    if let Some(hp) = openxr.get_hand_positions() {
        *hand_pose = hp;
    }

    if let Some(transforms) = openxr.get_view_positions() {
        camera_transforms_updated.send(XRCameraTransformsUpdated { transforms });
    }
}
