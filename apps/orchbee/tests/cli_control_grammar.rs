//! Composed CLI-first control grammar witness: the real `orchbee` binary is
//! probed with an explicitly unavailable runtime so every outcome is decided
//! by argv grammar alone. The matrix pins the FINAL-composed bounded command
//! grammar — `terminal send`, `terminal wait-output`, and
//! `workspace accept-directory` — against the shared option rules the native
//! parser already enforces.
//!
//! Environment discipline: each probe runs a private copy of the just-built
//! binary staged inside an exclusively created mode-0700 directory that holds
//! nothing else — `NativeCohort::current` inspects the running executable's
//! siblings, so a `cohort.json` left beside the build-tree original by a
//! developer or a staging step can never ride into the oracle.
//! `ORCHBEE_RUNTIME_BASE` points at a freshly minted empty directory with no
//! Site inside. Any argv the grammar admits must therefore stop at the
//! cohort safety check — `check-failed` at stage `cohort`, exit 1 — so a `2`
//! exit can only come from the parser. Negative controls already pass
//! because unknown words are grammar failures too.
#![cfg(target_os = "linux")]

use std::{
    fs,
    path::PathBuf,
    process::Output,
    sync::{Mutex, MutexGuard},
};

// The shared support module also carries helpers only other suites use.
#[allow(dead_code)]
mod support;

/// One process-internal guard serializes this pure-grammar suite's stage,
/// spawn, reap, and fixture-cleanup window. The observed first scene is an
/// intermittent ETXTBSY(26) at spawn that only reproduces while several
/// test threads stage copies and fork/exec concurrently; serial runs are
/// clean. Each probe takes the guard before staging and holds it until
/// drop — after `support::run`'s bounded wait has reaped every child and
/// `FixtureDir::drop` has removed the directory. This seals the window
/// inside the fixture without an exec retry, without touching product
/// code, and without constraining the real concurrent functional suites.
static STAGE_SERIAL: Mutex<()> = Mutex::new(());

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A canonical 32-lowercase-hex Project identity.
const PROJECT_ID: &str = "bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc";
/// A canonical 32-lowercase-hex Terminal identity; `Resource::Terminal`
/// admits no label form.
const TERMINAL_ID: &str = "abababababababababababababababab";
/// A canonical 32-lowercase-hex Workspace identity.
const WORKSPACE_ID: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
/// All-zero typed identities are rejected by every resource.
const ZERO_ID: &str = "00000000000000000000000000000000";

/// The closed `terminal send --key` vocabulary from CLI-SEND-IMPLEMENT-FINAL.
const SEND_KEYS: &[&str] = &[
    "enter",
    "escape",
    "tab",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "page-up",
    "page-down",
    "ctrl-c",
    "ctrl-d",
];

/// A minted, exclusively-created, mode-0700 runtime base with no Site inside:
/// the explicit unavailable-runtime environment for every probe.
fn base() -> PathBuf {
    support::runtime_base("grammar-witness")
}

/// The owned directory holding exactly one file — a private copy of the
/// just-built `orchbee`. `NativeCohort::current` resolves the running
/// executable's parent and opens `cohort.json` there, so a cohort staged
/// beside the build-tree original would silently ride into the oracle: the
/// same admitted argv could pass the cohort check and fail at Site discovery
/// instead, no longer proving grammar admission. Staging the copy makes the
/// empty-siblings invariant explicit rather than assumed. Dropping removes
/// the directory; every child spawned from the copy is already reaped by
/// `support::run`'s bounded wait before that can happen.
struct FixtureDir(PathBuf);

