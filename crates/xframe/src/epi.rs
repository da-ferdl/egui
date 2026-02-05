//! Platform-agnostic interface for writing apps using [`egui`] (epi = egui programming interface).
//!
//! `epi` provides interfaces for window management and serialization.
//!
//! Start by looking at the [`App`] trait, and implement [`App::update`].

#![warn(missing_docs)] // Let's keep `epi` well-documented.

pub use crate::native::winit_integration::UserEvent;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use static_assertions::assert_not_impl_any;

pub use winit::{event_loop::EventLoopBuilder, window::WindowAttributes};

/// Hook into the building of an event loop before it is run
///
/// You can configure any platform specific details required on top of the default configuration
/// done by `EFrame`.
pub type EventLoopBuilderHook = Box<dyn FnOnce(&mut EventLoopBuilder<UserEvent>)>;

/// Hook into the building of a the native window.
///
/// You can configure any platform specific details required on top of the default configuration
/// done by `eframe`.
pub type WindowBuilderHook = Box<dyn FnOnce(egui::ViewportBuilder) -> egui::ViewportBuilder>;

type DynError = Box<dyn std::error::Error + Send + Sync>;

/// This is how your app is created.
///
/// You can use the [`CreationContext`] to setup egui, restore state, setup OpenGL things, etc.
pub type AppCreator<'app> =
    Box<dyn 'app + FnOnce(&CreationContext<'_>) -> Result<Box<dyn 'app + App>, DynError>>;

/// Data that is passed to [`AppCreator`] that can be used to setup and initialize your app.
pub struct CreationContext<'s> {
    /// The egui Context.
    ///
    /// You can use this to customize the look of egui, e.g to call [`egui::Context::set_fonts`],
    /// [`egui::Context::set_visuals_of`] etc.
    pub egui_ctx: egui::Context,

    /// Information about the surrounding environment.
    pub integration_info: IntegrationInfo,

    /// You can use the storage to restore app state(requires the "persistence" feature).
    pub storage: Option<&'s dyn Storage>,

    /// The underlying WGPU render state.
    ///
    /// Only available when compiling with the `wgpu` feature and using [`Renderer::Wgpu`].
    ///
    /// Can be used to manage GPU resources for custom rendering with WGPU using [`egui::PaintCallback`]s.
    pub wgpu_render_state: Option<egui_wgpu::RenderState>,

    /// Raw platform window handle
    pub(crate) raw_window_handle: Result<RawWindowHandle, HandleError>,

    /// Raw platform display handle for window
    pub(crate) raw_display_handle: Result<RawDisplayHandle, HandleError>,
}

#[expect(unsafe_code)]
impl HasWindowHandle for CreationContext<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // Safety: the lifetime is correct.
        unsafe { Ok(WindowHandle::borrow_raw(self.raw_window_handle.clone()?)) }
    }
}

#[expect(unsafe_code)]
impl HasDisplayHandle for CreationContext<'_> {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // Safety: the lifetime is correct.
        unsafe { Ok(DisplayHandle::borrow_raw(self.raw_display_handle.clone()?)) }
    }
}

impl CreationContext<'_> {
    /// Create a new empty [CreationContext] for testing [App]s in kittest.
    #[doc(hidden)]
    pub fn _new_kittest(egui_ctx: egui::Context) -> Self {
        Self {
            egui_ctx,
            integration_info: IntegrationInfo::mock(),
            storage: None,
            wgpu_render_state: None,
            raw_window_handle: Err(HandleError::NotSupported),
            raw_display_handle: Err(HandleError::NotSupported),
        }
    }
}

// ----------------------------------------------------------------------------

/// Implement this trait to write apps that can be compiled for both web/wasm and desktop/native using [`eframe`](https://github.com/emilk/egui/tree/main/crates/eframe).
pub trait App {
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

