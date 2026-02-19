use crate::wgpu_winit_app::UserEvent;
use egui::epaint::MarginF32;
use winit::{
    dpi::PhysicalPosition,
    event::TouchPhase,
    event_loop::{EventLoopClosed, EventLoopProxy},
};

/// Proxy to send messages to xframe, which will be processed on the `winit::event_loop` thread.
///
/// The proxy can be cloned.
#[derive(Clone)]
pub struct XFrameProxy<T: 'static>(EventLoopProxy<UserEvent<T>>);
impl<T: 'static> XFrameProxy<T> {
    /// Creates a new [XFrameProxy].
    pub(crate) fn new(event_loop_proxy: EventLoopProxy<UserEvent<T>>) -> Self {
        Self(event_loop_proxy)
    }

    /// Use to send your custom events to the winit UI thread.
    ///
    /// Receive them by implementing the [`XFrameApp::on_user_event`] method.
    ///
    /// Returns `EventLoopClosed` error if the associated EventLoop no longer exists.
    pub fn send_custom_event(&self, event: T) -> Result<(), EventLoopClosed<()>> {
        // todo
        if self.0.send_event(UserEvent::ExtCustomEvent(event)).is_err() {
            return Err(EventLoopClosed(()));
        }

        Ok(())
    }

    /// -> Currently safe-area-insets are only provided for iOS. This can be used to retrieve the insets
    /// for other platforms and update the insets on [EguiContextSafeAreaExt::safe_area_insets].
    ///
    /// Returns `EventLoopClosed` error if the associated EventLoop no longer exists.
    pub fn send_safe_area_insets(&self, insets: MarginF32) -> Result<(), EventLoopClosed<()>> {
        if self
            .0
            .send_event(UserEvent::ExtSafeAreaInsets(insets))
            .is_err()
        {
            return Err(EventLoopClosed(()));
        }

        Ok(())
    }

    /// Can be used eg. on Android to inject touch events when the user touch / moves on the Android
    /// back areas.
    /// In that case the touch events are intercepted, winit receives no touch events.
    /// Android provides a api to get these events - here they can be sent to xframe.
    pub fn send_touch_location(
        &self,
        location: PhysicalPosition<f64>,
        touch_phase: TouchPhase,
    ) -> Result<(), EventLoopClosed<()>> {
        if self
            .0
            .send_event(UserEvent::ExtTouchLocation(location, touch_phase))
            .is_err()
        {
            return Err(EventLoopClosed(()));
        }

        Ok(())
    }
}
