use std::sync::Arc;

use warpui::integration::TestStep;
use warpui::windowing::WindowManager;
use warpui::{async_assert, WindowId};
use warpui_extras::owner_only_file::ExpectedContent;

use crate::ai::facts::view::AIFactPage;
use crate::ai::facts::{GlobalAgentRulesDocument, GlobalAgentRulesState};
use crate::integration_testing::view_getters::workspace_view;

/// Create (or replace) the standard global rules file with the given content.
///
/// The `key` is retained for API compatibility with older multi-rule steps; the
/// Global Rules surface is a single file, so `name` is ignored.
pub fn create_a_personal_rule(
    key: impl Into<String>,
    _name: impl Into<String>,
    content: impl Into<String>,
) -> TestStep {
    let key = key.into();
    let content = Arc::new(content.into());
    TestStep::new("Create the global rules file")
        .with_action(move |_app, _, data| {
            let document =
                GlobalAgentRulesDocument::standard().expect("home directory must be available");
            match document.load().expect("load global rules") {
                GlobalAgentRulesState::Missing => {
                    document
                        .create(content.as_str())
                        .expect("create global rules");
                }
                GlobalAgentRulesState::Present { content_hash, .. } => {
                    document
                        .save(content.as_str(), ExpectedContent::Hash(content_hash))
                        .expect("overwrite global rules");
                }
            }
            data.insert(key.clone(), document.path().to_path_buf());
        })
        .add_assertion(move |_app, _| {
            let document =
                GlobalAgentRulesDocument::standard().expect("home directory must be available");
            async_assert!(
                matches!(
                    document.load().expect("load global rules"),
                    GlobalAgentRulesState::Present { .. }
                ),
                "Global rules file exists"
            )
        })
}

/// Open the Global Rule editor for the standard document.
pub fn open_rule_pane(window_key: impl Into<String>, _key: impl Into<String>) -> TestStep {
    let window_key = window_key.into();

    TestStep::new("Open global rule pane").with_action(move |app, _, data| {
        let window_id: &WindowId = data.get(&window_key).expect("No saved window ID");
        workspace_view(app, *window_id).update(app, |workspace, ctx| {
            WindowManager::as_ref(ctx).show_window_and_focus_app(*window_id);

            let page = AIFactPage::GlobalRuleEditor;
            workspace.open_ai_fact_collection_pane(None, Some(page), ctx);
        })
    })
}

/// Update the global rules file content.
pub fn update_rule_content(
    _fact_key: impl Into<String>,
    new_content: impl Into<String>,
) -> TestStep {
    let new_content = Arc::new(new_content.into());
    TestStep::new("Update global rule content").with_action(move |_app, _, _data| {
        let document =
            GlobalAgentRulesDocument::standard().expect("home directory must be available");
        match document.load().expect("load global rules") {
            GlobalAgentRulesState::Missing => {
                document
                    .create(new_content.as_str())
                    .expect("create global rules");
            }
            GlobalAgentRulesState::Present { content_hash, .. } => {
                document
                    .save(new_content.as_str(), ExpectedContent::Hash(content_hash))
                    .expect("update global rules");
            }
        }
    })
}
