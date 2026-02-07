//! xframe - a highly opinionated version of eframe, the recommended [`egui`] framework crate
//!
//!
//! In short, you implement [`App`] (especially [`App::update`]) and then
//! call [`crate::run_native`] from your `main.rs` / `lib.rs`, or use [`crate::create_native_handler`]
//! to create your own winit app proxy.
//!
//! ## Feature flags

// Re-export all useful libraries:
pub use winit;
pub use {egui, egui::emath, egui::epaint};
pub use {egui_wgpu, wgpu};

mod epi;

// Re-export everything in `epi` so `xframe` users don't have to care about what `epi` is:
pub use epi::*;

/// Re-export `AndroidApp` so `xframe` users don't need to deal with version mismatches.
#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

pub(crate) mod stopwatch;

pub mod icon_data;
mod native;

#[cfg(feature = "persistence")]
pub use native::file_storage::storage_dir;

/// Provides a [winit::application::ApplicationHandler] implementation to create your
/// own proxy type.
pub fn create_native_handler<'a, U: Send>(
    app_name: &str,
    mut native_options: NativeOptions<U>,
    app_creator: AppCreator<'a>,
    event_loop: &winit::event_loop::EventLoop<UserEvent<U>>,
) -> Box<dyn winit::application::ApplicationHandler<UserEvent<U>> + 'a> {
    log::debug!("Using 'xframe::create_native_handler' with wgpu renderer");

    #[cfg(not(feature = "__screenshot"))]
    assert!(
        std::env::var("XFRAME_SCREENSHOT_TO").is_err(),
        "XFRAME_SCREENSHOT_TO found without compiling with the '__screenshot' feature"
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

    Box::new(native::run::create_wgpu(
        app_name,
        native_options,
        app_creator,
        event_loop,
    ))
}

/// The different problems that can occur when trying to run `xframe`.
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

/// Short for `Result<T, xframe::Error>`.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;
