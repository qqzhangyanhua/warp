use super::Locale;
use crate::i18n::Message;

pub(super) fn text(message: Message, locale: Locale) -> &'static str {
    match locale {
        Locale::En => en_text(message),
        Locale::ZhCn => zh_cn_text(message),
    }
}

fn en_text(message: Message) -> &'static str {
    match message {
        Message::SettingsSectionAbout => "About",
        Message::SettingsSectionAccount => "Account",
        Message::SettingsSectionAgents => "Agents",
        Message::SettingsSectionMcpServers => "MCP Servers",
        Message::SettingsSectionBillingAndUsage => "Billing and usage",
        Message::SettingsSectionAppearance => "Appearance",
        Message::SettingsSectionFeatures => "Features",
        Message::SettingsSectionKeybindings => "Keyboard shortcuts",
        Message::SettingsSectionPrivacy => "Privacy",
        Message::SettingsSectionReferrals => "Referrals",
        Message::SettingsSectionScripting => "Scripting",
        Message::SettingsSectionSharedBlocks => "Shared blocks",
        Message::SettingsSectionTeams => "Teams",
        Message::SettingsSectionWarpDrive => "Warp Drive",
        Message::SettingsSectionWarpify => "Warpify",
        Message::SettingsSectionWarpAgent => "Warp Agent",
        Message::SettingsSectionAgentProfiles => "Profiles",
        Message::SettingsSectionAgentMcpServers => "MCP servers",
        Message::SettingsSectionKnowledge => "Knowledge",
        Message::SettingsSectionThirdPartyCliAgents => "Third party CLI agents",
        Message::SettingsSectionCode => "Code",
        Message::SettingsSectionCodeIndexing => "Indexing and projects",
        Message::SettingsSectionEditorAndCodeReview => "Editor and Code Review",
        Message::SettingsSectionCloudPlatform => "Cloud platform",
        Message::SettingsSectionCloudEnvironments => "Environments",
        Message::SettingsSectionOzCloudApiKeys => "Oz Cloud API Keys",

        Message::SettingsGeneralCategory => "General",
        Message::SettingsThemesCategory => "Themes",
        Message::SettingsIconCategory => "Icon",
        Message::SettingsWindowCategory => "Window",
        Message::SettingsToolsPanelCategory => "Tools panel",
        Message::SettingsInputCategory => "Input",
        Message::SettingsPanesCategory => "Panes",
        Message::SettingsBlocksCategory => "Blocks",
        Message::SettingsTextCategory => "Text",
        Message::SettingsCursorCategory => "Cursor",
        Message::SettingsTabsCategory => "Tabs",
        Message::SettingsFullScreenAppsCategory => "Full-screen Apps",

        Message::SettingsLanguageLabel => "Language",
        Message::SettingsLanguageDescription => "Choose the display language for Warp.",
        Message::SettingsLanguageSystemOption => "System",
        Message::SettingsLanguageEnglishOption => "English",
        Message::SettingsLanguageSimplifiedChineseOption => "Simplified Chinese",
        Message::SettingsLanguageSystemDisplaysEnglish => "System currently displays in English.",

        Message::AppearanceCreateCustomTheme => "Create your own custom theme",
        Message::AppearanceThemeLight => "Light",
        Message::AppearanceThemeDark => "Dark",
        Message::AppearanceCurrentTheme => "Current theme",
        Message::AppearanceSyncWithOs => "Sync with OS",
        Message::AppearanceSyncWithOsDescription => {
            "Automatically switch between light and dark themes when your system does."
        },
        Message::AppearanceCustomizeAppIcon => "Customize your app icon",
        Message::AppearanceAppIconBundleRequired => "Changing the app icon requires the app to be bundled.",
        Message::AppearanceShowWarpInDock => "Show Warp in Dock",
        Message::AppearanceRestartForAppIcon => {
            "You may need to restart Warp for MacOS to apply the preferred icon style."
        },
        Message::AppearanceAppIconDefault => "Default",
        Message::AppearanceOpenNewWindowsWithCustomSize => "Open new windows with custom size",
        Message::AppearanceCustomWindowColumns => "Columns",
        Message::AppearanceCustomWindowRows => "Rows",
        Message::AppearanceWindowOpacityLabel => "Window Opacity",
        Message::AppearanceWindowOpacityUnsupported => "Transparency is not supported with your graphics drivers.",
        Message::AppearanceWindowOpacityTransparencyWarning => {
            "The selected graphics settings may not support rendering transparent windows."
        },
        Message::AppearanceWindowOpacityGraphicsSettingsSuggestion => {
            " Try changing the settings for the graphics backend or integrated GPU in Features > System."
        },
        Message::AppearanceWindowBlurRadiusLabel => "Window Blur Radius",
        Message::AppearanceWindowBlurTexture => "Use Window Blur (Acrylic texture)",
        Message::AppearanceWindowBlurTextureUnsupported => {
            "The selected hardware may not support rendering transparent windows."
        }
    }
}

