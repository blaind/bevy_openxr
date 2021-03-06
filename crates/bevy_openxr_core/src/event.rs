use bevy::transform::components::Transform;

use crate::View;

#[derive(Debug)]
pub(crate) enum XREvent {
    ViewSurfaceCreated(XRViewSurfaceCreated),
    ViewsCreated(XRViewsCreated),
}

/// Current state of XR hardware/session
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum XRState {
    Paused,
    Running,
    RunningFocused,
    Exiting,
    SkipFrame,
}

/// XR View has been configured/created
#[derive(Debug, PartialEq, Clone)]
pub struct XRViewSurfaceCreated {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct XRViewsCreated {
    pub views: Vec<View>,
}

#[derive(Debug)]
pub struct XRCameraTransformsUpdated {
    pub transforms: Vec<Transform>,
}
