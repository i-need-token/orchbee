//! Windows public CLI source admission: real inherited pipes validate fully
//! before runtime discovery. Deliberately invalid runtime configuration prevents
//! any Site access; no fixture or user terminal can receive input.
#![cfg(windows)]

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

const TERMINAL: &str = "00000000000000000000000000000001";

fn invoke(source: &[&str], payload: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(["terminal", "send", TERMINAL])
        .args(source)
        .args(["--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI");
    // Early validation may close the pipe before consuming all hostile input.
    let _ = child.stdin.take().expect("stdin").write_all(payload);
    child.wait_with_output().expect("CLI result")
}

#[test]
fn pipe_validation_precedes_runtime_discovery() {
    let control = invoke(&["--text", "ready"], &[]);
    assert!(!control.status.success());
    assert_ne!(control.status.code(), Some(2));
    for (source, payload) in [
        (vec!["--stdin"], "中文\r\n".as_bytes().to_vec()),
        (vec!["--stdin"], vec![0xff, 0, 0x80]),
        (
            vec!["--stdin", "--paste", "--enter"],
            b"literal `$(data)`".to_vec(),
        ),
    ] {
        let result = invoke(&source, &payload);
        assert_eq!(result.status.code(), control.status.code());
        assert_eq!(result.stdout, control.stdout);
        assert_eq!(result.stderr, control.stderr);
    }
    for payload in [Vec::new(), vec![b'x'; 1_048_577]] {
        let result = invoke(&["--stdin"], &payload);
        assert_eq!(result.status.code(), Some(2));
        let document: serde_json::Value =
            serde_json::from_slice(&result.stdout).expect("failure JSON");
        assert!(document.is_object());
        assert!(String::from_utf8_lossy(&result.stderr).contains("--stdin"));
    }
}

#[test]
fn actual_powershell_pipeline_reaches_the_same_post_source_boundary() {
    let control = invoke(&["--text", "ready"], &[]);
    let binary = env!("CARGO_BIN_EXE_orchbee").replace('\'', "''");
    let script = format!(
        "$OutputEncoding=[Text.UTF8Encoding]::new($false); '中文' | & '{binary}' terminal send {TERMINAL} --stdin --format json; exit $LASTEXITCODE"
    );
    for shell in ["powershell.exe", "pwsh.exe"] {
        let result = match Command::new(shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ])
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .env("LC_ALL", "en_US.UTF-8")
            .output()
        {
            Ok(output) => output,
            Err(error) if shell == "pwsh.exe" && error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => panic!("{shell}: {error}"),
        };
        assert_eq!(
            result.status.code(),
            control.status.code(),
            "{shell}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result_document: serde_json::Value =
            serde_json::from_slice(&result.stdout).expect("pipeline JSON");
        let control_document: serde_json::Value =
            serde_json::from_slice(&control.stdout).expect("control JSON");
        assert_eq!(result_document, control_document);
    }
}
