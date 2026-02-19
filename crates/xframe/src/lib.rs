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

pub mod icon_data;
mod wgpu_winit_app;

#[cfg(feature = "persistence")]
pub use wgpu_winit_app::file_storage::storage_dir;

/// Call this to get properties to setup your app and the runner to run your app.
pub fn get_create_context<T: Send>(
    app_name: &str,
    native_options: NativeOptions,
) -> Result<CreateContext<T>> {
    let app_name = app_name.to_string();
    let storage = if let Some(file) = &native_options.persistence_path {
        crate::wgpu_winit_app::create_storage_with_file(file)
    } else {
        crate::wgpu_winit_app::create_storage(
            native_options.viewport.app_id.as_ref().unwrap_or(&app_name),
        )
    };
    let egui_ctx = wgpu_winit_app::create_egui_context(storage.as_deref());
    let runner = Runner::new(app_name, native_options, storage, egui_ctx.clone())?;
    let proxy = XFrameProxy::new(runner.create_proxy());

    Ok(CreateContext {
        egui_ctx,
        proxy,
        runner,
    })
}

/// The different problems that can occur when trying to run `xframe`.
#[derive(Debug)]
pub enum Error {
    /// Something went wrong in user code when trying to gat android-app
    /// from native options.
    AndroidApp(Box<dyn std::error::Error + Send + Sync>),

    /// An error from [`winit`]
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
            Self::AndroidApp(err) => {
                write!(f, "AndroidApp error: {err}")
            }

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