impl FixtureDir {
    /// Copy the build-tree binary once into a freshly minted mode-0700
    /// directory, then verify by directory listing that the fixture holds
    /// exactly that one file — the structural evidence that no `cohort.json`
    /// or other program sits beside the binary the group will run.
    fn stage() -> (Self, PathBuf) {
        let directory = Self(support::runtime_base("grammar-binary"));
        let binary = directory.0.join("orchbee");
        fs::copy(env!("CARGO_BIN_EXE_orchbee"), &binary)
            .expect("copy the just-built orchbee into the isolated fixture directory");
        let names: Vec<String> = fs::read_dir(&directory.0)
            .expect("list the isolated fixture directory")
            .map(|entry| {
                entry
                    .expect("fixture directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            ["orchbee"],
            "the fixture directory must hold the copied binary and nothing else"
        );
        (directory, binary)
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One test group's owned environment: the unavailable runtime base plus the
/// isolated binary copy every argv in the group runs against.
struct Probe {
    base: PathBuf,
    binary: PathBuf,
    _fixture: FixtureDir,
    _serial: MutexGuard<'static, ()>,
}

/// Mint the probe environment — one binary copy per test group, staged only
/// after the serialization guard is taken so the copy, every spawn, every
/// reap, and the fixture cleanup share one serialized window.
fn probe() -> Probe {
    let serial = STAGE_SERIAL.lock().expect("grammar staging serialization");
    let (fixture, binary) = FixtureDir::stage();
    Probe {
        base: base(),
        binary,
        _fixture: fixture,
        _serial: serial,
    }
}

/// Spawn the isolated binary copy under the unavailable runtime with the
/// shared deterministic locale and wall-clock backstop.
fn run(probe: &Probe, arguments: &[&str]) -> Output {
    support::run(&probe.binary, &probe.base, arguments)
}

/// The grammar admitted the argv: with no cohort beside the isolated binary,
/// the only honest post-parse outcome is the cohort safety check —
/// `check-failed` at exit 1. Anything else (usage 2, a panic, a signal) fails
/// the witness, so a bare `!= 2` is never enough.
fn admitted(probe: &Probe, arguments: &[&str]) {
    let output = run(probe, arguments);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "grammar-admitted {arguments:?} must stop at the cohort check; stderr: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "text-envelope {arguments:?} must not write a stdout document; stderr: {stderr}"
    );
    assert!(
        stderr.contains("cohort"),
        "{arguments:?} must carry the cohort-stage diagnostic; stderr: {stderr}"
    );
}

/// A grammar-admitted argv under a selected JSON envelope still stops at the
/// cohort check: exit 1, and the failure document records kind `check-failed`
/// at stage `cohort`.
fn admitted_json(probe: &Probe, arguments: &[&str]) {
    let output = run(probe, arguments);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{arguments:?}; stderr: {stderr}"
    );
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cohort failure document is JSON");
    assert_eq!(document["failure"]["kind"], "check-failed", "{arguments:?}");
    assert_eq!(
        document["failure"]["details"]["stage"], "cohort",
        "{arguments:?}"
    );
    assert!(
        stderr.contains("cohort"),
        "{arguments:?} must carry the cohort-stage diagnostic; stderr: {stderr}"
    );
}

/// A grammar failure is classified before any runtime discovery: the only
/// observable contract is the public usage exit.
fn usage(probe: &Probe, arguments: &[&str]) {
    let output = run(probe, arguments);
    assert_eq!(
        output.status.code(),
        Some(2),
        "grammar must reject {arguments:?} before runtime; stdout: {:?}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Format-envelope and argv-envelope failures keep stdout empty.
fn usage_silent(probe: &Probe, arguments: &[&str]) {
    let output = run(probe, arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    assert!(
        output.stdout.is_empty(),
        "{arguments:?} must not write a stdout document; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A grammar failure after a unique valid `--format json` keeps the selected
/// envelope: a JSON failure document whose kind is `invalid-usage`.
fn usage_json(probe: &Probe, arguments: &[&str]) {
    usage_json_kind(probe, arguments, "invalid-usage");
}

/// The same envelope rule for every grammar-stage kind: a valid json
/// selection keeps the JSON document while `failure.kind` is the kind the
/// caller names (`invalid-identifier` for malformed targets and values).
fn usage_json_kind(probe: &Probe, arguments: &[&str], kind: &str) {
    let output = run(probe, arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("json-envelope failure is JSON");
    assert_eq!(document["failure"]["kind"], kind, "{arguments:?}");
}

/// Per-path help answers with zero runtime: exit 0 and a non-empty body even
/// though no cohort or Site exists.
fn help_ok(probe: &Probe, arguments: &[&str]) {
    let output = run(probe, arguments);
    assert_eq!(output.status.code(), Some(0), "{arguments:?}");
    assert!(
        !output.stdout.is_empty(),
        "{arguments:?} help body must be non-empty"
    );
}

// ---- terminal send ---------------------------------------------------------

/// CLI-SEND-IMPLEMENT-FINAL admits exactly
/// `terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>)
/// [--format text|json]` — text plus the folded CR stays within 4096 bytes,
/// and options are legal in any position before `--`.
#[test]
fn terminal_send_admitted_forms() -> TestResult {
    let probe = probe();
    let text_at_bound = "t".repeat(4096);
    let text_with_cr_at_bound = "t".repeat(4095);
    for arguments in [
        ["terminal", "send", TERMINAL_ID, "--text", "hello"].as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "hello",
            "--enter",
        ]
        .as_slice(),
        // Option before the operand is legal under the shared scan order.
        ["terminal", "send", "--key", "enter", TERMINAL_ID].as_slice(),
        // A value flag owns the next token even when it spells a flag.
        ["terminal", "send", TERMINAL_ID, "--text", "--key"].as_slice(),
        // Byte bound: 4096 literal bytes are legal with and without --enter
        // only when the folded record stays at or under 4096.
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            text_at_bound.as_str(),
        ]
        .as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            text_with_cr_at_bound.as_str(),
            "--enter",
        ]
        .as_slice(),
        // Global format selection in both spellings.
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "hi",
            "--format=text",
        ]
        .as_slice(),
        // A trailing `--` ends option scanning and changes nothing.
        ["terminal", "send", TERMINAL_ID, "--text", "hi", "--"].as_slice(),
        // UTF-8 payloads beyond ASCII are literal bytes, never expanded.
        ["terminal", "send", TERMINAL_ID, "--text", "多 字节"].as_slice(),
    ] {
        admitted(&probe, arguments);
    }
    // The separated `--format json` selector keeps the JSON failure envelope.
    admitted_json(
        &probe,
        &[
            "terminal",
            "send",
            TERMINAL_ID,
            "--key",
            "ctrl-c",
            "--format",
            "json",
        ],
    );
    // Every member of the closed key vocabulary is individually legal.
    for key in SEND_KEYS {
        admitted(&probe, &["terminal", "send", TERMINAL_ID, "--key", key]);
    }
    Ok(())
}

#[test]
fn terminal_send_usage_failures() -> TestResult {
    let probe = probe();
    let text_over_bound = "t".repeat(4097);
    let text_at_bound = "t".repeat(4096);
    for arguments in [
        // Missing operand and missing payload.
        ["terminal", "send"].as_slice(),
        ["terminal", "send", TERMINAL_ID].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--text"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--key"].as_slice(),
        // `--` is never consumed as a flag value.
        ["terminal", "send", TERMINAL_ID, "--key", "--"].as_slice(),
        // Empty or over-budget text: the folded CR counts.
        ["terminal", "send", TERMINAL_ID, "--text", ""].as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            text_over_bound.as_str(),
        ]
        .as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            text_at_bound.as_str(),
            "--enter",
        ]
        .as_slice(),
        // Payload forms are mutually exclusive; --enter belongs to --text only.
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "a",
            "--key",
            "enter",
        ]
        .as_slice(),
        ["terminal", "send", TERMINAL_ID, "--key", "enter", "--enter"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--enter"].as_slice(),
        // `--key` is a closed set; spelling and case are exact.
        ["terminal", "send", TERMINAL_ID, "--key", "pageup"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--key", "Page-Down"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--key", ""].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--key", "f1"].as_slice(),
        // Duplicates are rejected, including bare flags.
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "a",
            "--text",
            "b",
        ]
        .as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--key",
            "enter",
            "--key",
            "tab",
        ]
        .as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "a",
            "--enter",
            "--enter",
        ]
        .as_slice(),
        // Joined `--flag=value` is admitted for value options; a boolean
        // never takes one.
        ["terminal", "send", TERMINAL_ID, "--enter=x"].as_slice(),
        // Options of other commands are outside the send scope.
        ["terminal", "send", TERMINAL_ID, "--match", "x"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--rows", "5"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--timeout", "5"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--path", "/x"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--history"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--confirm"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--standalone"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--frobnicate"].as_slice(),
        // Terminal targets are typed IDs only; labels and the zero identity
        // are identifier failures.
        ["terminal", "send", "my-label", "--key", "enter"].as_slice(),
        ["terminal", "send", ZERO_ID, "--key", "enter"].as_slice(),
        // Extra positionals, including flag-looking words after `--`.
        ["terminal", "send", TERMINAL_ID, "extra", "--key", "enter"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--", "--key", "enter"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--text", "x", "-"].as_slice(),
    ] {
        usage(&probe, arguments);
    }
    // A selected `--format json` survives grammar failures as the envelope.
    usage_json(
        &probe,
        &[
            "terminal",
            "send",
            TERMINAL_ID,
            "--key",
            "enter",
            "--frobnicate",
            "--format",
            "json",
        ],
    );
    Ok(())
}

