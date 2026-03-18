mod app_icon;
mod app_life_cycle_handler;
mod event_loop_context;
mod wgpu_winit_app;
mod wgpu_winit_running;

pub(super) use app_life_cycle_handler::AppLifeCycleHandler;
pub(super) use wgpu_winit_app::WgpuWinitApp;
pub(super) use wgpu_winit_running::{SharedState, WgpuWinitRunning};

/// File storage which can be used by native backends.
#[cfg(feature = "persistence")]
pub mod file_storage;

// ----------------------------------------------------------------------------

use crate::{NativeOptions, Result, Storage};
use ahash::HashMap;
use egui::{
    DeferredViewportUiCallback, ImmediateViewport, OrderedViewportIdMap, RepaintCause,
    ViewportBuilder, ViewportClass, ViewportId, ViewportIdPair, ViewportIdSet, ViewportInfo,
    ViewportOutput, epaint::MarginF32,
};
use egui_winit::{ActionRequested, WindowSettings};
use std::{cell::RefCell, num::NonZeroU32, path::PathBuf, sync::Arc, time::Instant};
use winit::{
    dpi::PhysicalPosition,
    event::TouchPhase,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

// ----------------------------------------------------------------------------

/// The custom event `xframe` uses with the [`winit`] event loop.
#[derive(Debug)]
pub enum UserEvent<U: 'static> {
    /// A repaint is requested.
    RequestRepaint {
        /// What to repaint.
        viewport_id: ViewportId,

        /// When to repaint.
        when: Instant,

        /// What the cumulative pass number was when the repaint was _requested_.
        ///
        /// Note: The value can be `0` if the request was sent through the `RequestRepaintProxy`.
        /// In that case a `repaint_proxy_send_cause` is set.
        cumulative_pass_nr: u64,

        /// This is set if the request was sent through the `RequestRepaintProxy`.
        repaint_proxy_send_cause: Option<RepaintCause>,
    },

    /// Variant for 'xframe' to inject safe area insets on platforms where winit does
    /// not support them yet, through the 'XFrameProxy'.
    ExtSafeAreaInsets(MarginF32),

    /// Variant for 'xframe' to inject touch-location through the 'XFrameProxy'.
    ///
    /// Eg. for Android to send events that winit does not receive because the events
    /// happen on the back area.
    ///
    /// - boolean argument: 'true' - touch started / 'false' - touch moved
    ExtTouchLocation(PhysicalPosition<f64>, TouchPhase),

    /// Variant for 'xframe' users to run code / pass data to the event-loop
    /// thread through the 'XFrameProxy'.
    ExtCustomEvent(U),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    Wait,

    /// Causes a synchronous repaint inside the event handler. This should only
    /// be used in special situations if the window must be repainted while
    /// handling a specific event. This occurs on Windows when handling resizes.
    ///
    /// `RepaintNow` creates a new frame synchronously, and should therefore
    /// only be used for extremely urgent repaints.
    RepaintNow(WindowId),

    /// Queues a repaint for once the event loop handles its next redraw. Exists
    /// so that multiple input events can be handled in one frame. Does not
    /// cause any delay like `RepaintNow`.
    RepaintNext(WindowId),

    RepaintAt(WindowId, Instant),

    /// Causes a save of the client state when the persistence feature is enabled.
    //Save,

    /// Starts the process of ending xframe execution whilst allowing for proper
    /// clean up of resources.
    ///
    /// # Warning
    /// This event **must** occur before [`Exit`] to correctly exit xframe code.
    /// If in doubt, return this event.
    ///
    /// [`Exit`]: [EventResult::Exit]
    CloseRequested,

    /// The event loop will exit, now.
    /// The correct circumstance to return this event is in response to a winit "Destroyed" event.
    ///
    /// # Warning
    /// The [`CloseRequested`] **must** occur before this event to ensure that winit
    /// is able to remove any open windows. Otherwise the window(s) will remain open
    /// until the program terminates.
    ///
    /// [`CloseRequested`]: EventResult::CloseRequested
    Exit,
}

// ----------------------------------------------------------------------------

