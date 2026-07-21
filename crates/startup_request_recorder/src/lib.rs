use std::io::{self, Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_REQUEST_HEADER_SIZE: usize = 16 * 1024;
const DENIED_RESPONSE: &[u8] =
    b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
const PROXY_ENVIRONMENT_VARIABLES: [&str; 6] = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];
const NO_PROXY_ENVIRONMENT_VARIABLES: [&str; 2] = ["NO_PROXY", "no_proxy"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub target: String,
}

pub struct RequestRecorder {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl RequestRecorder {
    pub fn start() -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_requests = requests.clone();
        let worker_shutdown = shutdown.clone();
        let worker = thread::spawn(move || {
            while !worker_shutdown.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => handle_connection(stream, &worker_requests),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            address,
            requests,
            shutdown,
            worker: Some(worker),
        })
    }

    pub fn proxy_url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn proxy_environment(&self) -> impl Iterator<Item = (&'static str, String)> {
        let proxy_url = self.proxy_url();
        PROXY_ENVIRONMENT_VARIABLES
            .into_iter()
            .map(move |variable| (variable, proxy_url.clone()))
            .chain(
                NO_PROXY_ENVIRONMENT_VARIABLES
                    .into_iter()
                    .map(|variable| (variable, String::new())),
            )
    }

    pub fn requests(&self) -> io::Result<Vec<RecordedRequest>> {
        self.synchronize()?;
        Ok(self
            .requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone())
    }

    fn synchronize(&self) -> io::Result<()> {
        let mut stream = TcpStream::connect(self.address)?;
        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        stream.write_all(b"\r\n\r\n")?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;
        if response != DENIED_RESPONSE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "startup request recorder returned an invalid barrier response",
            ));
        }
        Ok(())
    }
}

impl Drop for RequestRecorder {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn handle_connection(mut stream: TcpStream, requests: &Mutex<Vec<RecordedRequest>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut header = Vec::new();
    let mut buffer = [0_u8; 1024];

    while header.len() < MAX_REQUEST_HEADER_SIZE {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                header.extend_from_slice(&buffer[..read]);
                if header.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => break,
        }
    }

    if let Some(request) = parse_request_line(&header) {
        requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
    }
    let _ = stream.write_all(DENIED_RESPONSE);
}

fn parse_request_line(header: &[u8]) -> Option<RecordedRequest> {
    let first_line = std::str::from_utf8(header).ok()?.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if !version.starts_with("HTTP/") || parts.next().is_some() {
        return None;
    }
    Some(RecordedRequest {
        method: method.to_owned(),
        target: target.to_owned(),
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