// ---- terminal wait-output --------------------------------------------------

/// CLI-FIRST-FLOW-ACCEPTED admits
/// `terminal wait-output <terminal-id> (--match <text> | --regex <pattern>)
/// [--rows <n>] [--timeout <ms>] [--format text|json]` with exactly one
/// matcher required: `--match` is a non-empty literal of at most 4096 bytes,
/// `--regex` a non-empty pattern of at most 1024 bytes compiled at parse
/// under the declared program and DFA budgets; `--rows` 1..=1000
/// (default 1000); `--timeout` 1..=3600000 (default 30000).
#[test]
fn terminal_wait_output_admitted_forms() -> TestResult {
    let probe = probe();
    let match_at_bound = "m".repeat(4096);
    let regex_at_bound = "r".repeat(1024);
    for arguments in [
        ["terminal", "wait-output", TERMINAL_ID, "--match", "ready"].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--rows",
            "1",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--rows",
            "1000",
        ]
        .as_slice(),
        // The minimal 1ms deadline argv is handled by
        // `admitted_wait_min_timeout` below: the total Wait budget includes
        // the preamble, so which honest outcome settles first is timing —
        // never something a flat `admitted` assertion may flatten.
        //
        // `--timeout 3600000` stays in the flat list: hours cannot elapse
        // during the cohort check, so the cohort-stage failure is stable.
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--timeout",
            "3600000",
        ]
        .as_slice(),
        ["terminal", "wait-output", "--match", "ready", TERMINAL_ID].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            match_at_bound.as_str(),
        ]
        .as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--match", "多 字节"].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--",
        ]
        .as_slice(),
        // `--regex` is the alternative matcher: same required-exactly-one
        // grammar, compiled at parse under the declared budgets.
        ["terminal", "wait-output", TERMINAL_ID, "--regex", "ready"].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--regex",
            "build (ok|FAILED)",
            "--rows",
            "50",
            "--timeout",
            "5000",
        ]
        .as_slice(),
        // Zero-width patterns are admissible grammar.
        ["terminal", "wait-output", TERMINAL_ID, "--regex", "^$"].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--regex",
            regex_at_bound.as_str(),
        ]
        .as_slice(),
    ] {
        admitted(&probe, arguments);
    }
    admitted_json(
        &probe,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--format",
            "json",
        ],
    );
    admitted_wait_min_timeout(&probe);
    Ok(())
}

