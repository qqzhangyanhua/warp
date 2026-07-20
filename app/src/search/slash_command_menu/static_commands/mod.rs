pub mod bindings;
pub mod commands;

use bitflags::bitflags;
pub use commands::SlashCommandId;
use warpui::AppContext;

bitflags! {
    /// Specifies the requirements for a slash command to be available.
    ///
    /// Each flag represents a requirement that the session context must satisfy. The command is
    /// available when the session supports *all* of the command's requirement flags.
    ///
    /// A few common cases:
    /// * If neither [`Self::AGENT_VIEW`] nor [`Self::TERMINAL_VIEW`] is set, the command is available in all modes.
    ///   A command should *not* set both flags to be available in both modes - this results in requirements that cannot be satisfied.
    /// * Most `/fork`-like slash commands require [`Self::NO_LRC_CONTROL`] and [`Self::ACTIVE_CONVERSATION`]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Availability: u16 {
        /// No requirements — always available.
        const ALWAYS = 0;
        /// Requires the agent view.
        const AGENT_VIEW = 1 << 0;
        /// Requires the terminal view.
        const TERMINAL_VIEW = 1 << 1;
        /// Requires a local session (not available in remote/cloud sessions).
        const LOCAL = 1 << 2;
        /// Requires a git repository.
        const REPOSITORY = 1 << 3;
        /// Requires that the agent is not currently in control of a long-running command.
        const NO_LRC_CONTROL = 1 << 4;
        /// Requires an active AI conversation.
        const ACTIVE_CONVERSATION = 1 << 5;
        /// Requires codebase context to be enabled.
        const CODEBASE_CONTEXT = 1 << 6;
        /// Requires AI to be globally enabled.
        const AI_ENABLED = 1 << 7;
        /// Requires a non-cloud-agent context.
        const NOT_CLOUD_AGENT = 1 << 8;
        /// Requires a cloud-agent context.
        const CLOUD_AGENT = 1 << 9;
        /// Set on the session context iff the slash command data source was constructed via
        /// `SlashCommandDataSource::for_cloud_mode_v2` *and* `FeatureFlag::CloudModeInputV2`
        /// is enabled. Commands that require this bit are hidden everywhere except the V2
        /// cloud-mode composing input.
        const CLOUD_MODE_V2_COMPOSER = 1 << 10;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Argument {
    pub hint_text: Option<&'static str>,
    pub is_optional: bool,
    /// If `true`, selecting the slash command from the menu (or via keybinding) will execute the
    /// slash command with no arguments.
    ///
    /// If `false`, selecting the slash command from the menu (or via keybinding) inserts the
    /// slash command into the input.
    ///
    /// Set this based on whether or not you want you think a user should always have the option to
    /// supply an argument.
    pub should_execute_on_selection: bool,
}

impl Argument {
    pub(super) fn optional() -> Self {
        Self {
            is_optional: true,
            ..Default::default()
        }
    }

    pub(super) fn required() -> Self {
        Self {
            is_optional: false,
            ..Default::default()
        }
    }

    pub(super) fn with_hint_text(mut self, text: &'static str) -> Self {
        self.hint_text = Some(text);
        self
    }

    pub(super) fn with_execute_on_selection(mut self) -> Self {
        self.should_execute_on_selection = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub icon_path: &'static str,
    /// Specifies the requirements for this command to be available. See [`Availability`].
    pub availability: Availability,
    /// Whether this command requires AI mode when executed.
    /// If true, AI mode will be activated when the command is accepted.
    pub auto_enter_ai_mode: bool,
    pub argument: Option<Argument>,
}

pub fn localized_command_description(ctx: &AppContext, description: &'static str) -> &'static str {
    if let Some(message) = slash_command_description_message(description) {
        crate::i18n::tr(ctx, message)
    } else {
        description
    }
}

fn slash_command_description_message(description: &str) -> Option<crate::i18n::Message> {
    use crate::i18n::Message;
    match description {
        "Start a new conversation" => Some(Message::SlashDescStartNewConversation),
        "Start a new cloud agent conversation" => Some(Message::SlashDescStartCloudAgentConversation),
        "Add a new MCP server via the MCP settings page" => Some(Message::SlashDescAddMcpServer),
        "Pull GitHub PR review comments" => Some(Message::SlashDescPullPrReviewComments),
        "Create an Oz environment (Docker image + repos) via guided setup" => Some(Message::SlashDescCreateOzEnvironment),
        "Create a new docker sandbox terminal session" => Some(Message::SlashDescCreateDockerSandbox),
        "Have Oz walk you through creating a new coding project" => Some(Message::SlashDescCreateCodingProject),
        "Open a skill's markdown file in ZYH's built-in editor" => Some(Message::SlashDescOpenSkillMarkdown),
        "Invoke a skill" => Some(Message::SlashDescInvokeSkill),
        "Add new Agent prompt" => Some(Message::SlashDescAddAgentPrompt),
        "Add a new global rule for the agent" => Some(Message::SlashDescAddGlobalRule),
        "Open a file in ZYH's code editor" => Some(Message::SlashDescOpenFileInEditor),
        "Rename the current tab" => Some(Message::SlashDescRenameCurrentTab),
        "Rename the current conversation" => Some(Message::SlashDescRenameCurrentConversation),
        "Set the color of the current tab" => Some(Message::SlashDescSetTabColor),
        "Fork the current conversation in a new pane or a new tab" => Some(Message::SlashDescForkConversation),
        "Hand off this conversation to a cloud agent" => Some(Message::SlashDescHandoffCloudAgent),
        "Open code review" => Some(Message::SlashDescOpenCodeReview),
        "Index this codebase" => Some(Message::SlashDescIndexCodebase),
        "Index this codebase and generate an AGENTS.md file" => Some(Message::SlashDescIndexCodebaseAgentsMd),
        "Open the project rules file (AGENTS.md)" => Some(Message::SlashDescOpenProjectRules),
        "Open MCP servers" => Some(Message::SlashDescOpenMcpServers),
        "Open settings file (TOML)" => Some(Message::SlashDescOpenSettingsToml),
        "Open the latest changelog" => Some(Message::SlashDescOpenChangelog),
        "Send feedback" => Some(Message::SlashDescSendFeedback),
        "Switch to another indexed repository" => Some(Message::SlashDescSwitchIndexedRepo),
        "View all of your global and project rules" => Some(Message::SlashDescViewAllRules),
        "Start a new conversation (alias for /agent)" => Some(Message::SlashDescStartConversationAlias),
        "Switch the base agent model" => Some(Message::SlashDescSwitchBaseModel),
        "Switch the cloud agent execution host" => Some(Message::SlashDescSwitchCloudHost),
        "Switch the cloud agent harness" => Some(Message::SlashDescSwitchCloudHarness),
        "Switch the cloud agent environment" => Some(Message::SlashDescSwitchCloudEnvironment),
        "Switch the active execution profile" => Some(Message::SlashDescSwitchExecutionProfile),
        "Prompt the agent to do some research and create a plan for a task" => Some(Message::SlashDescResearchAndPlan),
        "Break a task into subtasks and run them in parallel with multiple agents" => Some(Message::SlashDescParallelSubtasks),
        "Free up context by summarizing convo history" => Some(Message::SlashDescFreeContextSummarize),
        "Compact conversation and then send a follow-up prompt" => Some(Message::SlashDescCompactThenFollowUp),
        "Queue a prompt to send after the agent finishes responding" => Some(Message::SlashDescQueuePrompt),
        "Fork current conversation and compact it in the forked copy" => Some(Message::SlashDescForkAndCompact),
        "Fork conversation from a specific query" => Some(Message::SlashDescForkFromQuery),
        "Continue this cloud conversation locally" => Some(Message::SlashDescContinueCloudLocally),
        "Open billing and usage settings" => Some(Message::SlashDescOpenBillingUsage),
        "Start remote control for this session" => Some(Message::SlashDescStartRemoteControl),
        "Toggle credit usage details" => Some(Message::SlashDescToggleCreditUsage),
        "Open conversation history" => Some(Message::SlashDescOpenConversationHistory),
        "Search saved prompts" => Some(Message::SlashDescSearchSavedPrompts),
        "Rewind to a previous point in the conversation" => Some(Message::SlashDescRewindConversation),
        "Export current conversation to clipboard in markdown format" => Some(Message::SlashDescExportConversationClipboard),
        "Export current conversation to a markdown file" => Some(Message::SlashDescExportConversationFile),
        _ => None,
    }
}

pub fn localized_hint_text(ctx: &AppContext, hint_text: &'static str) -> &'static str {
    if let Some(message) = slash_hint_message(hint_text) {
        crate::i18n::tr(ctx, message)
    } else {
        hint_text
    }
}

fn slash_hint_message(hint_text: &str) -> Option<crate::i18n::Message> {
    use crate::i18n::Message;
    match hint_text {
        "<optional repo paths or GitHub URLs>" => Some(Message::SlashHintOptionalRepoPaths),
        "<describe what you want to build>" => Some(Message::SlashHintDescribeWhatToBuild),
        "<path/to/file[:line[:col]]> or \"@\" to search" => Some(Message::SlashHintPathOrAtSearch),
        "<tab name>" => Some(Message::SlashHintTabName),
        "<new title>" => Some(Message::SlashHintNewTitle),
        "<optional prompt to send in forked conversation>" => Some(Message::SlashHintOptionalForkPrompt),
        "<optional follow-up prompt>" => Some(Message::SlashHintOptionalFollowUp),
        "<describe your task>" => Some(Message::SlashHintDescribeTask),
        "<optional custom summarization instructions>" => Some(Message::SlashHintOptionalSummarizeInstructions),
        "<prompt to send after compaction>" => Some(Message::SlashHintPromptAfterCompaction),
        "<prompt to send when agent is done>" => Some(Message::SlashHintPromptWhenAgentDone),
        "<optional prompt to send after compaction>" => Some(Message::SlashHintOptionalPromptAfterCompaction),
        "<optional prompt to send in local conversation>" => Some(Message::SlashHintOptionalLocalPrompt),
        "<optional filename>" => Some(Message::SlashHintOptionalFilename),
        _ => None,
    }
}


impl StaticCommand {
    pub fn localized_description(&self, ctx: &AppContext) -> &'static str {
        localized_command_description(ctx, self.description)
    }

    pub fn matches_filter(&self, filter_text: &str) -> bool {
        if filter_text.is_empty() {
            return true;
        }

        let filter_lower = filter_text.to_lowercase();
        self.name
            .to_lowercase()
            .get(1..)
            .unwrap_or("")
            .starts_with(&filter_lower)
    }

    pub fn is_active(&self, session_context: Availability) -> bool {
        session_context.contains(self.availability)
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
