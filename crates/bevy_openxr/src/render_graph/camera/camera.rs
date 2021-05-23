use bevy::{
    prelude::*,
    render::{
        camera::{Camera, VisibleEntities},
        render_graph::base::camera::CAMERA_3D,
    },
};

use super::projection::XRProjection;

#[derive(Bundle)]
pub struct XRCameraBundle {
    pub camera: Camera,
    pub xr_projection: XRProjection,
    pub visible_entities: VisibleEntities,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

impl Default for XRCameraBundle {
    fn default() -> Self {
        XRCameraBundle {
            camera: Camera {
                name: Some(CAMERA_3D.to_string()),
                ..Default::default()
            },
            // FIXME: ..Default::default() here causes stack overflow? Wut?
            xr_projection: Default::default(),
            visible_entities: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
        }
    }
}