/// The minimal 1ms deadline is admitted grammar — and the WaitGate's
/// command-start clock is a total budget that also covers the preamble, so
/// two honest outcomes exist and which one settles first is timing. The
/// oracle therefore admits exactly those two outcomes under a JSON
/// envelope, each pinned to its real failure document instead of a loose
/// exit set: either the cohort check settles first (`check-failed`, stage
/// `cohort`, exit 1) or the gate's original clock settles first
/// (`timeout`, stage `wait`, `timeoutMs` equal to the admitted 1ms bound,
/// `completedObservations` 0 because the viewer loop never ran, exit 7).
/// Usage (2), a panic (101), a signal, a non-JSON stdout, or any other
/// failure kind still fails the witness.
fn admitted_wait_min_timeout(probe: &Probe) {
    let arguments = [
        "terminal",
        "wait-output",
        TERMINAL_ID,
        "--match",
        "ready",
        "--timeout",
        "1",
        "--format",
        "json",
    ];
    let output = run(probe, &arguments);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("minimal-timeout failure document is JSON");
    match output.status.code() {
        Some(1) => {
            assert_eq!(document["failure"]["kind"], "check-failed", "{arguments:?}");
            assert_eq!(
                document["failure"]["details"]["stage"], "cohort",
                "{arguments:?}"
            );
        }
        Some(7) => {
            let details = &document["failure"]["details"];
            assert_eq!(document["failure"]["kind"], "timeout", "{arguments:?}");
            assert_eq!(details["stage"], "wait", "{arguments:?}");
            assert_eq!(details["timeoutMs"], 1, "{arguments:?}");
            assert_eq!(
                details["observed"]["completedObservations"], 0,
                "{arguments:?}"
            );
        }
        other => panic!(
            "{arguments:?} must exit 1 (cohort check) or 7 (wait clock); \
             got {other:?}; stderr: {stderr}"
        ),
    }
}

