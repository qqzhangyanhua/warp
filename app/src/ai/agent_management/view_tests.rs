use super::new_agent_workspace_action;
use crate::server::telemetry::AgentModeEntrypoint;
use crate::workspace::WorkspaceAction;

#[test]
fn new_agent_opens_local_agent_mode() {
    assert!(matches!(
        new_agent_workspace_action(),
        WorkspaceAction::NewTabInAgentMode {
            entrypoint: AgentModeEntrypoint::AgentManagementView,
            zero_state_prompt_suggestion_type: None,
        }
    ));
}
