//! Externally visible ZYH product identity.
//!
//! Application names, installed CLI commands, URI schemes, local-control tooling,
//! and documentation use ZYH. Legacy Warp/Oz external aliases are rejected rather
//! than silently accepted. Internal Rust type names may retain `warp_*` lineage
//! when they cannot leak into user-visible or external contracts.

use warp_core::channel::{Channel, ChannelState};

/// Product display name for UI and documentation.
pub const PRODUCT_DISPLAY_NAME: &str = "ZYH";

/// Stable local-control / automation CLI name (production channel).
pub const LOCAL_CONTROL_COMMAND_STABLE: &str = "zyhctrl";

/// Stable installed agent/CLI command name (production channel).
pub const AGENT_CLI_COMMAND_STABLE: &str = "zyh";

/// Primary URI scheme for production desktop builds.
pub const URI_SCHEME_STABLE: &str = "zyh";

/// Legacy external URI schemes that must not be accepted.
pub const FORBIDDEN_LEGACY_URI_SCHEMES: &[&str] = &[
    "warp",
    "warplocal",
    "warppreview",
    "warpdev",
    "warpintegration",
    "warposs",
    "oz",
    "ozlocal",
];

/// Legacy external CLI / control command aliases that must not be advertised.
pub const FORBIDDEN_LEGACY_COMMAND_ALIASES: &[&str] = &[
    "warp",
    "warp-cli",
    "warpctrl",
    "warpctrl-dev",
    "warpctrl-preview",
    "warpctrl-local",
    "oz",
    "oz-dev",
    "oz-preview",
    "oz-local",
];

/// Legacy external environment variable prefixes that must not be accepted as
/// product configuration contracts (internal process bootstrap may still set
/// transitional names until shell scripts are migrated).
pub const FORBIDDEN_LEGACY_ENV_PREFIXES: &[&str] = &["WARP_", "OZ_"];

/// Supported ZYH environment variable prefix for user-facing configuration.
pub const ZYH_ENV_PREFIX: &str = "ZYH_";

/// Whether a URI scheme is a forbidden legacy product scheme.
pub fn is_forbidden_legacy_uri_scheme(scheme: &str) -> bool {
    FORBIDDEN_LEGACY_URI_SCHEMES
        .iter()
        .any(|forbidden| scheme.eq_ignore_ascii_case(forbidden))
}

/// Whether a command name is a forbidden legacy external alias.
pub fn is_forbidden_legacy_command_alias(name: &str) -> bool {
    FORBIDDEN_LEGACY_COMMAND_ALIASES
        .iter()
        .any(|forbidden| name == *forbidden)
}

/// Current channel's public URI scheme (ZYH family).
pub fn current_uri_scheme() -> &'static str {
    ChannelState::url_scheme()
}

/// Current channel's installed agent/CLI command name.
pub fn current_agent_cli_command() -> &'static str {
    ChannelState::channel().cli_command_name()
}

/// Current channel's local-control command name (`zyhctrl*`).
pub fn current_local_control_command() -> &'static str {
    ChannelState::channel().warpctrl_command_name()
}

/// URI scheme for a channel under the ZYH external contract.
pub fn uri_scheme_for_channel(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "zyh",
        Channel::Preview => "zyh-preview",
        Channel::Dev => "zyh-dev",
        Channel::Integration => "zyh-integration",
        Channel::Local => "zyh-local",
        Channel::Oss => "zyh-oss",
    }
}

/// Agent/CLI command name for a channel under the ZYH external contract.
pub fn agent_cli_command_for_channel(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "zyh",
        Channel::Dev => "zyh-dev",
        Channel::Preview => "zyh-preview",
        Channel::Local => "zyh-local",
        Channel::Integration => "zyh-integration",
        Channel::Oss => "zyh-oss",
    }
}

/// Local-control command name for a channel under the ZYH external contract.
pub fn local_control_command_for_channel(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "zyhctrl",
        Channel::Dev => "zyhctrl-dev",
        Channel::Preview => "zyhctrl-preview",
        Channel::Local => "zyhctrl-local",
        Channel::Integration => "zyhctrl-integration",
        Channel::Oss => "zyhctrl-oss",
    }
}

/// Error message when a legacy URI scheme is presented.
pub fn legacy_uri_scheme_rejected_message(scheme: &str) -> String {
    format!(
        "The URI scheme `{scheme}` is no longer supported. Use `{URI_SCHEME_STABLE}://` (or the channel-specific ZYH scheme) instead."
    )
}

/// Error message when a legacy command alias is used.
pub fn legacy_command_alias_rejected_message(name: &str) -> String {
    format!(
        "The command `{name}` is no longer supported. Use `{AGENT_CLI_COMMAND_STABLE}` / `{LOCAL_CONTROL_COMMAND_STABLE}` instead."
    )
}

/// Cloud/account deep-link hosts that must not be accepted as product actions.
pub const FORBIDDEN_CLOUD_URI_HOSTS: &[&str] = &[
    "shared_session",
    "drive",
    "team",
    "auth",
];

pub fn is_forbidden_cloud_uri_host(host: &str) -> bool {
    FORBIDDEN_CLOUD_URI_HOSTS
        .iter()
        .any(|forbidden| host.eq_ignore_ascii_case(forbidden))
}

pub fn cloud_uri_host_rejected_message(host: &str) -> String {
    format!(
        "The deep link host `{host}` is not available in ZYH. Cloud, Account, share, and Drive URIs are rejected."
    )
}

#[cfg(test)]
#[path = "external_contracts_tests.rs"]
mod tests;
