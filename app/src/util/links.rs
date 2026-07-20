use crate::channel::ChannelState;

// Legacy Warp marketing/docs hosts. GUI open_url policy blocks these (see is_warp_dev_url).
pub const USER_DOCS_URL: &str = "https://docs.warp.dev/";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const GITHUB_ISSUES_URL: &str = "https://github.com/warpdotdev/Warp/issues";
pub const SLACK_URL: &str = "http://go.warp.dev/join-preview";
pub const PRIVACY_POLICY_URL: &str = "https://www.warp.dev/privacy";

/// True when the URL host is `warp.dev` or any `*.warp.dev` subdomain.
/// Used to suppress GUI browser jumps to Warp product/marketing/docs sites.
pub fn is_warp_dev_url(url: &url::Url) -> bool {
    match url.host_str() {
        Some(host) => {
            let host = host.to_ascii_lowercase();
            host == "warp.dev" || host.ends_with(".warp.dev")
        }
        None => false,
    }
}

pub fn feedback_form_url() -> String {
    let mut url = url::Url::parse("https://github.com/warpdotdev/Warp/issues/new/choose")
        .expect("Should not fail to parse");
    if let Some(version) = ChannelState::app_version() {
        url.query_pairs_mut().append_pair("warp-version", version);
    }
    url.query_pairs_mut()
        .append_pair("os-version", &os_info::get().version().to_string());
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::is_warp_dev_url;

    #[test]
    fn detects_warp_dev_hosts() {
        for raw in [
            "https://warp.dev",
            "https://www.warp.dev/privacy",
            "https://docs.warp.dev/",
            "http://go.warp.dev/join-preview",
            "https://oz.warp.dev",
            "https://app.warp.dev/get_warp",
        ] {
            let url = url::Url::parse(raw).expect(raw);
            assert!(is_warp_dev_url(&url), "{raw}");
        }
    }

    #[test]
    fn allows_non_warp_dev_hosts() {
        for raw in [
            "https://github.com/warpdotdev/warp",
            "https://example.com",
            "mailto:support@warp.dev",
            "warp://session/abc",
        ] {
            let url = url::Url::parse(raw).expect(raw);
            assert!(!is_warp_dev_url(&url), "{raw}");
        }
    }
}
