use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::Duration;

use super::*;

fn send_request(recorder: &RequestRecorder, request: &[u8]) -> Vec<u8> {
    let address = recorder
        .proxy_url()
        .strip_prefix("http://")
        .expect("proxy URL should use HTTP")
        .to_owned();
    let mut stream = TcpStream::connect(address).expect("recorder should accept connections");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("read timeout should be configured");
    stream
        .write_all(request)
        .expect("request should be written");

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("recorder response should be readable");
    response
}

fn wait_for_requests(recorder: &RequestRecorder, count: usize) -> Vec<RecordedRequest> {
    for _ in 0..100 {
        let requests = recorder.requests().expect("recorder should synchronize");
        if requests.len() == count {
            return requests;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("request was not recorded");
}

#[test]
fn request_snapshot_flushes_connections_queued_before_the_snapshot() {
    let recorder = RequestRecorder::start().expect("recorder should start");
    let address = recorder
        .proxy_url()
        .strip_prefix("http://")
        .expect("proxy URL should use HTTP")
        .to_owned();
    let mut stream = TcpStream::connect(address).expect("recorder should accept connections");
    stream
        .write_all(b"GET http://queued.example.test/ HTTP/1.1\r\nHost: queued.example.test\r\n\r\n")
        .expect("queued request should be written");
    drop(stream);

    assert_eq!(
        recorder.requests().expect("recorder should synchronize"),
        vec![RecordedRequest {
            method: "GET".to_owned(),
            target: "http://queued.example.test/".to_owned(),
        }]
    );
}

#[test]
fn records_absolute_http_proxy_requests_and_denies_forwarding() {
    let recorder = RequestRecorder::start().expect("recorder should start");

    let response = send_request(
        &recorder,
        b"GET http://example.test/status?source=startup HTTP/1.1\r\nHost: example.test\r\n\r\n",
    );

    assert!(response.starts_with(b"HTTP/1.1 502 Bad Gateway\r\n"));
    assert_eq!(
        wait_for_requests(&recorder, 1),
        vec![RecordedRequest {
            method: "GET".to_owned(),
            target: "http://example.test/status?source=startup".to_owned(),
        }]
    );
}

#[test]
fn records_https_and_websocket_connect_targets_and_denies_forwarding() {
    let recorder = RequestRecorder::start().expect("recorder should start");

    let response = send_request(
        &recorder,
        b"CONNECT rtc.example.test:443 HTTP/1.1\r\nHost: rtc.example.test:443\r\n\r\n",
    );

    assert!(response.starts_with(b"HTTP/1.1 502 Bad Gateway\r\n"));
    assert_eq!(
        wait_for_requests(&recorder, 1),
        vec![RecordedRequest {
            method: "CONNECT".to_owned(),
            target: "rtc.example.test:443".to_owned(),
        }]
    );
}