    /// Called each time the UI needs repainting, which may be many times per second.
    ///
    /// Put your widgets into a [`egui::Panel`], [`egui::CentralPanel`], [`egui::Window`] or [`egui::Area`].
    ///
    /// The [`egui::Context`] can be cloned and saved if you like.
    ///
    /// To force a repaint, call [`egui::Context::request_repaint`] at any time (e.g. from another thread).
    ///
    /// This is called for the root viewport ([`egui::ViewportId::ROOT`]).
    /// Use [`egui::Context::show_viewport_deferred`] to spawn additional viewports (windows).
    /// (A "viewport" in egui means an native OS window).
    #[deprecated = "Use Self::ui instead"]
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        _ = (ctx, frame);
    }

    /// Called on shutdown, and perhaps at regular intervals. Allows you to save state.
    ///
    /// Only called when the "persistence" feature is enabled.
    ///
    /// On web the state is stored to "Local Storage".
    ///
    /// On native the path is picked using [`crate::storage_dir`].
    /// The path can be customized via [`NativeOptions::persistence_path`].
    fn save(&mut self, _storage: &mut dyn Storage) {}

    /// Called once on shutdown, after [`Self::save`].
    ///
    /// If you need to abort an exit use [`Self::on_close_event`].
    fn on_exit(&mut self) {}

    // ---------
    // Settings:

    /// Time between automatic calls to [`Self::save`]
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    /// Background color values for the app, e.g. what is sent to `gl.clearColor`.
    ///
    /// This is the background of your windows if you don't set a central panel.
    ///
    /// ATTENTION:
    /// Since these float values go to the render as-is, any color space conversion as done
    /// e.g. by converting from [`egui::Color32`] to [`egui::Rgba`] may cause incorrect results.
    /// egui recommends that rendering backends use a normal "gamma-space" (non-sRGB-aware) blending,
    ///  which means the values you return here should also be in `sRGB` gamma-space in the 0-1 range.
    /// You can use [`egui::Color32::to_normalized_gamma_f32`] for this.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // NOTE: a bright gray makes the shadows of the windows look weird.
        // We use a bit of transparency so that if the user switches on the
        // `transparent()` option they get immediate results.
        egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32()

        // _visuals.window_fill() would also be a natural choice
    }

    /// Controls whether or not the egui memory (window positions etc) will be
    /// persisted (only if the "persistence" feature is enabled).
    fn persist_egui_memory(&self) -> bool {
        true
    }

    /// A hook for manipulating or filtering raw input before it is processed by [`Self::update`].
    ///
    /// This function provides a way to modify or filter input events before they are processed by egui.
    ///
    /// It can be used to prevent specific keyboard shortcuts or mouse events from being processed by egui.
    ///
    /// Additionally, it can be used to inject custom keyboard or mouse events into the input stream, which can be useful for implementing features like a virtual keyboard.
    ///
    /// # Arguments
    ///
    /// * `_ctx` - The context of the egui, which provides access to the current state of the egui.
    /// * `_raw_input` - The raw input events that are about to be processed. This can be modified to change the input that egui processes.
    ///
    /// # Note
    ///
    /// This function does not return a value. Any changes to the input should be made directly to `_raw_input`.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {}
}

/// Selects the level of hardware graphics acceleration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HardwareAcceleration {
    /// Require graphics acceleration.
    Required,

    /// Prefer graphics acceleration, but fall back to software.
    Preferred,

    /// Do NOT use graphics acceleration.
    ///
    /// On some platforms (macOS) this is ignored and treated the same as [`Self::Preferred`].
    Off,
}

/// Options controlling the behavior of a native window.
///
/// Additional windows can be opened using (egui viewports)[`egui::viewport`].
///
/// Set the window title and size using [`Self::viewport`].
///
/// ### Application id
/// [`egui::ViewportBuilder::with_app_id`] is used for determining the folder to persist the app to.
///
/// On native the path is picked using [`crate::storage_dir`].
///
/// If you don't set an app id, the title argument to [`crate::run_native`]
/// will be used as app id instead.
pub struct NativeOptions {
    /// Controls the native window of the root viewport.
    ///
    /// This is where you set things like window title and size.
    ///
    /// If you don't set an icon, a default egui icon will be used.
    /// To avoid this, set the icon to [`egui::IconData::default`].
    pub viewport: egui::ViewportBuilder,

    /// Turn on vertical syncing, limiting the FPS to the display refresh rate.
    ///
    /// The default is `true`.
    pub vsync: bool,

    /// Set the level of the multisampling anti-aliasing (MSAA).
    ///
    /// Must be a power-of-two. Higher = more smooth 3D.
    ///
    /// A value of `0` turns it off (default).
    ///
    /// `egui` already performs anti-aliasing via "feathering"
    /// (controlled by [`egui::epaint::TessellationOptions`]),
    /// but if you are embedding 3D in egui you may want to turn on multisampling.
    pub multisampling: u16,

    /// Sets the number of bits in the depth buffer.
    ///
    /// `egui` doesn't need the depth buffer, so the default value is 0.
    pub depth_buffer: u8,

    /// Sets the number of bits in the stencil buffer.
    ///
    /// `egui` doesn't need the stencil buffer, so the default value is 0.
    pub stencil_buffer: u8,

    /// Specify whether or not hardware acceleration is preferred, required, or not.
    ///
    /// Default: [`HardwareAcceleration::Preferred`].
    pub hardware_acceleration: HardwareAcceleration,

    /// What rendering backend to use.
    #[cfg(any(feature = "glow", feature = "wgpu_no_default_features"))]
    pub renderer: Renderer,

