use warp_core::context_flag::ContextFlag;
use warp_core::features::FeatureFlag;
use warpui::ViewContext;

use super::{
    FeatureItem, FeatureSection, FeatureSectionData, ResourceCenterMainView, Section, Tip,
    TipAction, TipHint,
};
use crate::i18n::{tr, Message};

pub fn sections(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<Section> {
    let mut sections = vec![Section::Changelog()];

    if FeatureFlag::AvatarInTabBar.is_enabled() {
        return sections;
    }

    let get_started = FeatureSectionData {
        section_name: FeatureSection::GettingStarted,
        items: vec![
            FeatureItem::new(
                tr(ctx, Message::ResourceCreateFirstBlock),
                tr(ctx, Message::ResourceCreateFirstBlockDesc),
                Tip::Hint(TipHint::CreateBlock),
                ctx,
            ),
            FeatureItem::new(
                tr(ctx, Message::ResourceNavigateBlocks),
                tr(ctx, Message::ResourceNavigateBlocksDesc),
                Tip::Hint(TipHint::BlockSelect),
                ctx,
            ),
            FeatureItem::new(
                tr(ctx, Message::ResourceTakeActionOnBlock),
                tr(ctx, Message::ResourceTakeActionOnBlockDesc),
                Tip::Hint(TipHint::BlockAction),
                ctx,
            ),
            FeatureItem::new(
                tr(ctx, Message::ResourceOpenCommandPalette),
                tr(ctx, Message::ResourceOpenCommandPaletteDesc),
                Tip::Action(TipAction::CommandPalette),
                ctx,
            ),
            FeatureItem::new(
                tr(ctx, Message::ResourceSetYourTheme),
                tr(ctx, Message::ResourceSetYourThemeDesc),
                Tip::Action(TipAction::ThemePicker),
                ctx,
            ),
        ],
    };
    sections.push(Section::Feature(get_started));

    let maximize_warp = FeatureSectionData {
        section_name: FeatureSection::MaximizeWarp,
        items: maximize_warp_items(ctx),
    };
    sections.push(Section::Feature(maximize_warp));

    // Advanced setup previously linked out to docs.warp.dev / www.warp.dev; those
    // external Warp hosts are no longer used from the GUI.

    sections
}

fn maximize_warp_items(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<FeatureItem> {
    let mut maximize_warp_items = vec![];

    maximize_warp_items.push(FeatureItem::new(
        tr(ctx, Message::ResourceCommandSearch),
        tr(ctx, Message::ResourceCommandSearchDesc),
        Tip::Action(TipAction::CommandSearch),
        ctx,
    ));

    maximize_warp_items.push(FeatureItem::new(
        tr(ctx, Message::ResourceAiCommandSearch),
        tr(ctx, Message::ResourceAiCommandSearchDesc),
        Tip::Action(TipAction::AiCommandSearch),
        ctx,
    ));

    if ContextFlag::CreateNewSession.is_enabled() {
        maximize_warp_items.push(FeatureItem::new(
            tr(ctx, Message::ResourceSplitPanes),
            tr(ctx, Message::ResourceSplitPanesDesc),
            Tip::Action(TipAction::SplitPane),
            ctx,
        ));
    }

    if ContextFlag::LaunchConfigurations.is_enabled() {
        maximize_warp_items.push(FeatureItem::new(
            tr(ctx, Message::ResourceLaunchConfiguration),
            tr(ctx, Message::ResourceLaunchConfigurationDesc),
            Tip::Action(TipAction::SaveNewLaunchConfig),
            ctx,
        ));
    }

    maximize_warp_items
}
