use crate::{AppLifeCycleState, CurrentPlatform};
use egui::epaint::MarginF32;

pub trait EguiContextExt {
    fn safe_area_insets(&self) -> &MarginF32;

    fn app_life_cycle_state(&self) -> &AppLifeCycleState;

    fn current_platform(&self) -> &CurrentPlatform;
}
impl EguiContextExt for egui::Context {
    fn safe_area_insets(&self) -> &MarginF32 {
        // `egui::Context` from the used fork is !Send + !Sync, so this is safe tu use.
        #[expect(unsafe_code, static_mut_refs)]
        unsafe {
            &SAFE_AREA_INSETS
        }
    }

    fn app_life_cycle_state(&self) -> &AppLifeCycleState {
        // `egui::Context` from the used fork is !Send + !Sync, so this is safe tu use.
        #[expect(unsafe_code, static_mut_refs)]
        unsafe {
            &APP_LIFE_CYCLE_STATE
        }
    }

    fn current_platform(&self) -> &CurrentPlatform {
        &CURRENT_PLATFORM
    }
}

/// Updates safe-area-insets property which is used by [EguiContextSafeAreaExt::safe_area_insets].
pub(crate) fn update_egui_context_safe_area_insets(area: MarginF32) {
    // `egui::Context` from the used fork is !Send + !Sync, so this is safe.
    #[expect(unsafe_code)]
    unsafe {
        SAFE_AREA_INSETS = area
    };
}

/// Updates app-life-cycle-state property which is used by [EguiContextSafeAreaExt::app_life_cycle_state].
pub(crate) fn update_egui_context_life_cycle_state(state: AppLifeCycleState) {
    #[expect(unsafe_code)]
    unsafe {
        APP_LIFE_CYCLE_STATE = state;
    }
}

/// `egui::Context` from the used fork is !Send + !Sync, so this is safe.
static mut SAFE_AREA_INSETS: MarginF32 = MarginF32::ZERO;

/// `egui::Context` from the used fork is !Send + !Sync, so this is safe.
static mut APP_LIFE_CYCLE_STATE: AppLifeCycleState =
    AppLifeCycleState::Background { did_exit: false };

const CURRENT_PLATFORM: CurrentPlatform = CurrentPlatform::new();