#[test]
fn terminal_wait_output_usage_failures() -> TestResult {
    let probe = probe();
    let match_over_bound = "m".repeat(4097);
    let regex_over_bound = "r".repeat(1025);
    for arguments in [
        ["terminal", "wait-output"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--match"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--match", "--"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--match", ""].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            match_over_bound.as_str(),
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "a",
            "--match",
            "b",
        ]
        .as_slice(),
        // The two matchers are mutually exclusive — in either order — and
        // `--regex` carries its own bound, compile, and duplicate rules.
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--regex",
            "y",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--regex",
            "y",
            "--match",
            "x",
        ]
        .as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--regex"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--regex", "--"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--regex", ""].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--regex",
            regex_over_bound.as_str(),
        ]
        .as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--regex", "("].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--regex",
            "a",
            "--regex",
            "b",
        ]
        .as_slice(),
        // `--rows` shares the retained-row bound.
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--rows",
            "0",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--rows",
            "1001",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--rows",
            "abc",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--rows",
        ]
        .as_slice(),
        // `--timeout` is a bounded millisecond count.
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--timeout",
            "0",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--timeout",
            "3600001",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--timeout",
            "abc",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--timeout",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--timeout",
            "99999999999",
        ]
        .as_slice(),
        // wait-output never admits send's payload flags, read's --history,
        // or read's presentation flags.
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--unwrap",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--lines",
            "5",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--text",
            "y",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--key",
            "enter",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--enter",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--history",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--path",
            "/x",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--standalone",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--confirm",
        ]
        .as_slice(),
        ["terminal", "wait-output", "my-label", "--match", "x"].as_slice(),
        ["terminal", "wait-output", ZERO_ID, "--match", "x"].as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "extra",
            "--match",
            "x",
        ]
        .as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--history=x"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--match", "x", "-"].as_slice(),
    ] {
        usage(&probe, arguments);
    }
    usage_json(
        &probe,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--frobnicate",
            "--format",
            "json",
        ],
    );
    Ok(())
}

// ---- terminal read ---------------------------------------------------------

/// The read grammar admits
/// `terminal read <terminal-id> [--history --rows <n>] [--unwrap]
/// [--lines <n>] [--format text|json]`: `--history` and `--rows` stay a
/// required pair with `--rows` 1..=1000, and the two presentation options
/// compose with either source — `--unwrap` joins verified soft-wrap chains
/// into logical lines, `--lines` keeps the last 1..=65535 emitted entries.
/// Neither widens the fetched window.
#[test]
fn terminal_read_admitted_forms() -> TestResult {
    let probe = probe();
    for arguments in [
        ["terminal", "read", TERMINAL_ID].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--unwrap"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--lines", "1"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--lines", "65535"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--unwrap", "--lines", "20"].as_slice(),
        // The canonical grammar declares `--name value` and `--name=value`
        // synonymous: the joined form is admitted with the same bound.
        ["terminal", "read", TERMINAL_ID, "--lines=5"].as_slice(),
        [
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "100",
            "--unwrap",
            "--lines",
            "10",
        ]
        .as_slice(),
        ["terminal", "read", "--lines", "5", TERMINAL_ID].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--unwrap", "--"].as_slice(),
    ] {
        admitted(&probe, arguments);
    }
    admitted_json(
        &probe,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--lines",
            "3",
            "--format",
            "json",
        ],
    );
    Ok(())
}

#[test]
fn terminal_read_usage_failures() -> TestResult {
    let probe = probe();
    for arguments in [
        // `--lines` is a bounded count, never a bare flag or a duplicate.
        ["terminal", "read", TERMINAL_ID, "--lines"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--lines", "--"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--lines", "0"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--lines", "65536"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--lines", "abc"].as_slice(),
        [
            "terminal",
            "read",
            TERMINAL_ID,
            "--lines",
            "1",
            "--lines",
            "2",
        ]
        .as_slice(),
        ["terminal", "read", TERMINAL_ID, "--unwrap", "--unwrap"].as_slice(),
        // `--rows` still requires `--history`, and the pair stays bound to
        // the retained-row limit.
        ["terminal", "read", TERMINAL_ID, "--rows", "5"].as_slice(),
        [
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "1001",
        ]
        .as_slice(),
        // Read never admits the matcher or send flags.
        ["terminal", "read", TERMINAL_ID, "--regex", "x"].as_slice(),
        [
            "terminal",
            "read",
            TERMINAL_ID,
            "--unwrap",
            "--timeout",
            "5",
        ]
        .as_slice(),
    ] {
        usage(&probe, arguments);
    }
    usage_json(
        &probe,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--lines",
            "x",
            "--format",
            "json",
        ],
    );
    Ok(())
}

