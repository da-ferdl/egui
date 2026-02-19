use super::{Runner, XFrameProxy};

/// Data that is returned on [xframe::get_create_context] wich can be used to
/// setup and initialize your app.
///
/// To run your app use the given [CreateContext::runner].
pub struct CreateContext<T: 'static> {
    /// The egui Context.
    ///
    /// You can use this to customize the look of egui, e.g to call [`egui::Context::set_fonts`],
    /// [`egui::Context::set_visuals_of`] etc.
    pub egui_ctx: egui::Context,

    /// Use to send messages to xframe, which will be processed
    /// on the winit::event_loop thread.
    pub proxy: XFrameProxy<T>,

    /// Use the runner to run your app.
    pub runner: Runner<T>,
}
