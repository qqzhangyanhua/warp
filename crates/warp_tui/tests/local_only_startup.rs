#![cfg(unix)]

use std::fs::File;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::os::fd::FromRawFd;
use std::os::unix::io::AsRawFd;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use command::blocking::Command;
use instant::Instant;
use nix::poll::{poll, PollFd, PollFlags};
use nix::pty::{openpty, Winsize};
use startup_request_recorder::RequestRecorder;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);

struct ReapingChild(Child);

impl Deref for ReapingChild {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ReapingChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for ReapingChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
fn local_only_tui_reaches_terminal_input_without_device_authorization() {
    let recorder = RequestRecorder::start().expect("startup request recorder should start");
    let home = tempfile::tempdir().expect("Local-only TUI smoke HOME should be created");
    let pty = openpty(
        Some(&Winsize {
            ws_row: 40,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }),
        None,
    )
    .expect("PTY should be created");

    // SAFETY: openpty returns newly owned descriptors. Each descriptor is wrapped exactly once.
    let master = unsafe { File::from_raw_fd(pty.master) };
    let slave = unsafe { File::from_raw_fd(pty.slave) };
    let child_stdin = slave.try_clone().expect("PTY stdin should clone");
    let child_stdout = slave.try_clone().expect("PTY stdout should clone");

    let mut command = Command::new(env!("CARGO_BIN_EXE_warp-tui-oss"));
    command
        .env("HOME", home.path())
        .env("TERM", "xterm-256color")
        .envs(recorder.proxy_environment());
    let mut child = ReapingChild(
        command
            .stdin(Stdio::from(child_stdin))
            .stdout(Stdio::from(child_stdout))
            .stderr(Stdio::from(slave))
            .spawn()
            .expect("Local-only TUI smoke child should start"),
    );

    let mut reader = master.try_clone().expect("PTY reader should clone");
    let mut writer = master;
    let (output_tx, output_rx) = mpsc::channel();
    let reader_shutdown = Arc::new(AtomicBool::new(false));
    let reader_thread_shutdown = reader_shutdown.clone();
    let reader_thread = thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        let mut poll_fds = [PollFd::new(reader.as_raw_fd(), PollFlags::POLLIN)];
        while !reader_thread_shutdown.load(Ordering::Acquire) {
            if !matches!(poll(&mut poll_fds, 100), Ok(ready) if ready > 0) {
                continue;
            }
            let Ok(read) = reader.read(&mut buffer) else {
                break;
            };
            if read == 0 || output_tx.send(buffer[..read].to_vec()).is_err() {
                break;
            }
        }
    });

    let started_at = Instant::now();
    let mut last_probe = started_at;
    let mut output = Vec::new();
    let mut reached_terminal_input = false;

    while started_at.elapsed() < STARTUP_TIMEOUT {
        if let Ok(chunk) = output_rx.recv_timeout(Duration::from_millis(100)) {
            output.extend_from_slice(&chunk);
        }

        if String::from_utf8_lossy(&output).contains("shell mode") {
            reached_terminal_input = true;
            break;
        }

        if let Some(status) = child
            .try_wait()
            .expect("TUI child status should be readable")
        {
            panic!(
                "Local-only TUI exited before reaching terminal input ({status}):\n{}",
                String::from_utf8_lossy(&output)
            );
        }

        if started_at.elapsed() >= Duration::from_secs(1)
            && last_probe.elapsed() >= Duration::from_millis(500)
        {
            writer
                .write_all(b"!")
                .expect("terminal input probe should be written");
            writer.flush().expect("terminal input probe should flush");
            last_probe = Instant::now();
        }
    }

    if !reached_terminal_input {
        child.kill().expect("Timed-out TUI smoke child should stop");
        child.wait().expect("Timed-out TUI smoke child should reap");
        reader_shutdown.store(true, Ordering::Release);
        drop(writer);
        drop(output_rx);
        reader_thread.join().expect("PTY reader should stop");
        panic!(
            "Local-only TUI did not reach terminal input:\n{}",
            String::from_utf8_lossy(&output)
        );
    }

    let rendered = String::from_utf8_lossy(&output);
    assert!(
        !rendered.contains("Sign in to continue"),
        "Local-only TUI entered device authorization:\n{rendered}"
    );

    writer
        .write_all(&[3, 3])
        .expect("Ctrl-C shutdown should be written");
    writer.flush().expect("Ctrl-C shutdown should flush");

    let shutdown_deadline = Instant::now() + Duration::from_secs(5);
    let mut shutdown_status = None;
    while Instant::now() < shutdown_deadline {
        if let Some(status) = child
            .try_wait()
            .expect("TUI child status should be readable")
        {
            shutdown_status = Some(status);
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    if shutdown_status.is_none() {
        child.kill().expect("TUI smoke child should be terminated");
        child
            .wait()
            .expect("terminated TUI smoke child should reap");
    }
    drop(output_rx);
    reader_shutdown.store(true, Ordering::Release);
    drop(writer);
    reader_thread.join().expect("PTY reader should stop");
    if let Some(status) = shutdown_status {
        assert!(status.success(), "Local-only TUI shutdown failed: {status}");
    }
    let requests = recorder
        .requests()
        .expect("TUI request recorder should synchronize");
    assert!(
        requests.is_empty(),
        "TUI startup made app-initiated requests: {requests:#?}"
    );
}