// ---- workspace accept-directory --------------------------------------------

/// CLI-WORKSPACE-DIRECTORY-ACCEPTED admits
/// `workspace accept-directory <target> --path <absolute-path>
/// [--format text|json]`: the target keeps the Workspace ID-or-exact-label
/// rule, `--path` is required and must be the literal absolute POSIX path of
/// at most 4096 bytes without NUL — `~`/`$` are never expanded — and no other
/// option is admitted, `--confirm` included.
#[test]
fn workspace_accept_directory_admitted_forms() -> TestResult {
    let probe = probe();
    let path_at_bound = format!("/{}", "p".repeat(4095));
    for arguments in [
        ["workspace", "accept-directory", WORKSPACE_ID, "--path", "/"].as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
        ]
        .as_slice(),
        // Workspace targets keep the existing label form.
        [
            "workspace",
            "accept-directory",
            "my workspace",
            "--path",
            "/repo",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            "--path",
            "/repo",
            WORKSPACE_ID,
        ]
        .as_slice(),
        // Literal paths keep spaces and non-ASCII bytes verbatim.
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/dir with spaces/多",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            path_at_bound.as_str(),
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "--",
        ]
        .as_slice(),
    ] {
        admitted(&probe, arguments);
    }
    admitted_json(
        &probe,
        &[
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "--format",
            "json",
        ],
    );
    Ok(())
}

#[test]
fn workspace_accept_directory_usage_failures() -> TestResult {
    let probe = probe();
    let path_over_bound = format!("/{}", "p".repeat(4096));
    for arguments in [
        ["workspace", "accept-directory"].as_slice(),
        ["workspace", "accept-directory", WORKSPACE_ID].as_slice(),
        ["workspace", "accept-directory", WORKSPACE_ID, "--path"].as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "--",
        ]
        .as_slice(),
        // The path must be a literal absolute POSIX path of at most 4096
        // bytes: relative, tilde, and empty spellings are usage failures.
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "repo",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "~/repo",
        ]
        .as_slice(),
        ["workspace", "accept-directory", WORKSPACE_ID, "--path", ""].as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            path_over_bound.as_str(),
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/a",
            "--path",
            "/b",
        ]
        .as_slice(),
        // Only --path is admitted: --confirm and every other flag is out of
        // scope, including a second directory confirmation path.
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "--confirm",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "--label",
            "x",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "--standalone",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "--project",
            "x",
        ]
        .as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "extra",
            "--path",
            "/repo",
        ]
        .as_slice(),
        ["workspace", "accept-directory", ZERO_ID, "--path", "/repo"].as_slice(),
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path",
            "/repo",
            "-",
        ]
        .as_slice(),
    ] {
        usage(&probe, arguments);
    }
    usage_json(
        &probe,
        &[
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--frobnicate",
            "--format",
            "json",
        ],
    );
    Ok(())
}

// ---- help: zero runtime per path -------------------------------------------

/// Every admitted command path owns a help topic that answers before any
/// runtime discovery; the family controls already pass at this base.
#[test]
fn help_paths_answer_without_runtime() -> TestResult {
    let probe = probe();
    // Already-admitted family and root paths: the zero-runtime mechanism is
    // pinned green so the new-path failures below cannot be blamed on the
    // environment.
    for arguments in [
        ["help"].as_slice(),
        ["help", "terminal"].as_slice(),
        ["terminal", "--help"].as_slice(),
        ["help", "workspace"].as_slice(),
        ["workspace", "-h"].as_slice(),
        ["help", "terminal", "read"].as_slice(),
        ["help", "workspace", "create"].as_slice(),
    ] {
        help_ok(&probe, arguments);
    }
    // The composed matrix's new command paths must answer identically.
    for arguments in [
        ["help", "terminal", "send"].as_slice(),
        ["terminal", "send", "--help"].as_slice(),
        ["terminal", "send", "-h"].as_slice(),
        ["help", "terminal", "wait-output"].as_slice(),
        ["terminal", "wait-output", "--help"].as_slice(),
        ["help", "workspace", "accept-directory"].as_slice(),
        ["workspace", "accept-directory", "--help"].as_slice(),
    ] {
        help_ok(&probe, arguments);
    }
    Ok(())
}

