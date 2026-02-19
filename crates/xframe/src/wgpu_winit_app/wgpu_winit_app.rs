use super::{
    EventResult, SharedState, UserEvent, Viewport, Viewports, WgpuWinitRunning, create_window,
    event_loop_context, load_default_egui_icon, render_immediate_viewport,
};
use crate::{NativeOptions, Result, StartContext, Storage, XFrameApp};
use ahash::HashMap;
use egui::{ViewportBuilder, ViewportClass, ViewportId, ViewportIdPair, ViewportInfo};
use raw_window_handle::{HasDisplayHandle as _, HasWindowHandle as _};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    window::{Window, WindowId},
};

pub struct WgpuWinitApp<U: 'static> {
    windows_next_repaint_times: HashMap<WindowId, Instant>,
    return_result: Result<(), crate::Error>,

    repaint_proxy: EventLoopProxy<UserEvent<U>>,

    app_name: String,
    native_options: NativeOptions,

    /// Set until [Self::running] is set - there the props are passed
    /// to [WgpuWinitRunning].
    start_props: Option<(
        Option<Box<dyn Storage>>,
        egui::Context,
        Box<dyn XFrameApp<U>>,
    )>,

    /// Set when we are actually up and running.
    running: Option<WgpuWinitRunning<U>>,
}

impl<U: Send> WgpuWinitApp<U> {
    pub fn new(
        event_loop_proxy: EventLoopProxy<UserEvent<U>>,
        app_name: String,
        storage: Option<Box<dyn Storage>>,
        egui_ctx: egui::Context,
        app: Box<dyn XFrameApp<U>>,
        native_options: NativeOptions,
    ) -> Self {
        profiling::function_scope!();

        Self {
            windows_next_repaint_times: HashMap::default(),
            return_result: Ok(()),
            repaint_proxy: event_loop_proxy,
            app_name,
            native_options,
            start_props: Some((storage, egui_ctx, app)),
            running: None,
        }
    }

    /// Create a window for all viewports lacking one.
    fn initialized_all_windows(&mut self, event_loop: &ActiveEventLoop) {
        let Some(running) = &mut self.running else {
            return;
        };
        let mut shared = running.shared().borrow_mut();
        let SharedState {
            viewports,
            painter,
            viewport_from_window,
            ..
        } = &mut *shared;

        for viewport in viewports.values_mut() {
            viewport.initialize_window(
                event_loop,
                running.egui_ctx(),
                viewport_from_window,
                painter,
            );
        }
    }

