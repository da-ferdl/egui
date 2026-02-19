/// States of the host OS application process.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum AppLifeCycleState {
    /// The application is running in the background.
    ///
    /// Either there is no view / activity at all, or just currently not visible to the user - in any case not responding to user input.
    ///
    /// If `did_exit` is true the app will be terminated.
    Background { did_exit: bool },

    /// The application running in foreground but in paused state - not responding to user input.
    ///
    /// On IOS apps transition to this state on a phone call, when responding to a TouchID request, when entering the app switcher
    /// or the control center, or when the UIViewController is transitioning.
    ///
    /// On Android apps transition to this state when another activity is focused such as a split-screen app, on a phone call,
    /// at picture-in-picture app, on system dialogs, or another window.
    ///
    /// Apps in this state should assume that they may be sent to background at any time.
    ///
    /// - argument `was_active`: if `true` the previous state was `ForegroundActive`, if `false`
    /// the previous state was `Background`.
    ForegroundPaused { was_active: bool },

    /// The application running in foreground and responding to user input.
    ForegroundActive,
}
impl AppLifeCycleState {
    pub fn is_foreground_active(&self) -> bool {
        match &self {
            Self::ForegroundActive => true,
            _ => false,
        }
    }

    pub fn is_foreground_paused(&self) -> bool {
        match &self {
            Self::ForegroundPaused { .. } => true,
            _ => false,
        }
    }

    pub fn is_foreground_paused_was_active(&self) -> bool {
        match &self {
            Self::ForegroundPaused { was_active } => *was_active,
            _ => false,
        }
    }

    pub fn is_background(&self) -> bool {
        match &self {
            Self::Background { .. } => true,
            _ => false,
        }
    }

    pub fn is_did_exit(&self) -> bool {
        match &self {
            Self::Background { did_exit } => *did_exit,
            _ => false,
        }
    }
}