    /// This controls what happens when you close the main eframe window.
    ///
    /// If `true`, execution will continue after the eframe window is closed.
    /// If `false`, the app will close once the eframe window is closed.
    ///
    /// This is `true` by default, and the `false` option is only there
    /// so we can revert if we find any bugs.
    ///
    /// This feature was introduced in <https://github.com/emilk/egui/pull/1889>.
    ///
    /// When `true`, [`winit::platform::run_on_demand::EventLoopExtRunOnDemand`] is used.
    /// When `false`, [`winit::event_loop::EventLoop::run`] is used.
    pub run_and_return: bool,

    /// Hook into the building of an event loop before it is run.
    ///
    /// Specify a callback here in case you need to make platform specific changes to the
    /// event loop before it is run.
    ///
    /// Note: A [`NativeOptions`] clone will not include any `event_loop_builder` hook.
    pub event_loop_builder: Option<EventLoopBuilderHook>,

    /// Hook into the building of a window.
    ///
    /// Specify a callback here in case you need to make platform specific changes to the
    /// window appearance.
    ///
    /// Note: A [`NativeOptions`] clone will not include any `window_builder` hook.
    pub window_builder: Option<WindowBuilderHook>,

    /// On desktop: make the window position to be centered at initialization.
    ///
    /// Platform specific:
    ///
    /// Wayland desktop currently not supported.
    pub centered: bool,

    /// Configures wgpu instance/device/adapter/surface creation and renderloop.
    pub wgpu_options: egui_wgpu::WgpuConfiguration,

    /// Controls whether or not the native window position and size will be
    /// persisted (only if the "persistence" feature is enabled).
    pub persist_window: bool,

    /// The folder where `eframe` will store the app state. If not set, eframe will use a default
    /// data storage path for each target system.
    pub persistence_path: Option<std::path::PathBuf>,

    /// Controls whether to apply dithering to minimize banding artifacts.
    ///
    /// Dithering assumes an sRGB output and thus will apply noise to any input value that lies between
    /// two 8bit values after applying the sRGB OETF function, i.e. if it's not a whole 8bit value in "gamma space".
    /// This means that only inputs from texture interpolation and vertex colors should be affected in practice.
    ///
    /// Defaults to true.
    pub dithering: bool,

    /// Android application for `winit`'s event loop.
    ///
    /// This value is required on Android to correctly create the event loop. See
    /// [`EventLoopBuilder::build`] and [`with_android_app`] for details.
    ///
    /// [`EventLoopBuilder::build`]: winit::event_loop::EventLoopBuilder::build
    /// [`with_android_app`]: winit::platform::android::EventLoopBuilderExtAndroid::with_android_app
    #[cfg(target_os = "android")]
    pub android_app: Option<winit::platform::android::activity::AndroidApp>,
}

impl Clone for NativeOptions {
    fn clone(&self) -> Self {
        Self {
            viewport: self.viewport.clone(),

            event_loop_builder: None, // Skip any builder callbacks if cloning

            window_builder: None, // Skip any builder callbacks if cloning

            wgpu_options: self.wgpu_options.clone(),

            persistence_path: self.persistence_path.clone(),

            #[cfg(target_os = "android")]
            android_app: self.android_app.clone(),

            ..*self
        }
    }
}

impl Default for NativeOptions {
    fn default() -> Self {
        Self {
            viewport: Default::default(),

            vsync: true,
            multisampling: 0,
            depth_buffer: 0,
            stencil_buffer: 0,
            hardware_acceleration: HardwareAcceleration::Preferred,

            renderer: Renderer::default(),

            run_and_return: true,

            event_loop_builder: None,

            window_builder: None,

            centered: false,

            wgpu_options: egui_wgpu::WgpuConfiguration::default(),

            persist_window: true,

            persistence_path: None,

            dithering: true,

            #[cfg(target_os = "android")]
            android_app: None,
        }
    }
}

// ----------------------------------------------------------------------------

/// What rendering backend to use.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Renderer {
    /// Use [`egui_wgpu`] renderer for [`wgpu`](https://github.com/gfx-rs/wgpu).
    #[cfg(feature = "wgpu_no_default_features")]
    Wgpu,
}

#[cfg(any(feature = "glow", feature = "wgpu_no_default_features"))]
impl Default for Renderer {
    fn default() -> Self {
        Self::Wgpu
    }
}

#[cfg(any(feature = "glow", feature = "wgpu_no_default_features"))]
impl std::fmt::Display for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "wgpu".fmt(f)
    }
}

impl std::str::FromStr for Renderer {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        Ok(Self::Wgpu)
    }
}

// ----------------------------------------------------------------------------

/// Represents the surroundings of your app.
///
/// It provides methods to inspect the surroundings (are we on the web?),
/// access to persistent storage, and access to the rendering backend.
pub struct Frame {
    /// Information about the integration.
    pub(crate) info: IntegrationInfo,

