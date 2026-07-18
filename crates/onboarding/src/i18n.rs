/// A lightweight locale enum for the onboarding crate.
/// Mirrors `app::i18n::Locale` so the two crates stay compatible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
    En,
    ZhCn,
}

/// Message variants covering onboarding slide copy, common button labels,
/// feature-list items, and opt-out dialog strings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnboardingMessage {
    // --- intro_slide ---
    WelcomeToWarp,
    ModernTerminalDescription,
    AlreadyHaveAccount,
    LogIn,
    GetStarted,

    // --- intention_slide ---
    HowDoYouWantToWork,
    BuildFasterWithAgents,
    AgentCardDescription,
    JustUseTheTerminal,
    NoAiFeatures,
    TerminalCardDescription,

    // --- ai_setup_slide ---
    ChooseYourAiSetup,
    ChooseYourAiSetupDescription,
    UseWarpAgent,
    AccessMoreModels,
    WarpAgentDescription,
    UseThirdPartyAgents,
    ThirdPartyDescription,

    // --- ai_access_slide ---
    GetAiAccess,
    ConfigureAi,
    AiAccessSubtitleLoggedIn,
    AiAccessSubtitleAnonymous,
    Subscription,
    BestValue,
    SubscriptionPricing,
    SetUpLater,
    SetUpLaterDescription,
    IfBrowserHasntLaunched,
    CopyTheUrl,
    AndOpenThePageManually,
    ClickHere,
    ToPasteYourToken,

    // --- project_slide ---
    OpenAProject,
    ProjectSubtitle,
    OpenLocalFolder,
    Skip,
    InitializeProjectAutomatically,
    InitializeProjectDescription,
    GetWarping,

    // --- theme_picker_slide ---
    ChooseATheme,
    ThemeSubtitle,
    SyncWithOs,
    OptOutAnalytics,
    PrivacySettings,
    ByContinuingAgreeTo,
    TermsOfService,

    // --- customize_slide ---
    CustomizeYourWarp,
    CustomizeSubtitle,
    TabStyling,
    Vertical,
    Horizontal,
    ToolsPanel,
    Enabled,
    Disabled,
    FileExplorer,
    ConversationHistory,
    GlobalFileSearch,
    WarpDrive,
    CodeReview,

    // --- agent_slide ---
    CustomizeYourWarpAgent,
    AgentSlideSubtitle,
    DefaultModel,
    Recommended,
    Autonomy,
    Full,
    Partial,
    NoneAutonomy,
    FullAutonomyDescription,
    PartialAutonomyDescription,
    NoneAutonomyDescription,
    SetByTeamWorkspace,
    AutonomyTeamWorkspaceDescription,

    // --- third_party_slide ---
    CustomizeThirdPartyAgents,
    ThirdPartySubtitle,
    CliAgentToolbar,
    Notifications,

    // --- common buttons ---
    Back,
    Next,

    // --- feature lists ---
    AiFeature0,
    AiFeature1,
    AiFeature2,
    AiFeature3,
    AiFeature4,
    AiFeature5,
    WarpAgentFeature0,
    WarpAgentFeature1,
    WarpAgentFeature2,
    WarpAgentFeature3,
    WarpDriveFeature0,
    WarpDriveFeature1,

    // --- opt-out dialog ---
    AreYouSureNoAi,
    NoAiWarningBody,
    GiveMeAiFeatures,
    IDontWantAi,
    PlanSuccessfullyActivated,

    // --- onboarding callouts ---
    MeetWarpInput,
    UniversalInputDescription,
    TalkToAgent,
    TalkToAgentDescription,
    Submit,
    Finish,
    WelcomeTerminalMode,
    YoureInTerminalMode,
    TerminalModeDescription,
    EnableNaturalLanguageDetection,
    YoureInAgentMode,
    AgentModeProjectDescription,
    SkipInitialization,
    Initialize,
    AgentModeDescription,
    BackToTerminal,
}

