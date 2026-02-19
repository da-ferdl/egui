use winit::event::WindowEvent;

use crate::{AppLifeCycleState, EguiContextExt, update_egui_context_life_cycle_state};

/// Note: winit does not emit any event for Android that can be used as 'exit event', so on the winit-fork at
/// '/src/platform_impl/android/mod.rs' - line 291 - 'MainEvent::Destroy' a
/// 'winit::event::WindowEvent::Destroyed' custom invocation was added.
///
/// In general the app life cycle state is fiddled together by intercepting which events are sent at which
/// point from winit.
/// This is suboptimal and can break very quick - TODO: add host platform specific listeners!!
///
pub struct AppLifeCycleHandler {
    egui_ctx: egui::Context,
    is_first_mac_window_focused_event: bool,
}
impl AppLifeCycleHandler {
    /// Should be created when the application starts (right before [XFrameApp::on_start]).
    ///
    /// Sets the app-life-cycle-state to 'foreground-active' and returns that state
    /// together with `Self`.
    pub fn new(egui_ctx: egui::Context) -> (Self, AppLifeCycleState) {
        let next_state = AppLifeCycleState::ForegroundActive;
        update_egui_context_life_cycle_state(next_state);

        (
            Self {
                egui_ctx,
                is_first_mac_window_focused_event: true,
            },
            next_state,
        )
    }

    /// Returns a vec with app-life-cycle states (new and intermediate ones if needed) if the
    /// new state is different then the previous one, otherwise `None`.
    pub fn handle_resumed_event(&mut self) -> Option<Vec<AppLifeCycleState>> {
        let current_state = self.egui_ctx.app_life_cycle_state();

        if current_state.is_did_exit() {
            return None;
        }

        let next_state = AppLifeCycleState::ForegroundActive;
        let states = Self::get_states_vec(*current_state, next_state);

        if let Some(next) = states.last() {
            update_egui_context_life_cycle_state(*next);

            return Some(states);
        }

        None
    }

    /// Returns a vec with app-life-cycle states (new and intermediate ones if needed) if the
    /// new state is different then the previous one, otherwise `None`.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> Option<Vec<AppLifeCycleState>> {
        let current_state = self.egui_ctx.app_life_cycle_state();

        if current_state.is_did_exit() {
            return None;
        }

        let platform = self.egui_ctx.current_platform();

        let next_state = match &event {
            WindowEvent::CloseRequested => AppLifeCycleState::Background { did_exit: true },
            WindowEvent::Destroyed => AppLifeCycleState::Background { did_exit: true },
            WindowEvent::Focused(focused) => {
                // Ignore focus events on iOS because on iOS
                // on termination winit sends a 'focused true' event,
                // which makes it hard to determine when it is a exit.
                // -> on iOS there are other events which are used for the life-cycle state.
                if platform.is_ios {
                    return None;
                }

                // winit sends on macOS on start first a 'focused false' event and then a 'focused true',
                // so there the first focus event is ignored.
                if platform.is_macos && self.is_first_mac_window_focused_event {
                    self.is_first_mac_window_focused_event = false;

                    return None;
                }

                if *focused {
                    AppLifeCycleState::ForegroundActive
                } else {
                    if current_state.is_foreground_active() {
                        AppLifeCycleState::ForegroundPaused { was_active: true }
                    } else {
                        return None;
                    }
                }
            }
            // Apple specific - not received on other platforms.
            WindowEvent::Occluded(occluded) => {
                if *occluded && current_state.is_foreground_paused() {
                    AppLifeCycleState::Background { did_exit: false }
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        let states = Self::get_states_vec(*current_state, next_state);

        if let Some(next) = states.last() {
            update_egui_context_life_cycle_state(*next);

            return Some(states);
        }

        None
    }

    /// Returns a vec with app-life-cycle states (new and intermediate ones if needed) if the
    /// new state is different then the previous one, otherwise `None`.
    pub fn handle_suspended_event(&mut self) -> Option<Vec<AppLifeCycleState>> {
        let current_state = self.egui_ctx.app_life_cycle_state();

        if current_state.is_did_exit() {
            return None;
        }

        let next_state = match current_state {
            AppLifeCycleState::ForegroundActive => {
                AppLifeCycleState::ForegroundPaused { was_active: true }
            }
            AppLifeCycleState::ForegroundPaused { .. } => {
                AppLifeCycleState::Background { did_exit: false }
            }
            AppLifeCycleState::Background { .. } => return None,
        };
        let states = Self::get_states_vec(*current_state, next_state);

        if let Some(next) = states.last() {
            update_egui_context_life_cycle_state(*next);

            return Some(states);
        }

        None
    }

    /// Returns a vec with app-life-cycle states (new and intermediate ones if needed) if the
    /// new state is different then the previous one, otherwise `None`.
    pub fn handle_app_destroy_indication(&mut self) -> Option<Vec<AppLifeCycleState>> {
        let current_state = self.egui_ctx.app_life_cycle_state();

        if current_state.is_did_exit() {
            return None;
        }

        let next_state = AppLifeCycleState::Background { did_exit: true };
        let states = Self::get_states_vec(*current_state, next_state);

        if let Some(next) = states.last() {
            update_egui_context_life_cycle_state(*next);

            return Some(states);
        }

        None
    }

    /// Returns a vec of states - new one with intermediate states if needed or empty vec if current matches next.
    fn get_states_vec(prev: AppLifeCycleState, next: AppLifeCycleState) -> Vec<AppLifeCycleState> {
        let states_vec = match prev {
            AppLifeCycleState::Background { .. } => match next {
                AppLifeCycleState::Background { .. } => {
                    vec![next]
                }
                AppLifeCycleState::ForegroundPaused { .. } => {
                    vec![AppLifeCycleState::ForegroundPaused { was_active: false }]
                }
                AppLifeCycleState::ForegroundActive => {
                    vec![
                        AppLifeCycleState::ForegroundPaused { was_active: false },
                        next,
                    ]
                }
            },
            AppLifeCycleState::ForegroundPaused {
                was_active: prev_was_active,
            } => match next {
                AppLifeCycleState::Background { did_exit } => {
                    let mut vec = vec![];

                    if did_exit {
                        vec.push(AppLifeCycleState::Background { did_exit: false });
                    }
                    vec.push(next);

                    vec
                }
                AppLifeCycleState::ForegroundPaused {
                    was_active: next_was_active,
                } => {
                    if prev_was_active != next_was_active {
                        vec![next]
                    } else {
                        vec![]
                    }
                }
                AppLifeCycleState::ForegroundActive => {
                    vec![next]
                }
            },
            AppLifeCycleState::ForegroundActive => match next {
                AppLifeCycleState::Background { did_exit } => {
                    let mut vec = vec![
                        AppLifeCycleState::ForegroundPaused { was_active: true },
                        AppLifeCycleState::Background { did_exit: false },
                    ];
                    if did_exit {
                        vec.push(next);
                    }

                    vec
                }
                AppLifeCycleState::ForegroundPaused { .. } => {
                    vec![AppLifeCycleState::ForegroundPaused { was_active: true }]
                }
                AppLifeCycleState::ForegroundActive => {
                    vec![]
                }
            },
        };

        states_vec
    }
}
