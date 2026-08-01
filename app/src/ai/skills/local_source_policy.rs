//! Skills and Plugins are filesystem-only in the permanent ZYH product.
//!
//! Discovery uses global and project directories with documented provider
//! precedence. Users may install content through explicit Git or file
//! operations outside the app. The application never contacts a marketplace,
//! restores a remote catalog, or downloads or updates extensions in the
//! background. Agent Runs receive only explicitly selected local resources and
//! retain exact Resource Snapshots. Missing or malformed local resources fail
//! without a network fallback.

/// Product flag: only on-disk Skills and Plugins are supported sources.
pub const LOCAL_SKILLS_AND_PLUGINS_ONLY: bool = true;

/// Documented Skill provider precedence (highest first), matching
/// [`ai::skills::SKILL_PROVIDER_DEFINITIONS`]:
/// `.agents/skills` → `.zyh/skills` → `.claude/skills` → `.codex/skills` →
/// `.cursor/skills` → `.gemini/skills` → `.copilot/skills` → `.factory/skills` →
/// `.github/skills` → `.opencode/skills`.
pub const SKILL_PROVIDER_PRECEDENCE: &[&str] = &[
    ".agents/skills",
    ".zyh/skills",
    ".claude/skills",
    ".codex/skills",
    ".cursor/skills",
    ".gemini/skills",
    ".copilot/skills",
    ".factory/skills",
    ".github/skills",
    ".opencode/skills",
];

/// Relative plugins directory under the ZYH application home.
pub const ZYH_PLUGINS_DIR_NAME: &str = "plugins";

/// Guidance when marketplace or cloud catalog access is refused.
pub const MARKETPLACE_REMOVED_GUIDANCE: &str =
    "Skill and Plugin marketplaces are no longer available. \
Install Skills and Plugins locally under your ZYH home or project directories \
(for example ~/.zyh/skills or .agents/skills) using explicit Git or file operations.";

/// Guidance when a local Skill or Plugin is missing or unreadable.
pub const LOCAL_RESOURCE_MISSING_GUIDANCE: &str = "The requested local Skill or Plugin could not be loaded. \
Check that the file exists, is readable, and is not malformed. ZYH will not download a replacement.";

/// Whether the app may contact a marketplace or remote extension catalog.
pub fn may_use_marketplace_or_remote_catalog() -> bool {
    !LOCAL_SKILLS_AND_PLUGINS_ONLY
}

/// Whether the app may restore Skills from a hosted/cloud catalog.
pub fn may_restore_remote_skill_catalog() -> bool {
    !LOCAL_SKILLS_AND_PLUGINS_ONLY
}

/// Whether the app may clone skill repositories or otherwise fetch Skills over the network.
pub fn may_network_fetch_skills() -> bool {
    !LOCAL_SKILLS_AND_PLUGINS_ONLY
}

/// Whether the app may auto-install or auto-update Plugins in the background.
pub fn may_background_install_or_update_plugins() -> bool {
    !LOCAL_SKILLS_AND_PLUGINS_ONLY
}

/// Whether Agent resource selection may include only local filesystem content.
pub fn requires_local_resource_snapshots_only() -> bool {
    LOCAL_SKILLS_AND_PLUGINS_ONLY
}

/// Directory under the ZYH home where locally installed Plugins load from.
pub fn local_plugins_dir() -> Option<std::path::PathBuf> {
    warp_core::paths::warp_home_config_dir().map(|home| home.join(ZYH_PLUGINS_DIR_NAME))
}

/// Fail-closed result when a local resource is missing or malformed.
pub fn local_resource_unavailable_message() -> String {
    LOCAL_RESOURCE_MISSING_GUIDANCE.to_string()
}

/// Fail-closed result when marketplace access is requested.
pub fn marketplace_unavailable_message() -> String {
    MARKETPLACE_REMOVED_GUIDANCE.to_string()
}

#[cfg(test)]
#[path = "local_source_policy_tests.rs"]
mod tests;
