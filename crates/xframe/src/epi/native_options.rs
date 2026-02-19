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

    /// This controls what happens when you close the main xframe window.
    ///
    /// If `true`, execution will continue after the xframe window is closed.
    /// If `false`, the app will close once the xframe window is closed.
    ///
    /// This is `true` by default, and the `false` option is only there
    /// so we can revert if we find any bugs.
    ///
    /// This feature was introduced in <https://github.com/emilk/egui/pull/1889>.
    ///
    /// When `true`, [`winit::platform::run_on_demand::EventLoopExtRunOnDemand`] is used.
    /// When `false`, [`winit::event_loop::EventLoop::run`] is used.
    ///
    /// **Note:** On iOS `run_and_return` (`run_app_on_demand`) is not supported,
    /// so when compiling for iOS always `false` is used.
    pub run_and_return: bool,

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

    /// The folder where `xframe` will store the app state. If not set, xframe will use a default
    /// data storage path for each target system.
    pub persistence_path: Option<std::path::PathBuf>,

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

            run_and_return: true,

            centered: false,

            wgpu_options: egui_wgpu::WgpuConfiguration::default(),

            persist_window: true,

            persistence_path: None,

            #[cfg(target_os = "android")]
            android_app: None,
        }
    }
}
