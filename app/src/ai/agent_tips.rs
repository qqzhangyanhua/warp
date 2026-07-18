use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use ai::index::full_source_code_embedding::manager::CodebaseIndexManager;
use markdown_parser::FormattedTextFragment;
use warpui::keymap::Keystroke;
use warpui::r#async::{SpawnedFutureHandle, Timer};
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use crate::ai::persisted_workspace::PersistedWorkspace;
use crate::palette::PaletteMode;
use crate::server::telemetry::PaletteSource;
use crate::settings::AISettings;
use crate::terminal::input::SET_INPUT_MODE_AGENT_ACTION_NAME;
use crate::terminal::view::init::{
    CANCEL_COMMAND_KEYBINDING, SELECT_PREVIOUS_BLOCK_ACTION_NAME,
    TOGGLE_AUTOEXECUTE_MODE_KEYBINDING,
};
use crate::util::bindings::trigger_to_keystroke;
use crate::workspace::view::{
    TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME, TOGGLE_RIGHT_PANEL_BINDING_NAME,
};
use crate::workspace::WorkspaceAction;
use crate::workspaces::user_workspaces::UserWorkspaces;

/// Trait for tip implementations that can be displayed to users.
/// Tips provide helpful information with optional links and keybindings.
pub trait AITip: Clone {
    /// Returns the keystroke for this tip, if applicable.
    fn keystroke(&self, app: &AppContext) -> Option<Keystroke>;

    /// Returns the documentation link for this tip, if available.
    fn link(&self) -> Option<String>;

    /// Returns the raw description text for this tip.
    fn description(&self) -> &str;

    /// Converts the tip to formatted text fragments for rendering.
    /// Default implementation adds "Tip: " prefix and parses backtick-wrapped text as inline code.
    fn to_formatted_text(&self, _app: &AppContext) -> Vec<FormattedTextFragment> {
        let text = format!("Tip: {}", self.description());

        // Style backtick-wrapped text as inline code
        let parts: Vec<&str> = text.split('`').collect();
        let mut fragments = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i % 2 == 0 {
                fragments.push(FormattedTextFragment::plain_text(part.to_string()));
            } else {
                fragments.push(FormattedTextFragment::inline_code(part.to_string()));
            }
        }
        fragments
    }

    /// Checks if this tip is applicable in the current context.
    /// Default implementation returns true (tip is always applicable).
    fn is_tip_applicable(
        &self,
        _current_working_directory: Option<&str>,
        _app: &AppContext,
    ) -> bool {
        true
    }
}

/// Kinds of agent tips for organizing and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentTipKind {
    CodebaseContext,
    WarpDrive,
    General,
    Mcp,
    SlashCommands,
    /// Tips about adding context (files, blocks, URLs, images, @-mentions, rules)
    Context,
    /// Tips about code editors, file trees, and code review panes
    Code,
    /// Tips about local-to-cloud handoff
    Handoff,
}

