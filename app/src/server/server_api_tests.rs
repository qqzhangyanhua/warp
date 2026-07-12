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