// ---- shared option grammar -------------------------------------------------

/// The `--` terminator, `--format` envelope, value ownership, and flag
/// spellings follow the native parser's existing rules — pinned on commands
/// that already exist so the same rules are witnessed green before the new
/// commands land.
#[test]
fn shared_option_rules_on_existing_commands() -> TestResult {
    let probe = probe();
    // `--` ends option scanning; a post-terminator `--format` is a positional.
    usage(&probe, &["status", "--", "--format", "json"]);
    // The format envelope is global and silent on failure: missing, invalid,
    // or duplicated selections anywhere before `--` exit 2 with empty stdout.
    usage_silent(&probe, &["status", "--format"]);
    usage_silent(&probe, &["status", "--format", "bogus"]);
    usage_silent(&probe, &["status", "--format", "json", "--format", "text"]);
    usage_silent(&probe, &["status", "--format=json", "--format=json"]);
    usage_silent(&probe, &["status", "--format", "--"]);
    // Value flags own the next token but never `--`.
    usage(&probe, &["project", "create", "--label"]);
    usage(&probe, &["project", "create", "--label", "--"]);
    // Joined `--flag=value` is admitted for declared value options; a
    // boolean flag still never takes a value.
    usage(
        &probe,
        &["workspace", "rename", "w", "--label", "x", "--confirm=true"],
    );
    // `--version` is the sole root invocation.
    let output = run(&probe, &["--version"]);
    assert_eq!(output.status.code(), Some(0));
    usage(&probe, &["--version", "--format", "json"]);
    usage(&probe, &["status", "--version"]);
    // A business flag on a query path is a usage failure.
    usage(&probe, &["status", "--text", "x"]);
    usage(&probe, &["status", "--confirm"]);
    usage(&probe, &["project", "list", "--path", "/x"]);
    usage(&probe, &["terminal", "read", TERMINAL_ID, "--text", "x"]);
    usage(&probe, &["terminal", "read", TERMINAL_ID, "--match", "x"]);
    usage(&probe, &["terminal", "read", TERMINAL_ID, "--key", "enter"]);
    usage(&probe, &["--text", "x"]);
    // The same envelope rule applies inside the new commands' scope: a valid
    // unique json selection shapes the grammar failure.
    usage_json(&probe, &["status", "--frobnicate", "--format", "json"]);
    // A trailing `--` on a complete argv changes nothing: `status` is formed
    // at this base, so admission is proven by the same cohort-check outcome.
    admitted(&probe, &["status", "--"]);
    Ok(())
}

/// Cross-command leakage is rejected: options declared for one composed
/// command are usage failures everywhere else, and unknown help paths stay
/// grammar failures rather than runtime lookups.
#[test]
fn cross_command_scope_and_help_boundaries() -> TestResult {
    let probe = probe();
    for arguments in [
        ["terminal", "read", TERMINAL_ID, "--enter"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--timeout", "5"].as_slice(),
        ["terminal", "read", TERMINAL_ID, "--path", "/x"].as_slice(),
        ["project", "create", "--text", "x", "--label", "x"].as_slice(),
        [
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "x",
            "--key",
            "enter",
        ]
        .as_slice(),
        ["workspace", "list", "--match", "x"].as_slice(),
        // Read's presentation flags and wait-output's matcher stay scoped:
        // they are usage failures on send and on unrelated commands.
        ["terminal", "send", TERMINAL_ID, "--text", "x", "--unwrap"].as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "x",
            "--lines",
            "5",
        ]
        .as_slice(),
        [
            "terminal",
            "send",
            TERMINAL_ID,
            "--text",
            "x",
            "--regex",
            "y",
        ]
        .as_slice(),
        ["status", "--unwrap"].as_slice(),
        ["status", "--lines", "5"].as_slice(),
        ["status", "--regex", "x"].as_slice(),
        ["tab", "list", "--standalone", "--path", "/x"].as_slice(),
        ["help", "terminal", "frobnicate"].as_slice(),
        ["terminal", "frobnicate", "--help"].as_slice(),
        // A supplied operand is not a help path.
        ["terminal", "send", TERMINAL_ID, "--help"].as_slice(),
        ["help", "terminal", "send", TERMINAL_ID].as_slice(),
    ] {
        usage(&probe, arguments);
    }
    // `help` never selects a format; the rejected selection keeps the
    // requested envelope.
    usage_json(&probe, &["help", "terminal", "--format", "json"]);
    Ok(())
}

