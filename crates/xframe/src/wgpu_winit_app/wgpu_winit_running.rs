use super::{
    EventResult, Viewport, Viewports, app_icon::AppTitleIconSetter, handle_viewport_output,
    remove_viewports_not_in,
};
use crate::{AppLifeCycleState, Frame, Result, XFrameApp, wgpu_winit_app::AppLifeCycleHandler};
use ahash::HashMap;
use egui::{FullOutput, ViewportId, ViewportIdSet};
use egui_winit::ActionRequested;
use std::{cell::RefCell, num::NonZeroU32, rc::Rc, sync::Arc, time::Instant};
use winit::{
    event::{DeviceEvent, WindowEvent},
    window::{Window, WindowId},
};

/// State that is initialized when the application is first starts running via
/// a Resumed event. On Android this ensures that any graphics state is only
/// initialized once the application has an associated `SurfaceView`.
pub(crate) struct WgpuWinitRunning<U: 'static> {
    frame: Frame,
    beginning: Instant,
    is_first_frame: bool,
    egui_ctx: egui::Context,
    pending_full_output: egui::FullOutput,
    /// When set, it is time to close the native window.
    close: bool,
    can_drag_window: bool,
    #[cfg(feature = "persistence")]
    persist_window: bool,
    app_icon_setter: AppTitleIconSetter,

    /// The users application.
    app: Box<dyn XFrameApp<U>>,

    app_life_cycle_handler: AppLifeCycleHandler,

    /// Wrapped in an `Rc<RefCell<…>>` so it can be re-entrantly shared via a weak-pointer.
    shared: Rc<RefCell<SharedState>>,
}

impl<U: Send> WgpuWinitRunning<U> {
    pub fn new(
        frame: Frame,
        beginning: Instant,
        egui_ctx: egui::Context,
        app_icon_setter: AppTitleIconSetter,
        app: Box<dyn XFrameApp<U>>,
        shared: Rc<RefCell<SharedState>>,
    ) -> Self {
        let (app_life_cycle_handler, next_state) = AppLifeCycleHandler::new(egui_ctx.clone());

        let mut this = Self {
            frame,
            beginning,
            is_first_frame: true,
            egui_ctx,
            pending_full_output: Default::default(),
            close: false,
            can_drag_window: false,
            app_icon_setter,
            app,
            app_life_cycle_handler,
            shared,
        };

        this.handle_next_app_life_cycle_state(next_state);

        this
    }

    pub fn egui_ctx(&self) -> &egui::Context {
        &self.egui_ctx
    }

    pub fn shared(&self) -> &Rc<RefCell<SharedState>> {
        &self.shared
    }

    pub fn window(&self, window_id: WindowId) -> Option<Arc<Window>> {
        let shared = self.shared.borrow();
        let id = shared.viewport_from_window.get(&window_id)?;

        shared.viewports.get(id).map(|v| v.window.clone()).flatten()
    }

