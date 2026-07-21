use asset_macro::bundled_or_fetched_asset;
use markdown_parser::{FormattedTextFragment, FormattedTextLine};
use warp_core::send_telemetry_from_ctx;
use warpui::assets::asset_cache::AssetSource;
use warpui::{AppContext, SingletonEntity};

use super::{CTAButton, CheckboxConfig, LaunchModalEvent, Slide};
use crate::ai::ambient_agents::telemetry::{CloudAgentTelemetryEvent, CloudModeEntryPoint};
use crate::i18n::{tr_cached, Message};
use crate::terminal::view::OnboardingIntention;
use crate::ui_components::icons::Icon;
use crate::workspace::action::WorkspaceAction;
use crate::workspace::view::OnboardingTutorial;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::{AdminEnablementSetting, UgcCollectionEnablementSetting};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OzLaunchSlide {
    CloudAgents,
    AgentAutomations,
    AgentManagement,
    LaunchCredits,
}

impl Slide for OzLaunchSlide {
    fn modal_title(&self) -> String {
        tr_cached(Message::OzLaunchIntroducing).to_string()
    }

    fn modal_subtext_paragraphs(&self) -> Vec<FormattedTextLine> {
        vec![FormattedTextLine::Line(vec![
            FormattedTextFragment::plain_text(tr_cached(Message::OzLaunchSubtext)),
        ])]
    }

    fn first() -> Self {
        OzLaunchSlide::CloudAgents
    }

    fn next(&self) -> Option<Self> {
        match self {
            OzLaunchSlide::CloudAgents => Some(OzLaunchSlide::AgentAutomations),
            OzLaunchSlide::AgentAutomations => Some(OzLaunchSlide::AgentManagement),
            OzLaunchSlide::AgentManagement => Some(OzLaunchSlide::LaunchCredits),
            OzLaunchSlide::LaunchCredits => None,
        }
    }

    fn prev(&self) -> Option<Self> {
        match self {
            OzLaunchSlide::CloudAgents => None,
            OzLaunchSlide::AgentAutomations => Some(OzLaunchSlide::CloudAgents),
            OzLaunchSlide::AgentManagement => Some(OzLaunchSlide::AgentAutomations),
            OzLaunchSlide::LaunchCredits => Some(OzLaunchSlide::AgentManagement),
        }
    }