// ---- project rename ----------------------------------------------------------

/// `project rename <target> --label <label>` rides the shared mutation
/// grammar: one operand under the project target rules, one required
/// single-use label, the shared `--`/`--format` envelope rules, and no
/// `--confirm` — a rename is not destructive.
#[test]
fn project_rename_admitted_forms() -> TestResult {
    let probe = probe();
    for arguments in [
        ["project", "rename", PROJECT_ID, "--label", "new"].as_slice(),
        // Every non-ID token is a legal label candidate — uppercase hex,
        // wrong lengths, and non-hex 32-char strings included; only empty,
        // overlong, or zero-shaped operands are identifier failures.
        ["project", "rename", "web", "--label", "new"].as_slice(),
        ["project", "rename", "ABCDEF", "--label", "new"].as_slice(),
        [
            "project",
            "rename",
            "0123456789ABCDEF0123456789ABCDEF",
            "--label",
            "new",
        ]
        .as_slice(),
        [
            "project",
            "rename",
            "0000000000000000000000000000000g",
            "--label",
            "new",
        ]
        .as_slice(),
        // Option before the operand is legal under the shared scan order.
        ["project", "rename", "--label", "new", PROJECT_ID].as_slice(),
        // `--` shields a flag-spelling operand: `--help` is a literal label
        // target here, never the help suffix.
        ["project", "rename", "--label", "new", "--", "--help"].as_slice(),
    ] {
        admitted(&probe, arguments);
    }
    // A valid json selection before `--` keeps the envelope through the
    // post-parse stage; the post-terminator `--format=json` is the literal
    // label target, not a second format selection.
    admitted_json(
        &probe,
        &[
            "project",
            "rename",
            "--label",
            "new",
            "--format=json",
            "--",
            "--format=json",
        ],
    );
    admitted_json(
        &probe,
        &[
            "project", "rename", PROJECT_ID, "--label", "new", "--format", "json",
        ],
    );
    Ok(())
}

#[test]
fn project_rename_usage_failures() -> TestResult {
    let probe = probe();
    for arguments in [
        ["project", "rename"].as_slice(),
        ["project", "rename", PROJECT_ID].as_slice(),
        ["project", "rename", PROJECT_ID, PROJECT_ID, "--label", "x"].as_slice(),
        [
            "project", "rename", PROJECT_ID, "--label", "x", "--label", "y",
        ]
        .as_slice(),
        // `--confirm` belongs to the close commands alone.
        ["project", "rename", PROJECT_ID, "--label", "x", "--confirm"].as_slice(),
        ["project", "rename", PROJECT_ID, "--label", "x", "extra"].as_slice(),
        // After `--`, `--label` is a positional: the required option is
        // absent and the extra positional is unexpected either way.
        ["project", "rename", "--", "--label"].as_slice(),
        ["project", "rename", "--", "--label", "x"].as_slice(),
    ] {
        usage(&probe, arguments);
    }
    // A valid json selection anywhere before `--` shapes the failure.
    usage_json(&probe, &["project", "rename", "--format", "json"]);
    usage_json(
        &probe,
        &["project", "rename", PROJECT_ID, "--format=json", "--bogus"],
    );
    // Identifier failures exit 2 under the same envelope but name their own
    // kind: empty/overlong/zero-shaped targets and empty/overlong labels.
    let overlong = "x".repeat(513);
    for arguments in [
        ["project", "rename", "", "--label", "x"].as_slice(),
        ["project", "rename", overlong.as_str(), "--label", "x"].as_slice(),
        ["project", "rename", ZERO_ID, "--label", "x"].as_slice(),
        ["project", "rename", PROJECT_ID, "--label", ""].as_slice(),
        [
            "project",
            "rename",
            PROJECT_ID,
            "--label",
            overlong.as_str(),
        ]
        .as_slice(),
    ] {
        usage(&probe, arguments);
    }
    usage_json_kind(
        &probe,
        &[
            "project", "rename", ZERO_ID, "--label", "x", "--format", "json",
        ],
        "invalid-identifier",
    );
    Ok(())
}

#[test]
fn project_rename_help_paths() -> TestResult {
    let probe = probe();
    for arguments in [
        ["help", "project"].as_slice(),
        ["help", "project", "rename"].as_slice(),
        ["project", "rename", "--help"].as_slice(),
        ["project", "rename", "-h"].as_slice(),
    ] {
        help_ok(&probe, arguments);
    }
    Ok(())
}