    /// This is called both for the root viewport, and all deferred viewports
    pub fn run_ui_and_paint(&mut self, window_id: WindowId) -> Result<EventResult> {
        profiling::function_scope!();

        let Self {
            app,
            egui_ctx,
            shared,
            ..
        } = self;

        let Some(viewport_id) = shared
            .borrow()
            .viewport_from_window
            .get(&window_id)
            .copied()
        else {
            return Ok(EventResult::Wait);
        };

        profiling::finish_frame!();

        let (viewport_ui_cb, mut raw_input) = {
            profiling::scope!("Prepare");
            let mut shared_lock = shared.borrow_mut();

            let SharedState {
                viewports, painter, ..
            } = &mut *shared_lock;

            if viewport_id != ViewportId::ROOT {
                let Some(viewport) = viewports.get(&viewport_id) else {
                    return Ok(EventResult::Wait);
                };

                if viewport.viewport_ui_cb.is_none() {
                    // This will only happen if this is an immediate viewport.
                    // That means that the viewport cannot be rendered by itself and needs his parent to be rendered.
                    if let Some(viewport) = viewports.get(&viewport.ids.parent)
                        && let Some(window) = viewport.window.as_ref()
                    {
                        return Ok(EventResult::RepaintNext(window.id()));
                    }
                    return Ok(EventResult::Wait);
                }
            }

            let Some(viewport) = viewports.get_mut(&viewport_id) else {
                return Ok(EventResult::Wait);
            };

            let Viewport {
                viewport_ui_cb,
                window,
                egui_winit,
                info,
                ..
            } = viewport;

            let viewport_ui_cb = viewport_ui_cb.clone();

            let Some(window) = window else {
                return Ok(EventResult::Wait);
            };
            egui_winit::update_viewport_info(info, &egui_ctx, window, false);

            {
                profiling::scope!("set_window");
                pollster::block_on(painter.set_window(viewport_id, Some(Arc::clone(window))))?;
            }

            let Some(egui_winit) = egui_winit.as_mut() else {
                return Ok(EventResult::Wait);
            };
            let mut raw_input = egui_winit.take_egui_input(window);

            self.app_icon_setter.update();

            raw_input.time = Some(self.beginning.elapsed().as_secs_f64());
            raw_input.viewports = viewports
                .iter()
                .map(|(id, viewport)| (*id, viewport.info.clone()))
                .collect();

            painter.handle_screenshots(&mut raw_input.events);

            (viewport_ui_cb, raw_input)
        };

        // ------------------------------------------------------------
        // Safe-area

        // Removes the safe-area-insets from the 'raw_input', so they are not used
        // by egui on the topmost view and stores the insets, so they can be used
        // for the inner ui elements where needed.
        //
        // This insets are provided through the egui context extension `epi::EguiContextSafeAreaExt`.
        //
        // ATTENTION: The insets are set from egui-winit on the raw_input, but only for iOS.
        // At some point there should be support for Android + macOS to, as soon winit
        // provides that.
        // Until then safe-area-insets can be provided for other platforms from outside through
        // the `XFrameProxy`
        //
        // -> TODO: check safe-area handling on new releases - maybe adjustments are needed!
        if let Some(s) = raw_input.safe_area_insets.take() {
            crate::update_egui_context_safe_area_insets(s.0);
        }

        // ------------------------------------------------------------

        // Run user code - this can create immediate viewports, so hold no locks over this!
        //
        // If `viewport_ui_cb` is None, we are in the root viewport and will call [`crate::App::update`].
        // Runs the update, which could call immediate viewports,
        // so make sure we hold no locks here!
        let full_output = {
            let viewport_ui_cb = viewport_ui_cb.as_deref();

            raw_input.time = Some(self.beginning.elapsed().as_secs_f64());

            let close_requested = raw_input.viewport().close_requested();

            app.raw_input_hook(&self.egui_ctx, &mut raw_input);

            let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
                if let Some(viewport_ui_cb) = viewport_ui_cb {
                    // Child viewport
                    profiling::scope!("viewport_callback");
                    viewport_ui_cb(ui);
                } else {
                    {
                        profiling::scope!("App::logic");
                        app.logic(ui.ctx(), &mut self.frame);
                    }
                    {
                        profiling::scope!("App::ui");
                        app.ui(ui, &mut self.frame);
                    }
                }
            });

            let is_root_viewport = viewport_ui_cb.is_none();
            if is_root_viewport && close_requested {
                let canceled = full_output.viewport_output[&ViewportId::ROOT]
                    .commands
                    .contains(&egui::ViewportCommand::CancelClose);
                if canceled {
                    log::debug!(
                        "Closing of root viewport canceled with ViewportCommand::CancelClose"
                    );
                } else {
                    log::debug!(
                        "Closing root viewport (ViewportCommand::CancelClose was not sent)"
                    );
                    self.close = true;
                }
            }

