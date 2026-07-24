use warpui::{Entity, ModelContext, SingletonEntity, WindowId};

use super::view::feature_intro_modal::FeatureIntroId;

/// Holds one-time modal state without registering account, cloud, or quota listeners.
pub struct OneTimeModalModel {
    is_build_plan_migration_modal_open: bool,
    is_oz_launch_modal_open: bool,
    is_openwarp_launch_modal_open: bool,
    is_orchestration_launch_modal_open: bool,
    is_free_ai_removal_modal_open: bool,
    is_hoa_onboarding_open: bool,
    active_feature_intro: Option<FeatureIntroId>,
    target_window_id: Option<WindowId>,
}

impl OneTimeModalModel {
    pub fn new_local(_ctx: &mut ModelContext<Self>) -> Self {
        Self::closed()
    }

    #[cfg(test)]
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        Self::new_local(ctx)
    }

    fn closed() -> Self {
        Self {
            is_build_plan_migration_modal_open: false,
            is_oz_launch_modal_open: false,
            is_openwarp_launch_modal_open: false,
            is_orchestration_launch_modal_open: false,
            is_free_ai_removal_modal_open: false,
            is_hoa_onboarding_open: false,
            active_feature_intro: None,
            target_window_id: None,
        }
    }

    pub fn target_window_id(&self) -> Option<WindowId> {
        self.target_window_id
    }

    pub fn update_target_window_id(&mut self, window_id: WindowId, ctx: &mut ModelContext<Self>) {
        let was_visible = self.has_visible_content();
        self.target_window_id = Some(window_id);
        if was_visible {
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open: true });
        }
    }

    pub fn is_oz_launch_modal_open(&self) -> bool {
        self.is_visible(self.is_oz_launch_modal_open)
    }

    pub fn mark_oz_launch_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_oz_launch_modal_open, false, ctx);
    }

    pub fn is_openwarp_launch_modal_open(&self) -> bool {
        self.is_visible(self.is_openwarp_launch_modal_open)
    }

    pub fn mark_openwarp_launch_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_openwarp_launch_modal_open, false, ctx);
    }

    pub fn is_orchestration_launch_modal_open(&self) -> bool {
        self.is_visible(self.is_orchestration_launch_modal_open)
    }

    pub fn mark_orchestration_launch_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_orchestration_launch_modal_open, false, ctx);
    }

    pub fn is_free_ai_removal_modal_open(&self) -> bool {
        self.is_visible(self.is_free_ai_removal_modal_open)
    }

    pub fn mark_free_ai_removal_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_free_ai_removal_modal_open, false, ctx);
    }

    pub fn is_hoa_onboarding_open(&self) -> bool {
        self.is_visible(self.is_hoa_onboarding_open)
    }

    pub fn mark_hoa_onboarding_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_hoa_onboarding_open, false, ctx);
    }

    pub fn is_build_plan_migration_modal_open(&self) -> bool {
        self.is_visible(self.is_build_plan_migration_modal_open)
    }

    pub fn mark_build_plan_migration_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_build_plan_migration_modal_open, false, ctx);
    }

    pub fn active_feature_intro(&self) -> Option<FeatureIntroId> {
        self.target_window_id.and(self.active_feature_intro)
    }

    pub fn mark_feature_intro_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        if self.active_feature_intro.take().is_some() {
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open: false });
        }
    }

    pub fn is_any_modal_open(&self) -> bool {
        self.target_window_id.is_some()
            && (self.is_oz_launch_modal_open
                || self.is_openwarp_launch_modal_open
                || self.is_orchestration_launch_modal_open
                || self.is_build_plan_migration_modal_open
                || self.is_free_ai_removal_modal_open
                || self.is_hoa_onboarding_open)
    }

    #[cfg(debug_assertions)]
    pub fn force_open_oz_launch_modal(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_oz_launch_modal_open, true, ctx);
    }

    #[cfg(debug_assertions)]
    pub fn force_open_openwarp_launch_modal(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_openwarp_launch_modal_open, true, ctx);
    }

    #[cfg(debug_assertions)]
    pub fn force_open_orchestration_launch_modal(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_orchestration_launch_modal_open, true, ctx);
    }

    #[cfg(debug_assertions)]
    pub fn force_open_free_ai_removal_modal(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_free_ai_removal_modal_open, true, ctx);
    }

    #[cfg(debug_assertions)]
    pub fn force_open_build_plan_migration_modal(&mut self, ctx: &mut ModelContext<Self>) {
        Self::set_open(&mut self.is_build_plan_migration_modal_open, true, ctx);
    }

    #[cfg(debug_assertions)]
    pub fn force_open_feature_intro(&mut self, id: FeatureIntroId, ctx: &mut ModelContext<Self>) {
        if self.active_feature_intro != Some(id) {
            self.active_feature_intro = Some(id);
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open: true });
        }
    }

    fn is_visible(&self, is_open: bool) -> bool {
        is_open && self.target_window_id.is_some()
    }

    fn has_visible_content(&self) -> bool {
        self.is_any_modal_open() || self.active_feature_intro().is_some()
    }

    fn set_open(field: &mut bool, is_open: bool, ctx: &mut ModelContext<Self>) {
        if *field != is_open {
            *field = is_open;
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneTimeModalEvent {
    VisibilityChanged { is_open: bool },
}

impl Entity for OneTimeModalModel {
    type Event = OneTimeModalEvent;
}

impl SingletonEntity for OneTimeModalModel {}