pub(super) struct Viewport {
    pub ids: ViewportIdPair,
    pub class: ViewportClass,
    pub builder: ViewportBuilder,
    pub deferred_commands: Vec<egui::viewport::ViewportCommand>,
    pub info: ViewportInfo,
    pub actions_requested: Vec<ActionRequested>,

    /// `None` for sync viewports.
    pub viewport_ui_cb: Option<Arc<DeferredViewportUiCallback>>,

    /// Window surface state that's initialized when the app starts running via a Resumed event
    /// and on Android will also be destroyed if the application is paused.
    pub window: Option<Arc<Window>>,

    /// `window` and `egui_winit` are initialized together.
    pub egui_winit: Option<egui_winit::State>,
}
impl Viewport {
    /// Create winit window, if needed.
    pub fn initialize_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        egui_ctx: &egui::Context,
        windows_id: &mut HashMap<WindowId, ViewportId>,
        painter: &mut egui_wgpu::winit::Painter,
    ) {
        if self.window.is_some() {
            return; // we already have one
        }

        profiling::function_scope!();

        let viewport_id = self.ids.this;

        match egui_winit::create_window(egui_ctx, event_loop, &self.builder) {
            Ok(window) => {
                windows_id.insert(window.id(), viewport_id);

                let window = Arc::new(window);

                if let Err(err) =
                    pollster::block_on(painter.set_window(viewport_id, Some(Arc::clone(&window))))
                {
                    log::error!("on set_window: viewport_id {viewport_id:?} {err}");
                }

                self.egui_winit = Some(egui_winit::State::new(
                    egui_ctx.clone(),
                    viewport_id,
                    event_loop,
                    Some(window.scale_factor() as f32),
                    event_loop.system_theme(),
                    painter.max_texture_side(),
                ));

                egui_winit::update_viewport_info(&mut self.info, egui_ctx, &window, true);
                self.window = Some(window);
            }
            Err(err) => {
                log::error!("Failed to create window: {err}");
            }
        }
    }
}

pub(super) type Viewports = egui::OrderedViewportIdMap<Viewport>;

pub(super) fn create_window(
    egui_ctx: &egui::Context,
    event_loop: &ActiveEventLoop,
    storage: Option<&dyn Storage>,
    native_options: &mut NativeOptions,
) -> Result<(Window, ViewportBuilder), winit::error::OsError> {
    profiling::function_scope!();

    let window_settings = load_window_settings(storage);
    let viewport_builder = viewport_builder(
        egui_ctx.zoom_factor(),
        event_loop,
        native_options,
        window_settings,
    )
    .with_visible(false); // Start hidden until we render the first frame to fix white flash on startup (https://github.com/emilk/egui/pull/3631)

    let window = egui_winit::create_window(egui_ctx, event_loop, &viewport_builder)?;
    apply_window_settings(&window, window_settings);
    Ok((window, viewport_builder))
}

