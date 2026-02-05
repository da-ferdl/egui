//! xframe - a highly opinionated version of eframe, the recommended [`egui`] framework crate
//!
//!
//! In short, you implement [`App`] (especially [`App::update`]) and then
//! call [`crate::run_native`] from your `main.rs`, and/or use `eframe::WebRunner` from your `lib.rs`.
//!
//! ## Compiling for web
//! You need to install the `wasm32` target with `rustup target add wasm32-unknown-unknown`.
//!
//! Build the `.wasm` using `cargo build --target wasm32-unknown-unknown`
//! and then use [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) to generate the JavaScript glue code.
//!
//! See the [`eframe_template` repository](https://github.com/emilk/eframe_template/) for more.
//!
//! ## Simplified usage
//! If your app is only for native, and you don't need advanced features like state persistence,
//! then you can use the simpler function [`run_simple_native`].
//!
//! ## Usage, native:
//! ``` no_run
//! use eframe::egui;
//!
//! fn main() {
//!     let native_options = eframe::NativeOptions::default();
//!     eframe::run_native("My egui App", native_options, Box::new(|cc| Ok(Box::new(MyEguiApp::new(cc)))));
//! }
//!
//! #[derive(Default)]
//! struct MyEguiApp {}
//!
//! impl MyEguiApp {
//!     fn new(cc: &eframe::CreationContext<'_>) -> Self {
//!         // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_global_style.
//!         // Restore app state using cc.storage (requires the "persistence" feature).
//!         Self::default()
//!     }
//! }
//!
//! impl xframe::App for MyEguiApp {
//!    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
//!        egui::CentralPanel::default().show_inside(ui, |ui| {
//!            ui.heading("Hello World!");
//!        });
//!    }
//! }
//! ```
//!
//! ## Feature flags

// Re-export all useful libraries:
pub use {egui, egui::emath, egui::epaint};

#[cfg(feature = "glow")]
pub use {egui_glow, glow};

#[cfg(feature = "wgpu_no_default_features")]
pub use {egui_wgpu, wgpu};

mod epi;

// Re-export everything in `epi` so `xframe` users don't have to care about what `epi` is:
pub use epi::*;

pub(crate) mod stopwatch;

// ----------------------------------------------------------------------------
// When compiling natively

pub mod icon_data;
mod native;

pub use native::run::EframeWinitApplication;

#[cfg(not(target_os = "ios"))]
pub use native::run::EframePumpStatus;

#[cfg(feature = "persistence")]
pub use native::file_storage::storage_dir;

/// This is how you start a native (desktop) app.
///
/// The first argument is name of your app, which is an identifier
/// used for the save location of persistence (see [`App::save`]).
/// It is also used as the application id on wayland.
/// If you set no title on the viewport, the app id will be used
/// as the title.
///
/// For details about application ID conventions, see the
/// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
///
/// Call from `fn main` like this:
/// ``` no_run
/// use eframe::egui;
///
/// fn main() -> eframe::Result {
///     let native_options = eframe::NativeOptions::default();
///     eframe::run_native("MyApp", native_options, Box::new(|cc| Ok(Box::new(MyEguiApp::new(cc)))))
/// }
///
/// #[derive(Default)]
/// struct MyEguiApp {}
///
/// impl MyEguiApp {
///     fn new(cc: &eframe::CreationContext<'_>) -> Self {
///         // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_global_style.
///         // Restore app state using cc.storage (requires the "persistence" feature).
///         // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
///         // for e.g. egui::PaintCallback.
///         Self::default()
///     }
/// }
///
/// impl eframe::App for MyEguiApp {
///    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
///        egui::CentralPanel::default().show_inside(ui, |ui| {
///            ui.heading("Hello World!");
///        });
///    }
/// }
/// ```
///
/// # Errors
/// This function can fail if we fail to set up a graphics context.
#[allow(clippy::allow_attributes, clippy::needless_pass_by_value)]
pub fn run_native(
    app_name: &str,
    mut native_options: NativeOptions,
    app_creator: AppCreator<'_>,
) -> Result {
    let renderer = init_native(app_name, &mut native_options);

    match renderer {
        #[cfg(feature = "wgpu_no_default_features")]
        Renderer::Wgpu => {
            log::debug!("Using the wgpu renderer");
            native::run::run_wgpu(app_name, native_options, app_creator)
        }
    }
}

