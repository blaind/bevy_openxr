use bevy::app::{prelude::*, EventReader};
use bevy::ecs::system::IntoSystem;

mod device;
pub mod event;
pub mod hand_tracking;

#[cfg(target_os = "android")]
mod keyboard;

pub mod math;
mod runner;
mod swapchain;
mod systems;
mod xr_instance;

use bevy::render::renderer::TextureId;
use bevy::utils::tracing::debug;
pub use device::*;
use event::{XRState, XRViewSurfaceCreated};
pub use swapchain::*;
use systems::*;
pub use xr_instance::{set_xr_instance, XrInstance};

#[derive(Default)]
pub struct OpenXRCorePlugin;

impl Plugin for OpenXRCorePlugin {
    fn build(&self, app: &mut App) {
        debug!("Building OpenXRCorePlugin");
        let xr_instance = xr_instance::take_xr_instance();
        let options = XrOptions::default(); // FIXME user configurable?
        let (xr_device, wgpu_openxr) = xr_instance.into_device_with_options(options);

        app.insert_resource(xr_device)
            .add_event::<event::XRState>()
            .add_event::<event::XRViewSurfaceCreated>()
            .add_event::<event::XRViewsCreated>()
            .add_event::<event::XRCameraTransformsUpdated>()
            .init_resource::<XRConfigurationState>()
            .init_resource::<hand_tracking::HandPoseState>()
            .insert_resource(wgpu_openxr)
            .add_system_to_stage(CoreStage::PreUpdate, openxr_event_system.system())
            .add_system(xr_event_debug.system())
            .set_runner(runner::xr_runner); // FIXME conditional, or extract xr_events to whole new system? probably good

        #[cfg(target_os = "android")]
        app.add_startup_system(keyboard::setup_android_keyboard_event.system())
            .add_system_to_stage(
                CoreStage::PreUpdate,
                keyboard::android_keyboard_event.system(),
            );
    }
}

#[derive(Clone, Debug)]
pub struct XrOptions {
    pub view_type: openxr::ViewConfigurationType,
    pub hand_trackers: bool,
}

impl Default for XrOptions {
    fn default() -> Self {
        #[cfg(target_os = "android")]
        let hand_trackers = true;

        #[cfg(not(target_os = "android"))]
        let hand_trackers = false;

        Self {
            view_type: openxr::ViewConfigurationType::PRIMARY_STEREO,
            hand_trackers,
        }
    }
}

// TODO: proposal to rename into `XRInstance`
pub struct OpenXRStruct {
    event_storage: EventDataBufferHolder,
    session_state: XRState,
    previous_frame_state: XRState,
    pub handles: wgpu::OpenXRHandles,
    pub instance: openxr::Instance,
    pub options: XrOptions,
}

impl std::fmt::Debug for OpenXRStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenXRStruct[...]")
    }
}

impl OpenXRStruct {
    pub fn new(
        instance: openxr::Instance,
        handles: wgpu::OpenXRHandles,
        options: XrOptions,
    ) -> Self {
        OpenXRStruct {
            event_storage: EventDataBufferHolder(openxr::EventDataBuffer::new()),
            session_state: XRState::Paused,
            previous_frame_state: XRState::Paused,
            instance,
            handles,
            options,
        }
    }

    fn change_state(&mut self, state: XRState, state_flag: &mut bool) -> bool {
        if self.session_state != state {
            self.previous_frame_state = self.session_state;
            self.session_state = state;
            *state_flag = true;
            true
        } else {
            false
        }
    }

    fn get_changed_state(&self, state_flag: &bool) -> Option<XRState> {
        if *state_flag {
            Some(self.session_state)
        } else {
            None
        }
    }