pub(super) fn render_immediate_viewport(
    beginning: Instant,
    shared: &RefCell<SharedState>,
    immediate_viewport: ImmediateViewport<'_>,
) {
    profiling::function_scope!();

    let ImmediateViewport {
        ids,
        builder,
        mut viewport_ui_cb,
    } = immediate_viewport;

    let input = {
        let SharedState {
            egui_ctx,
            viewports,
            painter,
            viewport_from_window,
            ..
        } = &mut *shared.borrow_mut();

        let viewport = initialize_or_update_viewport(
            viewports,
            ids,
            ViewportClass::Immediate,
            builder,
            None,
            painter,
        );
        if viewport.window.is_none() {
            event_loop_context::with_current_event_loop(|event_loop| {
                viewport.initialize_window(event_loop, egui_ctx, viewport_from_window, painter);
            });
        }

        let (Some(window), Some(egui_winit)) = (&viewport.window, &mut viewport.egui_winit) else {
            return;
        };
        egui_winit::update_viewport_info(&mut viewport.info, egui_ctx, window, false);

        let mut input = egui_winit.take_egui_input(window);
        input.viewports = viewports
            .iter()
            .map(|(id, viewport)| (*id, viewport.info.clone()))
            .collect();
        input.time = Some(beginning.elapsed().as_secs_f64());
        input
    };

    let egui_ctx = shared.borrow().egui_ctx.clone();

    // ------------------------------------------

    // Run the user code, which could re-entrantly call this function again (!).
    // Make sure no locks are held during this call.
    let egui::FullOutput {
        platform_output,
        textures_delta,
        shapes,
        pixels_per_point,
        viewport_output,
    } = egui_ctx.run_ui(input, |ui| {
        viewport_ui_cb(ui);
    });

    // ------------------------------------------

    let mut shared_mut = shared.borrow_mut();
    let SharedState {
        viewports,
        painter,
        viewport_from_window,
        ..
    } = &mut *shared_mut;

    let Some(viewport) = viewports.get_mut(&ids.this) else {
        return;
    };
    viewport.info.events.clear(); // they should have been processed
    let (Some(egui_winit), Some(window)) = (&mut viewport.egui_winit, &viewport.window) else {
        return;
    };

    {
        profiling::scope!("set_window");
        if let Err(err) = pollster::block_on(painter.set_window(ids.this, Some(Arc::clone(window))))
        {
            log::error!(
                "when rendering viewport_id={:?}, set_window Error {err}",
                ids.this
            );
        }
    }

    let clipped_primitives = egui_ctx.tessellate(shapes, pixels_per_point);
    painter.paint_and_update_textures(
        ids.this,
        pixels_per_point,
        [0.0, 0.0, 0.0, 0.0],
        &clipped_primitives,
        &textures_delta,
        vec![],
    );

    egui_winit.handle_platform_output(window, platform_output);

    handle_viewport_output(
        &egui_ctx,
        &viewport_output,
        viewports,
        painter,
        viewport_from_window,
    );
}

pub(super) fn remove_viewports_not_in(
    viewports: &mut Viewports,
    painter: &mut egui_wgpu::winit::Painter,
    viewport_from_window: &mut HashMap<WindowId, ViewportId>,
    viewport_output: &OrderedViewportIdMap<ViewportOutput>,
) {
    let active_viewports_ids: ViewportIdSet = viewport_output.keys().copied().collect();

    // Prune dead viewports:
    viewports.retain(|id, _| active_viewports_ids.contains(id));
    viewport_from_window.retain(|_, id| active_viewports_ids.contains(id));
    painter.gc_viewports(&active_viewports_ids);
}

/// Add new viewports, and update existing ones:
pub(super) fn handle_viewport_output(
    egui_ctx: &egui::Context,
    viewport_output: &OrderedViewportIdMap<ViewportOutput>,
    viewports: &mut Viewports,
    painter: &mut egui_wgpu::winit::Painter,
    viewport_from_window: &mut HashMap<WindowId, ViewportId>,
) {
    for (
        viewport_id,
        ViewportOutput {
            parent,
            class,
            builder,
            viewport_ui_cb,
            mut commands,
            repaint_delay: _, // ignored - we listened to the repaint callback instead
        },
    ) in viewport_output.clone()
    {
        let ids = ViewportIdPair::from_self_and_parent(viewport_id, parent);

        let viewport =
            initialize_or_update_viewport(viewports, ids, class, builder, viewport_ui_cb, painter);

        if let Some(window) = viewport.window.as_ref() {
            let old_inner_size = window.inner_size();

            viewport.deferred_commands.append(&mut commands);

            egui_winit::process_viewport_commands(
                egui_ctx,
                &mut viewport.info,
                std::mem::take(&mut viewport.deferred_commands),
                window,
                &mut viewport.actions_requested,
            );

            // For Wayland : https://github.com/emilk/egui/issues/4196
            if cfg!(target_os = "linux") {
                let new_inner_size = window.inner_size();
                if new_inner_size != old_inner_size
                    && let (Some(width), Some(height)) = (
                        NonZeroU32::new(new_inner_size.width),
                        NonZeroU32::new(new_inner_size.height),
                    )
                {
                    painter.on_window_resized(viewport_id, width, height);
                }
            }
        }
    }

    remove_viewports_not_in(viewports, painter, viewport_from_window, viewport_output);
}

