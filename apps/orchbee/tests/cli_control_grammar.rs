//! Composed CLI-first control grammar witness: the real `orchbee` binary is
//! probed with an explicitly unavailable runtime so every outcome is decided
//! by argv grammar alone. The matrix pins the FINAL-composed bounded command
//! grammar — `terminal send`, `terminal wait-output`, and
//! `workspace accept-directory` — against the shared option rules the native
//! parser already enforces.
//!
//! Environment discipline: the unadorned build-tree binary carries no
//! `cohort.json`, and `ORCHBEE_RUNTIME_BASE` points at a freshly minted empty
//! directory with no Site inside. Any argv the grammar admits must therefore
//! fail at cohort or Site discovery — a `2` exit can only come from the
//! parser. New-command admission and per-path help are expected to fail at
//! this base (`f468be0e` does not yet know the commands); negative controls
//! already pass because unknown words are grammar failures too.
#![cfg(target_os = "linux")]

use std::{
    path::{Path, PathBuf},
    process::Output,
};

mod support;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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

/// Spawn the unadorned binary under the unavailable runtime with the shared
/// deterministic locale and wall-clock backstop.
fn run(base: &Path, arguments: &[&str]) -> Output {
    support::run(Path::new(env!("CARGO_BIN_EXE_orchbee")), base, arguments)
}