    fn init_run_state(
        &mut self,
        mut app: Box<dyn XFrameApp<U>>,
        egui_ctx: egui::Context,
        event_loop: &ActiveEventLoop,
        storage: Option<Box<dyn Storage>>,
        window: Window,
        builder: ViewportBuilder,
    ) -> crate::Result<&mut WgpuWinitRunning<U>> {
        profiling::function_scope!();
        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            egui_ctx.clone(),
            self.native_options.wgpu_options.clone(),
            self.native_options.viewport.transparent.unwrap_or(false),
            egui_wgpu::RendererOptions {
                // Sets the level of the multisampling anti-aliasing (MSAA).
                // Must be a power-of-two. Higher = more smooth 3D.
                // A value of `0` turns it off.
                //
                // `egui` already performs anti-aliasing via "feathering"
                // (controlled by [`egui::epaint::TessellationOptions`]),
                // but if you are embedding 3D in egui you may want to turn on multisampling.
                msaa_samples: 0,
                depth_stencil_format: egui_wgpu::depth_format_from_bits(
                    0, // `egui` doesn't need the depth buffer, so the value is set to 0
                    0, // `egui` doesn't need the stencil buffer, so the value is set to 0
                ),
                // Controls whether to apply dithering to minimize banding artifacts.
                //
                // Dithering assumes an sRGB output and thus will apply noise to any input value that lies between
                // two 8bit values after applying the sRGB OETF function, i.e. if it's not a whole 8bit value in "gamma space".
                // This means that only inputs from texture interpolation and vertex colors should be affected in practice.
                dithering: true,
                ..Default::default()
            },
        ));

        let mut viewport_info = ViewportInfo::default();
        egui_winit::update_viewport_info(&mut viewport_info, &egui_ctx, &window, true);

        {
            // Tell egui right away about native_pixels_per_point etc,
            // so that the app knows about it during app creation:
            let pixels_per_point = egui_winit::pixels_per_point(&egui_ctx, &window);

            egui_ctx.input_mut(|i| {
                i.raw
                    .viewports
                    .insert(ViewportId::ROOT, viewport_info.clone());
                i.pixels_per_point = pixels_per_point;
            });
        }

        let window = Arc::new(window);

        {
            profiling::scope!("set_window");
            pollster::block_on(painter.set_window(ViewportId::ROOT, Some(Arc::clone(&window))))?;
        }

        let wgpu_render_state = painter.render_state();

        {
            let event_loop_proxy = self.repaint_proxy.clone();

            egui_ctx.set_request_repaint_callback(move |info, repaint_proxy_send_cause| {
                log::trace!("request_repaint_callback: {info:?}");
                let when = Instant::now() + info.delay;
                let cumulative_pass_nr = info.current_cumulative_pass_nr;

                event_loop_proxy
                    .send_event(UserEvent::RequestRepaint {
                        when,
                        cumulative_pass_nr,
                        viewport_id: info.viewport_id,
                        repaint_proxy_send_cause,
                    })
                    .ok();
            });
        }

        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            event_loop.system_theme(),
            painter.max_texture_side(),
        );

        let frame = crate::epi::Frame {
            storage,
            wgpu_render_state: wgpu_render_state.clone(),
            raw_display_handle: window.display_handle().map(|h| h.as_raw()),
            raw_window_handle: window.window_handle().map(|h| h.as_raw()),
        };

        let start_context = StartContext {
            egui_ctx: &egui_ctx,
            storage: frame.storage(),
            wgpu_render_state,
            raw_display_handle: window.display_handle().map(|h| h.as_raw()),
            raw_window_handle: window.window_handle().map(|h| h.as_raw()),
        };
        app.on_start(&start_context);

        let mut viewport_from_window = HashMap::default();
        viewport_from_window.insert(window.id(), ViewportId::ROOT);

        let mut viewports = Viewports::default();
        viewports.insert(
            ViewportId::ROOT,
            Viewport {
                ids: ViewportIdPair::ROOT,
                class: ViewportClass::Root,
                builder,
                deferred_commands: vec![],
                info: viewport_info,
                actions_requested: Default::default(),
                viewport_ui_cb: None,
                window: Some(window),
                egui_winit: Some(egui_winit),
            },
        );

        let shared = Rc::new(RefCell::new(SharedState {
            egui_ctx: egui_ctx.clone(),
            viewport_from_window,
            viewports,
            painter,
            focused_viewport: Some(ViewportId::ROOT),
            resized_viewport: None,
        }));

        let beginning = Instant::now();
        {
            // Create a weak pointer so that we don't keep state alive for too long.
            let shared = Rc::downgrade(&shared);

            egui::Context::set_immediate_viewport_renderer(move |_egui_ctx, immediate_viewport| {
                if let Some(shared) = shared.upgrade() {
                    render_immediate_viewport(beginning.clone(), &shared, immediate_viewport);
                } else {
                    log::warn!("render_sync_callback called after window closed");
                }
            });
        }

        let icon = self
            .native_options
            .viewport
            .icon
            .clone()
            .unwrap_or_else(|| Rc::new(load_default_egui_icon()));

        Ok(self.running.insert(WgpuWinitRunning::new(
            frame,
            beginning,
            egui_ctx,
            super::app_icon::AppTitleIconSetter::new(
                self.native_options
                    .viewport
                    .title
                    .clone()
                    .unwrap_or_else(|| self.app_name.clone()),
                Some(icon),
            ),
            app,
            shared,
        )))
    }

    fn check_redraw_requests(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();

        let Self {
            windows_next_repaint_times,
            running,
            ..
        } = self;

        windows_next_repaint_times.retain(|window_id, repaint_time| {
            if now < *repaint_time {
                return true; // not yet ready
            }

            event_loop.set_control_flow(ControlFlow::Poll);

            if let Some(window) = running.as_ref().and_then(|r| r.window(*window_id)) {
                log::trace!("request_redraw for {window_id:?}");
                window.request_redraw();
            } else {
                log::trace!("No window found for {window_id:?}");
            }

            false
        });

        let next_repaint_time = windows_next_repaint_times.values().min().copied();
        if let Some(next_repaint_time) = next_repaint_time {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_repaint_time));
        }
    }

    fn handle_event_result(
        &mut self,
        event_loop: &ActiveEventLoop,
        event_result: Result<EventResult>,
    ) {
        let mut exit = false;

        log::trace!("event_result: {event_result:?}");

        let mut event_result = event_result;

        if cfg!(target_os = "windows")
            && let Ok(EventResult::RepaintNow(window_id)) = event_result
        {
            log::trace!("RepaintNow of {window_id:?}");
            self.windows_next_repaint_times
                .insert(window_id, Instant::now());

            // Fix flickering on Windows, see https://github.com/emilk/egui/pull/2280
            event_result = self.run_ui_and_paint(event_loop, window_id);
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
            EventResult::Exit => {
                exit = true;
                event_result
            }
            EventResult::CloseRequested => {
                // The windows need to be dropped whilst the event loop is running to allow for proper cleanup.
                self.destroy();
                event_result
            }
        });

        if let Err(err) = combined_result {
            log::error!("Exiting because of error: {err}");
            exit = true;
            self.return_result = Err(err);
        }

        if exit {
            if self.native_options.run_and_return {
                log::debug!("Asking to exit event loop…");
                event_loop.exit();
            } else {
                log::debug!("Quitting…");
                self.destroy();

                log::debug!("Exiting with return code 0");

                std::process::exit(0);
            }
        }

        self.check_redraw_requests(event_loop);
    }
}