    pub fn handle_openxr_events(&mut self) -> Option<XRState> {
        let mut state_changed = false;

        while let Some(event) = self.instance.poll_event(&mut self.event_storage.0).unwrap() {
            match event {
                openxr::Event::SessionStateChanged(e) => {
                    println!("entered state {:?}", e.state());

                    match e.state() {
                        // XR Docs: The application is ready to call xrBeginSession and sync its frame loop with the runtime.
                        openxr::SessionState::READY => {
                            // if on oculus, set refresh rate
                            if let Some(display_refresh_rate_fb) =
                                self.instance.exts().fb_display_refresh_rate
                            {
                                let mut rate: f32 = 0.0;

                                unsafe {
                                    (display_refresh_rate_fb.get_display_refresh_rate)(
                                        self.handles.session.as_raw(),
                                        &mut rate,
                                    )
                                };

                                println!("Current refresh rate: {:?}", rate);

                                let request_refresh_rate = 90.;

                                let ret = unsafe {
                                    (display_refresh_rate_fb.request_display_refresh_rate)(
                                        self.handles.session.as_raw(),
                                        request_refresh_rate,
                                    )
                                };

                                println!(
                                    "Requested refresh rate change to {} - result: {:?}",
                                    request_refresh_rate, ret
                                );
                            }

                            self.handles.session.begin(self.options.view_type).unwrap();
                            self.change_state(XRState::Running, &mut state_changed);
                        }
                        // XR Docs: The application should exit its frame loop and call xrEndSession.
                        openxr::SessionState::STOPPING => {
                            self.handles.session.end().unwrap();
                            // TODO500: FIXME add a graceful cleanup of all OpenXR resources here
                            self.change_state(XRState::Paused, &mut state_changed);
                        }
                        // XR Docs:
                        // EXITING: The application should end its XR experience and not automatically restart it.
                        // LOSS_PENDING: The session is in the process of being lost. The application should destroy the current session and can optionally recreate it.
                        openxr::SessionState::EXITING | openxr::SessionState::LOSS_PENDING => {
                            self.change_state(XRState::Exiting, &mut state_changed);
                            return self.get_changed_state(&state_changed);
                        }
                        // XR Docs: The application has synced its frame loop with the runtime and is visible to the user but cannot receive XR input.
                        openxr::SessionState::VISIBLE => {
                            self.change_state(XRState::Running, &mut state_changed);
                        }
                        // XR Docs: The application has synced its frame loop with the runtime, is visible to the user and can receive XR input.
                        openxr::SessionState::FOCUSED => {
                            self.change_state(XRState::RunningFocused, &mut state_changed);
                        }
                        // XR Docs: The initial state after calling xrCreateSession or returned to after calling xrEndSession.
                        openxr::SessionState::IDLE => {
                            // FIXME is this handling ok?
                            self.change_state(XRState::Paused, &mut state_changed);
                        }
                        openxr::SessionState::SYNCHRONIZED => {
                            self.change_state(XRState::Running, &mut state_changed);
                        }
                        _ => {}
                    }
                }
                openxr::Event::InstanceLossPending(_) => {
                    self.change_state(XRState::Exiting, &mut state_changed);
                    return self.get_changed_state(&state_changed);
                }
                openxr::Event::EventsLost(e) => {
                    println!("lost {} events", e.lost_event_count());
                }
                openxr::Event::ReferenceSpaceChangePending(reference_space) => {
                    println!(
                        "OpenXR: Event: ReferenceSpaceChangePending {:?}",
                        reference_space.reference_space_type()
                    );
                }
                openxr::Event::PerfSettingsEXT(_) => {
                    println!("OpenXR: Event: PerfSettingsEXT");
                }
                openxr::Event::VisibilityMaskChangedKHR(_) => {
                    println!("OpenXR: Event: VisibilityMaskChangedKHR");
                }
                openxr::Event::InteractionProfileChanged(_) => {
                    println!("OpenXR: Event: InteractionProfileChanged");
                }
                openxr::Event::MainSessionVisibilityChangedEXTX(_) => {
                    println!("OpenXR: Event: MainSessionVisibilityChangedEXTX");
                }
                _ => {
                    println!("OpenXR: Event: unknown")
                }
            }
        }

        match self.session_state {
            XRState::Paused => std::thread::sleep(std::time::Duration::from_millis(100)),
            _ => (),
        }

        self.get_changed_state(&state_changed)
    }

    pub fn is_running(&self) -> bool {
        self.session_state == XRState::Running || self.session_state == XRState::RunningFocused
    }
}

pub struct EventDataBufferHolder(openxr::EventDataBuffer);

// FIXME FIXME FIXME UB AND BAD THINGS CAN/WILL HAPPEN. Required by EventDataBuffer
// read openxr docs about whether EventDataBuffer is thread-safe
// or move to resourcesmut?
// FIXME or process events in own thread?
unsafe impl Sync for EventDataBufferHolder {}
unsafe impl Send for EventDataBufferHolder {}

fn xr_event_debug(mut state_events: EventReader<XRState>) {
    for event in state_events.iter() {
        println!("#STATE EVENT: {:#?}", event);
    }
}

#[derive(Debug)]
pub enum Error {
    XR(openxr::sys::Result),
}

impl From<openxr::sys::Result> for Error {
    fn from(e: openxr::sys::Result) -> Self {
        Error::XR(e)
    }
}

#[derive(Default)]
pub struct XRConfigurationState {
    pub texture_view_ids: Option<Vec<TextureId>>,
    pub next_swap_chain_index: usize,
    pub last_view_surface: Option<XRViewSurfaceCreated>,
}
