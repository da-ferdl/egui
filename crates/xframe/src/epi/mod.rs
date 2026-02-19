//! Platform-agnostic interface for writing apps using [`egui`] (epi = egui programming interface).
//!
//! `epi` provides interfaces for window management and serialization.
//!
//! Start by looking at the [`XFrameApp`] trait, and implement [`XFrameApp::ui`].

mod app_life_cycle_state;
mod create_context;
mod current_platform;
mod egui_context_ext;
mod frame;
mod native_options;
mod runner;
mod start_context;
mod storage;
mod x_frame_app;
mod x_frame_proxy;

pub use app_life_cycle_state::AppLifeCycleState;
pub use create_context::CreateContext;
pub use current_platform::CurrentPlatform;
pub use egui_context_ext::EguiContextExt;
pub(crate) use egui_context_ext::{
    update_egui_context_life_cycle_state, update_egui_context_safe_area_insets,
};
pub use frame::Frame;
pub use native_options::NativeOptions;
pub use runner::Runner;
pub use start_context::StartContext;
pub use storage::*;
pub use x_frame_app::XFrameApp;
pub use x_frame_proxy::XFrameProxy;

pub use winit::{event_loop::EventLoopBuilder, window::WindowAttributes};

pub type DynError = Box<dyn std::error::Error + Send + Sync>;
