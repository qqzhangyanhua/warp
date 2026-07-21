use url::Url;
use warp_core::channel::ChannelState;

#[test]
#[should_panic(
    expected = "Local-only Mode attempted forbidden Warp request: GET https://app.warp.dev/forbidden"
)]
fn local_only_forbidden_warp_request_guard_panics_on_warp_host() {
    let mut client = http_client::Client::new_for_test();
    super::install_local_only_forbidden_warp_request_guard(&mut client);

    let (client, request) = client.get("https://app.warp.dev/forbidden").build_split();
    let request = request.expect("request should build");

    futures::executor::block_on(client.execute(request)).unwrap();
}

#[test]
fn local_only_forbidden_request_guard_blocks_warp_identity_and_sentry_hosts() {
    let urls = [
        "https://app.warp.dev/forbidden",
        "https://warp-server-staging.firebaseapp.com/identity",
        "https://o123.ingest.sentry.io/api/1/store/",
        "https://events.rudderstack.com/v1/batch",
    ];

    for url in urls {
        assert!(
            super::is_forbidden_local_only_request_url(&url.parse().expect("url should parse")),
            "{url} should be forbidden"
        );
    }
}

#[test]
fn local_only_forbidden_request_guard_blocks_configured_warp_service_origins() {
    for root_url in [ChannelState::server_root_url(), ChannelState::oz_root_url()] {
        let url = Url::parse(root_url.as_ref())
            .expect("configured Warp service URL should parse")
            .join("local-only-smoke")
            .expect("smoke path should join");

        assert!(
            super::is_forbidden_local_only_request_url(&url),
            "configured Warp service origin should be forbidden: {url}"
        );
    }
}

#[test]
fn local_only_forbidden_request_guard_allows_configured_provider_and_explicit_mcp_origins() {
    let allowed_urls = [
        // Configured Provider Origin.
        "http://127.0.0.1:11434/v1/chat/completions",
        // Explicitly configured MCP origin.
        "https://mcp.example.test/sse",
    ];

    for url in allowed_urls {
        assert!(
            !super::is_forbidden_local_only_request_url(
                &url.parse().expect("allowed URL should parse")
            ),
            "Local-only Mode should allow configured Provider and MCP origins: {url}"
        );
    }
}

#[test]
fn agent_api_safe_diagnostic_excludes_error_payloads() {
    let error = super::AIApiError::Other(anyhow::anyhow!(
        "request failed for https://user:secret-token@provider.example/v1"
    ));

    let diagnostic = error.safe_diagnostic();

    assert_eq!(diagnostic, "unexpected_agent_api_error");
    assert!(!diagnostic.contains("provider.example"));
    assert!(!diagnostic.contains("secret-token"));
}
