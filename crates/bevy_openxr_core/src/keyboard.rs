use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::input::mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::ElementState;
use bevy::prelude::*;
use bevy::window::WindowId;

pub(crate) struct InputMetadata {
    window_size: Option<Vec2>,
    previous_mouse_position: Option<Vec2>,
    previous_mouse_states: [bool; 5],
}

pub(crate) fn setup_android_keyboard_event(mut commands: Commands) {
    commands.insert_resource(InputMetadata {
        window_size: None,
        previous_mouse_position: None,
        previous_mouse_states: [false, false, false, false, false],
    })
}

pub(crate) fn android_keyboard_event(
    mut keyboard_input_events: EventWriter<KeyboardInput>,
    mut mouse_wheel_events: EventWriter<MouseWheel>,
    mut mouse_button_input_events: EventWriter<MouseButtonInput>,
    mut cursor_moved_events: EventWriter<CursorMoved>,
    mut mouse_motion_events: EventWriter<MouseMotion>,
    mut keyboard_metadata: ResMut<InputMetadata>,
) {
    if let None = keyboard_metadata.window_size {
        if let Some(native_window) = ndk_glue::native_window().as_ref() {
            // FIXME: can these change over lifetime?
            keyboard_metadata.window_size = Some(Vec2::new(
                ndk_glue::native_window().as_ref().unwrap().width() as f32,
                ndk_glue::native_window().as_ref().unwrap().height() as f32,
            ));
        } else {
            return;
        }
    }

    let has_events = match ndk_glue::input_queue().as_ref() {
        Some(iq) => iq.has_events().unwrap(),
        None => return,
    };

    if !has_events {
        return;
    }

    loop {
        let event = match ndk_glue::input_queue().as_ref().unwrap().get_event() {
            Some(event) => event,
            None => break,
        };

        let mut handled = false;

        match &event {
            ndk::event::InputEvent::KeyEvent(key_event) => {
                let scan_code = key_event.scan_code();
                let key_code = key_event.key_code();
                let action = key_event.action();

                let converted_key_code = convert_key_code(key_code);
                let state = convert_key_state(action);

                if converted_key_code.is_some() && state.is_some() {
                    let keyboard_input = KeyboardInput {
                        scan_code: scan_code as u32,
                        key_code: converted_key_code,
                        state: state.unwrap(),
                    };

                    //println!("Key event: {:?}", keyboard_input);
                    keyboard_input_events.send(keyboard_input);
                    let handled = true;
                } else {
                    /* do not print by default
                    println!(
                        "!! Unknown android key event scan_code={:?}, key_code={:?}, action={:?}",
                        scan_code, key_code, action
                    );
                    */
                }

                /*
                println!(
                    "KEY EVENT: device_id={:?} action={:?} down_time={:?} event_time={:?} key_code={:?} repeat_count={:?} scan_code={:?}",
                    key_event.device_id(),
                    key_event.action(),
                    key_event.down_time(),
                    key_event.event_time(),
                    key_event.key_code(),
                    key_event.repeat_count(),
                    key_event.scan_code()
                );
                */
            }
            ndk::event::InputEvent::MotionEvent(motion_event) => {
                let action = motion_event.action();

                match action {
                    ndk::event::MotionAction::HoverMove | ndk::event::MotionAction::Move => {
                        // move when pointer not down
                        if let Some(pointer) = motion_event.pointers().next() {
                            let position = Vec2::new(
                                pointer.x(),
                                keyboard_metadata.window_size.unwrap().y - pointer.y() - 1., // FIXME okay? 0 -- height - 1
                            );

                            cursor_moved_events.send(CursorMoved {
                                id: WindowId::default(),
                                position,
                            });

                            if let Some(previous_position) =
                                &mut keyboard_metadata.previous_mouse_position
                            {
                                mouse_motion_events.send(MouseMotion {
                                    delta: position - *previous_position,
                                });
                            }

                            keyboard_metadata.previous_mouse_position = Some(position);
                        }
                    }
                    ndk::event::MotionAction::Scroll => {
                        // mouse wheel
                        if let Some(pointer) = motion_event.pointers().next() {
                            // bevy: bottom left = (0, 0)
                            let axis_vscroll = pointer.axis_value(ndk::event::Axis::Vscroll);
                            let axis_hscroll = pointer.axis_value(ndk::event::Axis::Hscroll);

                            mouse_wheel_events.send(MouseWheel {
                                unit: MouseScrollUnit::Pixel, // ?
                                x: axis_hscroll,
                                y: axis_vscroll,
                            });
                        }
                    }
                    ndk::event::MotionAction::ButtonPress
                    | ndk::event::MotionAction::ButtonRelease => {
                        // contains state of all buttons
                        let button_state = motion_event.button_state();

                        let pressed_states = [
                            button_state.primary(),
                            button_state.secondary(),
                            button_state.teriary(),
                            button_state.back(),
                            button_state.forward(),
                        ];

                        const buttons: [MouseButton; 5] = [
                            MouseButton::Left,
                            MouseButton::Right,
                            MouseButton::Middle,
                            // TODO: educated guesses below
                            MouseButton::Other(4),
                            MouseButton::Other(5),
                        ];

                        debug_assert_eq!(pressed_states.len(), buttons.len());
                        debug_assert_eq!(
                            keyboard_metadata.previous_mouse_states.len(),
                            buttons.len()
                        );

                        for (idx, is_pressed) in pressed_states.iter().enumerate() {
                            if keyboard_metadata.previous_mouse_states[idx] == *is_pressed {
                                // same state as previous
                                continue;
                            }

                            let event = MouseButtonInput {
                                button: buttons[idx],
                                state: match is_pressed {
                                    true => ElementState::Pressed,
                                    false => ElementState::Released,
                                },
                            };

                            mouse_button_input_events.send(event);
                            keyboard_metadata.previous_mouse_states[idx] = *is_pressed;
                        }
                    }

                    ndk::event::MotionAction::Down => (),
                    ndk::event::MotionAction::Up => (),
                    ndk::event::MotionAction::Cancel => (),
                    ndk::event::MotionAction::Outside => (),
                    ndk::event::MotionAction::PointerDown => (),
                    ndk::event::MotionAction::PointerUp => (),
                    ndk::event::MotionAction::HoverEnter => (),
                    ndk::event::MotionAction::HoverExit => (),
                }

                /*
                println!(
                    "MOTION EVENT: device_id={:?} source={:?} action={:?} pointer_index={:?} history_size={:?} button_state={:?} down_time={:?}",
                    motion_event.device_id(),
                    motion_event.source(),
                    motion_event.action(),
                    motion_event.pointer_index(),
                    motion_event.history_size(),
                    motion_event.button_state(),
                    motion_event.down_time()
                );
                */
            }
        }

        ndk_glue::input_queue()
            .as_ref()
            .unwrap()
            .finish_event(event, true);
    }
}