impl<U: Send> WgpuWinitApp<U> {
    fn egui_ctx(&self) -> Option<&egui::Context> {
        self.running.as_ref().map(|r| r.egui_ctx())
    }

    fn window_id_from_viewport_id(&self, id: ViewportId) -> Option<WindowId> {
        Some(
            self.running
                .as_ref()?
                .shared()
                .borrow()
                .viewports
                .get(&id)?
                .window
                .as_ref()?
                .id(),
        )
    }

    fn destroy(&mut self) {
        if let Some(mut running) = self.running.take() {
            running.on_destroy_event();
        }
    }

    fn run_ui_and_paint(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
    ) -> Result<EventResult> {
        self.initialized_all_windows(event_loop);

        if let Some(running) = &mut self.running {
            running.run_ui_and_paint(window_id)
        } else {
            Ok(EventResult::Wait)
        }
    }

    #[cfg(target_os = "android")]
    fn android_recreate_window(
        view_port_builder: ViewportBuilder,
        event_loop: &ActiveEventLoop,
        running: &WgpuWinitRunning<U>,
    ) {
        let SharedState {
            egui_ctx,
            viewports,
            viewport_from_window,
            painter,
            ..
        } = &mut *running.shared().borrow_mut();

        super::initialize_or_update_viewport(
            viewports,
            ViewportIdPair::ROOT,
            ViewportClass::Root,
            view_port_builder,
            None,
            painter,
        )
        .initialize_window(event_loop, egui_ctx, viewport_from_window, painter);
    }

    fn handle_resumed(&mut self, event_loop: &ActiveEventLoop) -> crate::Result<EventResult> {
        log::debug!("Event::Resumed");

        let running = if let Some(running) = &mut self.running {
            #[cfg(target_os = "android")]
            {
                let view_port_builder = self.native_options.viewport.clone();
                Self::android_recreate_window(view_port_builder, event_loop, running);
            }
            running.on_resumed_event();
            running
        } else {
            let (storage, egui_ctx, app) = self
                .start_props
                .take()
                .expect("Single-use start props have unexpectedly already been taken");

            let (window, builder) = create_window(
                &egui_ctx,
                event_loop,
                storage.as_deref(),
                &mut self.native_options,
            )?;
            self.init_run_state(app, egui_ctx, event_loop, storage, window, builder)?
        };

        let viewport = &running.shared().borrow().viewports[&ViewportId::ROOT];
        if let Some(window) = &viewport.window {
            Ok(EventResult::RepaintNow(window.id()))
        } else {
            Ok(EventResult::Wait)
        }
    }

    #[cfg(target_os = "android")]
    fn android_drop_window(running: &mut WgpuWinitRunning<U>) -> Result<(), egui_wgpu::WgpuError> {
        let mut shared = running.shared().borrow_mut();
        shared.viewports.remove(&ViewportId::ROOT);
        pollster::block_on(shared.painter.set_window(ViewportId::ROOT, None))?;
        Ok(())
    }

    fn handle_suspended(&mut self, _: &ActiveEventLoop) -> crate::Result<EventResult> {
        let Some(running) = &mut self.running else {
            return Ok(EventResult::Exit);
        };

        running.on_suspended_event();

        #[cfg(target_os = "android")]
        Self::android_drop_window(running)?;

        if let Some(window) = &running
            .shared()
            .borrow()
            .viewports
            .get(&ViewportId::ROOT)
            .map(|v| v.window.clone())
            .flatten()
        {
            return Ok(EventResult::RepaintNow(window.id()));
        }

        Ok(EventResult::Wait)
    }

    fn handle_device_event(
        &mut self,
        _: &ActiveEventLoop,
        _: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) -> crate::Result<EventResult> {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event
            && let Some(running) = &mut self.running
        {
            let mut shared = running.shared().borrow_mut();
            if let Some(viewport) = shared
                .focused_viewport
                .and_then(|viewport| shared.viewports.get_mut(&viewport))
            {
                if let Some(egui_winit) = viewport.egui_winit.as_mut() {
                    egui_winit.on_mouse_motion(delta);
                }

                if let Some(window) = viewport.window.as_ref() {
                    return Ok(EventResult::RepaintNext(window.id()));
                }
            }
        }

        Ok(EventResult::Wait)
    }
}