    fn display_text(&self) -> Option<&'static str> {
        Some(match self {
            OzLaunchSlide::CloudAgents => tr_cached(Message::OzLaunchCloudAgents),
            OzLaunchSlide::AgentAutomations => tr_cached(Message::OzLaunchAgentAutomations),
            OzLaunchSlide::AgentManagement => tr_cached(Message::OzLaunchAgentManagement),
            OzLaunchSlide::LaunchCredits => tr_cached(Message::OzLaunchALittleGift),
        })
    }

    fn short_label(&self) -> &'static str {
        match self {
            OzLaunchSlide::CloudAgents => tr_cached(Message::OzLaunchCloudAgents),
            OzLaunchSlide::AgentAutomations => tr_cached(Message::OzLaunchAgentAutomations),
            OzLaunchSlide::AgentManagement => tr_cached(Message::OzLaunchAgentManagement),
            OzLaunchSlide::LaunchCredits => tr_cached(Message::OzLaunchCredits),
        }
    }

    fn title(&self) -> &'static str {
        match self {
            OzLaunchSlide::CloudAgents => tr_cached(Message::OzLaunchCloudAgentsTitle),
            OzLaunchSlide::AgentAutomations => tr_cached(Message::OzLaunchAutomationsTitle),
            OzLaunchSlide::AgentManagement => tr_cached(Message::OzLaunchManagementTitle),
            OzLaunchSlide::LaunchCredits => tr_cached(Message::OzLaunchCreditsTitle),
        }
    }

    fn title_icon(&self) -> Option<Icon> {
        None
    }

    fn content(&self) -> &'static str {
        match self {
            OzLaunchSlide::CloudAgents => tr_cached(Message::OzLaunchCloudAgentsContent),
            OzLaunchSlide::AgentAutomations => tr_cached(Message::OzLaunchAutomationsContent),
            OzLaunchSlide::AgentManagement => tr_cached(Message::OzLaunchManagementContent),
            OzLaunchSlide::LaunchCredits => tr_cached(Message::OzLaunchCreditsContent),
        }
    }

    fn image(&self) -> AssetSource {
        // TODO: Replace with new images once provided.
        match self {
            OzLaunchSlide::CloudAgents => {
                bundled_or_fetched_asset!("png/oz_cloud_agents.png")
            }
            OzLaunchSlide::AgentAutomations => {
                bundled_or_fetched_asset!("png/oz_agent_automations.png")
            }
            OzLaunchSlide::AgentManagement => {
                bundled_or_fetched_asset!("png/oz_agent_management.png")
            }
            OzLaunchSlide::LaunchCredits => {
                bundled_or_fetched_asset!("png/oz_launch_credits.png")
            }
        }
    }

    fn all() -> Vec<Self> {
        vec![
            OzLaunchSlide::CloudAgents,
            OzLaunchSlide::AgentAutomations,
            OzLaunchSlide::AgentManagement,
            OzLaunchSlide::LaunchCredits,
        ]
    }

    fn cta_button(&self) -> CTAButton<Self> {
        match self {
            OzLaunchSlide::CloudAgents
            | OzLaunchSlide::AgentAutomations
            | OzLaunchSlide::AgentManagement => {
                let next = self.next().expect("Non-final slides should have a next");
                CTAButton::next_slide(
                    next,
                    tr_cached(Message::OzLaunchNext).replace("{}", next.short_label()),
                )
            }
            OzLaunchSlide::LaunchCredits => {
                CTAButton::custom(tr_cached(Message::OzLaunchTryItOut), |ctx| {
                    send_telemetry_from_ctx!(
                        CloudAgentTelemetryEvent::EnteredCloudMode {
                            entry_point: CloudModeEntryPoint::OzLaunchModal,
                        },
                        ctx
                    );
                    ctx.emit(LaunchModalEvent::Close);
                    ctx.dispatch_typed_action(&WorkspaceAction::StartAgentOnboardingTutorial(
                        OnboardingTutorial::NoProject {
                            intention: OnboardingIntention::AgentDrivenDevelopment,
                        },
                    ));
                    ctx.dispatch_typed_action(&WorkspaceAction::AddAmbientAgentTab);
                })
            }
        }
    }

    fn secondary_cta_button(&self) -> Option<CTAButton<Self>> {
        match self {
            OzLaunchSlide::LaunchCredits => {
                Some(CTAButton::close(tr_cached(Message::AuthSkipForNow)))
            }
            OzLaunchSlide::CloudAgents
            | OzLaunchSlide::AgentAutomations
            | OzLaunchSlide::AgentManagement => None,
        }
    }

    fn checkbox_config(&self) -> Option<CheckboxConfig> {
        Some(CheckboxConfig {
            label: tr_cached(Message::OzLaunchSyncConversations),
            description: tr_cached(Message::OzLaunchSyncConversationsDesc),
        })
    }

    fn should_show_checkbox(&self, app: &AppContext) -> bool {
        let cloud_storage_setting =
            UserWorkspaces::as_ref(app).get_cloud_conversation_storage_enablement_setting();
        let ugc_setting = UserWorkspaces::as_ref(app).get_ugc_collection_enablement_setting();

        // Show checkbox only when user has control over cloud storage AND UGC is not force-enabled.
        matches!(
            cloud_storage_setting,
            AdminEnablementSetting::RespectUserSetting
        ) && !matches!(ugc_setting, UgcCollectionEnablementSetting::Enable)
    }

    fn on_close(&self, ctx: &mut warpui::ViewContext<super::LaunchModal<Self>>) {
        ctx.dispatch_typed_action(&WorkspaceAction::StartAgentOnboardingTutorial(
            OnboardingTutorial::NoProject {
                intention: OnboardingIntention::AgentDrivenDevelopment,
            },
        ));
    }
}

pub fn init(app: &mut warpui::AppContext) {
    super::init::<OzLaunchSlide>(app);
}
