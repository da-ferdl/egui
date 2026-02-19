use crate::AppLifeCycleState;

use super::{Frame, StartContext, Storage};
use winit::event::{DeviceEvent, WindowEvent};

/// Implement this trait to write apps that can be compiled for desktop and mobile apps.
pub trait XFrameApp<T> {
    /// Emitted on application start - at this point window,
    /// wgpu renderer, painter, etc. is setup and ui ready to display.
    fn on_start(&mut self, start_context: &StartContext<'_>) {
        let _ = start_context;
    }

    /// Called once before each call to [`Self::ui`],
    /// and additionally also called when the UI is hidden, but [`egui::Context::request_repaint`] was called.
    ///
    /// You may NOT show any ui or do any painting during the call to [`Self::logic`].
    ///
    /// The [`egui::Context`] can be cloned and saved if you like.
    ///
    /// To force another call to [`Self::logic`], call [`egui::Context::request_repaint`] at any time (e.g. from another thread).
    fn logic(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        _ = (ctx, frame);
    }

    /// A hook for manipulating or filtering raw input before it is processed by [`Self::logic`] and [`Self::ui`].
    ///
    /// This function provides a way to modify or filter input events before they are processed by egui.
    ///
    /// It can be used to prevent specific keyboard shortcuts or mouse events from being processed by egui.
    ///
    /// Additionally, it can be used to inject custom keyboard or mouse events into the input stream, which can be useful for implementing features like a virtual keyboard.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context of the egui, which provides access to the current state of the egui.
    /// * `raw_input` - The raw input events that are about to be processed. This can be modified to change the input that egui processes.
    ///
    /// # Note
    ///
    /// This function does not return a value. Any changes to the input should be made directly to `raw_input`.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let _ = (ctx, raw_input);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    ///
    /// The given [`egui::Ui`] has no margin or background color.
    /// You can wrap your UI code in [`egui::CentralPanel`] or a [`egui::Frame::central_panel`] to remedy this.
    ///
    /// The [`egui::Ui::ctx`] can be cloned and saved if you like.
    /// To force a repaint, call [`egui::Context::request_repaint`] at any time (e.g. from another thread).
    ///
    /// This is called for the root viewport ([`egui::ViewportId::ROOT`]).
    /// Use [`egui::Context::show_viewport_deferred`] to spawn additional viewports (windows).
    /// (A "viewport" in egui means an native OS window).
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame);

    /// Use to receive your custom event that you have sent with [`XFrameProxy::send_custom_event`].
    fn on_user_event(&mut self, event: T) {
        let _ = event;
    }

    /// Called whenever the app-life-cycle state changes.
    fn on_app_life_cycle_state_change(&mut self, next_state: AppLifeCycleState) {
        let _ = next_state;
    }

    /// Called every time the app-life-cycle state switches from
    /// `ForegroundActive` to `ForegroundPaused` - allows you to save state.
    ///
    /// Only called when the "persistence" feature is enabled.
    ///
    /// On native the path is picked using [`crate::storage_dir`].
    /// The path can be customized via [`NativeOptions::persistence_path`].
    fn on_save(&mut self, storage: &mut dyn Storage) {
        let _ = storage;
    }

    /// Called once on shutdown.
    fn on_exit(&mut self) {}

    /// **Settings** - background color values for the app, e.g. what is sent to `gl.clearColor`.
    ///
    /// This is the background of your windows if you don't set a central panel.
    ///
    /// ATTENTION:
    /// Since these float values go to the render as-is, any color space conversion as done
    /// e.g. by converting from [`egui::Color32`] to [`egui::Rgba`] may cause incorrect results.
    /// egui recommends that rendering backends use a normal "gamma-space" (non-sRGB-aware) blending,
    ///  which means the values you return here should also be in `sRGB` gamma-space in the 0-1 range.
    /// You can use [`egui::Color32::to_normalized_gamma_f32`] for this.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        // visuals.window_fill() would also be a natural choice
        let _ = visuals;

        // NOTE: a bright gray makes the shadows of the windows look weird.
        // We use a bit of transparency so that if the user switches on the
        // `transparent()` option they get immediate results.
        egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32()
    }

    /// Controls whether or not the egui memory (window positions etc) will be
    /// persisted (only if the "persistence" feature is enabled).
    fn persist_egui_memory(&self) -> bool {
        true
    }

    /// Winit application handler `WindowEvent` - use if you want to intercept window events from winit
    /// before they are processed by egui.
    ///
    /// -> return the event back to the caller if it should be processed, otherwise return `None` - event is ignored.
    fn winit_intercept_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
        Some(event)
    }

    /// Winit application handler `DeviceEvent` - use if you want to intercept device events from winit
    /// before they are processed by egui.
    ///
    /// -> return the event back to the caller if it should be processed, otherwise return `None` - event is ignored.
    fn winit_intercept_device_event(&mut self, event: DeviceEvent) -> Option<DeviceEvent> {
        Some(event)
    }
}
