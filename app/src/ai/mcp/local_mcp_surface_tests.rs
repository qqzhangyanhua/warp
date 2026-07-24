use super::{local_mcp_surface, McpSettingsCardKind};

#[test]
fn local_surface_excludes_gallery_cloud_and_managed_sources() {
    let policy = local_mcp_surface();
    assert!(!policy.allows_gallery());
    assert!(!policy.allows_cloud_objects());
    assert!(!policy.allows_managed_resolution());
    assert!(!policy.managed_resolution_unavailable_message().is_empty());
}

#[test]
fn only_file_based_settings_cards_are_retained() {
    let policy = local_mcp_surface();
    assert!(policy.allows_settings_card(McpSettingsCardKind::FileBased));
    assert!(!policy.allows_settings_card(McpSettingsCardKind::Gallery));
    assert!(!policy.allows_settings_card(McpSettingsCardKind::CloudTemplate));
    assert!(!policy.allows_settings_card(McpSettingsCardKind::CloudInstallation));
}
