use super::winit_integration::{UserEvent, WinitApp};
use crate::{
    Result, epi,
    native::{event_loop_context, winit_integration::EventResult},
};
use ahash::HashMap;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

/// Wraps a [`WinitApp`] to implement [`ApplicationHandler`]. This handles redrawing, exit states, and
/// some events, but otherwise forwards events to the [`WinitApp`].
struct WinitAppWrapper<T: WinitApp> {
    windows_next_repaint_times: HashMap<WindowId, Instant>,
    winit_app: T,
    return_result: Result<(), crate::Error>,
    run_and_return: bool,
}

impl<T: WinitApp> WinitAppWrapper<T> {
    fn new(winit_app: T, run_and_return: bool) -> Self {
        Self {
            windows_next_repaint_times: HashMap::default(),
            winit_app,
            return_result: Ok(()),
            run_and_return,
        }
    }

    fn handle_event_result(
        &mut self,
        event_loop: &ActiveEventLoop,
        event_result: Result<EventResult>,
    ) {
        let mut exit = false;
        let mut save = false;

        log::trace!("event_result: {event_result:?}");

        let mut event_result = event_result;

        if cfg!(target_os = "windows")
            && let Ok(EventResult::RepaintNow(window_id)) = event_result
        {
            log::trace!("RepaintNow of {window_id:?}");
            self.windows_next_repaint_times
                .insert(window_id, Instant::now());

            // Fix flickering on Windows, see https://github.com/emilk/egui/pull/2280
            event_result = self.winit_app.run_ui_and_paint(event_loop, window_id);
        }

        let combined_result = event_result.map(|event_result| match event_result {
            EventResult::Wait => {
                event_loop.set_control_flow(ControlFlow::Wait);
                event_result
            }
            EventResult::RepaintNow(window_id) => {
                log::trace!("RepaintNow of {window_id:?}",);
                self.windows_next_repaint_times
                    .insert(window_id, Instant::now());
                event_result
            }
            EventResult::RepaintNext(window_id) => {
                log::trace!("RepaintNext of {window_id:?}",);
                self.windows_next_repaint_times
                    .insert(window_id, Instant::now());
                event_result
            }
            EventResult::RepaintAt(window_id, repaint_time) => {
                self.windows_next_repaint_times.insert(
                    window_id,
                    self.windows_next_repaint_times
                        .get(&window_id)
                        .map_or(repaint_time, |last| (*last).min(repaint_time)),
                );
                event_result
            }
            EventResult::Save => {
                save = true;
                event_result
            }
            EventResult::Exit => {
                exit = true;
                event_result
            }
            EventResult::CloseRequested => {
                // The windows need to be dropped whilst the event loop is running to allow for proper cleanup.
                self.winit_app.save_and_destroy();
                event_result
            }
        });

        if let Err(err) = combined_result {
            log::error!("Exiting because of error: {err}");
            exit = true;
            self.return_result = Err(err);
        }

        if save {
            log::debug!("Received an EventResult::Save - saving app state");
            self.winit_app.save();
        }

        if exit {
            if self.run_and_return {
                log::debug!("Asking to exit event loop…");
                event_loop.exit();
            } else {
                log::debug!("Quitting - saving app state…");
                self.winit_app.save_and_destroy();

                log::debug!("Exiting with return code 0");

                std::process::exit(0);
            }
        }

        self.check_redraw_requests(event_loop);
    }

    fn check_redraw_requests(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();

        self.windows_next_repaint_times
            .retain(|window_id, repaint_time| {
                if now < *repaint_time {
                    return true; // not yet ready
                }

                event_loop.set_control_flow(ControlFlow::Poll);

                if let Some(window) = self.winit_app.window(*window_id) {
                    log::trace!("request_redraw for {window_id:?}");
                    window.request_redraw();
                } else {
                    log::trace!("No window found for {window_id:?}");
                }
                false
            });

        let next_repaint_time = self.windows_next_repaint_times.values().min().copied();
        if let Some(next_repaint_time) = next_repaint_time {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_repaint_time));
        }
    }
}

impl<T: WinitApp> ApplicationHandler<UserEvent> for WinitAppWrapper<T> {
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        profiling::scope!("Event::Suspended");

        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = self.winit_app.suspended(event_loop);
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        profiling::scope!("Event::Resumed");

