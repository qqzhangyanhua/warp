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
    if crate::i18n::active_locale(ctx) != crate::i18n::Locale::ZhCn {
        return description;
    }

    match description {
        "Start a new conversation" => "开始新对话",
        "Start a new cloud agent conversation" => "开始新的云 Agent 对话",
        "Add a new MCP server via the MCP settings page" => "通过 MCP 设置页面添加新的 MCP 服务器",
        "Pull GitHub PR review comments" => "拉取 GitHub PR 审查评论",
        "Create an Oz environment (Docker image + repos) via guided setup" => {
            "通过引导式设置创建 Oz 环境（Docker 镜像 + 仓库）"
        }
        "Create a new docker sandbox terminal session" => "创建新的 Docker 沙盒终端会话",
        "Have Oz walk you through creating a new coding project" => "让 Oz 引导你创建新的代码项目",
        "Open a skill's markdown file in ZYH's built-in editor" => {
            "在 ZYH 内置编辑器中打开技能的 Markdown 文件"
        }
        "Invoke a skill" => "调用技能",
        "Add new Agent prompt" => "添加新的 Agent 提示词",
        "Add a new global rule for the agent" => "为 Agent 添加新的全局规则",
        "Open a file in ZYH's code editor" => "在 ZYH 代码编辑器中打开文件",
        "Rename the current tab" => "重命名当前标签页",
        "Rename the current conversation" => "重命名当前对话",
        "Set the color of the current tab" => "设置当前标签页颜色",
        "Fork the current conversation in a new pane or a new tab" => {
            "在新面板或新标签页中分叉当前对话"
        }
        "Hand off this conversation to a cloud agent" => "将此对话移交给云 Agent",
        "Open code review" => "打开 Code Review",
        "Index this codebase" => "索引此代码库",
        "Index this codebase and generate an AGENTS.md file" => "索引此代码库并生成 AGENTS.md 文件",
        "Open the project rules file (AGENTS.md)" => "打开项目规则文件 (AGENTS.md)",
        "Open MCP servers" => "打开 MCP 服务器",
        "Open settings file (TOML)" => "打开设置文件 (TOML)",
        "Open the latest changelog" => "打开最新变更日志",
        "Send feedback" => "发送反馈",
        "Switch to another indexed repository" => "切换到另一个已索引仓库",
        "View all of your global and project rules" => "查看所有全局规则和项目规则",
        "Start a new conversation (alias for /agent)" => "开始新对话（/agent 的别名）",
        "Switch the base agent model" => "切换基础 Agent 模型",
        "Switch the cloud agent execution host" => "切换云 Agent 执行主机",
        "Switch the cloud agent harness" => "切换云 Agent harness",
        "Switch the cloud agent environment" => "切换云 Agent 环境",
        "Switch the active execution profile" => "切换当前执行配置",
        "Prompt the agent to do some research and create a plan for a task" => {
            "提示 Agent 调研并为任务制定计划"
        }
        "Break a task into subtasks and run them in parallel with multiple agents" => {
            "将任务拆分为子任务并用多个 Agent 并行运行"
        }
        "Free up context by summarizing convo history" => "通过总结对话历史释放上下文",
        "Compact conversation and then send a follow-up prompt" => "压缩对话后发送后续提示词",
        "Queue a prompt to send after the agent finishes responding" => {
            "排队一个在 Agent 回复完成后发送的提示词"
        }
        "Fork current conversation and compact it in the forked copy" => {
            "分叉当前对话并在分叉副本中压缩"
        }
        "Fork conversation from a specific query" => "从指定查询分叉对话",
        "Continue this cloud conversation locally" => "在本地继续此云对话",
        "Open billing and usage settings" => "打开账单和用量设置",
        "Start remote control for this session" => "为此会话启动远程控制",
        "Toggle credit usage details" => "切换点数用量详情",
        "Open conversation history" => "打开对话历史",
        "Search saved prompts" => "搜索已保存提示词",
        "Rewind to a previous point in the conversation" => "回退到对话中的之前位置",
        "Export current conversation to clipboard in markdown format" => {
            "将当前对话以 Markdown 格式导出到剪贴板"
        }
        "Export current conversation to a markdown file" => "将当前对话导出为 Markdown 文件",
        _ => description,
    }
}

pub fn localized_hint_text(ctx: &AppContext, hint_text: &'static str) -> &'static str {
    if crate::i18n::active_locale(ctx) != crate::i18n::Locale::ZhCn {
        return hint_text;
    }

    match hint_text {
        "<optional repo paths or GitHub URLs>" => "<可选仓库路径或 GitHub URL>",
        "<describe what you want to build>" => "<描述你想构建的内容>",
        "<path/to/file[:line[:col]]> or \"@\" to search" => "<文件/路径[:行[:列]]> 或 \"@\" 搜索",
        "<tab name>" => "<标签页名称>",
        "<new title>" => "<新标题>",
        "<optional prompt to send in forked conversation>" => "<发送到分叉对话的可选提示词>",
        "<optional follow-up prompt>" => "<可选后续提示词>",
        "<describe your task>" => "<描述你的任务>",
        "<optional custom summarization instructions>" => "<可选自定义总结指令>",
        "<prompt to send after compaction>" => "<压缩后发送的提示词>",
        "<prompt to send when agent is done>" => "<Agent 完成后发送的提示词>",
        "<optional prompt to send after compaction>" => "<压缩后发送的可选提示词>",
        "<optional prompt to send in local conversation>" => "<发送到本地对话的可选提示词>",
        "<optional filename>" => "<可选文件名>",
        _ => hint_text,
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