static DEFAULT_TIPS: LazyLock<Vec<AgentTip>> = LazyLock::new(|| {
    vec![
        AgentTip {
            description: "`/` to open the slash-command menu and access quick agent actions.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/slash-commands".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: "<keybinding> to toggle natural language detection and switch between agent and terminal input.".to_string(),
            link: Some("https://docs.warp.dev/terminal/input/universal-input#input-modes".to_string()),
            binding_name: Some(SET_INPUT_MODE_AGENT_ACTION_NAME),
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "`/plan` <prompt> to create a plan for the agent before executing.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/planning".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: "<keybinding> to open the Command Palette and access ZYH actions and shortcuts.".to_string(),
            link: Some("https://docs.warp.dev/terminal/command-palette".to_string()),
            binding_name: Some(TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME),
            action: Some(WorkspaceAction::OpenPalette {
                mode: PaletteMode::Command,
                source: PaletteSource::AgentTip,
                query: None,
            }),
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Store reusable workflows, notebooks, and prompts in your".to_string(),
            link: Some("https://docs.warp.dev/knowledge-and-collaboration/warp-drive".to_string()),
            binding_name: None,
            action: Some(WorkspaceAction::OpenWarpDrive),
            kind: AgentTipKind::WarpDrive,
        },
        AgentTip {
            description: "Enter a new prompt to redirect the agent while it's running.".to_string(),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "`@` to add context from files, blocks, or ZYH Drive objects to your prompt.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/using-to-add-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "<keybinding> to attach the prior command output as agent context.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/blocks-as-context#attaching-blocks-as-context".to_string()),
            binding_name: Some(SELECT_PREVIOUS_BLOCK_ACTION_NAME),
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "`/init` to index the repo so the agent can understand your codebase.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/codebase-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::CodebaseContext,
        },
        AgentTip {
            description: "Add agent profiles to customize permissions and models per session.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/agent-profiles-permissions".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Right-click a block to fork the conversation from that point.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents/conversation-forking".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Right-click a block to copy a conversation's output.".to_string(),
            link: Some("https://docs.warp.dev/terminal/blocks/block-actions#copy-input-output-of-block".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Drag an image into the pane to attach it as agent context.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/images-as-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "Prompt the agent to control interactive tools like node, python, postgres, gdb, or vim.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/full-terminal-use".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "<keybinding> to open the code review panel and review the agent's changes.".to_string(),
            link: Some("https://docs.warp.dev/code/code-review".to_string()),
            binding_name: Some(TOGGLE_RIGHT_PANEL_BINDING_NAME),
            action: None,
            kind: AgentTipKind::Code,
        },
        AgentTip {
            description: "`/add-mcp` to add an MCP server to your workspace.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/mcp".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Mcp,
        },
        AgentTip {
            description: "`/open-mcp-servers` to view and share MCP servers with your team.".to_string(),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::Mcp,
        },
        AgentTip {
            description: "`/create-environment` to turn a repo into a remote docker environment an agent can run in.".to_string(),
            link: Some("https://docs.warp.dev/reference/cli/integration-setup".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "`/add-prompt` to create a reusable prompt for repeatable workflows.".to_string(),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::WarpDrive,
        },
        AgentTip {
            description: "`/add-rule` to create a global agent rule.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/rules".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "`/fork` to create a fresh copy of the current conversation, optionally with a new prompt.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents/conversation-forking".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: "`/open-code-review` to open the code review panel and inspect agent-generated diffs.".to_string(),
            link: None,
            binding_name: None,
            action: Some(WorkspaceAction::ToggleRightPanel),
            kind: AgentTipKind::Code,
        },
        AgentTip {
            description: "`/new` to start a new agent conversation with clean context.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: "`/compact` to summarize the current conversation and free up space in the context window.".to_string(),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: "`/usage` to show your current AI credits usage.".to_string(),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Use the `oz` command to run an Oz agent in headless mode, useful for remote machines.".to_string(),
            link: Some("https://docs.warp.dev/reference/cli".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Right-click selected text to attach it as agent context.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/blocks-as-context#attaching-blocks-as-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "Use `AGENTS.md` or `CLAUDE.md` to apply project-scoped rules.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/rules#project-rules-1".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "Paste a URL to attach that webpage as context for the agent.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/local-agents/agent-context/urls-as-context".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::Context,
        },
        AgentTip {
            description: "Warpify a remote SSH session to enable Oz inside that environment.".to_string(),
            link: Some("https://docs.warp.dev/terminal/warpify".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Switch agent profiles to quickly change models and agent permissions.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/agent-profiles-permissions".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "`/init` to generate a `WARP.md` file and define project rules for the agent.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/rules".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::SlashCommands,
        },
        AgentTip {
            description: "<keybinding> to auto-approve the agent's commands and diffs for the rest of the session.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/capabilities/full-terminal-use#session-level-approvals".to_string()),
            binding_name: Some(TOGGLE_AUTOEXECUTE_MODE_KEYBINDING),
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "Type `&` or use the handoff chip to move a local conversation to the cloud.".to_string(),
            link: None,
            binding_name: None,
            action: None,
            kind: AgentTipKind::Handoff,
        },
        AgentTip {
            description: "Enable desktop notifications to get an alert when an agent needs your attention.".to_string(),
            link: Some("https://docs.warp.dev/agent-platform/cloud-agents/managing-cloud-agents#in-app-agent-notifications".to_string()),
            binding_name: None,
            action: None,
            kind: AgentTipKind::General,
        },
        AgentTip {
            description: "<keybinding> to cancel the current agent task.".to_string(),
            link: None,
            binding_name: Some(CANCEL_COMMAND_KEYBINDING),
            action: None,
            kind: AgentTipKind::General,
        },
    ]
});

#[derive(Clone, Debug)]
pub struct AgentTip {
    /// The text that will be displayed to the user. This is parsed such that:
    /// "Tip: " is added as a prefix,
    /// "<keybinding>" is replaced with user-defined and platform-specific keybinding referenced by binding_name,
    /// `text` that is wrapped in backticks is formatted as inline code
    pub description: String,
    pub link: Option<String>,
    pub binding_name: Option<&'static str>,
    pub action: Option<WorkspaceAction>,
    /// The kind of the tip, used for filtering and organization
    pub kind: AgentTipKind,
}

fn localized_tip_description(ctx: &AppContext, en: &str) -> String {
    if crate::i18n::active_locale(ctx) != crate::i18n::Locale::ZhCn {
        return en.to_string();
    }

    match en {
        "`/` to open the slash-command menu and access quick agent actions." =>
            "使用 `/` 打开快捷命令菜单，访问 Agent 快捷操作。".into(),
        "<keybinding> to toggle natural language detection and switch between agent and terminal input." =>
            "按 <keybinding> 切换自然语言检测，在 Agent 输入和终端输入之间切换。".into(),
        "`/plan` <prompt> to create a plan for the agent before executing." =>
            "使用 `/plan` <提示词> 在执行前为 Agent 创建执行计划。".into(),
        "<keybinding> to open the Command Palette and access ZYH actions and shortcuts." =>
            "按 <keybinding> 打开命令面板，访问 ZYH 操作和快捷键。".into(),
        "Store reusable workflows, notebooks, and prompts in your" =>
            "在 ZYH Drive 中存储可复用的工作流、笔记本和提示词。".into(),
        "Enter a new prompt to redirect the agent while it's running." =>
            "输入新的提示词，在 Agent 运行时重定向其任务。".into(),
        "`@` to add context from files, blocks, or ZYH Drive objects to your prompt." =>
            "使用 `@` 将文件、块或 ZYH Drive 对象作为上下文添加到提示词中。".into(),
        "<keybinding> to attach the prior command output as agent context." =>
            "按 <keybinding> 将上一条命令输出附加为 Agent 上下文。".into(),
        "`/init` to index the repo so the agent can understand your codebase." =>
            "使用 `/init` 索引仓库，让 Agent 理解你的代码库。".into(),
        "Add agent profiles to customize permissions and models per session." =>
            "添加 Agent 配置文件，按会话自定义权限和模型。".into(),
        "Right-click a block to fork the conversation from that point." =>
            "右键点击一个块，从该位置分叉对话。".into(),
        "Right-click a block to copy a conversation's output." =>
            "右键点击一个块，复制对话输出。".into(),
        "Drag an image into the pane to attach it as agent context." =>
            "将图片拖入面板，将其作为 Agent 上下文附加。".into(),
        "Prompt the agent to control interactive tools like node, python, postgres, gdb, or vim." =>
            "提示 Agent 控制交互式工具，如 node、python、postgres、gdb 或 vim。".into(),
        "<keybinding> to open the code review panel and review the agent's changes." =>
            "按 <keybinding> 打开 Code Review 面板，审查 Agent 的更改。".into(),
        "`/add-mcp` to add an MCP server to your workspace." =>
            "使用 `/add-mcp` 向工作区添加 MCP 服务器。".into(),
        "`/open-mcp-servers` to view and share MCP servers with your team." =>
            "使用 `/open-mcp-servers` 查看并与团队共享 MCP 服务器。".into(),
        "`/create-environment` to turn a repo into a remote docker environment an agent can run in." =>
            "使用 `/create-environment` 将仓库转换为 Agent 可运行的远程 Docker 环境。".into(),
        "`/add-prompt` to create a reusable prompt for repeatable workflows." =>
            "使用 `/add-prompt` 创建可复用的提示词，用于重复性工作流。".into(),
        "`/add-rule` to create a global agent rule." =>
            "使用 `/add-rule` 创建全局 Agent 规则。".into(),
        "`/fork` to create a fresh copy of the current conversation, optionally with a new prompt." =>
            "使用 `/fork` 创建当前对话的新副本，可选择附带新提示词。".into(),
        "`/open-code-review` to open the code review panel and inspect agent-generated diffs." =>
            "使用 `/open-code-review` 打开 Code Review 面板，检查 Agent 生成的差异。".into(),
        "`/new` to start a new agent conversation with clean context." =>
            "使用 `/new` 以干净上下文开始新的 Agent 对话。".into(),
        "`/compact` to summarize the current conversation and free up space in the context window." =>
            "使用 `/compact` 总结当前对话，释放上下文窗口空间。".into(),
        "`/usage` to show your current AI credits usage." =>
            "使用 `/usage` 查看当前 AI 点数用量。".into(),
        "Use the `oz` command to run an Oz agent in headless mode, useful for remote machines." =>
            "使用 `oz` 命令以无头模式运行 Oz Agent，适用于远程机器。".into(),
        "Right-click selected text to attach it as agent context." =>
            "右键点击选中文本，将其作为 Agent 上下文附加。".into(),
        "Use `AGENTS.md` or `CLAUDE.md` to apply project-scoped rules." =>
            "使用 `AGENTS.md` 或 `CLAUDE.md` 应用项目级别的规则。".into(),
        "Paste a URL to attach that webpage as context for the agent." =>
            "粘贴 URL，将该网页作为 Agent 上下文附加。".into(),
        "Warpify a remote SSH session to enable Oz inside that environment." =>
            "对远程 SSH 会话启用 Warpify，在该环境中使用 Oz。".into(),
        "Switch agent profiles to quickly change models and agent permissions." =>
            "切换 Agent 配置文件，快速更改模型和 Agent 权限。".into(),
        "`/init` to generate a `WARP.md` file and define project rules for the agent." =>
            "使用 `/init` 生成 `WARP.md` 文件，为 Agent 定义项目规则。".into(),
        "<keybinding> to auto-approve the agent's commands and diffs for the rest of the session." =>
            "按 <keybinding> 自动批准 Agent 在当前会话后续的命令和差异。".into(),
        "Type `&` or use the handoff chip to move a local conversation to the cloud." =>
            "输入 `&` 或使用移交芯片，将本地对话转移到云端。".into(),
        "Enable desktop notifications to get an alert when an agent needs your attention." =>
            "启用桌面通知，在 Agent 需要你关注时收到提醒。".into(),
        "<keybinding> to cancel the current agent task." =>
            "按 <keybinding> 取消当前 Agent 任务。".into(),
        "Hold <keybinding> to speak your prompt directly to the agent." =>
            "按住 <keybinding> 直接对 Agent 说出你的提示词。".into(),
        _ => en.to_string(),
    }
}

impl AITip for AgentTip {
    fn keystroke(&self, app: &AppContext) -> Option<Keystroke> {
        let binding_name = self.binding_name?;

        // Special case: voice input uses settings, not editable bindings
        if binding_name == "FN" {
            return AISettings::as_ref(app).voice_input_toggle_key.keystroke();
        }

        if let Some(binding) = app.editable_bindings().find(|b| b.name == binding_name) {
            return trigger_to_keystroke(binding.trigger);
        }
        None
    }

    fn link(&self) -> Option<String> {
        self.link.clone()
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn to_formatted_text(&self, app: &AppContext) -> Vec<FormattedTextFragment> {
        let prefix = match crate::i18n::active_locale(app) {
            crate::i18n::Locale::ZhCn => "提示：",
            _ => "Tip: ",
        };
        let desc = localized_tip_description(app, &self.description);
        let mut text = format!("{}{}", prefix, desc);

        // Replace <keybinding> with the actual keybinding string
        if let Some(keystroke) = self.keystroke(app) {
            text = text.replace("<keybinding>", &keystroke.displayed());
        }

        // Style backtick-wrapped text as inline code
        let parts: Vec<&str> = text.split('`').collect();
        let mut fragments = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i % 2 == 0 {
                fragments.push(FormattedTextFragment::plain_text(part.to_string()));
            } else {
                fragments.push(FormattedTextFragment::inline_code(part.to_string()));
            }
        }

        fragments
    }

    fn is_tip_applicable(&self, current_working_directory: Option<&str>, app: &AppContext) -> bool {
        // Tips about indexing the repo are only applicable if the current directory is not already indexed.
        if matches!(self.kind, AgentTipKind::CodebaseContext) {
            let Some(cwd) = current_working_directory else {
                return true;
            };
            let Some(root) = PersistedWorkspace::as_ref(app).root_for_workspace(Path::new(cwd))
            else {
                return true;
            };
            return CodebaseIndexManager::as_ref(app)
                .get_codebase_index_status_for_path(root, app)
                .is_none();
        }
        // Handoff tips only apply when the feature is available and enabled.
        if matches!(self.kind, AgentTipKind::Handoff) {
            return AISettings::as_ref(app).is_cloud_handoff_enabled(app);
        }
        // Tips whose description references a keybinding placeholder should only be shown
        // when the keybinding is actually configured, so we never display the raw
        // "<keybinding>" string to users.
        if self.description.contains("<keybinding>") && self.keystroke(app).is_none() {
            return false;
        }
        true
    }
}

impl WorkspaceAction {
    pub fn display_text(&self) -> Option<String> {
        match self {
            WorkspaceAction::OpenPalette { .. } => Some("Open palette".to_string()),
            WorkspaceAction::OpenWarpDrive => Some("ZYH Drive.".to_string()),
            WorkspaceAction::ToggleRightPanel => Some("Show diff view".to_string()),
            _ => None,
        }
    }
}

/// Helper function to build the list of agent tips, including the voice tip if enabled.
pub fn get_agent_tips(ctx: &AppContext) -> Vec<AgentTip> {
    let mut tips = DEFAULT_TIPS.clone();

    if cfg!(feature = "voice_input")
        && UserWorkspaces::as_ref(ctx).is_voice_enabled()
        && AISettings::as_ref(ctx).is_voice_input_enabled(ctx)
    {
        tips.push(AgentTip {
            description: "Hold <keybinding> to speak your prompt directly to the agent."
                .to_string(),
            link: Some(
                "https://docs.warp.dev/agent-platform/local-agents/interacting-with-agents/voice"
                    .to_string(),
            ),
            binding_name: Some("FN"),
            action: None,
            kind: AgentTipKind::General,
        });
    }

    tips
}

/// A model for managing tips with cooldown logic.
/// Generic over any type implementing the AITip trait.
pub struct AITipModel<T: AITip> {
    tips: Vec<T>,
    current_tip: Option<T>,
    cooldown_handle: Option<SpawnedFutureHandle>,
}

impl<T: AITip + 'static> AITipModel<T> {
    /// Creates a new AITipModel with the given tips.
    /// Selects a random initial tip from the provided tips.
    ///
    /// # Panics
    /// Panics if the tips vector is empty.
    pub fn new(tips: Vec<T>) -> Self {
        use rand::seq::SliceRandom;
        debug_assert!(!tips.is_empty(), "AITipModel must have at least one tip");

        let mut rng = rand::thread_rng();
        let current_tip = tips.choose(&mut rng).cloned();

        Self {
            tips,
            current_tip,
            cooldown_handle: None,
        }
    }

    /// Returns the current tip, if one has been selected.
    pub fn current_tip(&self) -> Option<&T> {
        self.current_tip.as_ref()
    }
}

impl<T: AITip + 'static> Entity for AITipModel<T> {
    type Event = ();
}

// Specific implementation for AgentTip
impl AITipModel<AgentTip> {
    /// Creates a new AITipModel for AgentTips.
    /// This is the constructor used for the singleton model.
    pub fn new_for_agent_tips(ctx: &AppContext) -> Self {
        let tips = get_agent_tips(ctx);
        // Pick an applicable tip so we never show a raw "<keybinding>" placeholder on first render.
        let current_tip = Self::pick_random_applicable_tip(&tips, None, ctx);

        Self {
            tips,
            current_tip,
            cooldown_handle: None,
        }
    }

    /// Rebuilds the tip pool from current settings and invalidates the current tip
    /// if it is no longer applicable. Resets the cooldown timer so the revalidated
    /// tip is shown for the full cooldown period before the next rotation.
    pub fn revalidate_tips(&mut self, ctx: &mut ModelContext<Self>) {
        self.tips = get_agent_tips(ctx);

        // If the current tip is no longer in the pool or no longer applicable, pick a new one.
        let should_replace = self
            .current_tip
            .as_ref()
            .map(|current_tip| {
                let still_in_pool = self
                    .tips
                    .iter()
                    .any(|tip| tip.description == current_tip.description);

                !still_in_pool || !current_tip.is_tip_applicable(None, ctx)
            })
            .unwrap_or(true);

        if should_replace {
            let new_tip = Self::pick_random_applicable_tip(&self.tips, None, ctx);
            if new_tip.is_some() || self.current_tip.is_some() {
                self.current_tip = new_tip;
                self.reset_cooldown(ctx);
                ctx.notify();
            }
        }
    }

    /// Refreshes the current tip with a new random selection that is applicable
    /// for the given working directory.
    /// Only updates if not in cooldown period (60 seconds).
    pub fn maybe_refresh_tip(
        &mut self,
        current_working_directory: Option<&str>,
        ctx: &mut ModelContext<Self>,
    ) {
        // Don't update if cooldown is active
        if self.cooldown_handle.is_some() {
            return;
        }

        // Rebuild tips from current settings so changes are picked up.
        self.tips = get_agent_tips(ctx);

        self.current_tip =
            Self::pick_random_applicable_tip(&self.tips, current_working_directory, ctx);

        // Start 60-second cooldown
        let handle = ctx.spawn(
            async {
                Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
        ctx.notify();
    }

    /// Picks a random applicable tip from the given pool, filtered by working directory.
    /// Returns `None` if no tips are applicable.
    fn pick_random_applicable_tip(
        tips: &[AgentTip],
        current_working_directory: Option<&str>,
        ctx: &AppContext,
    ) -> Option<AgentTip> {
        use rand::seq::SliceRandom;
        let available: Vec<&AgentTip> = tips
            .iter()
            .filter(|tip| tip.is_tip_applicable(current_working_directory, ctx))
            .collect();
        let mut rng = rand::thread_rng();
        available.choose(&mut rng).copied().cloned()
    }

    /// Resets the cooldown timer so the current tip is shown for the full
    /// cooldown period before the next rotation.
    fn reset_cooldown(&mut self, ctx: &mut ModelContext<Self>) {
        if let Some(handle) = self.cooldown_handle.take() {
            handle.abort();
        }
        let handle = ctx.spawn(
            async {
                Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
    }
}

impl SingletonEntity for AITipModel<AgentTip> {}

// Specific implementation for CloudModeTip
impl AITipModel<crate::terminal::view::ambient_agent::CloudModeTip> {
    /// Refreshes the current tip with a new random selection.
    /// Only updates if not in cooldown period (60 seconds).
    pub fn maybe_refresh_tip(&mut self, ctx: &mut ModelContext<Self>) {
        // Don't update if cooldown is active
        if self.cooldown_handle.is_some() {
            return;
        }

        use rand::seq::SliceRandom;

        // Select a random tip
        let mut rng = rand::thread_rng();
        self.current_tip = self.tips.choose(&mut rng).cloned();

        // Start 60-second cooldown
        let handle = ctx.spawn(
            async {
                Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
        ctx.notify();
    }

    /// Resets the cooldown timer without changing the current tip.
    /// This ensures the current tip will be shown for the full cooldown period.
    pub fn reset_cooldown(&mut self, ctx: &mut ModelContext<Self>) {
        // Cancel any existing cooldown
        if let Some(handle) = self.cooldown_handle.take() {
            handle.abort();
        }

        // Start a new 60-second cooldown
        let handle = ctx.spawn(
            async {
                Timer::after(Duration::from_secs(60)).await;
            },
            |me, _, _| {
                me.cooldown_handle = None;
            },
        );
        self.cooldown_handle = Some(handle);
    }
}
