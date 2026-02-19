#[derive(Clone)]
pub struct CurrentPlatform {
    pub is_android: bool,
    pub is_ios: bool,
    pub is_mobile: bool,
    pub is_macos: bool,
    pub is_linux: bool,
    pub is_windows: bool,
    pub is_desktop: bool,
}
impl CurrentPlatform {
    pub const fn new() -> Self {
        const IS_ANDROID: bool = cfg!(target_os = "android");
        const IS_IOS: bool = cfg!(target_os = "ios");
        const IS_MOBILE: bool = IS_ANDROID || IS_IOS;
        const IS_DESKTOP: bool = !IS_MOBILE;
        const IS_MACOS: bool = cfg!(target_os = "macos");
        const IS_LINUX: bool = cfg!(target_os = "linux");
        const IS_WINDOWS: bool = cfg!(target_os = "windows");

        Self {
            is_android: IS_ANDROID,
            is_ios: IS_IOS,
            is_mobile: IS_MOBILE,
            is_macos: IS_MACOS,
            is_linux: IS_LINUX,
            is_windows: IS_WINDOWS,
            is_desktop: IS_DESKTOP,
        }
    }
}
