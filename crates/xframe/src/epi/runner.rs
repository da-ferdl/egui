use super::XFrameApp;
use crate::{
    NativeOptions, Result, Storage,
    wgpu_winit_app::{UserEvent, WgpuWinitApp},
};
use winit::event_loop::{EventLoop, EventLoopProxy};

pub struct Runner<T: 'static>(
    Option<(
        String,
        NativeOptions,
        Option<Box<dyn Storage>>,
        egui::Context,
        EventLoop<UserEvent<T>>,
    )>,
);
impl<T: Send> Runner<T> {
    pub(crate) fn new(
        app_name: String,
        mut native_options: NativeOptions,
        storage: Option<Box<dyn Storage>>,
        egui_ctx: egui::Context,
    ) -> Result<Self> {
        let event_loop = create_event_loop(&mut native_options)?;
        Ok(Self(Some((
            app_name,
            native_options,
            storage,
            egui_ctx,
            event_loop,
        ))))
    }

    pub(crate) fn create_proxy(&self) -> EventLoopProxy<UserEvent<T>> {
        let event_loop = &self
            .0
            .as_ref()
            .expect("props should be set at this point")
            .4;

        event_loop.create_proxy()
    }

    pub fn run_app(mut self, app: Box<dyn XFrameApp<T>>) -> Result<()> {
        #[allow(unused_mut)]
        let (app_name, mut native_options, storage, egui_ctx, mut event_loop) =
            self.0.take().expect("props should be set at this point");

        let proxy = event_loop.create_proxy();

        #[cfg(target_os = "ios")]
        if native_options.run_and_return {
            // On iOS `run_and_return` (`run_app_on_demand`) is not supported,
            // so when compiling for iOS always `false` is used.
            native_options.run_and_return = false;
        }

        let run_and_return = native_options.run_and_return;
        let mut wgpu_winit_app =
            WgpuWinitApp::new(proxy, app_name, storage, egui_ctx, app, native_options);

        #[cfg(not(target_os = "ios"))]
        if run_and_return {
            use winit::platform::run_on_demand::EventLoopExtRunOnDemand as _;

            log::trace!("Entering the winit event loop (run_app_on_demand)…");

            event_loop.run_app_on_demand(&mut wgpu_winit_app)?;

            log::debug!("eframe window closed");

            return Ok(());
        }

        log::trace!("Entering the winit event loop (run_app)…");

        event_loop.run_app(&mut wgpu_winit_app)?;

        log::debug!("winit event loop unexpectedly returned");

        Ok(())
    }
}

fn create_event_loop<T>(_native_options: &mut NativeOptions) -> Result<EventLoop<UserEvent<T>>> {
    #[cfg(target_os = "android")]
    use winit::platform::android::EventLoopBuilderExtAndroid as _;

    let mut builder = EventLoop::with_user_event();

    #[cfg(target_os = "android")]
    let mut builder =
        builder.with_android_app(_native_options.android_app.take().ok_or_else(|| {
            crate::Error::AndroidApp(Box::from(
                "`NativeOptions` is missing required `android_app`",
            ))
        })?);

    let event_loop = builder.build()?;

    Ok(event_loop)
}