fn zh_cn_text(message: Message) -> &'static str {
    match message {
        Message::SettingsSectionAbout => "关于",
        Message::SettingsSectionAccount => "账户",
        Message::SettingsSectionAgents => "Agents",
        Message::SettingsSectionMcpServers => "MCP Servers",
        Message::SettingsSectionBillingAndUsage => "账单与用量",
        Message::SettingsSectionAppearance => "外观",
        Message::SettingsSectionFeatures => "功能",
        Message::SettingsSectionKeybindings => "键盘快捷键",
        Message::SettingsSectionPrivacy => "隐私",
        Message::SettingsSectionReferrals => "推荐奖励",
        Message::SettingsSectionScripting => "脚本",
        Message::SettingsSectionSharedBlocks => "共享块",
        Message::SettingsSectionTeams => "团队",
        Message::SettingsSectionWarpDrive => "Warp Drive",
        Message::SettingsSectionWarpify => "Warpify",
        Message::SettingsSectionWarpAgent => "Warp Agent",
        Message::SettingsSectionAgentProfiles => "配置档案",
        Message::SettingsSectionAgentMcpServers => "MCP servers",
        Message::SettingsSectionKnowledge => "知识库",
        Message::SettingsSectionThirdPartyCliAgents => "第三方 CLI agents",
        Message::SettingsSectionCode => "Code",
        Message::SettingsSectionCodeIndexing => "索引与项目",
        Message::SettingsSectionEditorAndCodeReview => "编辑器与代码审查",
        Message::SettingsSectionCloudPlatform => "云平台",
        Message::SettingsSectionCloudEnvironments => "环境",
        Message::SettingsSectionOzCloudApiKeys => "Oz Cloud API Keys",

        Message::SettingsGeneralCategory => "通用",
        Message::SettingsThemesCategory => "主题",
        Message::SettingsIconCategory => "图标",
        Message::SettingsWindowCategory => "窗口",
        Message::SettingsToolsPanelCategory => "工具面板",
        Message::SettingsInputCategory => "输入",
        Message::SettingsPanesCategory => "窗格",
        Message::SettingsBlocksCategory => "块",
        Message::SettingsTextCategory => "文本",
        Message::SettingsCursorCategory => "光标",
        Message::SettingsTabsCategory => "标签页",
        Message::SettingsFullScreenAppsCategory => "全屏 App",

        Message::SettingsLanguageLabel => "语言",
        Message::SettingsLanguageDescription => "选择 Warp 的界面显示语言。",
        Message::SettingsLanguageSystemOption => "跟随系统",
        Message::SettingsLanguageEnglishOption => "English",
        Message::SettingsLanguageSimplifiedChineseOption => "简体中文",
        Message::SettingsLanguageSystemDisplaysEnglish => "当前跟随系统会按 English 显示。",

        Message::AppearanceCreateCustomTheme => "创建自定义主题",
        Message::AppearanceThemeLight => "浅色",
        Message::AppearanceThemeDark => "深色",
        Message::AppearanceCurrentTheme => "当前主题",
        Message::AppearanceSyncWithOs => "跟随 OS",
        Message::AppearanceSyncWithOsDescription => "当系统切换浅色或深色模式时，自动切换主题。",
        Message::AppearanceCustomizeAppIcon => "自定义 App 图标",
        Message::AppearanceAppIconBundleRequired => "更改 App 图标需要使用已打包的 App。",
        Message::AppearanceShowWarpInDock => "在 Dock 中显示 Warp",
        Message::AppearanceRestartForAppIcon => "你可能需要重启 Warp，macOS 才会应用首选图标样式。",
        Message::AppearanceAppIconDefault => "默认",
        Message::AppearanceOpenNewWindowsWithCustomSize => "新窗口使用自定义尺寸打开",
        Message::AppearanceCustomWindowColumns => "列数",
        Message::AppearanceCustomWindowRows => "行数",
        Message::AppearanceWindowOpacityLabel => "窗口不透明度",
        Message::AppearanceWindowOpacityUnsupported => "你的图形驱动不支持透明效果。",
        Message::AppearanceWindowOpacityTransparencyWarning => {
            "当前图形设置可能不支持渲染透明窗口。"
        }
        Message::AppearanceWindowOpacityGraphicsSettingsSuggestion => {
            " 请在功能 > 系统中尝试更改图形后端或集成 GPU 设置。"
        }
        Message::AppearanceWindowBlurRadiusLabel => "窗口模糊半径",
        Message::AppearanceWindowBlurTexture => "使用窗口模糊（亚克力纹理）",
        Message::AppearanceWindowBlurTextureUnsupported => "当前硬件可能不支持渲染透明窗口。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_messages_have_non_empty_text() {
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

    const ALL_MESSAGES: &[Message] = &[
        Message::SettingsSectionAbout,
        Message::SettingsSectionAccount,
        Message::SettingsSectionAgents,
        Message::SettingsSectionMcpServers,
        Message::SettingsSectionBillingAndUsage,
        Message::SettingsSectionAppearance,
        Message::SettingsSectionFeatures,
        Message::SettingsSectionKeybindings,
        Message::SettingsSectionPrivacy,
        Message::SettingsSectionReferrals,
        Message::SettingsSectionScripting,
        Message::SettingsSectionSharedBlocks,
        Message::SettingsSectionTeams,
        Message::SettingsSectionWarpDrive,
        Message::SettingsSectionWarpify,
        Message::SettingsSectionWarpAgent,
        Message::SettingsSectionAgentProfiles,
        Message::SettingsSectionAgentMcpServers,
        Message::SettingsSectionKnowledge,
        Message::SettingsSectionThirdPartyCliAgents,
        Message::SettingsSectionCode,
        Message::SettingsSectionCodeIndexing,
        Message::SettingsSectionEditorAndCodeReview,
        Message::SettingsSectionCloudPlatform,
        Message::SettingsSectionCloudEnvironments,
        Message::SettingsSectionOzCloudApiKeys,
        Message::SettingsGeneralCategory,
        Message::SettingsThemesCategory,
        Message::SettingsIconCategory,
        Message::SettingsWindowCategory,
        Message::SettingsToolsPanelCategory,
        Message::SettingsInputCategory,
        Message::SettingsPanesCategory,
        Message::SettingsBlocksCategory,
        Message::SettingsTextCategory,
        Message::SettingsCursorCategory,
        Message::SettingsTabsCategory,
        Message::SettingsFullScreenAppsCategory,
        Message::SettingsLanguageLabel,
        Message::SettingsLanguageDescription,
        Message::SettingsLanguageSystemOption,
        Message::SettingsLanguageEnglishOption,
        Message::SettingsLanguageSimplifiedChineseOption,
        Message::SettingsLanguageSystemDisplaysEnglish,
        Message::AppearanceCreateCustomTheme,
        Message::AppearanceThemeLight,
        Message::AppearanceThemeDark,
        Message::AppearanceCurrentTheme,
        Message::AppearanceSyncWithOs,
        Message::AppearanceSyncWithOsDescription,
        Message::AppearanceCustomizeAppIcon,
        Message::AppearanceAppIconBundleRequired,
        Message::AppearanceShowWarpInDock,
        Message::AppearanceRestartForAppIcon,
        Message::AppearanceAppIconDefault,
        Message::AppearanceOpenNewWindowsWithCustomSize,
        Message::AppearanceCustomWindowColumns,
        Message::AppearanceCustomWindowRows,
        Message::AppearanceWindowOpacityLabel,
        Message::AppearanceWindowOpacityUnsupported,
        Message::AppearanceWindowOpacityTransparencyWarning,
        Message::AppearanceWindowOpacityGraphicsSettingsSuggestion,
        Message::AppearanceWindowBlurRadiusLabel,
        Message::AppearanceWindowBlurTexture,
        Message::AppearanceWindowBlurTextureUnsupported,
    ];
}