/// The grammar admitted the argv: whatever follows (cohort, Site, or service
/// discovery under the unavailable runtime) is not a usage rejection.
fn admitted(base: &Path, arguments: &[&str]) {
    let output = run(base, arguments);
    assert_ne!(
        output.status.code(),
        Some(2),
        "grammar must admit {arguments:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A grammar failure is classified before any runtime discovery: the only
/// observable contract is the public usage exit.
fn usage(base: &Path, arguments: &[&str]) {
    let output = run(base, arguments);
    assert_eq!(
        output.status.code(),
        Some(2),
        "grammar must reject {arguments:?} before runtime; stdout: {:?}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Format-envelope and argv-envelope failures keep stdout empty.
fn usage_silent(base: &Path, arguments: &[&str]) {
    let output = run(base, arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    assert!(
        output.stdout.is_empty(),
        "{arguments:?} must not write a stdout document; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A grammar failure after a unique valid `--format json` keeps the selected
/// envelope: a JSON failure document whose kind is `invalid-usage`.
fn usage_json(base: &Path, arguments: &[&str]) {
    let output = run(base, arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("json-envelope failure is JSON");
    assert_eq!(
        document["failure"]["kind"], "invalid-usage",
        "{arguments:?}"
    );
}

/// Per-path help answers with zero runtime: exit 0 and a non-empty body even
/// though no cohort or Site exists.
fn help_ok(base: &Path, arguments: &[&str]) {
    let output = run(base, arguments);
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
    let base = base();
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
            "--key",
            "ctrl-c",
            "--format",
            "json",
        ]
        .as_slice(),
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
        admitted(&base, arguments);
    }
    // Every member of the closed key vocabulary is individually legal.
    for key in SEND_KEYS {
        admitted(&base, &["terminal", "send", TERMINAL_ID, "--key", key]);
    }
    Ok(())
}

#[test]
fn terminal_send_usage_failures() -> TestResult {
    let base = base();
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
        // Joined `--flag=value` exists only for `--format`.
        ["terminal", "send", TERMINAL_ID, "--text=x"].as_slice(),
        ["terminal", "send", TERMINAL_ID, "--key=enter"].as_slice(),
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
        usage(&base, arguments);
    }
    // A selected `--format json` survives grammar failures as the envelope.
    usage_json(
        &base,
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
/// `terminal wait-output <terminal-id> --match <text> [--rows <n>]
/// [--timeout <ms>] [--format text|json]` with `--match` required, non-empty,
/// and at most 4096 bytes; `--rows` 1..=1000 (default 1000); `--timeout`
/// 1..=3600000 (default 30000).
#[test]
fn terminal_wait_output_admitted_forms() -> TestResult {
    let base = base();
    let match_at_bound = "m".repeat(4096);
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
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--timeout",
            "1",
        ]
        .as_slice(),
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
            "--format",
            "json",
        ]
        .as_slice(),
        [
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "ready",
            "--",
        ]
        .as_slice(),
    ] {
        admitted(&base, arguments);
    }
    Ok(())
}

#[test]
fn terminal_wait_output_usage_failures() -> TestResult {
    let base = base();
    let match_over_bound = "m".repeat(4097);
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
        // wait-output never admits send's payload flags or read's --history.
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
        ["terminal", "wait-output", TERMINAL_ID, "--match=x"].as_slice(),
        ["terminal", "wait-output", TERMINAL_ID, "--match", "x", "-"].as_slice(),
    ] {
        usage(&base, arguments);
    }
    usage_json(
        &base,
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

// ---- workspace accept-directory --------------------------------------------

/// CLI-WORKSPACE-DIRECTORY-ACCEPTED admits
/// `workspace accept-directory <target> --path <absolute-path>
/// [--format text|json]`: the target keeps the Workspace ID-or-exact-label
/// rule, `--path` is required and must be the literal absolute POSIX path of
/// at most 4096 bytes without NUL — `~`/`$` are never expanded — and no other
/// option is admitted, `--confirm` included.
#[test]
fn workspace_accept_directory_admitted_forms() -> TestResult {
    let base = base();
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
            "--format",
            "json",
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
        admitted(&base, arguments);
    }
    Ok(())
}

#[test]
fn workspace_accept_directory_usage_failures() -> TestResult {
    let base = base();
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
        [
            "workspace",
            "accept-directory",
            WORKSPACE_ID,
            "--path=/repo",
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
        usage(&base, arguments);
    }
    usage_json(
        &base,
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
    let base = base();
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
        help_ok(&base, arguments);
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
        help_ok(&base, arguments);
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
    let base = base();
    // `--` ends option scanning; a post-terminator `--format` is a positional.
    usage(&base, &["status", "--", "--format", "json"]);
    // The format envelope is global and silent on failure: missing, invalid,
    // or duplicated selections anywhere before `--` exit 2 with empty stdout.
    usage_silent(&base, &["status", "--format"]);
    usage_silent(&base, &["status", "--format", "bogus"]);
    usage_silent(&base, &["status", "--format", "json", "--format", "text"]);
    usage_silent(&base, &["status", "--format=json", "--format=json"]);
    usage_silent(&base, &["status", "--format", "--"]);
    // Value flags own the next token but never `--`.
    usage(&base, &["project", "create", "--label"]);
    usage(&base, &["project", "create", "--label", "--"]);
    // Joined `--flag=value` exists only for `--format`.
    usage(
        &base,
        &["workspace", "create", "--project", "p", "--label=x"],
    );
    usage(&base, &["workspace", "list", "--project=p"]);
    // `--version` is the sole root invocation.
    let output = run(&base, &["--version"]);
    assert_eq!(output.status.code(), Some(0));
    usage(&base, &["--version", "--format", "json"]);
    usage(&base, &["status", "--version"]);
    // A business flag on a query path is a usage failure.
    usage(&base, &["status", "--text", "x"]);
    usage(&base, &["status", "--confirm"]);
    usage(&base, &["project", "list", "--path", "/x"]);
    usage(&base, &["terminal", "read", TERMINAL_ID, "--text", "x"]);
    usage(&base, &["terminal", "read", TERMINAL_ID, "--match", "x"]);
    usage(&base, &["terminal", "read", TERMINAL_ID, "--key", "enter"]);
    usage(&base, &["--text", "x"]);
    // The same envelope rule applies inside the new commands' scope: a valid
    // unique json selection shapes the grammar failure.
    usage_json(&base, &["status", "--frobnicate", "--format", "json"]);
    // A trailing `--` on a complete argv changes nothing.
    let output = run(&base, &["status", "--"]);
    assert_ne!(output.status.code(), Some(2));
    Ok(())
}

/// Cross-command leakage is rejected: options declared for one composed
/// command are usage failures everywhere else, and unknown help paths stay
/// grammar failures rather than runtime lookups.
#[test]
fn cross_command_scope_and_help_boundaries() -> TestResult {
    let base = base();
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
        ["tab", "list", "--standalone", "--path", "/x"].as_slice(),
        ["help", "terminal", "frobnicate"].as_slice(),
        ["terminal", "frobnicate", "--help"].as_slice(),
        // A supplied operand is not a help path.
        ["terminal", "send", TERMINAL_ID, "--help"].as_slice(),
        ["help", "terminal", "send", TERMINAL_ID].as_slice(),
    ] {
        usage(&base, arguments);
    }
    // `help` never selects a format; the rejected selection keeps the
    // requested envelope.
    usage_json(&base, &["help", "terminal", "--format", "json"]);
    Ok(())
}
