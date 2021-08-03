use bevy::{
    prelude::*,
    render::camera::{Camera, CameraProjection},
};
use bevy_openxr_core::{event, math::XRMatrixComputation};

use super::projection::XRProjection;

pub(crate) fn openxr_camera_system(
    mut camera_query: Query<(&mut Camera, &mut XRProjection, &mut Transform)>,
    mut view_surface_created_events: EventReader<event::XRViewSurfaceCreated>,
    mut views_created_events: EventReader<event::XRViewsCreated>,
    mut camera_transforms_updated: EventReader<event::XRCameraTransformsUpdated>,
) {
    // FIXME: remove
    for event in view_surface_created_events.iter() {
        for (_, mut camera_projection, _) in camera_query.iter_mut() {
            // this is actually unnecessary?
            camera_projection.update(event.width as f32, event.height as f32);
        }
    }

    // initialize projection matrices on view creation
    for event in views_created_events.iter() {
        for (mut camera, mut camera_projection, _) in camera_query.iter_mut() {
            camera.depth_calculation = camera_projection.depth_calculation();
            camera.projection_matrices = event
                .views
                .iter()
                .map(|view| camera_projection.get_projection_matrix_fov(&view.fov))
                .collect::<Vec<_>>();
        }
    }

    for event in camera_transforms_updated.iter() {
        for (mut camera, _, mut transform) in camera_query.iter_mut() {
            if event.transforms.len() > 0 {
                // FIXME: get an average of cameras?
                *transform = event.transforms[0];
            }

            camera.position_matrices = event
                .transforms
                .iter()
                .map(|transform| transform.compute_xr_matrix())
                .collect::<Vec<_>>();
        }
    }
}