            self.pending_full_output.append(full_output);
            std::mem::take(&mut self.pending_full_output)
        };

        // ------------------------------------------------------------

        let mut shared_mut = shared.borrow_mut();

        let SharedState {
            egui_ctx,
            viewports,
            painter,
            viewport_from_window,
            ..
        } = &mut *shared_mut;

        let FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output,
        } = full_output;

        remove_viewports_not_in(viewports, painter, viewport_from_window, &viewport_output);

        let Some(viewport) = viewports.get_mut(&viewport_id) else {
            return Ok(EventResult::Wait);
        };

        viewport.info.events.clear(); // they should have been processed

        let Viewport {
            window: Some(window),
            egui_winit: Some(egui_winit),
            ..
        } = viewport
        else {
            return Ok(EventResult::Wait);
        };

        egui_winit.handle_platform_output(window, platform_output);

        let clipped_primitives = egui_ctx.tessellate(shapes, pixels_per_point);

        let mut screenshot_commands = vec![];
        viewport.actions_requested.retain(|cmd| {
            if let ActionRequested::Screenshot(info) = cmd {
                screenshot_commands.push(info.clone());
                false
            } else {
                true
            }
        });
        let _ = painter.paint_and_update_textures(
            viewport_id,
            pixels_per_point,
            app.clear_color(&egui_ctx.global_style().visuals),
            &clipped_primitives,
            &textures_delta,
            screenshot_commands,
        );

        for action in viewport.actions_requested.drain(..) {
            match action {
                ActionRequested::Screenshot { .. } => {
                    // already handled above
                }
                ActionRequested::Cut => {
                    egui_winit.egui_input_mut().events.push(egui::Event::Cut);
                }
                ActionRequested::Copy => {
                    egui_winit.egui_input_mut().events.push(egui::Event::Copy);
                }
                ActionRequested::Paste => {
                    if let Some(contents) = egui_winit.clipboard_text() {
                        let contents = contents.replace("\r\n", "\n");
                        if !contents.is_empty() {
                            egui_winit
                                .egui_input_mut()
                                .events
                                .push(egui::Event::Paste(contents));
                        }
                    }
                }
            }
        }

        // previous -> EpiIntegration::post_rendering
        if std::mem::take(&mut self.is_first_frame) {
            // We keep hidden until we've painted something. See https://github.com/emilk/egui/pull/2279
            window.set_visible(true);
        }

        let active_viewports_ids: ViewportIdSet = viewport_output.keys().copied().collect();

        handle_viewport_output(
            &self.egui_ctx,
            &viewport_output,
            viewports,
            painter,
            viewport_from_window,
        );

        // Prune dead viewports:
        viewports.retain(|id, _| active_viewports_ids.contains(id));
        viewport_from_window.retain(|_, id| active_viewports_ids.contains(id));
        painter.gc_viewports(&active_viewports_ids);

        let window = viewport_from_window
            .get(&window_id)
            .and_then(|id| viewports.get(id))
            .and_then(|vp| vp.window.as_ref());

        if let Some(window) = window
            && window.is_minimized() == Some(true)
        {
            // On Mac, a minimized Window uses up all CPU:
            // https://github.com/emilk/egui/issues/325
            profiling::scope!("minimized_sleep");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if self.close {
            Ok(EventResult::CloseRequested)
        } else {
            Ok(EventResult::Wait)
        }
    }

    pub fn on_window_event(&mut self, window_id: WindowId, event: &WindowEvent) -> EventResult {
        let life_cycle_state_result = self.app_life_cycle_handler.handle_window_event(event);
        self.handle_app_life_cycle_result(life_cycle_state_result);

        let Self { shared, .. } = self;
        let mut shared = shared.borrow_mut();

        let viewport_id = shared.viewport_from_window.get(&window_id).copied();

        // On Windows, if a window is resized by the user, it should repaint synchronously, inside the
        // event handler. If this is not done, the compositor will assume that the window does not want
        // to redraw and continue ahead.
        //
        // In xframe's case, that causes the window to rapidly flicker, as it struggles to deliver
        // new frames to the compositor in time. The flickering is technically glutin or glow's fault, but we should be responding properly
        // to resizes anyway, as doing so avoids dropping frames.
        //
        // See: https://github.com/emilk/egui/issues/903
        let mut repaint_asap = false;

        // On MacOS the asap repaint is not enough. The drawn frames must be synchronized with
        // the CoreAnimation transactions driving the window resize process.
        //
        // Thus, Painter, responsible for wgpu surfaces and their resize, has to be notified of the
        // resize lifecycle, yet winit does not provide any events for that. To work around,
        // the last resized viewport is tracked until any next non-resize event is received.
        //
        // Accidental state change during the resize process due to an unexpected event fire
        // is ok, state will switch back upon next resize event.
        //
        // See: https://github.com/emilk/egui/issues/903
        if let Some(id) = viewport_id
            && shared.resized_viewport == viewport_id
        {
            shared.painter.on_window_resize_state_change(id, false);
            shared.resized_viewport = None;
        }

        match event {
            WindowEvent::Focused(focused) => {
                let focused = if cfg!(target_os = "macos")
                    && let Some(viewport_id) = viewport_id
                    && let Some(viewport) = shared.viewports.get(&viewport_id)
                    && let Some(window) = &viewport.window
                {
                    // TODO(emilk): remove this work-around once we update winit
                    // https://github.com/rust-windowing/winit/issues/4371
                    // https://github.com/emilk/egui/issues/7588
                    window.has_focus()
                } else {
                    *focused
                };

                shared.focused_viewport = focused.then_some(viewport_id).flatten();
            }

            WindowEvent::Resized(physical_size) => {
                // Resize with 0 width and height is used by winit to signal a minimize event on Windows.
                // See: https://github.com/rust-windowing/winit/issues/208
                // This solves an issue where the app would panic when minimizing on Windows.
                if let Some(id) = viewport_id
                    && let (Some(width), Some(height)) = (
                        NonZeroU32::new(physical_size.width),
                        NonZeroU32::new(physical_size.height),
                    )
                {
                    if shared.resized_viewport != viewport_id {
                        shared.resized_viewport = viewport_id;
                        shared.painter.on_window_resize_state_change(id, true);
                    }
                    shared.painter.on_window_resized(id, width, height);
                    repaint_asap = true;
                }
            }

            WindowEvent::CloseRequested => {
                if viewport_id == Some(ViewportId::ROOT) && self.close {
                    log::debug!(
                        "Received WindowEvent::CloseRequested for main viewport - shutting down."
                    );
                    return EventResult::CloseRequested;
                }

                log::debug!("Received WindowEvent::CloseRequested for viewport {viewport_id:?}");

                if let Some(viewport_id) = viewport_id
                    && let Some(viewport) = shared.viewports.get_mut(&viewport_id)
                {
                    // Tell viewport it should close:
                    viewport.info.events.push(egui::ViewportEvent::Close);

                    // We may need to repaint both us and our parent to close the window,
                    // and perhaps twice (once to notice the close-event, once again to enforce it).
                    // `request_repaint_of` does a double-repaint though:
                    self.egui_ctx.request_repaint_of(viewport_id);
                    self.egui_ctx.request_repaint_of(viewport.ids.parent);
                }
            }

            _ => {}
        }

        let event_response = viewport_id
            .and_then(|viewport_id| {
                use winit::event::{ElementState, MouseButton};

                let viewport = shared.viewports.get_mut(&viewport_id)?;
                let window = viewport.window.as_deref()?;
                let egui_winit = viewport.egui_winit.as_mut()?;

                if let WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                    ..
                } = event
                {
                    self.can_drag_window = true;
                }

                Some(egui_winit.on_window_event(window, event))
            })
            .unwrap_or_default();

        if self.close {
            EventResult::CloseRequested
        } else if event_response.repaint {
            if repaint_asap {
                EventResult::RepaintNow(window_id)
            } else {
                EventResult::RepaintNext(window_id)
            }
        } else {
            EventResult::Wait
        }
    }

    pub fn on_resumed_event(&mut self) {
        let life_cycle_state_result = self.app_life_cycle_handler.handle_resumed_event();
        self.handle_app_life_cycle_result(life_cycle_state_result);
    }

    pub fn on_suspended_event(&mut self) {
        let life_cycle_state_result = self.app_life_cycle_handler.handle_suspended_event();
        self.handle_app_life_cycle_result(life_cycle_state_result);
    }

    pub fn on_destroy_event(&mut self) {
        profiling::function_scope!();

        let life_cycle_state_result = self.app_life_cycle_handler.handle_app_destroy_indication();
        self.handle_app_life_cycle_result(life_cycle_state_result);

        let mut shared = self.shared.borrow_mut();
        shared.painter.destroy();
    }

    pub fn on_app_user_event(&mut self, event: U) {
        self.app.on_user_event(event);
    }

    /// Sends the event to the user app - the user app returns the event back
    /// if it should be processed, otherwise `None`.
    pub fn on_app_intercept_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
        self.app.winit_window_event_hook(event)
    }

    /// Sends the event to the user app - the user app returns the event back
    /// if it should be processed, otherwise `None`.
    pub fn on_app_intercept_device_event(&mut self, event: DeviceEvent) -> Option<DeviceEvent> {
        self.app.winit_device_event_hook(event)
    }

    fn handle_app_life_cycle_result(&mut self, result: Option<Vec<AppLifeCycleState>>) {
        let Some(states_vec) = result else {
            return;
        };

        for next_state in states_vec {
            self.handle_next_app_life_cycle_state(next_state);
        }
    }

    fn handle_next_app_life_cycle_state(&mut self, next_state: AppLifeCycleState) {
        if next_state.is_did_exit() {
            self.app.on_app_life_cycle_state_change(next_state);
            self.app.on_exit();
            return;
        }

        if next_state.is_foreground_paused_was_active() {
            self.handle_app_save();
        }

        self.app.on_app_life_cycle_state_change(next_state);
    }

    /// Handles saving of the application state
    fn handle_app_save(&mut self) {
        let shared = self.shared.borrow();
        // This is done because of the "save on suspend" logic on Android. Once the application is suspended, there is no window associated to it.
        let window = if let Some(Viewport { window, .. }) = shared.viewports.get(&ViewportId::ROOT)
        {
            window.as_deref()
        } else {
            None
        };

        #[cfg(not(feature = "persistence"))]
        let _ = window;

        #[cfg(feature = "persistence")]
        if let Some(storage) = self.frame.storage_mut() {
            profiling::function_scope!();

            if let Some(window) = window
                && self.persist_window
            {
                profiling::scope!("native_window");
                crate::epi::set_value(
                    storage,
                    super::STORAGE_WINDOW_KEY,
                    &WindowSettings::from_window(self.egui_ctx.zoom_factor(), window),
                );
            }
            if self.app.persist_egui_memory() {
                profiling::scope!("egui_memory");
                self.egui_ctx.memory(|mem| {
                    crate::epi::set_value(storage, super::STORAGE_EGUI_MEMORY_KEY, mem)
                });
            }
            {
                profiling::scope!("App::on_save");
                self.app.on_save(storage);
            }

            profiling::scope!("Storage::flush");
            storage.flush();
        }
    }
}

// ----------------------------------------------------------------------------

/// Everything needed by the immediate viewport renderer.\
///
/// This is shared by all viewports.
///
/// Wrapped in an `Rc<RefCell<…>>` so it can be re-entrantly shared via a weak-pointer.
pub(crate) struct SharedState {
    pub egui_ctx: egui::Context,
    pub viewports: Viewports,
    pub painter: egui_wgpu::winit::Painter,
    pub viewport_from_window: HashMap<WindowId, ViewportId>,
    pub focused_viewport: Option<ViewportId>,
    pub resized_viewport: Option<ViewportId>,
}
