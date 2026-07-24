use warpui::integration::{AssertionCallback, AssertionWithDataCallback};
use warpui::{async_assert, async_assert_eq};

use crate::ai::facts::view::AIFactPage;
use crate::ai::facts::{GlobalAgentRulesDocument, GlobalAgentRulesState};
use crate::integration_testing::view_getters::workspace_view;

/// Assert that the standard global rules file exists with the given content.
pub fn assert_rule_exists(
    _expected_id_key: impl Into<String>,
    expected_content: impl Into<String>,
) -> AssertionWithDataCallback {
    let expected_content = expected_content.into();
    Box::new(move |_app, _window_id, _data| {
        let document =
            GlobalAgentRulesDocument::standard().expect("home directory must be available");
        match document.load().expect("load global rules") {
            GlobalAgentRulesState::Present { content, .. } => {
                async_assert_eq!(
                    content,
                    expected_content,
                    "Global rules content should match"
                )
            }
            GlobalAgentRulesState::Missing => {
                async_assert!(false, "Global rules file should exist")
            }
        }
    })
}

/// Assert that the global rules file is present (count is 0 or 1).
pub fn assert_rule_count(expected_count: usize) -> AssertionCallback {
    Box::new(move |_app, _| {
        let document =
            GlobalAgentRulesDocument::standard().expect("home directory must be available");
        let count = match document.load().expect("load global rules") {
            GlobalAgentRulesState::Present { .. } => 1,
            GlobalAgentRulesState::Missing => 0,
        };
        async_assert_eq!(count, expected_count, "Global rules presence should match")
    })
}

pub fn assert_rule_pane_open(_key: impl Into<String>) -> AssertionWithDataCallback {
    Box::new(move |app, window_id, _data| {
        workspace_view(app, window_id).read(app, |workspace, _ctx| {
            workspace.ai_fact_view().read(app, |ai_fact_view, _ctx| {
                let current_page = ai_fact_view.current_page();
                async_assert_eq!(
                    current_page,
                    AIFactPage::GlobalRuleEditor,
                    "Global rule editor should be open"
                )
            })
        })
    })
}
