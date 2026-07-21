#![cfg(unix)]

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use command::blocking::Command;
use instant::Instant;
use nix::pty::{openpty, Winsize};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);

#[test]
fn local_only_tui_reaches_terminal_input_without_device_authorization() {
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

    let mut child = Command::new(env!("CARGO_BIN_EXE_warp-tui-oss"))
        .env("HOME", home.path())
        .env("TERM", "xterm-256color")
        .stdin(Stdio::from(child_stdin))
        .stdout(Stdio::from(child_stdout))
        .stderr(Stdio::from(slave))
        .spawn()
        .expect("Local-only TUI smoke child should start");

    let mut reader = master.try_clone().expect("PTY reader should clone");
    let mut writer = master;
    let (output_tx, output_rx) = mpsc::channel();
    let reader_thread = thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        while let Ok(read) = reader.read(&mut buffer) {
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
    while Instant::now() < shutdown_deadline {
        if let Some(status) = child
            .try_wait()
            .expect("TUI child status should be readable")
        {
            drop(output_rx);
            reader_thread.join().expect("PTY reader should stop");
            assert!(status.success(), "Local-only TUI shutdown failed: {status}");
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }

    child.kill().expect("TUI smoke child should be terminated");
    child
        .wait()
        .expect("terminated TUI smoke child should reap");
    drop(output_rx);
    reader_thread.join().expect("PTY reader should stop");
}