pub(super) fn initialize_or_update_viewport<'a>(
    viewports: &'a mut Viewports,
    ids: ViewportIdPair,
    class: ViewportClass,
    mut builder: ViewportBuilder,
    viewport_ui_cb: Option<Arc<dyn Fn(&mut egui::Ui) + Send + Sync>>,
    painter: &mut egui_wgpu::winit::Painter,
) -> &'a mut Viewport {
    use std::collections::btree_map::Entry;

    profiling::function_scope!();

    if builder.icon.is_none() {
        // Inherit icon from parent
        builder.icon = viewports
            .get_mut(&ids.parent)
            .and_then(|vp| vp.builder.icon.clone());
    }

    match viewports.entry(ids.this) {
        Entry::Vacant(entry) => {
            // New viewport:
            log::debug!("Creating new viewport {:?} ({:?})", ids.this, builder.title);
            entry.insert(Viewport {
                ids,
                class,
                builder,
                deferred_commands: vec![],
                info: Default::default(),
                actions_requested: Vec::new(),
                viewport_ui_cb,
                window: None,
                egui_winit: None,
            })
        }

        Entry::Occupied(mut entry) => {
            // Patch an existing viewport:
            let viewport = entry.get_mut();

            viewport.class = class;
            viewport.ids.parent = ids.parent;
            viewport.viewport_ui_cb = viewport_ui_cb;

            let (mut delta_commands, recreate) = viewport.builder.patch(builder);

            if recreate {
                log::debug!(
                    "Recreating window for viewport {:?} ({:?})",
                    ids.this,
                    viewport.builder.title
                );
                viewport.window = None;
                viewport.egui_winit = None;
                if let Err(err) = pollster::block_on(painter.set_window(viewport.ids.this, None)) {
                    log::error!(
                        "when rendering viewport_id={:?}, set_window Error {err}",
                        viewport.ids.this
                    );
                }
            }

            viewport.deferred_commands.append(&mut delta_commands);

            entry.into_mut()
        }
    }
}

#[cfg_attr(target_os = "ios", allow(dead_code, unused_variables, unused_mut))]
pub fn viewport_builder(
    egui_zoom_factor: f32,
    event_loop: &ActiveEventLoop,
    native_options: &mut crate::epi::NativeOptions,
    window_settings: Option<WindowSettings>,
) -> ViewportBuilder {
    profiling::function_scope!();

    let mut viewport_builder = native_options.viewport.clone();

    // On some Linux systems, a window size larger than the monitor causes crashes,
    // and on Windows the window does not appear at all.
    let clamp_size_to_monitor_size = viewport_builder.clamp_size_to_monitor_size.unwrap_or(true);

    // Always use the default window size / position on iOS. Trying to restore the previous position
    // causes the window to be shown too small.
    #[cfg(not(target_os = "ios"))]
    let inner_size_points = if let Some(mut window_settings) = window_settings {
        // Restore pos/size from previous session

        if clamp_size_to_monitor_size {
            window_settings.clamp_size_to_sane_values(largest_monitor_point_size(
                egui_zoom_factor,
                event_loop,
            ));
        }
        window_settings.clamp_position_to_monitors(egui_zoom_factor, event_loop);

        viewport_builder = window_settings.initialize_viewport_builder(
            egui_zoom_factor,
            event_loop,
            viewport_builder,
        );
        window_settings.inner_size_points()
    } else {
        if let Some(pos) = viewport_builder.position {
            viewport_builder = viewport_builder.with_position(pos);
        }

        if clamp_size_to_monitor_size && let Some(initial_window_size) = viewport_builder.inner_size
        {
            let initial_window_size = egui::NumExt::at_most(
                initial_window_size,
                largest_monitor_point_size(egui_zoom_factor, event_loop),
            );
            viewport_builder = viewport_builder.with_inner_size(initial_window_size);
        }

        viewport_builder.inner_size
    };

    #[cfg(not(target_os = "ios"))]
    if native_options.centered {
        profiling::scope!("center");
        if let Some(monitor) = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
        {
            let monitor_size = monitor
                .size()
                .to_logical::<f32>(egui_zoom_factor as f64 * monitor.scale_factor());
            let inner_size = inner_size_points.unwrap_or(egui::Vec2 { x: 800.0, y: 600.0 });
            if 0.0 < monitor_size.width && 0.0 < monitor_size.height {
                let x = (monitor_size.width - inner_size.x) / 2.0;
                let y = (monitor_size.height - inner_size.y) / 2.0;
                viewport_builder = viewport_builder.with_position([x, y]);
            }
        }
    }

    viewport_builder
}