    /// A place where you can store custom data in a way that persists when you restart the app.
    pub(crate) storage: Option<Box<dyn Storage>>,

    /// Can be used to manage GPU resources for custom rendering with WGPU using [`egui::PaintCallback`]s.
    #[doc(hidden)]
    pub wgpu_render_state: Option<egui_wgpu::RenderState>,

    /// Raw platform window handle
    pub(crate) raw_window_handle: Result<RawWindowHandle, HandleError>,

    /// Raw platform display handle for window
    pub(crate) raw_display_handle: Result<RawDisplayHandle, HandleError>,
}

// Implementing `Clone` would violate the guarantees of `HasWindowHandle` and `HasDisplayHandle`.
assert_not_impl_any!(Frame: Clone);

#[expect(unsafe_code)]
impl HasWindowHandle for Frame {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // Safety: the lifetime is correct.
        unsafe { Ok(WindowHandle::borrow_raw(self.raw_window_handle.clone()?)) }
    }
}

#[expect(unsafe_code)]
impl HasDisplayHandle for Frame {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // Safety: the lifetime is correct.
        unsafe { Ok(DisplayHandle::borrow_raw(self.raw_display_handle.clone()?)) }
    }
}

impl Frame {
    /// Create a new empty [Frame] for testing [App]s in kittest.
    #[doc(hidden)]
    pub fn _new_kittest() -> Self {
        Self {
            info: IntegrationInfo::mock(),
            raw_display_handle: Err(HandleError::NotSupported),
            raw_window_handle: Err(HandleError::NotSupported),
            storage: None,
            wgpu_render_state: None,
        }
    }

    /// Information about the integration.
    pub fn info(&self) -> &IntegrationInfo {
        &self.info
    }

    /// A place where you can store custom data in a way that persists when you restart the app.
    pub fn storage(&self) -> Option<&dyn Storage> {
        self.storage.as_deref()
    }

    /// A place where you can store custom data in a way that persists when you restart the app.
    pub fn storage_mut(&mut self) -> Option<&mut (dyn Storage + 'static)> {
        self.storage.as_deref_mut()
    }

    /// The underlying WGPU render state.
    ///
    /// Only available when compiling with the `wgpu` feature and using [`Renderer::Wgpu`].
    ///
    /// Can be used to manage GPU resources for custom rendering with WGPU using [`egui::PaintCallback`]s.
    pub fn wgpu_render_state(&self) -> Option<&egui_wgpu::RenderState> {
        self.wgpu_render_state.as_ref()
    }
}

/// Information about the integration passed to the use app each frame.
#[derive(Clone, Debug)]
pub struct IntegrationInfo {
    /// Seconds of cpu usage (in seconds) on the previous frame.
    ///
    /// This includes [`App::update`] as well as rendering (except for vsync waiting).
    ///
    /// For a more detailed view of cpu usage, connect your preferred profiler by enabling it's feature in [`profiling`](https://crates.io/crates/profiling).
    ///
    /// `None` if this is the first frame.
    pub cpu_usage: Option<f32>,
}

impl IntegrationInfo {
    fn mock() -> Self {
        Self { cpu_usage: None }
    }
}

// ----------------------------------------------------------------------------

/// A place where you can store custom data in a way that persists when you restart the app.
///
/// On the web this is backed by [local storage](https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage).
/// On desktop this is backed by the file system.
///
/// See [`CreationContext::storage`] and [`App::save`].
pub trait Storage {
    /// Get the value for the given key.
    fn get_string(&self, key: &str) -> Option<String>;

    /// Set the value for the given key.
    fn set_string(&mut self, key: &str, value: String);

    /// write-to-disk or similar
    fn flush(&mut self);
}

/// Get and deserialize the [RON](https://github.com/ron-rs/ron) stored at the given key.
#[cfg(feature = "ron")]
pub fn get_value<T: serde::de::DeserializeOwned>(storage: &dyn Storage, key: &str) -> Option<T> {
    profiling::function_scope!(key);
    let value = storage.get_string(key)?;
    match ron::from_str(&value) {
        Ok(value) => Some(value),
        Err(err) => {
            // This happens on when we break the format, e.g. when updating egui.
            log::debug!("Failed to decode RON: {err}");
            None
        }
    }
}

/// Serialize the given value as [RON](https://github.com/ron-rs/ron) and store with the given key.
#[cfg(feature = "ron")]
pub fn set_value<T: serde::Serialize>(storage: &mut dyn Storage, key: &str, value: &T) {
    profiling::function_scope!(key);
    match ron::ser::to_string(value) {
        Ok(string) => storage.set_string(key, string),
        Err(err) => log::error!("eframe failed to encode data using ron: {err}"),
    }
}

/// [`Storage`] key used for app
pub const APP_KEY: &str = "app";