impl<U: Send> ApplicationHandler<UserEvent<U>> for WgpuWinitApp<U> {
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        profiling::scope!("Event::Suspended");

        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = self.handle_suspended(event_loop);
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        profiling::scope!("Event::Resumed");

        // Nb: Make sure this guard is dropped after this function returns.
        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = self.handle_resumed(event_loop);
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        // On Mac, Cmd-Q we get here and then `run_app_on_demand` doesn't return (despite its name),
        // so we need to save state now:
        log::debug!("Received Event::LoopExiting…");
        event_loop_context::with_event_loop_context(event_loop, move || {
            self.destroy();
        });
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        profiling::function_scope!(egui_winit::short_device_event_description(&event));

        // Gives the running app the chance to intercept the event
        // and to ignore it if the app decides to do so.
        let event = if let Some(running) = &mut self.running {
            match running.on_app_intercept_device_event(event) {
                Some(e) => e,
                None => return,
            }
        } else {
            event
        };

        // Nb: Make sure this guard is dropped after this function returns.
        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = self.handle_device_event(event_loop, device_id, event);
            self.handle_event_result(event_loop, event_result);
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent<U>) {
        profiling::function_scope!(match &event {
            UserEvent::RequestRepaint { .. } => "UserEvent::RequestRepaint",
            UserEvent::ExtCustomEvent(_) => "UserEvent::ExtCustomEvent",
            UserEvent::ExtSafeAreaInsets(_) => "UserEvent::ExtSafeAreaInsets",
            UserEvent::ExtTouchLocation(_, _) => "UserEvent::ExtTouchLocation",
        });

        let (when, mut cumulative_pass_nr, viewport_id, repaint_proxy_send_cause) = match event {
            UserEvent::RequestRepaint {
                viewport_id,
                when,
                cumulative_pass_nr,
                repaint_proxy_send_cause,
            } => (
                when,
                cumulative_pass_nr,
                viewport_id,
                repaint_proxy_send_cause,
            ),
            UserEvent::ExtSafeAreaInsets(insets) => {
                crate::update_egui_context_safe_area_insets(insets);
                return;
            }
            UserEvent::ExtTouchLocation(location, touch_phase) => {
                let window_id = match self.egui_ctx() {
                    Some(ctx) => match self.window_id_from_viewport_id(ctx.viewport_id()) {
                        Some(id) => id,
                        None => return,
                    },
                    None => return,
                };

                self.window_event(
                    event_loop,
                    window_id,
                    winit::event::WindowEvent::Touch(winit::event::Touch {
                        device_id: winit::event::DeviceId::dummy(),
                        phase: touch_phase,
                        location,
                        force: None,
                        id: 0,
                    }),
                );

                return;
            }
            UserEvent::ExtCustomEvent(event) => {
                if let Some(running) = &mut self.running {
                    running.on_app_user_event(event);
                    return;
                }

                if let Some((_, _, app)) = &mut self.start_props {
                    app.on_user_event(event);
                    return;
                }

                return;
            }
        };

        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = {
                let current_pass_nr = self
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
                    self.egui_ctx().map(|ctx| {
                        ctx.adjust_viewport_state_for_repaint_proxy_event(&viewport_id, cause)
                    });
                }

                if current_pass_nr == cumulative_pass_nr
                    || current_pass_nr == cumulative_pass_nr + 1
                {
                    log::trace!("UserEvent::RequestRepaint scheduling repaint at {when:?}");
                    if let Some(window_id) = self.window_id_from_viewport_id(viewport_id) {
                        Ok(EventResult::RepaintAt(window_id, when))
                    } else {
                        Ok(EventResult::Wait)
                    }
                } else {
                    log::trace!("Got outdated UserEvent::RequestRepaint");
                    Ok(EventResult::Wait) // old request - we've already repainted
                }
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

        // Gives the running app the chance to intercept the event
        // and to ignore it if the app decides to do so.
        let event = if let Some(running) = &mut self.running {
            match running.on_app_intercept_window_event(event) {
                Some(e) => e,
                None => return,
            }
        } else {
            event
        };

        // Nb: Make sure this guard is dropped after this function returns.
        event_loop_context::with_event_loop_context(event_loop, move || {
            let event_result = match event {
                winit::event::WindowEvent::RedrawRequested => {
                    self.run_ui_and_paint(event_loop, window_id)
                }
                _ => {
                    self.initialized_all_windows(event_loop);

                    if let Some(running) = &mut self.running {
                        Ok(running.on_window_event(window_id, &event))
                    } else {
                        // running is removed to get ready for exiting
                        Ok(EventResult::Exit)
                    }
                }
            };

            self.handle_event_result(event_loop, event_result);
        });
    }
}