/// Resolves a message to its translated text for the given locale.
pub fn tr(message: OnboardingMessage, locale: Locale) -> &'static str {
    match locale {
        Locale::En => en_text(message),
        Locale::ZhCn => zh_cn_text(message),
    }
}

fn en_text(message: OnboardingMessage) -> &'static str {
    match message {
        OnboardingMessage::WelcomeToWarp => "Welcome to ZYH",
        OnboardingMessage::ModernTerminalDescription => {
            "A modern terminal with state of the art agents built in."
        },
        OnboardingMessage::AlreadyHaveAccount => "Already have an account? ",
        OnboardingMessage::LogIn => "Log in",
        OnboardingMessage::GetStarted => "Get started",

        OnboardingMessage::HowDoYouWantToWork => "How do you want to work?",
        OnboardingMessage::BuildFasterWithAgents => "Build faster with agents",
        OnboardingMessage::AgentCardDescription => {
            "Get AI features to accelerate terminal and agent-driven workflows:"
        },
        OnboardingMessage::JustUseTheTerminal => "Just use the terminal",
        OnboardingMessage::NoAiFeatures => "No AI features",
        OnboardingMessage::TerminalCardDescription => {
            "A modern terminal optimized for speed, context, and control without AI."
        },

        OnboardingMessage::ChooseYourAiSetup => "Choose your AI setup",
        OnboardingMessage::ChooseYourAiSetupDescription => {
            "Choose if you'd like to use ZYH Agent or third party agents."
        },
        OnboardingMessage::UseWarpAgent => "Use ZYH Agent",
        OnboardingMessage::AccessMoreModels => "Access more models",
        OnboardingMessage::WarpAgentDescription => {
            "State of the art agent harness deeply integrated into the terminal."
        },
        OnboardingMessage::UseThirdPartyAgents => "Use third party agents",
        OnboardingMessage::ThirdPartyDescription => {
            "Use agents like Claude Code, Codex, and Gemini."
        },

        OnboardingMessage::GetAiAccess => "Get AI access",
        OnboardingMessage::ConfigureAi => "Configure AI",
        OnboardingMessage::AiAccessSubtitleLoggedIn => {
            "Save with a recurring plan, or explore ZYH's AI before committing."
        },
        OnboardingMessage::AiAccessSubtitleAnonymous => {
            "Add an OpenAI-compatible provider from Settings when you are ready."
        },
        OnboardingMessage::Subscription => "Subscription",
        OnboardingMessage::BestValue => "Best value",
        OnboardingMessage::SubscriptionPricing => {
            "Starting at $18 / mo, available with monthly or annual plans. Includes base credits, \
             frontier models, cloud agents, collaboration, and more."
        },
        OnboardingMessage::SetUpLater => "Set up later",
        OnboardingMessage::SetUpLaterDescription => {
            "Explore ZYH's built-in AI features before committing to a plan, or bring your own \
             inference."
        },
        OnboardingMessage::IfBrowserHasntLaunched => "If your browser hasn't launched, ",
        OnboardingMessage::CopyTheUrl => "copy the URL",
        OnboardingMessage::AndOpenThePageManually => " and open the page manually. ",
        OnboardingMessage::ClickHere => "Click here",
        OnboardingMessage::ToPasteYourToken => " to paste your token from the browser.",

        OnboardingMessage::OpenAProject => "Open a project",
        OnboardingMessage::ProjectSubtitle => {
            "Set up a project to optimize it for coding in ZYH."
        },
        OnboardingMessage::OpenLocalFolder => "Open local folder",
        OnboardingMessage::Skip => "Skip",
        OnboardingMessage::InitializeProjectAutomatically => "Initialize project automatically",
        OnboardingMessage::InitializeProjectDescription => {
            "Prepares the project environment, builds an index of your code, and generates project \
             rules\u{2014}giving the agent deeper understanding and better performance."
        },
        OnboardingMessage::GetWarping => "Get started",

        OnboardingMessage::ChooseATheme => "Choose a theme",
        OnboardingMessage::ThemeSubtitle => {
            "Click or use arrow keys to select, Enter to confirm."
        },
        OnboardingMessage::SyncWithOs => "Sync light/dark theme with OS",
        OnboardingMessage::OptOutAnalytics => {
            "If you'd like to opt out of analytics, you can adjust your "
        },
        OnboardingMessage::PrivacySettings => "Privacy Settings",
        OnboardingMessage::ByContinuingAgreeTo => "By continuing, you agree to ZYH's ",
        OnboardingMessage::TermsOfService => "Terms of Service",

        OnboardingMessage::CustomizeYourWarp => "Customize your ZYH",
        OnboardingMessage::CustomizeSubtitle => {
            "Tailor your features and UI to your working style."
        },
        OnboardingMessage::TabStyling => "Tab styling",
        OnboardingMessage::Vertical => "Vertical",
        OnboardingMessage::Horizontal => "Horizontal",
        OnboardingMessage::ToolsPanel => "Tools panel",
        OnboardingMessage::Enabled => "Enabled",
        OnboardingMessage::Disabled => "Disabled",
        OnboardingMessage::FileExplorer => "File explorer",
        OnboardingMessage::ConversationHistory => "Conversation history",
        OnboardingMessage::GlobalFileSearch => "Global file search",
        OnboardingMessage::WarpDrive => "ZYH Drive",
        OnboardingMessage::CodeReview => "Code review",

        OnboardingMessage::CustomizeYourWarpAgent => "Customize your ZYH Agent",
        OnboardingMessage::AgentSlideSubtitle => "Select your ZYH Agent's defaults.",
        OnboardingMessage::DefaultModel => "Default model",
        OnboardingMessage::Recommended => "Recommended",
        OnboardingMessage::Autonomy => "Autonomy",
        OnboardingMessage::Full => "Full",
        OnboardingMessage::Partial => "Partial",
        OnboardingMessage::NoneAutonomy => "None",
        OnboardingMessage::FullAutonomyDescription => {
            "ZYH Agent runs commands, writes code, and reads files without asking."
        },
        OnboardingMessage::PartialAutonomyDescription => {
            "ZYH Agent can plan, read files, and execute low-risk commands. Asks before making any \
             changes or executing sensitive commands."
        },
        OnboardingMessage::NoneAutonomyDescription => {
            "ZYH Agent takes no actions without your approval."
        },
        OnboardingMessage::SetByTeamWorkspace => "Set by Team Workspace",
        OnboardingMessage::AutonomyTeamWorkspaceDescription => {
            "Autonomy settings are configured as part of your team workspace."
        },

        OnboardingMessage::CustomizeThirdPartyAgents => "Customize third party agents",
        OnboardingMessage::ThirdPartySubtitle => {
            "Select defaults for using agents like Claude Code, Codex, and Gemini."
        },
        OnboardingMessage::CliAgentToolbar => "CLI agent toolbar",
        OnboardingMessage::Notifications => "Notifications",

        OnboardingMessage::Back => "Back",
        OnboardingMessage::Next => "Next",

        OnboardingMessage::AiFeature0 => {
            "Use frontier and open-weight models with ZYH Agent"
        },
        OnboardingMessage::AiFeature1 => "Hand off agent work to cloud agents",
        OnboardingMessage::AiFeature2 => "Automatically diagnose and fix terminal errors",
        OnboardingMessage::AiFeature3 => "Agentic control of long-running commands and TUIs",
        OnboardingMessage::AiFeature4 => "Review code diffs and send comments directly to agents",
        OnboardingMessage::AiFeature5 => {
            "Remote control for Claude Code, Codex, and other agents"
        },
        OnboardingMessage::WarpAgentFeature0 => {
            "Best harness for terminal tasks and agentic coding"
        },
        OnboardingMessage::WarpAgentFeature1 => {
            "Frontier models from OpenAI, Anthropic, and Google"
        },
        OnboardingMessage::WarpAgentFeature2 => {
            "Model routing across frontier and open-weight models"
        },
        OnboardingMessage::WarpAgentFeature3 => "Multi-agent orchestration",
        OnboardingMessage::WarpDriveFeature0 => "ZYH Drive",
        OnboardingMessage::WarpDriveFeature1 => "Session Sharing",

        OnboardingMessage::AreYouSureNoAi => "Are you sure you don't want AI?",
        OnboardingMessage::NoAiWarningBody => {
            "Without AI, you'll still get a modern, high-performance terminal with ZYH Drive, \
             Session Sharing, and other great features."
        },
        OnboardingMessage::GiveMeAiFeatures => "Give me AI features",
        OnboardingMessage::IDontWantAi => "I don't want AI",
        OnboardingMessage::PlanSuccessfullyActivated => "Plan successfully activated!",

        OnboardingMessage::MeetWarpInput => "Meet the ZYH input",
        OnboardingMessage::UniversalInputDescription => {
            "Your terminal input accepts both terminal commands and agent prompts and automatically \
             detects which you're using. Use {} to lock the input to Agent mode (natural language) \
             or Terminal mode (commands)."
        },
        OnboardingMessage::TalkToAgent => "Talk to the agent",
        OnboardingMessage::TalkToAgentDescription => {
            "You can type in natural language to engage the agent. Submit the query below to start: \
             What tests exist in this repo, how are they structured, and what do they cover?"
        },
        OnboardingMessage::Submit => "Submit",
        OnboardingMessage::Finish => "Finish",
        OnboardingMessage::WelcomeTerminalMode => "Welcome to terminal mode",
        OnboardingMessage::YoureInTerminalMode => "You’re in terminal mode",
        OnboardingMessage::TerminalModeDescription => {
            "Run commands here, just like a regular terminal. If you type a question or task using \
             natural language, ZYH can suggest opening it in agent mode. You can always override \
             using {}."
        },
        OnboardingMessage::EnableNaturalLanguageDetection => "Enable Natural Language Detection",
        OnboardingMessage::YoureInAgentMode => "You're in agent mode",
        OnboardingMessage::AgentModeProjectDescription => {
            "Agent mode gives your questions and tasks their own conversation, so you can ask \
             follow-ups without leaving your terminal workflow.\n\nSubmit the query below to have \
             the agent initialize this project, or ⊗ to clear the input and start your own!"
        },
        OnboardingMessage::SkipInitialization => "Skip initialization",
        OnboardingMessage::Initialize => "Initialize",
        OnboardingMessage::AgentModeDescription => {
            "Agent mode gives your questions and tasks their own conversation, so you can ask \
             follow-ups without leaving your terminal workflow. Press {} to return to terminal mode \
             at any point."
        },
        OnboardingMessage::BackToTerminal => "Back to terminal",
    }
}