pub fn apply_window_settings(
    window: &winit::window::Window,
    window_settings: Option<WindowSettings>,
) {
    profiling::function_scope!();
    if let Some(window_settings) = window_settings {
        window_settings.initialize_window(window);
    }
}

#[cfg(not(target_os = "ios"))]
fn largest_monitor_point_size(egui_zoom_factor: f32, event_loop: &ActiveEventLoop) -> egui::Vec2 {
    profiling::function_scope!();
    let mut max_size = egui::Vec2::ZERO;

    let available_monitors = {
        profiling::scope!("available_monitors");
        event_loop.available_monitors()
    };

    for monitor in available_monitors {
        let size = monitor
            .size()
            .to_logical::<f32>(egui_zoom_factor as f64 * monitor.scale_factor());
        let size = egui::vec2(size.width, size.height);
        max_size = max_size.max(size);
    }

    if max_size == egui::Vec2::ZERO {
        egui::Vec2::splat(16000.0)
    } else {
        max_size
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "persistence")]
pub(super) const STORAGE_EGUI_MEMORY_KEY: &str = "egui";

#[cfg(feature = "persistence")]
pub(super) const STORAGE_WINDOW_KEY: &str = "window";

/// For loading/saving app state and/or egui memory to disk.
pub(super) fn create_storage(_app_name: &str) -> Option<Box<dyn crate::epi::Storage>> {
    #[cfg(feature = "persistence")]
    if let Some(storage) = file_storage::FileStorage::from_app_id(_app_name) {
        return Some(Box::new(storage));
    }
    None
}

#[allow(clippy::allow_attributes, clippy::unnecessary_wraps)]
pub(super) fn create_storage_with_file(
    _file: impl Into<PathBuf>,
) -> Option<Box<dyn crate::epi::Storage>> {
    #[cfg(feature = "persistence")]
    return Some(Box::new(file_storage::FileStorage::from_ron_filepath(
        _file,
    )));
    #[cfg(not(feature = "persistence"))]
    None
}

// ----------------------------------------------------------------------------

pub(super) fn load_window_settings(
    _storage: Option<&dyn crate::epi::Storage>,
) -> Option<WindowSettings> {
    profiling::function_scope!();
    #[cfg(feature = "persistence")]
    {
        epi::get_value(_storage?, STORAGE_WINDOW_KEY)
    }
    #[cfg(not(feature = "persistence"))]
    None
}

pub(super) fn load_egui_memory(_storage: Option<&dyn crate::epi::Storage>) -> Option<egui::Memory> {
    profiling::function_scope!();
    #[cfg(feature = "persistence")]
    {
        epi::get_value(_storage?, STORAGE_EGUI_MEMORY_KEY)
    }
    #[cfg(not(feature = "persistence"))]
    None
}

/// Create an egui context, restoring it from storage if possible.
pub(super) fn create_egui_context(storage: Option<&dyn crate::Storage>) -> egui::Context {
    profiling::function_scope!();

    pub const IS_MOBILE: bool = cfg!(any(target_os = "android", target_os = "ios",));

    let egui_ctx = egui::Context::default();

    egui_ctx.set_embed_viewports(IS_MOBILE);

    egui_ctx.options_mut(|o| {
        // xframe supports multi-pass (Context::request_discard).
        #[expect(clippy::unwrap_used)]
        {
            o.max_passes = 2.try_into().unwrap();
        }
    });

    let memory = load_egui_memory(storage).unwrap_or_default();
    egui_ctx.memory_mut(|mem| *mem = memory);

    egui_ctx
}
