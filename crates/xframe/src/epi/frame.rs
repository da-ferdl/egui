use super::Storage;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use static_assertions::assert_not_impl_any;

/// Represents the surroundings of your app.
///
/// It provides methods to inspect the surroundings, access to persistent storage, and access to the rendering backend.
pub struct Frame {
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
            raw_display_handle: Err(HandleError::NotSupported),
            raw_window_handle: Err(HandleError::NotSupported),
            storage: None,
            wgpu_render_state: None,
        }
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