fn zh_cn_text(message: OnboardingMessage) -> &'static str {
    match message {
        OnboardingMessage::WelcomeToWarp => "欢迎使用 ZYH",
        OnboardingMessage::ModernTerminalDescription => {
            "内置最先进 AI Agent 的现代终端。"
        },
        OnboardingMessage::AlreadyHaveAccount => "已有账户？",
        OnboardingMessage::LogIn => "登录",
        OnboardingMessage::GetStarted => "开始使用",

        OnboardingMessage::HowDoYouWantToWork => "你想如何使用？",
        OnboardingMessage::BuildFasterWithAgents => "使用 Agent 加速开发",
        OnboardingMessage::AgentCardDescription => "获取 AI 功能以加速终端和 Agent 驱动的工作流：",
        OnboardingMessage::JustUseTheTerminal => "仅使用终端",
        OnboardingMessage::NoAiFeatures => "无 AI 功能",
        OnboardingMessage::TerminalCardDescription => "为速度、上下文和控制而优化的现代终端，不含 AI。",

        OnboardingMessage::ChooseYourAiSetup => "选择你的 AI 设置",
        OnboardingMessage::ChooseYourAiSetupDescription => {
            "选择使用 ZYH Agent 还是第三方 Agent。"
        },
        OnboardingMessage::UseWarpAgent => "使用 ZYH Agent",
        OnboardingMessage::AccessMoreModels => "访问更多模型",
        OnboardingMessage::WarpAgentDescription => "深度集成到终端中的最先进 Agent 框架。",
        OnboardingMessage::UseThirdPartyAgents => "使用第三方 Agent",
        OnboardingMessage::ThirdPartyDescription => "使用 Claude Code、Codex、Gemini 等 Agent。",

        OnboardingMessage::GetAiAccess => "获取 AI 访问权限",
        OnboardingMessage::ConfigureAi => "配置 AI",
        OnboardingMessage::AiAccessSubtitleLoggedIn => {
            "选择订阅计划以节省费用，或先体验 ZYH 的 AI 功能再决定。"
        },
        OnboardingMessage::AiAccessSubtitleAnonymous => {
            "准备就绪后，可在设置中添加兼容 OpenAI 的推理服务商。"
        },
        OnboardingMessage::Subscription => "订阅",
        OnboardingMessage::BestValue => "最超值",
        OnboardingMessage::SubscriptionPricing => {
            "每月 $18 起，支持月付或年付。包含基础额度、前沿模型、云端 Agent、协作等功能。"
        },
        OnboardingMessage::SetUpLater => "稍后设置",
        OnboardingMessage::SetUpLaterDescription => {
            "在决定订阅之前，先体验 ZYH 内置的 AI 功能，或使用你自己的推理服务。"
        },
        OnboardingMessage::IfBrowserHasntLaunched => "如果浏览器没有自动打开，",
        OnboardingMessage::CopyTheUrl => "复制 URL",
        OnboardingMessage::AndOpenThePageManually => " 并手动打开页面。",
        OnboardingMessage::ClickHere => "点击此处",
        OnboardingMessage::ToPasteYourToken => " 粘贴来自浏览器的令牌。",

        OnboardingMessage::OpenAProject => "打开项目",
        OnboardingMessage::ProjectSubtitle => "设置项目以优化 ZYH 中的编码体验。",
        OnboardingMessage::OpenLocalFolder => "打开本地文件夹",
        OnboardingMessage::Skip => "跳过",
        OnboardingMessage::InitializeProjectAutomatically => "自动初始化项目",
        OnboardingMessage::InitializeProjectDescription => {
            "准备项目环境、构建代码索引并生成项目规则\u{2014}\u{2014}让 Agent 获得更深入的理解和更出色的性能。"
        },
        OnboardingMessage::GetWarping => "立即开始",

        OnboardingMessage::ChooseATheme => "选择主题",
        OnboardingMessage::ThemeSubtitle => "点击或使用方向键选择，按 Enter 确认。",
        OnboardingMessage::SyncWithOs => "与系统同步浅色/深色主题",
        OnboardingMessage::OptOutAnalytics => "如果你想退出数据分析，可以调整你的",
        OnboardingMessage::PrivacySettings => "隐私设置",
        OnboardingMessage::ByContinuingAgreeTo => "继续即表示你同意 ZYH 的",
        OnboardingMessage::TermsOfService => "服务条款",

        OnboardingMessage::CustomizeYourWarp => "自定义你的 ZYH",
        OnboardingMessage::CustomizeSubtitle => "根据你的工作风格定制功能和界面。",
        OnboardingMessage::TabStyling => "标签页样式",
        OnboardingMessage::Vertical => "垂直",
        OnboardingMessage::Horizontal => "水平",
        OnboardingMessage::ToolsPanel => "工具面板",
        OnboardingMessage::Enabled => "启用",
        OnboardingMessage::Disabled => "禁用",
        OnboardingMessage::FileExplorer => "文件浏览器",
        OnboardingMessage::ConversationHistory => "对话历史",
        OnboardingMessage::GlobalFileSearch => "全局文件搜索",
        OnboardingMessage::WarpDrive => "ZYH Drive",
        OnboardingMessage::CodeReview => "代码审查",

        OnboardingMessage::CustomizeYourWarpAgent => "自定义你的 ZYH Agent",
        OnboardingMessage::AgentSlideSubtitle => "选择 ZYH Agent 的默认设置。",
        OnboardingMessage::DefaultModel => "默认模型",
        OnboardingMessage::Recommended => "推荐",
        OnboardingMessage::Autonomy => "自主程度",
        OnboardingMessage::Full => "完全自主",
        OnboardingMessage::Partial => "部分自主",
        OnboardingMessage::NoneAutonomy => "无自主",
        OnboardingMessage::FullAutonomyDescription => {
            "ZYH Agent 无需询问即可运行命令、编写代码和读取文件。"
        },
        OnboardingMessage::PartialAutonomyDescription => {
            "ZYH Agent 可以规划、读取文件并执行低风险命令。在进行任何更改或执行敏感命令前会请求确认。"
        },
        OnboardingMessage::NoneAutonomyDescription => {
            "未经你的批准，ZYH Agent 不会执行任何操作。"
        },
        OnboardingMessage::SetByTeamWorkspace => "由团队工作区设置",
        OnboardingMessage::AutonomyTeamWorkspaceDescription => {
            "自主程度设置已在团队工作区中配置。"
        },

        OnboardingMessage::CustomizeThirdPartyAgents => "自定义第三方 Agent",
        OnboardingMessage::ThirdPartySubtitle => "为 Claude Code、Codex、Gemini 等 Agent 设置默认选项。",
        OnboardingMessage::CliAgentToolbar => "CLI Agent 工具栏",
        OnboardingMessage::Notifications => "通知",

        OnboardingMessage::Back => "返回",
        OnboardingMessage::Next => "下一步",

        OnboardingMessage::AiFeature0 => "使用 ZYH Agent 驱动前沿模型和开源模型",
        OnboardingMessage::AiFeature1 => "将 Agent 工作交给云端 Agent 处理",
        OnboardingMessage::AiFeature2 => "自动诊断和修复终端错误",
        OnboardingMessage::AiFeature3 => "对长时间运行的命令和 TUI 进行 Agent 控制",
        OnboardingMessage::AiFeature4 => "审查代码差异并直接向 Agent 发送评论",
        OnboardingMessage::AiFeature5 => "远程控制 Claude Code、Codex 等 Agent",
        OnboardingMessage::WarpAgentFeature0 => "终端任务和 Agent 编码的最佳框架",
        OnboardingMessage::WarpAgentFeature1 => "来自 OpenAI、Anthropic 和 Google 的前沿模型",
        OnboardingMessage::WarpAgentFeature2 => "跨前沿模型和开源模型的路由",
        OnboardingMessage::WarpAgentFeature3 => "多 Agent 编排",
        OnboardingMessage::WarpDriveFeature0 => "ZYH Drive",
        OnboardingMessage::WarpDriveFeature1 => "会话共享",

        OnboardingMessage::AreYouSureNoAi => "确定不需要 AI 吗？",
        OnboardingMessage::NoAiWarningBody => {
            "即便没有 AI，你仍然可以获得一个现代、高性能的终端，支持 ZYH Drive、会话共享等出色功能。"
        },
        OnboardingMessage::GiveMeAiFeatures => "我要 AI 功能",
        OnboardingMessage::IDontWantAi => "我不需要 AI",
        OnboardingMessage::PlanSuccessfullyActivated => "计划已成功激活！",

        OnboardingMessage::MeetWarpInput => "认识 ZYH 输入框",
        OnboardingMessage::UniversalInputDescription => {
            "终端输入框既能输入终端命令，也能输入 Agent 提示词，并会自动识别你正在使用哪一种。使用 {} 可将输入锁定为 \
             Agent 模式（自然语言）或终端模式（命令）。"
        },
        OnboardingMessage::TalkToAgent => "和 Agent 对话",
        OnboardingMessage::TalkToAgentDescription => {
            "你可以用自然语言与 Agent 交互。提交下方问题开始：这个仓库里有哪些测试，它们如何组织，覆盖了什么？"
        },
        OnboardingMessage::Submit => "提交",
        OnboardingMessage::Finish => "完成",
        OnboardingMessage::WelcomeTerminalMode => "欢迎使用终端模式",
        OnboardingMessage::YoureInTerminalMode => "你正在使用终端模式",
        OnboardingMessage::TerminalModeDescription => {
            "在这里运行命令，就像使用普通终端一样。如果你输入自然语言问题或任务，ZYH 可以建议在 Agent 模式中打开。\
             你随时可以使用 {} 手动切换。"
        },
        OnboardingMessage::EnableNaturalLanguageDetection => "启用自然语言检测",
        OnboardingMessage::YoureInAgentMode => "你正在使用 Agent 模式",
        OnboardingMessage::AgentModeProjectDescription => {
            "Agent 模式会为你的问题和任务创建独立对话，因此你可以继续追问而不离开终端工作流。\n\n提交下方查询，\
             让 Agent 初始化此项目；也可以点击 ⊗ 清空输入并开始你自己的任务。"
        },
        OnboardingMessage::SkipInitialization => "跳过初始化",
        OnboardingMessage::Initialize => "初始化",
        OnboardingMessage::AgentModeDescription => {
            "Agent 模式会为你的问题和任务创建独立对话，因此你可以继续追问而不离开终端工作流。按 {} 可随时返回终端模式。"
        },
        OnboardingMessage::BackToTerminal => "返回终端",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_onboarding_messages_have_non_empty_text() {
        for message in ALL_MESSAGES {
            assert!(
                !en_text(*message).is_empty(),
                "missing English text for {message:?}"
            );
            assert!(
                !zh_cn_text(*message).is_empty(),
                "missing Chinese text for {message:?}"
            );
        }
    }

    const ALL_MESSAGES: &[OnboardingMessage] = &[
        OnboardingMessage::WelcomeToWarp,
        OnboardingMessage::ModernTerminalDescription,
        OnboardingMessage::AlreadyHaveAccount,
        OnboardingMessage::LogIn,
        OnboardingMessage::GetStarted,
        OnboardingMessage::HowDoYouWantToWork,
        OnboardingMessage::BuildFasterWithAgents,
        OnboardingMessage::AgentCardDescription,
        OnboardingMessage::JustUseTheTerminal,
        OnboardingMessage::NoAiFeatures,
        OnboardingMessage::TerminalCardDescription,
        OnboardingMessage::ChooseYourAiSetup,
        OnboardingMessage::ChooseYourAiSetupDescription,
        OnboardingMessage::UseWarpAgent,
        OnboardingMessage::AccessMoreModels,
        OnboardingMessage::WarpAgentDescription,
        OnboardingMessage::UseThirdPartyAgents,
        OnboardingMessage::ThirdPartyDescription,
        OnboardingMessage::GetAiAccess,
        OnboardingMessage::ConfigureAi,
        OnboardingMessage::AiAccessSubtitleLoggedIn,
        OnboardingMessage::AiAccessSubtitleAnonymous,
        OnboardingMessage::Subscription,
        OnboardingMessage::BestValue,
        OnboardingMessage::SubscriptionPricing,
        OnboardingMessage::SetUpLater,
        OnboardingMessage::SetUpLaterDescription,
        OnboardingMessage::IfBrowserHasntLaunched,
        OnboardingMessage::CopyTheUrl,
        OnboardingMessage::AndOpenThePageManually,
        OnboardingMessage::ClickHere,
        OnboardingMessage::ToPasteYourToken,
        OnboardingMessage::OpenAProject,
        OnboardingMessage::ProjectSubtitle,
        OnboardingMessage::OpenLocalFolder,
        OnboardingMessage::Skip,
        OnboardingMessage::InitializeProjectAutomatically,
        OnboardingMessage::InitializeProjectDescription,
        OnboardingMessage::GetWarping,
        OnboardingMessage::ChooseATheme,
        OnboardingMessage::ThemeSubtitle,
        OnboardingMessage::SyncWithOs,
        OnboardingMessage::OptOutAnalytics,
        OnboardingMessage::PrivacySettings,
        OnboardingMessage::ByContinuingAgreeTo,
        OnboardingMessage::TermsOfService,
        OnboardingMessage::CustomizeYourWarp,
        OnboardingMessage::CustomizeSubtitle,
        OnboardingMessage::TabStyling,
        OnboardingMessage::Vertical,
        OnboardingMessage::Horizontal,
        OnboardingMessage::ToolsPanel,
        OnboardingMessage::Enabled,
        OnboardingMessage::Disabled,
        OnboardingMessage::FileExplorer,
        OnboardingMessage::ConversationHistory,
        OnboardingMessage::GlobalFileSearch,
        OnboardingMessage::WarpDrive,
        OnboardingMessage::CodeReview,
        OnboardingMessage::CustomizeYourWarpAgent,
        OnboardingMessage::AgentSlideSubtitle,
        OnboardingMessage::DefaultModel,
        OnboardingMessage::Recommended,
        OnboardingMessage::Autonomy,
        OnboardingMessage::Full,
        OnboardingMessage::Partial,
        OnboardingMessage::NoneAutonomy,
        OnboardingMessage::FullAutonomyDescription,
        OnboardingMessage::PartialAutonomyDescription,
        OnboardingMessage::NoneAutonomyDescription,
        OnboardingMessage::SetByTeamWorkspace,
        OnboardingMessage::AutonomyTeamWorkspaceDescription,
        OnboardingMessage::CustomizeThirdPartyAgents,
        OnboardingMessage::ThirdPartySubtitle,
        OnboardingMessage::CliAgentToolbar,
        OnboardingMessage::Notifications,
        OnboardingMessage::Back,
        OnboardingMessage::Next,
        OnboardingMessage::AiFeature0,
        OnboardingMessage::AiFeature1,
        OnboardingMessage::AiFeature2,
        OnboardingMessage::AiFeature3,
        OnboardingMessage::AiFeature4,
        OnboardingMessage::AiFeature5,
        OnboardingMessage::WarpAgentFeature0,
        OnboardingMessage::WarpAgentFeature1,
        OnboardingMessage::WarpAgentFeature2,
        OnboardingMessage::WarpAgentFeature3,
        OnboardingMessage::WarpDriveFeature0,
        OnboardingMessage::WarpDriveFeature1,
        OnboardingMessage::AreYouSureNoAi,
        OnboardingMessage::NoAiWarningBody,
        OnboardingMessage::GiveMeAiFeatures,
        OnboardingMessage::IDontWantAi,
        OnboardingMessage::PlanSuccessfullyActivated,
        OnboardingMessage::MeetWarpInput,
        OnboardingMessage::UniversalInputDescription,
        OnboardingMessage::TalkToAgent,
        OnboardingMessage::TalkToAgentDescription,
        OnboardingMessage::Submit,
        OnboardingMessage::Finish,
        OnboardingMessage::WelcomeTerminalMode,
        OnboardingMessage::YoureInTerminalMode,
        OnboardingMessage::TerminalModeDescription,
        OnboardingMessage::EnableNaturalLanguageDetection,
        OnboardingMessage::YoureInAgentMode,
        OnboardingMessage::AgentModeProjectDescription,
        OnboardingMessage::SkipInitialization,
        OnboardingMessage::Initialize,
        OnboardingMessage::AgentModeDescription,
        OnboardingMessage::BackToTerminal,
    ];
}
