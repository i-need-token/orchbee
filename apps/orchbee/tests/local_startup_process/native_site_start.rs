//! `site start` witnesses: the headless connect-or-activate command runs the
//! shared activation core against a real staged cohort — cold start, warm
//! idempotence, overlapping starts, the marker-without-owner refusal, the
//! single-deadline timeout, inherited signal death, cohort failures, and the
//! lossless runtimeBase encoding. Every witness is non-TTY.

use super::native_cli_query::{run_cli, run_json, validate, validators};
use super::*;
use orchbee_local_site_runtime::LockAttempt;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::ExitStatusExt;
use std::process::Output;

/// Run `site start` with a caller-chosen LANG/base override; identical to
/// `run_cli` apart from the extra environment pair.
fn run_start(fixture: &Fixture, arguments: &[&str], lang: &str, base: &std::path::Path) -> Output {
    Command::new(fixture.cohort.join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| panic!("spawn site start {arguments:?}: {error}"))
}

/// A live Server admitting through the real client socket is the only proof
/// a start succeeded; the marker alone never counts.
fn assert_server_running(fixture: &Fixture) {
    assert!(
        fixture.site().client_socket_exists().unwrap_or(false),
        "started outcome without a live client socket"
    );
    assert_eq!(
        fixture.processes("orchbee-server").len(),
        1,
        "exactly one managed Server"
    );
}

#[test]
fn site_start_invalid_grammar_is_zero_effect() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    for arguments in [
        vec!["site"],
        vec!["site", "stop"],
        vec!["site", "start", "extra"],
        vec!["site", "start", "--project", "x"],
        vec!["site", "start", "--format", "yaml"],
    ] {
        let output = run_cli(&fixture, &arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // Parse failure is the one guaranteed zero-effect answer: no runtime
    // directory, no marker, no process.
    assert!(!fixture.runtime().exists());
    assert!(fixture.processes("orchbee-server").is_empty());
    Ok(())
}

#[test]
fn site_start_cold_then_warm_reports_the_actual_branch() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let validators = validators(&fixture);

    // Cold start: this invocation spawns the Server.
    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.site.start", &output);
    assert_eq!(document["result"]["outcome"], "started");
    assert_eq!(
        document["result"]["scope"],
        format!("orchbee-native-{}", nix::unistd::geteuid())
    );
    assert_eq!(
        document["result"]["runtimeBase"]["path"],
        fixture.base.to_str().expect("fixture base is UTF-8")
    );
    assert!(
        document["result"]["runtimeBase"]
            .get("pathBytesHex")
            .is_none(),
        "UTF-8 base never emits pathBytesHex"
    );
    assert_server_running(&fixture);
    let server_pid = fixture.processes("orchbee-server")[0];

    // Warm start: the same Site admits the probe — no second spawn.
    let output = run_cli(&fixture, &["site", "start"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "Local Site is already running.\n"
    );
    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(document["result"]["outcome"], "already-running");
    assert_eq!(
        fixture.processes("orchbee-server"),
        vec![server_pid],
        "warm start must not spawn a second Server"
    );

    // The dropped probe leaked nothing: an ordinary query still admits.
    let output = run_cli(&fixture, &["status"]);
    assert_eq!(output.status.code(), Some(0));
    Ok(())
}

#[test]
fn site_start_overlapping_invocations_spawn_exactly_one_server() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut children = Vec::new();
    for _ in 0..3 {
        children.push(
            Command::new(fixture.cohort.join("orchbee"))
                .args(["site", "start", "--format", "json"])
                .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
                .env("LANG", "en_US.UTF-8")
                .env_remove("LC_ALL")
                .env_remove("LC_MESSAGES")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        );
    }
    let mut started = 0;
    let mut already_running = 0;
    for child in children {
        let output = support::bounded_wait_with_output(child, STARTUP_OR_CREATE_TIMEOUT)
            .unwrap_or_else(|timeout| panic!("overlapping site start: {timeout}"));
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        match serde_json::from_slice::<serde_json::Value>(&output.stdout)?["result"]["outcome"]
            .as_str()
            .expect("outcome string")
        {
            "started" => started += 1,
            "already-running" => already_running += 1,
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(started, 1, "exactly one invocation performs the spawn");
    assert_eq!(already_running, 2, "the rest observe the running Site");
    assert_server_running(&fixture);
    Ok(())
}

#[test]
fn site_start_marker_without_owner_is_refused_and_untouched() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let site = fixture.site();
    // A committed marker with no live owner is the wedge state: refused,
    // never repaired, never retried inside the same invocation.
    site.commit_activation()?;
    let marker = fixture.runtime().join("activation.started");
    let before = fs::metadata(&marker)?;

    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(document["failure"]["kind"], "rejected");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "recovery-required"
    );
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("requires recovery"), "{stderr}");

    // Fail-closed means byte identity: the marker is the same file, still
    // empty, and no Server was spawned to adopt it.
    let after = fs::metadata(&marker)?;
    assert_eq!(before.ino(), after.ino());
    assert_eq!(after.len(), 0);
    assert!(fixture.processes("orchbee-server").is_empty());
    Ok(())
}