/// Provides a proxy for your native eframe application to run on your own event loop.
///
/// See `run_native` for details about `app_name`.
///
/// Call from `fn main` like this:
/// ``` no_run
/// use eframe::{egui, UserEvent};
/// use winit::event_loop::{ControlFlow, EventLoop};
///
/// fn main() -> eframe::Result {
///     let native_options = eframe::NativeOptions::default();
///     let eventloop = EventLoop::<UserEvent>::with_user_event().build()?;
///     eventloop.set_control_flow(ControlFlow::Poll);
///
///     let mut winit_app = eframe::create_native(
///         "MyExtApp",
///         native_options,
///         Box::new(|cc| Ok(Box::new(MyEguiApp::new(cc)))),
///         &eventloop,
///     );
///
///     eventloop.run_app(&mut winit_app)?;
///
///     Ok(())
/// }
///
/// #[derive(Default)]
/// struct MyEguiApp {}
///
/// impl MyEguiApp {
///     fn new(cc: &eframe::CreationContext<'_>) -> Self {
///         Self::default()
///     }
/// }
///
/// impl eframe::App for MyEguiApp {
///    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
///        egui::CentralPanel::default().show_inside(ui, |ui| {
///            ui.heading("Hello World!");
///        });
///    }
/// }
/// ```
///
/// See the `external_eventloop` example for a more complete example.
pub fn create_native<'a>(
    app_name: &str,
    native_options: NativeOptions,
    app_creator: AppCreator<'a>,
    event_loop: &winit::event_loop::EventLoop<UserEvent>,
) -> EframeWinitApplication<'a> {
    EframeWinitApplication::new(create_native_handler(
        app_name,
        native_options,
        app_creator,
        event_loop,
    ))
}

/// Provides a [winit::application::ApplicationHandler] implementation to create your
/// own proxy type like the [EframeWinitApplication] returned from [create_native].
///
/// For usage details and examples please take a look at [create_native] and [EframeWinitApplication],
/// which wraps a [winit::application::ApplicationHandler] internally.
pub fn create_native_handler<'a>(
    app_name: &str,
    mut native_options: NativeOptions,
    app_creator: AppCreator<'a>,
    event_loop: &winit::event_loop::EventLoop<UserEvent>,
) -> Box<dyn winit::application::ApplicationHandler<UserEvent> + 'a> {
    let renderer = init_native(app_name, &mut native_options);

    match renderer {
        #[cfg(feature = "wgpu_no_default_features")]
        Renderer::Wgpu => {
            log::debug!("Using the wgpu renderer");
            Box::new(native::run::create_wgpu(
                app_name,
                native_options,
                app_creator,
                event_loop,
            ))
        }
    }
}

fn init_native(app_name: &str, native_options: &mut NativeOptions) -> Renderer {
    #[cfg(not(feature = "__screenshot"))]
    assert!(
        std::env::var("EFRAME_SCREENSHOT_TO").is_err(),
        "EFRAME_SCREENSHOT_TO found without compiling with the '__screenshot' feature"
    );

    #[cfg(target_os = "ios")]
    if native_options.run_and_return {
        // On iOS 'run_and_return' ('EventLoop::run_app_on_demand')
        // is not not supported, so it is changed to 'false'.
        native_options.run_and_return = false;
    }

    if native_options.viewport.title.is_none() {
        native_options.viewport.title = Some(app_name.to_owned());
    }

    let renderer = native_options.renderer;

    renderer
}

// ----------------------------------------------------------------------------

/// The different problems that can occur when trying to run `eframe`.
#[derive(Debug)]
pub enum Error {
    /// Something went wrong in user code when creating the app.
    AppCreation(Box<dyn std::error::Error + Send + Sync>),

    Winit(winit::error::OsError),

    /// An error from [`winit::event_loop::EventLoop`].
    WinitEventLoop(winit::error::EventLoopError),

    /// An error from [`wgpu`].
    Wgpu(egui_wgpu::WgpuError),
}

impl std::error::Error for Error {}

impl From<winit::error::OsError> for Error {
    #[inline]
    fn from(err: winit::error::OsError) -> Self {
        Self::Winit(err)
    }
}

impl From<winit::error::EventLoopError> for Error {
    #[inline]
    fn from(err: winit::error::EventLoopError) -> Self {
        Self::WinitEventLoop(err)
    }
}

impl From<egui_wgpu::WgpuError> for Error {
    #[inline]
    fn from(err: egui_wgpu::WgpuError) -> Self {
        Self::Wgpu(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppCreation(err) => write!(f, "app creation error: {err}"),

            Self::Winit(err) => {
                write!(f, "winit error: {err}")
            }

            Self::WinitEventLoop(err) => {
                write!(f, "winit EventLoopError: {err}")
            }

            Self::Wgpu(err) => {
                write!(f, "WGPU error: {err}")
            }
        }
    }
}

/// Short for `Result<T, eframe::Error>`.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;