        // Nb: Make sure this guard is dropped after this function returns.
        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = self.winit_app.resumed(event_loop);
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        // On Mac, Cmd-Q we get here and then `run_app_on_demand` doesn't return (despite its name),
        // so we need to save state now:
        log::debug!("Received Event::LoopExiting - saving app state…");
        event_loop_context::with_event_loop_context(event_loop, move || {
            self.winit_app.save_and_destroy();
        });
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        profiling::function_scope!(egui_winit::short_device_event_description(&event));

        // Nb: Make sure this guard is dropped after this function returns.
        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = self.winit_app.device_event(event_loop, device_id, event);
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        profiling::function_scope!(match &event {
            UserEvent::RequestRepaint { .. } => "UserEvent::RequestRepaint",
            UserEvent::ExtCustomEvent(_) => "UserEvent::ExtCustomEvent",
        });

        // A external custom user-event should already be handled and not reach this internal 'ApplicationHandler' implementation.
        // But if not done so and it arrives here, the event can be ignored with a early return, there is nothing to do in that case.
        if let UserEvent::ExtCustomEvent(_) = event {
            log::warn!(
                "A external custom user event reached this internal 'ApplicationHandler' implementation - make sure to handle your external custom events beforehand."
            );
            return;
        }

        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = match event {
                UserEvent::RequestRepaint {
                    when,
                    mut cumulative_pass_nr,
                    viewport_id,
                    repaint_proxy_send_cause,
                } => {
                    let current_pass_nr = self
                        .winit_app
                        .egui_ctx()
                        .map_or(0, |ctx| ctx.cumulative_pass_nr_for(viewport_id));

                    // If 'repaint_proxy_send_cause' is set, this is a repaint request sent
                    // through the `RepaintRequestProxy`.
                    if let Some(cause) = repaint_proxy_send_cause {
                        log::trace!(
                            "UserEvent::RequestRepaint event received from a RepaintRequestProxy"
                        );

                        // In this case the 'cumulative_pass_nr' should be zero because at
                        // the send moment the proxy has no access to the context.
                        // But if for what ever reasons the property is still used (not zero),
                        // we do not adjust it here.
                        if cumulative_pass_nr == 0 {
                            cumulative_pass_nr = current_pass_nr;
                        }

                        // Adjusts the view port state properties, which the proxy
                        // could not set at the send moment.
                        self.winit_app.egui_ctx().map(|ctx| {
                            ctx.adjust_viewport_state_for_repaint_proxy_event(&viewport_id, cause)
                        });
                    }

                    if current_pass_nr == cumulative_pass_nr
                        || current_pass_nr == cumulative_pass_nr + 1
                    {
                        log::trace!("UserEvent::RequestRepaint scheduling repaint at {when:?}");
                        if let Some(window_id) =
                            self.winit_app.window_id_from_viewport_id(viewport_id)
                        {
                            Ok(EventResult::RepaintAt(window_id, when))
                        } else {
                            Ok(EventResult::Wait)
                        }
                    } else {
                        log::trace!("Got outdated UserEvent::RequestRepaint");
                        Ok(EventResult::Wait) // old request - we've already repainted
                    }
                }

                // Code above checked if this is a custom user event and returned in that case,
                // so this is safe and will never be reached.
                UserEvent::ExtCustomEvent(_) => unreachable!(),
            };
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            log::trace!("Woke up to check next_repaint_time");
        }

        self.check_redraw_requests(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        profiling::function_scope!(egui_winit::short_window_event_description(&event));

        // Nb: Make sure this guard is dropped after this function returns.
        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = match event {
                winit::event::WindowEvent::RedrawRequested => {
                    self.winit_app.run_ui_and_paint(event_loop, window_id)
                }
                _ => self.winit_app.window_event(event_loop, window_id, event),
            };

            self.handle_event_result(event_loop, event_result);
        });
    }
}

// ----------------------------------------------------------------------------

pub fn create_wgpu<'a>(
    app_name: &str,
    native_options: epi::NativeOptions,
    app_creator: epi::AppCreator<'a>,
    event_loop: &EventLoop<UserEvent>,
) -> impl ApplicationHandler<UserEvent> + 'a {
    use super::wgpu_integration::WgpuWinitApp;

    let run_and_return = native_options.run_and_return;

    let wgpu_xframe = WgpuWinitApp::new(event_loop, app_name, native_options, app_creator);
    WinitAppWrapper::new(wgpu_xframe, run_and_return)
}