#[test]
fn site_start_held_activation_lock_times_out_without_repair() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let site = fixture.site();
    let _held = match site.try_acquire_activation_lock()? {
        LockAttempt::Acquired(lock) => lock,
        LockAttempt::Contended => panic!("fixture activation lock must be free"),
    };
    let began = Instant::now();
    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    let elapsed = began.elapsed();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(document["failure"]["kind"], "timeout");
    assert_eq!(document["failure"]["details"]["stage"], "activation");
    // One absolute budget: the wait ends near 20s, never resets per retry.
    assert!(elapsed >= Duration::from_secs(19), "elapsed {elapsed:?}");
    // Timeout means readiness was unconfirmed — here provably nothing was
    // spawned behind the held lock, and nothing was torn down or repaired.
    assert!(fixture.processes("orchbee-server").is_empty());
    assert!(!fixture.runtime().join("activation.started").exists());
    Ok(())
}

#[test]
fn site_start_sigterm_is_signal_death_not_a_cancel_envelope() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let site = fixture.site();
    // Hold the activation lock so the child spins inside the activation
    // loop: the signal lands mid-wait, exercising raw inherited semantics.
    let held = match site.try_acquire_activation_lock()? {
        LockAttempt::Acquired(lock) => lock,
        LockAttempt::Contended => panic!("fixture activation lock must be free"),
    };
    let child = Command::new(fixture.cohort.join("orchbee"))
        .args(["site", "start", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
        .env("LANG", "en_US.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    thread::sleep(Duration::from_millis(1500));
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )?;
    let output = support::bounded_wait_with_output(child, EXIT_TIMEOUT + Duration::from_secs(3))
        .unwrap_or_else(|timeout| panic!("signaled site start: {timeout}"));
    // WIFSIGNALED, not a fabricated exit-130 envelope: signal death is the
    // honest inherited behavior.
    assert_eq!(
        output.status.signal(),
        Some(nix::sys::signal::Signal::SIGTERM as i32),
        "expected signal death, got {:?}",
        output.status
    );
    assert_eq!(output.status.code(), None);
    assert!(output.stdout.is_empty(), "no document on signal death");

    drop(held);
    // The lock's release is the only recovery: a fresh invocation converges.
    let output = run_cli(&fixture, &["site", "start"]);
    assert_eq!(output.status.code(), Some(0));
    assert_server_running(&fixture);
    Ok(())
}

#[test]
fn site_start_cohort_failures_classify_typed() -> TestResult {
    let _guard = process_witness_guard();
    // Missing manifest: NotFound → service-unavailable.
    let fixture = Fixture::new();
    fs::remove_file(fixture.cohort.join("cohort.json"))?;
    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "cohort");

    // Tampered artifact: hash mismatch is InvalidData → incompatible.
    let fixture = Fixture::new();
    let server = fixture.cohort.join("orchbee-server");
    fs::write(&server, b"not a server")?;
    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    assert_eq!(output.status.code(), Some(6));
    assert_eq!(document["failure"]["kind"], "incompatible");

    // Malformed manifest: same typed verdict.
    let fixture = Fixture::new();
    fs::write(fixture.cohort.join("cohort.json"), b"{not json")?;
    let (output, document) = run_json(&fixture, &["site", "start", "--format", "json"]);
    assert_eq!(output.status.code(), Some(6));
    assert_eq!(document["failure"]["kind"], "incompatible");
    Ok(())
}

#[test]
fn site_start_non_utf8_base_encodes_path_bytes_hex() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let parent = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/obx-\xff-start".to_vec(),
    ));
    let weird = parent.join(format!("{}", std::process::id()));
    fs::create_dir_all(&weird)?;
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&weird, fs::Permissions::from_mode(0o700))?;
    fixture.register_runtime_base(&weird);

    let (output, document) = {
        let output = run_start(
            &fixture,
            &["site", "start", "--format", "json"],
            "en_US.UTF-8",
            &weird,
        );
        let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        (output, document)
    };
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let base = &document["result"]["runtimeBase"];
    let hex = base["pathBytesHex"].as_str().expect("pathBytesHex");
    assert!(
        base.get("path").is_none(),
        "non-UTF-8 base never emits path"
    );
    assert_eq!(
        *hex,
        weird
            .as_os_str()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(fixture.processes("orchbee-server").len(), 1);
    fs::remove_dir_all(&parent)?;
    Ok(())
}

#[test]
fn site_start_text_output_is_localized() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let output = run_start(&fixture, &["site", "start"], "zh_CN.UTF-8", &fixture.base);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout)?, "本地 Site 已启动。\n");
    let output = run_start(&fixture, &["site", "start"], "zh_CN.UTF-8", &fixture.base);
    assert_eq!(String::from_utf8(output.stdout)?, "本地 Site 已在运行。\n");
    Ok(())
}