fn convert_key_state(input: ndk::event::KeyAction) -> Option<ElementState> {
    match input {
        ndk::event::KeyAction::Down => Some(ElementState::Pressed),
        ndk::event::KeyAction::Up => Some(ElementState::Released),
        ndk::event::KeyAction::Multiple => None, // ??
    }
}

fn convert_key_code(input: ndk::event::Keycode) -> Option<KeyCode> {
    // FIXME: untested and incomplete list!

    match input {
        ndk::event::Keycode::Unknown => None,
        ndk::event::Keycode::SoftLeft => None,
        ndk::event::Keycode::SoftRight => None,

        ndk::event::Keycode::Home => Some(KeyCode::Home),
        ndk::event::Keycode::Back => Some(KeyCode::Back),

        ndk::event::Keycode::Call => None,
        ndk::event::Keycode::Endcall => None,

        ndk::event::Keycode::Keycode0 => Some(KeyCode::Key0),
        ndk::event::Keycode::Keycode1 => Some(KeyCode::Key1),
        ndk::event::Keycode::Keycode2 => Some(KeyCode::Key2),
        ndk::event::Keycode::Keycode3 => Some(KeyCode::Key3),
        ndk::event::Keycode::Keycode4 => Some(KeyCode::Key4),
        ndk::event::Keycode::Keycode5 => Some(KeyCode::Key5),
        ndk::event::Keycode::Keycode6 => Some(KeyCode::Key6),
        ndk::event::Keycode::Keycode7 => Some(KeyCode::Key7),
        ndk::event::Keycode::Keycode8 => Some(KeyCode::Key8),
        ndk::event::Keycode::Keycode9 => Some(KeyCode::Key9),

        ndk::event::Keycode::Star => None,
        ndk::event::Keycode::Pound => None,
        ndk::event::Keycode::DpadUp => Some(KeyCode::Up),
        ndk::event::Keycode::DpadDown => Some(KeyCode::Down),
        ndk::event::Keycode::DpadLeft => Some(KeyCode::Left),
        ndk::event::Keycode::DpadRight => Some(KeyCode::Right),

        ndk::event::Keycode::DpadCenter => None,
        ndk::event::Keycode::VolumeUp => None,
        ndk::event::Keycode::VolumeDown => None,
        ndk::event::Keycode::Power => None,
        ndk::event::Keycode::Camera => None,
        ndk::event::Keycode::Clear => None,

        ndk::event::Keycode::A => Some(KeyCode::A),
        ndk::event::Keycode::B => Some(KeyCode::B),
        ndk::event::Keycode::C => Some(KeyCode::C),
        ndk::event::Keycode::D => Some(KeyCode::D),
        ndk::event::Keycode::E => Some(KeyCode::E),
        ndk::event::Keycode::F => Some(KeyCode::F),
        ndk::event::Keycode::G => Some(KeyCode::G),
        ndk::event::Keycode::H => Some(KeyCode::H),
        ndk::event::Keycode::I => Some(KeyCode::I),
        ndk::event::Keycode::J => Some(KeyCode::J),
        ndk::event::Keycode::K => Some(KeyCode::K),
        ndk::event::Keycode::L => Some(KeyCode::L),
        ndk::event::Keycode::M => Some(KeyCode::M),
        ndk::event::Keycode::N => Some(KeyCode::N),
        ndk::event::Keycode::O => Some(KeyCode::O),
        ndk::event::Keycode::P => Some(KeyCode::P),
        ndk::event::Keycode::Q => Some(KeyCode::Q),
        ndk::event::Keycode::R => Some(KeyCode::R),
        ndk::event::Keycode::S => Some(KeyCode::S),
        ndk::event::Keycode::T => Some(KeyCode::T),
        ndk::event::Keycode::U => Some(KeyCode::U),
        ndk::event::Keycode::V => Some(KeyCode::V),
        ndk::event::Keycode::W => Some(KeyCode::W),
        ndk::event::Keycode::X => Some(KeyCode::X),
        ndk::event::Keycode::Y => Some(KeyCode::Y),
        ndk::event::Keycode::Z => Some(KeyCode::Z),

        ndk::event::Keycode::Comma => Some(KeyCode::Comma),
        ndk::event::Keycode::Period => Some(KeyCode::Period),
        ndk::event::Keycode::AltLeft => Some(KeyCode::LAlt),
        ndk::event::Keycode::AltRight => Some(KeyCode::RAlt),
        ndk::event::Keycode::ShiftLeft => Some(KeyCode::LShift),
        ndk::event::Keycode::ShiftRight => Some(KeyCode::RShift),
        ndk::event::Keycode::Tab => Some(KeyCode::Tab),
        ndk::event::Keycode::Space => Some(KeyCode::Space),

        ndk::event::Keycode::Sym => None,
        ndk::event::Keycode::Explorer => None,
        ndk::event::Keycode::Envelope => None,

        ndk::event::Keycode::Enter => Some(KeyCode::Return),
        ndk::event::Keycode::Del => Some(KeyCode::Delete),

        ndk::event::Keycode::Grave => None,
        ndk::event::Keycode::Minus => Some(KeyCode::Minus),
        ndk::event::Keycode::Equals => Some(KeyCode::Equals),

        ndk::event::Keycode::LeftBracket => None,
        ndk::event::Keycode::RightBracket => None,

        ndk::event::Keycode::Backslash => Some(KeyCode::Backslash),
        ndk::event::Keycode::Semicolon => Some(KeyCode::Semicolon),
        ndk::event::Keycode::Apostrophe => Some(KeyCode::Apostrophe),
        ndk::event::Keycode::Slash => Some(KeyCode::Slash),
        ndk::event::Keycode::At => Some(KeyCode::At),

        ndk::event::Keycode::Num => None,
        ndk::event::Keycode::Headsethook => None,
        ndk::event::Keycode::Focus => None,

        ndk::event::Keycode::Plus => Some(KeyCode::Plus),

        ndk::event::Keycode::Menu => None,
        ndk::event::Keycode::Notification => None,
        ndk::event::Keycode::Search => None,
        ndk::event::Keycode::MediaPlayPause => None,
        ndk::event::Keycode::MediaStop => None,
        ndk::event::Keycode::MediaNext => None,
        ndk::event::Keycode::MediaPrevious => None,
        ndk::event::Keycode::MediaRewind => None,
        ndk::event::Keycode::MediaFastForward => None,
        ndk::event::Keycode::Mute => None,

        ndk::event::Keycode::PageUp => Some(KeyCode::PageUp),
        ndk::event::Keycode::PageDown => Some(KeyCode::PageDown),

        ndk::event::Keycode::Pictsymbols => None,
        ndk::event::Keycode::SwitchCharset => None,

        ndk::event::Keycode::ButtonA => None,
        ndk::event::Keycode::ButtonB => None,
        ndk::event::Keycode::ButtonC => None,
        ndk::event::Keycode::ButtonX => None,
        ndk::event::Keycode::ButtonY => None,
        ndk::event::Keycode::ButtonZ => None,
        ndk::event::Keycode::ButtonL1 => None,
        ndk::event::Keycode::ButtonR1 => None,
        ndk::event::Keycode::ButtonL2 => None,
        ndk::event::Keycode::ButtonR2 => None,
        ndk::event::Keycode::ButtonThumbl => None,
        ndk::event::Keycode::ButtonThumbr => None,
        ndk::event::Keycode::ButtonStart => None,
        ndk::event::Keycode::ButtonSelect => None,
        ndk::event::Keycode::ButtonMode => None,

        ndk::event::Keycode::Escape => Some(KeyCode::Escape),

        ndk::event::Keycode::ForwardDel => None,
        ndk::event::Keycode::CtrlLeft => None,
        ndk::event::Keycode::CtrlRight => None,
        ndk::event::Keycode::CapsLock => None,

        ndk::event::Keycode::ScrollLock => Some(KeyCode::Scroll),

        ndk::event::Keycode::MetaLeft => None,
        ndk::event::Keycode::MetaRight => None,
        ndk::event::Keycode::Function => None,
        ndk::event::Keycode::Sysrq => None,
        ndk::event::Keycode::Break => None,

        ndk::event::Keycode::MoveHome => Some(KeyCode::Home),
        ndk::event::Keycode::MoveEnd => Some(KeyCode::End),
        ndk::event::Keycode::Insert => Some(KeyCode::Insert),

        ndk::event::Keycode::Forward => None,
        ndk::event::Keycode::MediaPlay => None,
        ndk::event::Keycode::MediaPause => None,
        ndk::event::Keycode::MediaClose => None,
        ndk::event::Keycode::MediaEject => None,
        ndk::event::Keycode::MediaRecord => None,

        ndk::event::Keycode::F1 => Some(KeyCode::F1),
        ndk::event::Keycode::F2 => Some(KeyCode::F2),
        ndk::event::Keycode::F3 => Some(KeyCode::F3),
        ndk::event::Keycode::F4 => Some(KeyCode::F4),
        ndk::event::Keycode::F5 => Some(KeyCode::F5),
        ndk::event::Keycode::F6 => Some(KeyCode::F6),
        ndk::event::Keycode::F7 => Some(KeyCode::F7),
        ndk::event::Keycode::F8 => Some(KeyCode::F8),
        ndk::event::Keycode::F9 => Some(KeyCode::F9),
        ndk::event::Keycode::F10 => Some(KeyCode::F10),
        ndk::event::Keycode::F11 => Some(KeyCode::F11),
        ndk::event::Keycode::F12 => Some(KeyCode::F12),
        ndk::event::Keycode::NumLock => Some(KeyCode::Numlock),
        ndk::event::Keycode::Numpad0 => Some(KeyCode::Numpad0),
        ndk::event::Keycode::Numpad1 => Some(KeyCode::Numpad1),
        ndk::event::Keycode::Numpad2 => Some(KeyCode::Numpad2),
        ndk::event::Keycode::Numpad3 => Some(KeyCode::Numpad3),
        ndk::event::Keycode::Numpad4 => Some(KeyCode::Numpad4),
        ndk::event::Keycode::Numpad5 => Some(KeyCode::Numpad5),
        ndk::event::Keycode::Numpad6 => Some(KeyCode::Numpad6),
        ndk::event::Keycode::Numpad7 => Some(KeyCode::Numpad7),
        ndk::event::Keycode::Numpad8 => Some(KeyCode::Numpad8),
        ndk::event::Keycode::Numpad9 => Some(KeyCode::Numpad9),
        ndk::event::Keycode::NumpadDivide => Some(KeyCode::NumpadDivide),
        ndk::event::Keycode::NumpadMultiply => Some(KeyCode::NumpadMultiply),
        ndk::event::Keycode::NumpadSubtract => Some(KeyCode::NumpadSubtract),
        ndk::event::Keycode::NumpadAdd => Some(KeyCode::NumpadAdd),

        ndk::event::Keycode::NumpadDot => None,

        ndk::event::Keycode::NumpadComma => Some(KeyCode::NumpadComma),
        ndk::event::Keycode::NumpadEnter => Some(KeyCode::NumpadEnter),
        ndk::event::Keycode::NumpadEquals => Some(KeyCode::NumpadEquals),

        ndk::event::Keycode::NumpadLeftParen => None,
        ndk::event::Keycode::NumpadRightParen => None,
        ndk::event::Keycode::VolumeMute => None,
        ndk::event::Keycode::Info => None,
        ndk::event::Keycode::ChannelUp => None,
        ndk::event::Keycode::ChannelDown => None,
        ndk::event::Keycode::ZoomIn => None,
        ndk::event::Keycode::ZoomOut => None,
        ndk::event::Keycode::Tv => None,
        ndk::event::Keycode::Window => None,
        ndk::event::Keycode::Guide => None,
        ndk::event::Keycode::Dvr => None,
        ndk::event::Keycode::Bookmark => None,
        ndk::event::Keycode::Captions => None,
        ndk::event::Keycode::Settings => None,
        ndk::event::Keycode::TvPower => None,
        ndk::event::Keycode::TvInput => None,
        ndk::event::Keycode::StbPower => None,
        ndk::event::Keycode::StbInput => None,
        ndk::event::Keycode::AvrPower => None,
        ndk::event::Keycode::AvrInput => None,
        ndk::event::Keycode::ProgRed => None,
        ndk::event::Keycode::ProgGreen => None,
        ndk::event::Keycode::ProgYellow => None,
        ndk::event::Keycode::ProgBlue => None,
        ndk::event::Keycode::AppSwitch => None,
        ndk::event::Keycode::Button1 => None,
        ndk::event::Keycode::Button2 => None,
        ndk::event::Keycode::Button3 => None,
        ndk::event::Keycode::Button4 => None,
        ndk::event::Keycode::Button5 => None,
        ndk::event::Keycode::Button6 => None,
        ndk::event::Keycode::Button7 => None,
        ndk::event::Keycode::Button8 => None,
        ndk::event::Keycode::Button9 => None,
        ndk::event::Keycode::Button10 => None,
        ndk::event::Keycode::Button11 => None,
        ndk::event::Keycode::Button12 => None,
        ndk::event::Keycode::Button13 => None,
        ndk::event::Keycode::Button14 => None,
        ndk::event::Keycode::Button15 => None,
        ndk::event::Keycode::Button16 => None,
        ndk::event::Keycode::LanguageSwitch => None,
        ndk::event::Keycode::MannerMode => None,
        ndk::event::Keycode::Keycode3dMode => None,
        ndk::event::Keycode::Contacts => None,
        ndk::event::Keycode::Calendar => None,
        ndk::event::Keycode::Music => None,
        ndk::event::Keycode::Calculator => None,
        ndk::event::Keycode::ZenkakuHankaku => None,
        ndk::event::Keycode::Eisu => None,
        ndk::event::Keycode::Muhenkan => None,
        ndk::event::Keycode::Henkan => None,
        ndk::event::Keycode::KatakanaHiragana => None,
        ndk::event::Keycode::Yen => None,
        ndk::event::Keycode::Ro => None,
        ndk::event::Keycode::Kana => None,
        ndk::event::Keycode::Assist => None,
        ndk::event::Keycode::BrightnessDown => None,
        ndk::event::Keycode::BrightnessUp => None,
        ndk::event::Keycode::MediaAudioTrack => None,
        ndk::event::Keycode::Sleep => None,
        ndk::event::Keycode::Wakeup => None,
        ndk::event::Keycode::Pairing => None,
        ndk::event::Keycode::MediaTopMenu => None,
        ndk::event::Keycode::Keycode11 => None,
        ndk::event::Keycode::Keycode12 => None,
        ndk::event::Keycode::LastChannel => None,
        ndk::event::Keycode::TvDataService => None,
        ndk::event::Keycode::VoiceAssist => None,

        ndk::event::Keycode::TvRadioService => None,
        ndk::event::Keycode::TvTeletext => None,
        ndk::event::Keycode::TvNumberEntry => None,
        ndk::event::Keycode::TvTerrestrialAnalog => None,
        ndk::event::Keycode::TvTerrestrialDigital => None,
        ndk::event::Keycode::TvSatellite => None,
        ndk::event::Keycode::TvSatelliteBs => None,
        ndk::event::Keycode::TvSatelliteCs => None,
        ndk::event::Keycode::TvSatelliteService => None,
        ndk::event::Keycode::TvNetwork => None,
        ndk::event::Keycode::TvAntennaCable => None,
        ndk::event::Keycode::TvInputHdmi1 => None,
        ndk::event::Keycode::TvInputHdmi2 => None,
        ndk::event::Keycode::TvInputHdmi3 => None,
        ndk::event::Keycode::TvInputHdmi4 => None,
        ndk::event::Keycode::TvInputComposite1 => None,
        ndk::event::Keycode::TvInputComposite2 => None,
        ndk::event::Keycode::TvInputComponent1 => None,
        ndk::event::Keycode::TvInputComponent2 => None,
        ndk::event::Keycode::TvInputVga1 => None,
        ndk::event::Keycode::TvAudioDescription => None,
        ndk::event::Keycode::TvAudioDescriptionMixUp => None,
        ndk::event::Keycode::TvAudioDescriptionMixDown => None,
        ndk::event::Keycode::TvZoomMode => None,
        ndk::event::Keycode::TvContentsMenu => None,
        ndk::event::Keycode::TvMediaContextMenu => None,
        ndk::event::Keycode::TvTimerProgramming => None,

        ndk::event::Keycode::Help => None,

        ndk::event::Keycode::NavigatePrevious => None,
        ndk::event::Keycode::NavigateNext => None,
        ndk::event::Keycode::NavigateIn => None,
        ndk::event::Keycode::NavigateOut => None,

        ndk::event::Keycode::StemPrimary => None,
        ndk::event::Keycode::Stem1 => None,
        ndk::event::Keycode::Stem2 => None,
        ndk::event::Keycode::Stem3 => None,

        ndk::event::Keycode::DpadUpLeft => None,
        ndk::event::Keycode::DpadDownLeft => None,
        ndk::event::Keycode::DpadUpRight => None,
        ndk::event::Keycode::DpadDownRight => None,
        ndk::event::Keycode::MediaSkipForward => None,
        ndk::event::Keycode::MediaSkipBackward => None,
        ndk::event::Keycode::MediaStepForward => None,
        ndk::event::Keycode::MediaStepBackward => None,
        ndk::event::Keycode::SoftSleep => None,
        ndk::event::Keycode::Cut => Some(KeyCode::Cut),
        ndk::event::Keycode::Copy => Some(KeyCode::Copy),
        ndk::event::Keycode::Paste => Some(KeyCode::Paste),
        ndk::event::Keycode::SystemNavigationUp => None,
        ndk::event::Keycode::SystemNavigationDown => None,
        ndk::event::Keycode::SystemNavigationLeft => None,
        ndk::event::Keycode::SystemNavigationRight => None,
        ndk::event::Keycode::AllApps => None,
        ndk::event::Keycode::Refresh => None,
        ndk::event::Keycode::ThumbsUp => None,
        ndk::event::Keycode::ThumbsDown => None,
        ndk::event::Keycode::ProfileSwitch => None,
    }
}
