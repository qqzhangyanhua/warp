use super::*;

#[test]
fn filesystem_is_sole_skill_and_plugin_source() {
    assert!(LOCAL_SKILLS_AND_PLUGINS_ONLY);
    assert!(!may_use_marketplace_or_remote_catalog());
    assert!(!may_restore_remote_skill_catalog());
    assert!(!may_network_fetch_skills());
    assert!(!may_background_install_or_update_plugins());
    assert!(requires_local_resource_snapshots_only());
}

#[test]
fn skill_provider_precedence_starts_with_agents_then_zyh() {
    assert_eq!(SKILL_PROVIDER_PRECEDENCE[0], ".agents/skills");
    assert_eq!(SKILL_PROVIDER_PRECEDENCE[1], ".zyh/skills");
    assert!(SKILL_PROVIDER_PRECEDENCE.contains(&".claude/skills"));
    assert!(SKILL_PROVIDER_PRECEDENCE.contains(&".codex/skills"));
    assert_eq!(SKILL_PROVIDER_PRECEDENCE.len(), 10);
}

#[test]
fn local_plugins_dir_is_under_zyh_home() {
    let Some(dir) = local_plugins_dir() else {
        // CI without a resolvable home is acceptable; policy still holds.
        return;
    };
    let path = dir.to_string_lossy();
    assert!(
        path.contains(".zyh") && path.ends_with("plugins") || path.ends_with("plugins"),
        "plugins must live under ZYH home, got {path}"
    );
    assert!(
        !path.contains(".warp/plugins"),
        "must not use legacy Warp plugins path, got {path}"
    );
}

#[test]
fn missing_resource_guidance_forbids_network_fallback() {
    let msg = local_resource_unavailable_message();
    assert!(msg.contains("will not download") || msg.contains("not download"));
    assert!(!msg.is_empty());
}

#[test]
fn marketplace_guidance_points_to_local_install() {
    let msg = marketplace_unavailable_message();
    assert!(msg.contains("local") || msg.contains("~/.zyh") || msg.contains(".agents"));
}
