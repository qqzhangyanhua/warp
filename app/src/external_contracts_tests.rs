use super::*;
use warp_core::channel::Channel;

#[test]
fn production_names_are_zyh() {
    assert_eq!(PRODUCT_DISPLAY_NAME, "ZYH");
    assert_eq!(AGENT_CLI_COMMAND_STABLE, "zyh");
    assert_eq!(LOCAL_CONTROL_COMMAND_STABLE, "zyhctrl");
    assert_eq!(URI_SCHEME_STABLE, "zyh");
    assert_eq!(agent_cli_command_for_channel(Channel::Stable), "zyh");
    assert_eq!(
        local_control_command_for_channel(Channel::Stable),
        "zyhctrl"
    );
    assert_eq!(uri_scheme_for_channel(Channel::Stable), "zyh");
}

#[test]
fn channel_command_and_scheme_families_use_zyh_prefix() {
    for channel in [
        Channel::Stable,
        Channel::Dev,
        Channel::Preview,
        Channel::Local,
        Channel::Integration,
        Channel::Oss,
    ] {
        let scheme = uri_scheme_for_channel(channel);
        assert!(
            scheme.starts_with("zyh"),
            "scheme for {channel:?} must be zyh*, got {scheme}"
        );
        assert!(
            !is_forbidden_legacy_uri_scheme(scheme),
            "current scheme must not be listed as forbidden"
        );

        let cli = agent_cli_command_for_channel(channel);
        assert!(
            cli.starts_with("zyh"),
            "cli for {channel:?} must be zyh*, got {cli}"
        );
        assert!(!is_forbidden_legacy_command_alias(cli));

        let ctrl = local_control_command_for_channel(channel);
        assert!(
            ctrl.starts_with("zyhctrl"),
            "control cli for {channel:?} must be zyhctrl*, got {ctrl}"
        );
        assert!(!is_forbidden_legacy_command_alias(ctrl));
    }
}

#[test]
fn channel_state_exposes_zyh_external_contracts() {
    // Runtime channel is whatever this test binary was built as; values must
    // still be ZYH family and not legacy aliases.
    let scheme = ChannelState::url_scheme();
    assert!(scheme.starts_with("zyh"), "got {scheme}");
    assert!(!is_forbidden_legacy_uri_scheme(scheme));

    let cli = ChannelState::channel().cli_command_name();
    assert!(cli.starts_with("zyh") || cli == "zyh-oss", "got {cli}");

    let ctrl = ChannelState::channel().warpctrl_command_name();
    assert!(ctrl.starts_with("zyhctrl"), "got {ctrl}");
}

#[test]
fn rejects_legacy_uri_schemes() {
    for scheme in FORBIDDEN_LEGACY_URI_SCHEMES {
        assert!(is_forbidden_legacy_uri_scheme(scheme));
        assert!(is_forbidden_legacy_uri_scheme(&scheme.to_ascii_uppercase()));
        let msg = legacy_uri_scheme_rejected_message(scheme);
        assert!(msg.contains("no longer supported"));
        assert!(msg.contains("zyh://") || msg.contains("ZYH"));
    }
}

#[test]
fn rejects_legacy_command_aliases() {
    for name in ["warpctrl", "oz", "warp", "oz-local"] {
        assert!(is_forbidden_legacy_command_alias(name));
        let msg = legacy_command_alias_rejected_message(name);
        assert!(msg.contains("zyh") || msg.contains("zyhctrl"));
    }
}

#[test]
fn rejects_cloud_deep_link_hosts() {
    for host in FORBIDDEN_CLOUD_URI_HOSTS {
        assert!(is_forbidden_cloud_uri_host(host));
        assert!(cloud_uri_host_rejected_message(host).contains("not available"));
    }
    assert!(!is_forbidden_cloud_uri_host("settings"));
    assert!(!is_forbidden_cloud_uri_host("action"));
    assert!(!is_forbidden_cloud_uri_host("tab_config"));
}

#[test]
fn env_prefix_contract() {
    assert_eq!(ZYH_ENV_PREFIX, "ZYH_");
    assert!(FORBIDDEN_LEGACY_ENV_PREFIXES.contains(&"WARP_"));
}
