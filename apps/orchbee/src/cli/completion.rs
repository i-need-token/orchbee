//! Static completion scripts for the implemented grammar. The POSIX shells
//! run the same state machine — a faithful projection of `invocation::scan`/
//! `resolve` — and emit only the candidates the parser could still accept.
//! The scripts are emitted to stdout only; nothing is installed, no user
//! files are read or written, and no resource or Site state is ever
//! enumerated.
//!
//! Walker state (per completed word):
//! - `state` — positional position: verb | sub | deep | after | operand | help |
//!   completion (`deep` is the third level under `agent integration`)
//! - `done` — the argv can no longer produce a legal request (invalid or
//!   terminal): no candidates
//! - `ended` — `--` was seen: option termination, and `--` is never consumed
//!   as a flag value (checked before value ownership)
//! - `value` — a declared flag owns the next word: `format` | `mouse` |
//!   `direction` | `key` (`terminal send --key`) | `filter` (every other
//!   value-owning flag)
//! - `seen_*` — one consumed bit per `--format`, per `--mouse`, and per
//!   business flag; duplicates are usage except for the grammar's
//!   declared repeatable flags
//! - `seen_flags` — the consumed business flags as a list, so a command's
//!   admission check is its own declared scope instead of an enumeration
//!   of every flag it rejects
//! - `seen_provider2` — a second `--provider` stands: legal only toward
//!   `agent doctor`, so it retires `agent integration` from the viable
//!   words and is a dead prefix once that family is committed
//!
//! Walk precedence per completed word (mirrors `format_envelope` + `scan`):
//! 1. `--` → ended, always (the parser never takes it as a value)
//! 2. done | ended → ignore (nothing more is legal)
//! 3. value set → the word is the flag's value; a `--format` value outside
//!    {text,json} or a `--mouse` value outside {off,on} makes the parse fail
//! 4. `--format*` → legal only where options may still appear
//!    (verb/sub/after/operand, leaf `status`); a second selector or an
//!    invalid value is usage
//! 5. `--mouse`/`--mouse=off|on` → the `tui`-scoped option: legal at verb
//!    (only `tui` stays viable afterward) or on the `tui` leaf; a second
//!    `--mouse` or an invalid value — joined or separate — is usage
//! 6. business flags → legal iff a command reachable here admits the flag:
//!    verb records it (checked when the command lands), sub requires the
//!    flag's owning family, deep admits the `agent integration` union scope,
//!    after/operand require membership in the resolved command's flag scope
//!    (`--workspace`/`--standalone` stay exclusive); operand-taking paths
//!    consume one positional first; leaf/help/completion reject all options
//!    - joined `--flag=value` → the flag part drives the same admission and
//!      bookkeeping as the separate spelling for the grammar's declared
//!      value flags; the inline value never claims the next word, and a
//!      boolean or unknown name in that shape is usage
//! 7. `--help`/`--version` → terminal flags: nothing may follow → done
//! 8. unknown `-x` → usage → done
//! 9. other word → positional dispatch:
//!    - verb: family→sub | `status`→leaf (fails if a business flag or
//!      `--mouse` was seen) | `tui`→leaf (fails if a business flag or
//!      `--format` was seen) | `help`→help / `completion`→completion (both
//!      fail if any option was seen); families fail if `--mouse` was seen
//!    - sub: family:verb → after, or `agent:integration` → deep; the verb's
//!      declared flag scope must cover everything already seen
//!    - deep: `agent:integration:<verb>` → after under the same union scope
//!    - after: operand-taking paths → operand; anything else → done
//!    - operand/leaf/completion → done
//!    - help: HELP_TOPICS / HELP_CHILDREN / HELP_LEVEL3 walk
//!
//! Candidate precedence at the cursor (same order as the parser's legality):
//! - dead consumed prefix → nothing: when the already-typed words leave no
//!   legal root word (verb) or no legal verb (sub), every candidate is
//!   suppressed before the pending-value, `--format=` override, and offer
//!   arms below — `--workspace x --standalone` and `--project x terminal`
//!   complete nothing
//! - done | ended → nothing
//! - value == format → text json
//! - value == mouse → off on
//! - value == direction → the resolved command's direction words
//! - value == key → the closed `terminal send --key` vocabulary
//! - value == provider → the closed `agent integration` provider vocabulary
//! - value == filter → nothing (no resource enumeration)
//! - cur == `--format=*` → `--format=text`/`--format=json` only where a
//!   format option is legal and none was consumed — never after
//!   done/ended/invalid/help/completion/tui or inside a pending flag value
//! - cur == `--enum=<prefix>` → that flag's own closed vocabulary with the
//!   `--flag=` spelling preserved, only where the flag could still be
//!   admitted (`--mouse=` under a viable `tui`; `--direction=` once the
//!   command resolves; `--key=`/`--until=`/`--provider=` inside their
//!   owning scopes); a freeform flag's value is never guessed
//! - verb → root words still viable under the seen business flags + format
//!   flags + `--help`/`--version` only when nothing was consumed
//! - sub → verbs whose flag scope covers the seen flags + format flags
//! - deep → `agent integration` verbs while the seen flags stay inside the
//!   union scope + format flags
//! - leaf → `status`: format flags + `--help` when nothing was seen; `tui`:
//!   `--help` and `--mouse` only while `--mouse` is unconsumed — a consumed
//!   `--mouse` makes every further token usage
//! - after → format flags + the command's unused business flags + `--help`
//!   on every command path with no option seen; the operand-taking `get`/
//!   `api schema`/mutation paths take a legal suffix help before their
//!   operand
//! - operand → format flags + the command's unused business flags
//! - help → help topics / children
//! - completion → shells + `--help`

mod powershell;

use crate::invocation::CompletionShell;

/// Top-level words the grammar accepts before any option.
const ROOT_WORDS: &[&str] = &[
    "agent",
    "api",
    "completion",
    "config",
    "help",
    "panel",
    "project",
    "site",
    "status",
    "tab",
    "terminal",
    "tui",
    "workspace",
    "worktree",
    "launch-profile",
];

/// Root words that still parse when a `--format` selection was consumed:
/// every data command family accepts it; `help`/`completion`/`tui`/`--version`
/// reject it.
const FORMAT_ROOT_WORDS: &[&str] = &[
    "agent",
    "api",
    "config",
    "panel",
    "project",
    "site",
    "status",
    "tab",
    "terminal",
    "workspace",
    "worktree",
    "launch-profile",
];

/// family → the verbs it implements.
const FAMILY_VERBS: &[(&str, &[&str])] = &[
    (
        "agent",
        &[
            "doctor",
            "explain",
            "get",
            "list",
            "names",
            "prompt",
            "read",
            "rename",
            "resolve",
            "send-keys",
            "start",
            "wait",
        ],
    ),
    ("api", &["schema", "snapshot"]),
    ("launch-profile", &["create", "delete", "get", "list"]),
    ("config", &["check"]),
    (
        "panel",
        &[
            "close",
            "current",
            "get",
            "list",
            "move",
            "process-info",
            "split",
        ],
    ),
    ("project", &["create", "get", "list", "rename"]),
    ("site", &["diagnose", "start", "stop", "stop-status"]),
    ("tab", &["close", "create", "get", "list", "move", "rename"]),
    ("terminal", &["get", "list", "read", "send", "wait-output"]),
    (
        "workspace",
        &[
            "accept-directory",
            "close",
            "close-resume",
            "close-status",
            "create",
            "get",
            "list",
            "rename",
        ],
    ),
    ("worktree", &["list"]),
];

/// Every business flag the grammar admits (`--format` is orthogonal), in the
/// order the state machine records them.
const BUSINESS_FLAGS: &[&str] = &[
    "--request-id",
    "--retry",
    "--detach",
    "--kind",
    "--project",
    "--workspace",
    "--tab",
    "--standalone",
    "--label",
    "--direction",
    "--cols",
    "--rows",
    "--cell-width",
    "--cell-height",
    "--confirm",
    "--panel",
    "--current",
    "--target-panel",
    "--into-tab",
    "--new-tab",
    "--history",
    "--path",
    "--match",
    "--regex",
    "--unwrap",
    "--lines",
    "--timeout",
    "--text",
    "--enter",
    "--key",
    "--file",
    "--stdin",
    "--paste",
    "--directory",
    "--exec",
    "--cwd",
    "--env",
    "--arg",
    "--include-ended",
    "--limit",
    "--cursor",
    "--until",
    "--after-revision",
    "--provider",
    "--asset-root",
    "--profile",
    "--name",
    "--session",
    "--panel",
    "--clear",
    "--wait",
    "--forever",
    "--adapter-source",
    "--request",
    "--generation",
];

/// The business flags every `agent integration <verb>` admits — also the
/// union scope checked when the `agent integration` sub-family itself lands.
const INTEGRATION_COMMAND_FLAGS: &[&str] = &["--provider", "--adapter-source"];

/// `agent integration`'s third-level verbs, offered at the `deep` state.
const INTEGRATION_VERBS: &[&str] = &["disable", "enable", "explain", "list"];

/// Business flags whose seen bit never makes a repeat `done` — they stay
/// offerable and keep consuming values while remaining scope-tracked.
const REPEATABLE_FLAGS: &[&str] = &[
    "--env",
    "--arg",
    "--asset-root",
    "--provider",
    "--until",
    "--key",
];

/// `<family>:<verb>` (or `agent:integration:<verb>`) → the business flags
/// that command admits. Commands not listed here admit none.
const COMMAND_FLAGS: &[(&str, &[&str])] = &[
    (
        "site:stop",
        &[
            "--confirm",
            "--request-id",
            "--retry",
            "--detach",
            "--timeout",
        ],
    ),
    (
        "agent:start",
        &["--kind", "--panel", "--current", "--timeout", "--arg"],
    ),
    ("agent:list", &["--include-ended", "--limit", "--cursor"]),
    ("agent:get", &["--session", "--name", "--panel"]),
    (
        "agent:read",
        &[
            "--session",
            "--panel",
            "--name",
            "--history",
            "--rows",
            "--unwrap",
            "--lines",
        ],
    ),
    ("agent:send-keys", &["--session", "--name", "--panel"]),
    (
        "agent:prompt",
        &[
            "--session",
            "--panel",
            "--name",
            "--wait",
            "--until",
            "--timeout",
            "--forever",
        ],
    ),
    ("agent:explain", &["--session", "--name", "--panel"]),
    ("agent:resolve", &["--session", "--name", "--panel"]),
    (
        "agent:rename",
        &["--session", "--name", "--panel", "--clear"],
    ),
    ("agent:doctor", &["--provider", "--asset-root"]),
    (
        "agent:wait",
        &[
            "--session",
            "--panel",
            "--name",
            "--until",
            "--after-revision",
            "--forever",
            "--timeout",
        ],
    ),
    ("agent:integration:list", INTEGRATION_COMMAND_FLAGS),
    ("agent:integration:explain", INTEGRATION_COMMAND_FLAGS),
    ("agent:integration:enable", INTEGRATION_COMMAND_FLAGS),
    ("agent:integration:disable", INTEGRATION_COMMAND_FLAGS),
    ("project:create", &["--label"]),
    ("project:rename", &["--label"]),
    ("workspace:accept-directory", &["--path"]),
    ("workspace:list", &["--project"]),
    (
        "workspace:create",
        &[
            "--project",
            "--label",
            "--cwd",
            "--env",
            "--exec",
            "--arg",
            "--profile",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
        ],
    ),
    ("workspace:rename", &["--label"]),
    ("workspace:close", &["--confirm"]),
    ("workspace:close-status", &["--project", "--workspace"]),
    (
        "workspace:close-resume",
        &[
            "--project",
            "--workspace",
            "--request",
            "--generation",
            "--confirm",
        ],
    ),
    ("tab:list", &["--workspace", "--standalone"]),
    (
        "tab:create",
        &[
            "--workspace",
            "--standalone",
            "--label",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
            "--exec",
            "--cwd",
            "--env",
            "--arg",
            "--profile",
        ],
    ),
    ("tab:rename", &["--label"]),
    ("tab:move", &["--direction"]),
    ("tab:close", &["--confirm"]),
    ("panel:current", &["--panel", "--current"]),
    ("panel:process-info", &["--panel", "--current"]),
    ("panel:list", &["--tab"]),
    (
        "panel:split",
        &[
            "--direction",
            "--label",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
            "--exec",
            "--cwd",
            "--env",
            "--arg",
            "--profile",
        ],
    ),
    ("panel:close", &["--confirm"]),
    (
        "panel:move",
        &[
            "--target-panel",
            "--into-tab",
            "--new-tab",
            "--direction",
            "--label",
        ],
    ),
    (
        "terminal:read",
        &["--history", "--lines", "--rows", "--unwrap"],
    ),
    (
        "terminal:send",
        &["--text", "--enter", "--key", "--file", "--stdin", "--paste"],
    ),
    (
        "terminal:wait-output",
        &["--match", "--regex", "--rows", "--timeout"],
    ),
    ("worktree:list", &["--project", "--directory"]),
    ("launch-profile:create", &["--name", "--exec", "--arg"]),
];

/// `<family>:<verb>` → the words its `--direction` flag accepts. Only
/// direction-admitting commands appear.
const DIRECTION_VALUES: &[(&str, &[&str])] = &[
    ("tab:move", &["first", "last", "next", "previous"]),
    ("panel:split", &["above", "below", "left", "right"]),
    ("panel:move", &["above", "below", "left", "right"]),
];

/// The closed semantic-key vocabulary `terminal send --key` accepts — the
/// same names `invocation::TerminalSendKey` parses.
const KEY_VALUES: &[&str] = &[
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

/// The fourteen `agent wait --until` condition names — the same
/// vocabulary `invocation::AgentWaitCondition` parses.
const UNTIL_VALUES: &[&str] = &[
    "blocked",
    "done",
    "needs-attention",
    "failed",
    "succeeded",
    "canceled",
    "interrupted",
    "working",
    "idle",
    "unknown",
    "ended",
    "fresh",
    "stale",
    "disconnected",
];

/// The canonical provider ids `agent doctor --provider` accepts — the same
/// closed vocabulary `invocation` validates.
const PROVIDER_VALUES: &[&str] = &[
    "claude-code",
    "codex",
    "gemini",
    "github-copilot",
    "grok",
    "cline",
    "opencode",
    "pi",
    "omp",
    "qwen-code",
];

/// `<family>:<verb>` paths that consume one positional operand.
const OPERAND_PATHS: &[&str] = &[
    "site:stop-status",
    "agent:start",
    "agent:explain",
    "agent:get",
    "agent:read",
    "agent:send-keys",
    "agent:prompt",
    "agent:wait",
    "agent:resolve",
    "agent:rename",
    "api:schema",
    "launch-profile:delete",
    "launch-profile:get",
    "panel:close",
    "panel:get",
    "panel:process-info",
    "panel:move",
    "panel:split",
    "project:get",
    "project:rename",
    "tab:close",
    "tab:get",
    "tab:move",
    "tab:rename",
    "terminal:get",
    "terminal:read",
    "terminal:send",
    "terminal:wait-output",
    "workspace:accept-directory",
    "workspace:close",
    "workspace:get",
    "workspace:rename",
];

/// Help topics that lead anywhere (`help <word>`); leaf topics accept no
/// deeper word.
const HELP_TOPICS: &[&str] = &[
    "agent",
    "api",
    "completion",
    "config",
    "panel",
    "project",
    "site",
    "status",
    "tab",
    "terminal",
    "tui",
    "workspace",
    "worktree",
    "launch-profile",
];

/// `help <topic>` → the deeper words that complete a help path.
const HELP_CHILDREN: &[(&str, &[&str])] = &[
    (
        "agent",
        &[
            "doctor",
            "explain",
            "get",
            "integration",
            "list",
            "names",
            "rename",
            "resolve",
            "wait",
        ],
    ),
    ("api", &["schema", "snapshot"]),
    ("launch-profile", &["create", "delete", "get", "list"]),
    ("config", &["check"]),
    (
        "panel",
        &[
            "close",
            "current",
            "get",
            "list",
            "move",
            "process-info",
            "split",
        ],
    ),
    ("project", &["create", "get", "list", "rename"]),
    ("site", &["diagnose", "start", "stop", "stop-status"]),
    ("tab", &["close", "create", "get", "list", "move", "rename"]),
    ("terminal", &["get", "list", "read", "send", "wait-output"]),
    (
        "workspace",
        &[
            "accept-directory",
            "close",
            "close-resume",
            "close-status",
            "create",
            "get",
            "list",
            "rename",
        ],
    ),
    ("worktree", &["list"]),
];

/// `help <topic> <child>` → third-level words. Only `agent integration`
/// reaches a third help level.
const HELP_LEVEL3: &[(&str, &[&str])] = &[("agent:integration", INTEGRATION_VERBS)];

const SHELLS: &[&str] = &["bash", "fish", "powershell", "zsh"];
const FORMAT_VALUES: &[&str] = &["text", "json"];

pub(crate) fn render(shell: CompletionShell) -> String {
    match shell {
        CompletionShell::Bash => bash(),
        CompletionShell::Zsh => zsh(),
        CompletionShell::Fish => fish(),
        CompletionShell::PowerShell => powershell::render(),
    }
}

/// `flag` → the walker's one-shot seen bit for it.
fn flag_bit(flag: &str) -> &'static str {
    match flag {
        "--kind" => "seen_kind",
        "--project" => "seen_project",
        "--panel" => "seen_panel",
        "--current" => "seen_current",
        "--workspace" => "seen_workspace",
        "--tab" => "seen_tab",
        "--standalone" => "seen_standalone",
        "--label" => "seen_label",
        "--direction" => "seen_direction",
        "--cols" => "seen_cols",
        "--rows" => "seen_rows",
        "--cell-width" => "seen_cell_width",
        "--cell-height" => "seen_cell_height",
        "--confirm" => "seen_confirm",
        "--target-panel" => "seen_target_panel",
        "--into-tab" => "seen_into_tab",
        "--new-tab" => "seen_new_tab",
        "--history" => "seen_history",
        "--path" => "seen_path",
        "--match" => "seen_match",
        "--regex" => "seen_regex",
        "--unwrap" => "seen_unwrap",
        "--lines" => "seen_lines",
        "--timeout" => "seen_timeout",
        "--text" => "seen_text",
        "--enter" => "seen_enter",
        "--key" => "seen_key",
        "--file" => "seen_file",
        "--stdin" => "seen_stdin",
        "--paste" => "seen_paste",
        "--directory" => "seen_directory",
        "--exec" => "seen_exec",
        "--cwd" => "seen_cwd",
        "--env" => "seen_env",
        "--arg" => "seen_arg",
        "--include-ended" => "seen_include_ended",
        "--limit" => "seen_limit",
        "--cursor" => "seen_cursor",
        "--until" => "seen_until",
        "--after-revision" => "seen_after_revision",
        "--provider" => "seen_provider",
        "--asset-root" => "seen_asset_root",
        "--profile" => "seen_profile",
        "--name" => "seen_name",
        "--session" => "seen_session",
        "--clear" => "seen_clear",
        "--wait" => "seen_prompt_wait",
        "--forever" => "seen_forever",
        "--adapter-source" => "seen_adapter_source",
        "--request" => "seen_request",
        "--request-id" => "seen_request_id",
        "--retry" => "seen_retry",
        "--detach" => "seen_detach",
        "--generation" => "seen_generation",
        other => unreachable!("flag {other} lacks a seen bit"),
    }
}

/// `flag` → the value kind it leaves pending: `direction` candidates are
/// per-command, `provider`/`key` have closed vocabularies; every other
/// owned value stays opaque (`filter`).
fn flag_value(flag: &str) -> Option<&'static str> {
    match flag {
        "--direction" => Some("direction"),
        "--key" => Some("key"),
        "--until" => Some("until"),
        "--provider" => Some("provider"),
        "--standalone" | "--confirm" | "--new-tab" | "--history" | "--enter" | "--stdin"
        | "--include-ended" | "--unwrap" | "--paste" | "--current" | "--clear" | "--wait"
        | "--forever" | "--retry" | "--detach" => None,
        _ => Some("filter"),
    }
}

/// `<family>:<verb>` → the business flags it admits (`--format` excluded).
fn command_flags(command: &str) -> &'static [&'static str] {
    COMMAND_FLAGS
        .iter()
        .find(|(path, _)| *path == command)
        .map(|(_, flags)| *flags)
        .unwrap_or(&[])
}

/// `flag` → the families owning it, in `FAMILY_VERBS` order. A flag belongs
/// to a family when any command under it — including the third-level
/// `agent integration` paths — declares it.
fn families_of(flag: &str) -> Vec<&'static str> {
    FAMILY_VERBS
        .iter()
        .map(|(family, _)| *family)
        .filter(|family| {
            COMMAND_FLAGS.iter().any(|(command, flags)| {
                flags.contains(&flag) && command.split(':').next() == Some(*family)
            })
        })
        .collect()
}

/// bash/zsh: the seen-bits outside the `agent integration` union scope that
/// could possibly stand when `integration` or one of its verbs lands —
/// i.e. the flags `agent`'s sub-state admits at all, plus the repeated
/// `--provider` bit whose resolve-time single-use contract excludes it.
/// Flags no `agent` path owns are already `done` before the landing check
/// can see them.
fn integration_bad_bits() -> String {
    let mut bits = BUSINESS_FLAGS
        .iter()
        .filter(|flag| {
            !INTEGRATION_COMMAND_FLAGS.contains(flag) && families_of(flag).contains(&"agent")
        })
        .map(|flag| flag_bit(flag))
        .collect::<Vec<_>>();
    // A repeated `--provider` parses only under `agent doctor`: the second
    // occurrence is inside the union scope's spelling but outside its
    // grammar, so it lands in the bad set rather than the scope check.
    bits.push("seen_provider2");
    bits.join(" || ")
}

/// `agent:integration:<flag>` patterns — the flags admitted at the `deep`
/// state, where the resolved command is not yet chosen.
fn deep_flag_patterns() -> String {
    INTEGRATION_COMMAND_FLAGS
        .iter()
        .map(|flag| format!("agent:integration:{flag}"))
        .collect::<Vec<_>>()
        .join("|")
}

/// `agent:integration:<verb>` patterns — the words legal at the `deep`
/// state and inside `help agent integration`.
fn deep_verb_patterns() -> String {
    INTEGRATION_VERBS
        .iter()
        .map(|verb| format!("agent:integration:{verb}"))
        .collect::<Vec<_>>()
        .join("|")
}

/// bash/zsh `help <topic> <child> <TAB>` offers.
fn help_level3_cases_bash() -> String {
    HELP_LEVEL3
        .iter()
        .map(|(path, words)| format!("            {path}) candidates=\"{}\" ;;", words.join(" ")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish `help <topic> <child> <TAB>` offers.
fn help_level3_cases_fish() -> String {
    HELP_LEVEL3
        .iter()
        .map(|(path, words)| {
            format!(
                "                case '{path}'; set candidates {}",
                words.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish `deep`-state flag admission arms — the `agent integration` union
/// scope while no verb is chosen yet.
fn deep_flag_cases_fish() -> String {
    INTEGRATION_COMMAND_FLAGS
        .iter()
        .map(|flag| format!("                            case 'agent:integration:{flag}'"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish `agent:integration:<verb>` case words — legal at the `deep` state
/// and inside `help agent integration`.
fn deep_verb_cases_fish() -> String {
    INTEGRATION_VERBS
        .iter()
        .map(|verb| format!("'agent:integration:{verb}'"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `family:verb` pairs, one per line of a shell `case` pattern list.
fn path_patterns(families: &[(&str, &[&str])]) -> String {
    families
        .iter()
        .flat_map(|(family, verbs)| verbs.iter().map(move |verb| format!("{family}:{verb}")))
        .collect::<Vec<_>>()
        .join("|")
}

/// The business flags that take a value — every `BUSINESS_FLAGS` member
/// minus the booleans. Only these admit the `--flag=value` joined spelling;
/// a boolean or unknown name in that shape stays a dead token.
fn value_business_flags() -> impl Iterator<Item = &'static str> {
    BUSINESS_FLAGS
        .iter()
        .copied()
        .filter(|flag| flag_value(flag).is_some())
}

/// bash/zsh `seen` bookkeeping arms, one per business flag. A repeatable
/// flag sets its bit — scope checks still see it — but never turns `done`.
/// Every consumed flag also joins `seen_flags`, the consumed-name list the
/// command-scope admission checks walk (the same model fish uses). The
/// `joined` variant is the `--flag=value` twin: identical bookkeeping
/// except the value arrived inline, so no pending `value=` is set.
fn seen_cases_bash(joined: bool) -> String {
    let flags: Vec<&&str> = if joined {
        // The joined arm already gated on value-taking flags; booleans
        // would be dead arms.
        BUSINESS_FLAGS
            .iter()
            .filter(|flag| flag_value(flag).is_some())
            .collect()
    } else {
        BUSINESS_FLAGS.iter().collect()
    };
    flags
        .into_iter()
        .map(|flag| {
            let bit = flag_bit(flag);
            let assign = match flag_value(flag) {
                Some(kind) if !joined => {
                    format!("{bit}=1; value={kind}; seen_flags+=({flag})")
                }
                _ => format!("{bit}=1; seen_flags+=({flag})"),
            };
            if *flag == "--provider" {
                // Repeatable only toward `agent doctor`: once the
                // integration family is committed (the `deep` sub-family or
                // a resolved `integration:<verb>`), a second `--provider` is
                // a dead prefix; anywhere earlier it records
                // `seen_provider2`, which retires `integration` from the
                // still-viable verbs while `doctor` stays legal.
                format!(
                    "                        --provider)\n                            if ((seen_provider)); then\n                                seen_provider2=1\n                                case \"$family:$sub\" in\n                                    agent:integration|agent:integration:*) done=1 ;;\n                                    *) {assign} ;;\n                                esac\n                            else {assign}; fi ;;"
                )
            } else if REPEATABLE_FLAGS.contains(flag) {
                format!("                        {flag})\n                            {assign} ;;")
            } else {
                format!("                        {flag})\n                            if (({bit})); then done=1; else {assign}; fi ;;")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh sub-state admission arms: a business flag is tentatively legal
/// iff the current family owns it on at least one verb.
fn sub_flag_cases_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = families_of(flag)
                .iter()
                .map(|family| format!("{family}:{flag}"))
                .collect::<Vec<_>>()
                .join("|");
            match *flag {
                "--panel" => format!("                            {patterns}) ((seen_current)) && done=1 ;;"),
                "--current" => format!("                            {patterns}) ((seen_panel)) && done=1 ;;"),
                "--workspace" => {
                    format!(
                        "                            {patterns}) ((seen_standalone)) && done=1 ;;"
                    )
                }
                "--standalone" => {
                    format!(
                        "                            {patterns}) ((seen_workspace)) && done=1 ;;"
                    )
                }
                "--profile" => {
                    format!(
                        "                            {patterns}) ((seen_exec || seen_arg)) && done=1 ;;"
                    )
                }
                "--exec" | "--arg" => {
                    format!("                            {patterns}) ((seen_profile)) && done=1 ;;")
                }
                _ => format!("                            {patterns}) ;;"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `<command>:<flag>` admission patterns; the four `agent:integration:*`
/// commands share one scope, so they fold into a single glob arm.
fn flag_admission_patterns(flag: &str, separator: &str, quote: bool) -> String {
    let mut seen_integration = false;
    COMMAND_FLAGS
        .iter()
        .filter(|(_, flags)| flags.contains(&flag))
        .filter_map(|(command, _)| {
            if command.starts_with("agent:integration:") {
                if seen_integration {
                    return None;
                }
                seen_integration = true;
                let pattern = format!("agent:integration:*:{flag}");
                return Some(if quote {
                    format!("'{pattern}'")
                } else {
                    pattern
                });
            }
            let pattern = format!("{command}:{flag}");
            Some(if quote {
                format!("'{pattern}'")
            } else {
                pattern
            })
        })
        .collect::<Vec<_>>()
        .join(separator)
}

/// bash/zsh after/operand admission arms: a business flag is legal iff the
/// resolved command's scope contains it.
fn after_flag_cases_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = flag_admission_patterns(flag, "|", false);
            match *flag {
                "--panel" => format!("                            {patterns}) if ((seen_current)) || [[ $family:$sub == panel:process-info && $state == operand ]]; then done=1; fi ;;"),
                "--current" => format!("                            {patterns}) if ((seen_panel)) || [[ $family:$sub == panel:process-info && $state == operand ]]; then done=1; fi ;;"),
                "--workspace" => {
                    format!(
                        "                            {patterns}) ((seen_standalone)) && done=1 ;;"
                    )
                }
                "--standalone" => {
                    format!(
                        "                            {patterns}) ((seen_workspace)) && done=1 ;;"
                    )
                }
                // `terminal send` payload sources are mutually exclusive and
                // `--enter` rides on the text-bearing forms only.
                "--text" => {
                    format!(
                        "                            {patterns}) ((seen_key || seen_file || seen_stdin)) && done=1 ;;"
                    )
                }
                "--key" => format!(
                    "                            {patterns}) ((seen_text || seen_enter || seen_file || seen_stdin)) && done=1 ;;"
                ),
                "--enter" => {
                    format!("                            {patterns}) ((seen_key)) && done=1 ;;")
                }
                "--file" => {
                    format!(
                        "                            {patterns}) ((seen_text || seen_key || seen_stdin)) && done=1 ;;"
                    )
                }
                "--stdin" => {
                    format!(
                        "                            {patterns}) ((seen_text || seen_key || seen_file)) && done=1 ;;"
                    )
                }
                // `--profile` excludes the explicit `--exec`/`--arg` launch;
                // either explicit half excludes it back.
                "--profile" => {
                    format!(
                        "                            {patterns}) ((seen_exec || seen_arg)) && done=1 ;;"
                    )
                }
                "--exec" | "--arg" => {
                    format!("                            {patterns}) ((seen_profile)) && done=1 ;;")
                }
                _ => format!("                            {patterns}) ;;"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh verb-landing arms: the chosen command's own declared flags
/// admit the consumed set — no per-command enumeration of the flags it
/// rejects. `admit` is the same consumed-list check fish runs.
fn scope_cases_bash() -> String {
    FAMILY_VERBS
        .iter()
        .flat_map(|(family, verbs)| {
            verbs.iter().map(move |verb| {
                let command = format!("{family}:{verb}");
                let allowed = command_flags(&command).join(" ");
                // `--workspace`/`--standalone` stay exclusive wherever both
                // are admitted — only tab commands admit them. `terminal
                // send` is dead once two payload sources stand, or `--key`
                // stands next to `--enter`; a lone `--enter` stays viable
                // because a text-bearing source may still follow. The
                // `--profile`/explicit-launch pair is exclusive on the two
                // profile-selecting commands.
                let mut exclusive = String::new();
                if matches!(command.as_str(), "panel:current" | "panel:process-info") {
                    exclusive.push_str("((seen_panel && seen_current)) && done=1\n                                        ");
                }
                if *family == "tab" {
                    exclusive.push_str("((seen_workspace && seen_standalone)) && done=1\n                                        ");
                }
                if command == "tab:create" || command == "panel:split" {
                    exclusive.push_str("((seen_profile && (seen_exec || seen_arg))) && done=1\n                                        ");
                }
                if command == "terminal:send" {
                    exclusive.push_str("(((seen_text + seen_key + seen_file + seen_stdin) >= 2 || (seen_key && seen_enter))) && done=1\n                                        ");
                }
                let exclusive = exclusive.as_str();
                format!("                                    {command})\n                                        {exclusive}admit '{allowed}' \"${{seen_flags[@]}}\" || done=1 ;;")
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh sub-state offers: a verb stays viable while every consumed
/// flag is inside that verb's declared scope — the consumed-list `admit`
/// check — and no scoped exclusive pair stands violated. `agent`
/// additionally offers `integration` while the seen flags stay inside the
/// third level's union scope.
fn verb_cases_bash() -> String {
    FAMILY_VERBS
        .iter()
        .map(|(family, verbs)| {
            let mut lines = verbs
                .iter()
                .enumerate()
                .map(|(index, verb)| {
                    let command = format!("{family}:{verb}");
                    let allowed = command_flags(&command).join(" ");
                    let guard = if matches!(command.as_str(), "panel:current" | "panel:process-info") {
                        " && ((!seen_panel || !seen_current))"
                    } else if command == "tab:create" {
                        " && ((!seen_workspace || !seen_standalone)) && ((!seen_profile || (!seen_exec && !seen_arg)))"
                    } else if *family == "tab" {
                        " && ((!seen_workspace || !seen_standalone))"
                    } else if command == "panel:split" {
                        " && ((!seen_profile || (!seen_exec && !seen_arg)))"
                    } else if *verb == "send" {
                        " && (((seen_text + seen_key + seen_file + seen_stdin) < 2) && (!seen_key || !seen_enter))"
                    } else {
                        ""
                    };
                    let assign = if index == 0 {
                        format!("verbs=\" {verb}\"")
                    } else {
                        format!("verbs=\"$verbs {verb}\"")
                    };
                    format!(
                        "                admit '{allowed}' \"${{seen_flags[@]}}\"{guard} && {assign}"
                    )
                })
                .collect::<Vec<_>>();
            if *family == "agent" {
                lines.push(format!(
                    "                (({})) || verbs=\"$verbs integration\"",
                    integration_bad_bits()
                ));
            }
            format!("            {family})\n{} ;;", lines.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh verb-state narrowing: each consumed business flag intersects the
/// still-viable root words with the families that declare it.
fn narrow_lines_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .filter_map(|flag| {
            let keep = families_of(flag).join(" ");
            match *flag {
                "--panel" => Some(format!("        if ((seen_panel && seen_current)); then possible=\"\"; elif ((seen_panel || seen_current)); then narrow \"{keep}\"; fi")),
                "--current" => None,
                "--workspace" => Some(format!(
                    "        if ((seen_workspace || seen_standalone)); then\n            if ((seen_workspace && seen_standalone)); then possible=\"\"\n            else narrow \"{keep}\"; fi\n        fi"
                )),
                // Folded into the --workspace arm: the exclusive pair shares
                // one family set.
                "--standalone" => None,
                _ => Some(format!("        (({})) && narrow \"{keep}\"", flag_bit(flag))),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh after/operand offers: the resolved command's unused business
/// flags. An exclusive flag is offered only while its pair is also unseen.
/// The four `agent:integration:*` commands share one scope, so they emit a
/// single glob arm.
fn extras_cases_bash() -> String {
    let mut integration_done = false;
    COMMAND_FLAGS
        .iter()
        .filter_map(|(command, flags)| {
            let label = if command.starts_with("agent:integration:") {
                if integration_done {
                    return None;
                }
                integration_done = true;
                "agent:integration:*"
            } else {
                command
            };
            let lines = flags
                .iter()
                .map(|flag| {
                    let cond = match *flag {
                        "--panel" | "--current" => "seen_panel || seen_current".to_owned(),
                        "--workspace" | "--standalone" => {
                            "seen_workspace || seen_standalone".to_owned()
                        }
                        // `terminal send`: payload sources exclude each
                        // other and `--enter` is offered only once a
                        // text-bearing source stands.
                        "--text" => {
                            "seen_text || seen_key || seen_file || seen_stdin || seen_paste"
                                .to_owned()
                        }
                        "--paste" => "seen_paste || seen_text || seen_key".to_owned(),
                        "--key" => {
                            "seen_text || seen_enter || seen_file || seen_stdin || seen_paste"
                                .to_owned()
                        }
                        "--enter" => {
                            "!(seen_text || seen_file || seen_stdin) || seen_enter".to_owned()
                        }
                        "--file" | "--stdin" => {
                            "seen_text || seen_key || seen_file || seen_stdin".to_owned()
                        }
                        // `--arg` is only legal next to `--exec` and never
                        // beside `--profile`; the bit never dedups it, so
                        // repeats stay offerable.
                        "--arg" if *command == "agent:start" => "0".to_owned(),
                        "--arg" => "!seen_exec || seen_profile".to_owned(),
                        // `--profile` excludes the explicit launch pair.
                        "--profile" => "seen_profile || seen_exec || seen_arg".to_owned(),
                        "--exec" => "seen_exec || seen_profile".to_owned(),
                        // `--until`, `--asset-root`, and doctor's
                        // `--provider` are repeatable — they stay offerable
                        // on every pass so another selector can follow.
                        // Integration's `--provider` is single-use: its seen
                        // bit gates the offer there.
                        "--provider" if command.starts_with("agent:integration:") => {
                            "seen_provider".to_owned()
                        }
                        "--provider" | "--asset-root" | "--until" => "0".to_owned(),
                        _ => flag_bit(flag).to_owned(),
                    };
                    if *command == "panel:process-info" && matches!(*flag, "--panel" | "--current") {
                        format!("                [[ $state != operand ]] && (( !({cond}) )) && extras=\"$extras {flag}\"")
                    } else { format!("                (({cond})) || extras=\"$extras {flag}\"") }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(format!("            {label})\n{lines} ;;"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh typed `--enum=<prefix>` candidate arms: the flag keeps its
/// spelling and completes only its own closed vocabulary, only where the
/// walker could still admit the flag. `sub` offers reuse the computed
/// viable-verb list (the flag is useless when no admitting verb remains);
/// `after|operand` check the resolved command's scope; `deep` checks the
/// integration union. `--direction`'s vocabulary is per-command, so it
/// offers only once the command is resolved. Freeform flags are absent —
/// their values are never guessed.
fn joined_offer_cases_bash() -> String {
    let joined = |flag: &str, values: &[&str]| {
        values
            .iter()
            .map(|value| format!("{flag}={value}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let direction = DIRECTION_VALUES
        .iter()
        .map(|(command, values)| {
            format!(
                "                        {command}) candidates=\"{}\" ;;",
                joined("--direction", values)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let key = joined("--key", KEY_VALUES);
    let until = joined("--until", UNTIL_VALUES);
    let provider = joined("--provider", PROVIDER_VALUES);
    format!(
        concat!(
            "            --direction)\n",
            "                if ((!seen_direction)); then\n",
            "                    case \"$state\" in\n",
            "                        after|operand|operand2)\n",
            "                            case \"$family:$sub\" in\n",
            "{direction}\n",
            "                            esac ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            // `--key` shares `terminal send`'s payload-source exclusion:
            // the offer dies with any other source or a consumed `--key`.
            "            --key)\n",
            "                if ((!seen_text && !seen_enter && !seen_file && !seen_stdin && !seen_paste)); then\n",
            "                    case \"$state\" in\n",
            "                        sub) [[ \" $verbs \" == *\" send \"* ]] && candidates=\"{key}\" ;;\n",
            "                        after|operand|operand2) [[ $family:$sub == terminal:send ]] && candidates=\"{key}\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            // `--until` is repeatable: it stays offerable while `agent wait`\n",
            // is still the live command.\n",
            "            --until)\n",
            "                case \"$state\" in\n",
            "                    sub) [[ \" $verbs \" == *\" wait \"* ]] && candidates=\"{until}\" ;;\n",
            "                    after|operand|operand2) [[ $family:$sub == agent:wait ]] && candidates=\"{until}\" ;;\n",
            "                esac ;;\n",
            // `--provider` is repeatable toward `agent doctor` (an unbounded\n",
            // count still parses there) and single-use under `agent\n",
            // integration:* — one consumed occurrence retires the offer on\n",
            // those paths only.\n",
            "            --provider)\n",
            "                case \"$state\" in\n",
            "                    sub) [[ \" $verbs \" == *\" doctor \"* || \" $verbs \" == *\" integration \"* ]] && candidates=\"{provider}\" ;;\n",
            "                    deep) ((!seen_provider)) && candidates=\"{provider}\" ;;\n",
            "                    after|operand|operand2)\n",
            "                        case \"$family:$sub\" in\n",
            "                            agent:doctor) candidates=\"{provider}\" ;;\n",
            "                            agent:integration:*) ((!seen_provider)) && candidates=\"{provider}\" ;;\n",
            "                        esac ;;\n",
            "                esac ;;\n",
        ),
        direction = direction,
        key = key,
        until = until,
        provider = provider,
    )
}

/// bash/zsh direction-value offers while a `--direction` value is pending.
fn direction_cases_bash() -> String {
    DIRECTION_VALUES
        .iter()
        .map(|(command, values)| {
            format!(
                "            {command}) candidates=\"{}\" ;;",
                values.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The bash/zsh "no option was consumed" condition that keeps `--help`
/// offers legal: the consumed-flag list is empty and neither orthogonal
/// option bit stands.
fn no_options_bash() -> &'static str {
    "!seen_format && !seen_mouse && !${#seen_flags[@]}"
}

fn bash() -> String {
    format!(
        concat!(
            "# bash completion for orchbee — a static projection of the implemented grammar.\n",
            "# No dynamic state: no Site, resource, or runtime enumeration.\n",
            "_orchbee() {{\n",
            "    local cur candidates=\"\" extras=\"\" verbs=\"\" possible=\"\"\n",
            "    local flag_name=\"\" cur_name=\"\"\n",
            "    local state=verb help_a=\"\" help_b=\"\" help_c=\"\" family=\"\" sub=\"\" leaf=\"\"\n",
            "    local seen_format=0 seen_mouse=0 {seen_decls} seen_provider2=0\n",
            "    local -a seen_flags\n",
            "    local ended=0 done=0 dead=0 value=\"\"\n",
            "    local i word\n",
            "    # Each consumed business flag narrows the viable root words to the\n",
            "    # families that declare it.\n",
            "    narrow() {{\n",
            "        local keep=\" $1 \" word out=\"\"\n",
            "        for word in $possible; do\n",
            "            [[ $keep == *\" $word \"* ]] && out=\"$out $word\"\n",
            "        done\n",
            "        possible=\"${{out# }}\"\n",
            "    }}\n",
            "    # $1 is the resolved command's allowed flags joined by spaces; the\n",
            "    # rest are every consumed business flag. Succeeds iff each consumed\n",
            "    # flag is inside the allowed list.\n",
            "    admit() {{\n",
            "        local allowed=\" $1 \" flag\n",
            "        shift\n",
            "        for flag in \"$@\"; do [[ $allowed == *\" $flag \"* ]] || return 1; done\n",
            "    }}\n",
            "    cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n",
            "    for ((i = 1; i < COMP_CWORD; i++)); do\n",
            "        word=\"${{COMP_WORDS[i]}}\"\n",
            "        # `--` terminates options and is never consumed as a flag value.\n",
            "        if [[ $word == -- ]]; then ended=1; continue; fi\n",
            "        if ((ended || done)); then continue; fi\n",
            "        # A declared flag owns the next word whole — even one spelled like a flag.\n",
            "        if [[ -n $value ]]; then\n",
            "            if [[ $value == format && $word != text && $word != json ]]; then done=1; fi\n",
            "            if [[ $value == mouse && $word != off && $word != on ]]; then done=1; fi\n",
            "            value=\"\"\n",
            "            continue\n",
            "        fi\n",
            "        case \"$word\" in\n",
            "            --format|--format=*)\n",
            "                case \"$state\" in\n",
            "                    verb|sub|deep|after|operand|operand2) ;;\n",
            "                    leaf) [[ $leaf == status ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "                        --format)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; value=format; fi ;;\n",
            "                        --format=text|--format=json)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; fi ;;\n",
            "                        *) done=1 ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --mouse|--mouse=*)\n",
            "                # `--mouse` is the TUI leaf's own option: legal before the\n",
            "                # command word (only `tui` stays viable) or on that leaf.\n",
            "                # Both spellings are the grammar — the two-token form\n",
            "                # leaves a pending value, the joined form carries it and\n",
            "                # stays inside the closed {{off,on}} vocabulary.\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    leaf) [[ $leaf == tui ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    if ((seen_mouse)); then done=1; else\n",
            "                        case \"$word\" in\n",
            "                            --mouse) seen_mouse=1; value=mouse ;;\n",
            "                            --mouse=off|--mouse=on) seen_mouse=1 ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac\n",
            "                    fi\n",
            "                fi ;;\n",
            "            {flag_patterns})\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "{sub_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    deep)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "                            {deep_flag_patterns}) ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after|operand|operand2)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "{after_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "{seen_cases}\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --*=*)\n",
            "                # A joined `--flag=value` business token: the flag part\n",
            "                # drives the same state admission and bookkeeping as the\n",
            "                # two-token spelling, and the inline value never claims\n",
            "                # the next word. Booleans and unknown names stay dead.\n",
            "                flag_name=\"${{word%%=*}}\"\n",
            "                case \"$flag_name\" in\n",
            "                    {value_flag_patterns}) ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$state\" in\n",
            "                        verb) ;;\n",
            "                        sub)\n",
            "                            case \"$family:$flag_name\" in\n",
            "{sub_flag_cases}\n",
            "                                *) done=1 ;;\n",
            "                            esac ;;\n",
            "                        deep)\n",
            "                            case \"$family:$sub:$flag_name\" in\n",
            "                                {deep_flag_patterns}) ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac ;;\n",
            "                        after|operand|operand2)\n",
            "                            case \"$family:$sub:$flag_name\" in\n",
            "{after_flag_cases}\n",
            "                                *) done=1 ;;\n",
            "                            esac ;;\n",
            "                        *) done=1 ;;\n",
            "                    esac\n",
            "                fi\n",
            "                if ((!done)); then\n",
            "                    case \"$flag_name\" in\n",
            "{joined_seen_cases}\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --help|--version|--skill) done=1 ;;\n",
            "            -*) done=1 ;;\n",
            "            *)\n",
            "                case \"$state\" in\n",
            "                    verb)\n",
            "                        case \"$word\" in\n",
            "                            status)\n",
            "                                if ((${{#seen_flags[@]}} || seen_mouse)); then done=1; else leaf=status; state=leaf; fi ;;\n",
            "                            tui)\n",
            "                                if ((seen_format || ${{#seen_flags[@]}})); then done=1; else leaf=tui; state=leaf; fi ;;\n",
            "                            help|completion)\n",
            "                                if ((seen_format || ${{#seen_flags[@]}} || seen_mouse)); then done=1; else state=$word; fi ;;\n",
            "                            {families}) if ((seen_mouse)); then done=1; else family=$word; state=sub; fi ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            agent:integration)\n",
            "                                sub=integration; state=deep\n",
            "                                # The sub-family's union scope must cover\n",
            "                                # every flag already recorded.\n",
            "                                (({integration_bad})) && done=1 ;;\n",
            "                            {paths})\n",
            "                                sub=$word; state=after\n",
            "                                # The chosen verb's flag scope must cover\n",
            "                                # every flag already recorded.\n",
            "                                case \"$family:$sub\" in\n",
            "{scope_cases}\n",
            "                                esac ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    deep)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "                            {deep_verb_patterns})\n",
            "                                sub=\"integration:$word\"; state=after\n",
            "                                # Every `agent integration` verb shares the\n",
            "                                # same flag scope — the union check.\n",
            "                                (({integration_bad})) && done=1 ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub\" in\n",
            "                            panel:process-info) if ((seen_panel || seen_current)); then done=1; else state=operand; fi ;;\n",
            "                            {operand_paths}) state=operand ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    operand) if [[ $family:$sub == agent:send-keys || $family:$sub == agent:start ]]; then state=operand; elif [[ $family:$sub == agent:rename || $family:$sub == agent:prompt ]] && ((seen_clear == 0 && seen_name == 0 && seen_session == 0)); then state=operand2; else done=1; fi ;;\n",
            "                    leaf|operand2|completion) done=1 ;;\n",
            "                    help)\n",
            "                        if [[ -z $help_a ]]; then\n",
            "                            case \"$word\" in\n",
            "                                {topics}) help_a=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        elif [[ -z $help_b ]]; then\n",
            "                            case \"$help_a:$word\" in\n",
            "                                {help_paths}) help_b=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        elif [[ -z $help_c ]]; then\n",
            "                            case \"$help_a:$help_b:$word\" in\n",
            "                                {deep_verb_patterns}) help_c=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        else done=1; fi ;;\n",
            "                esac ;;\n",
            "        esac\n",
            "    done\n",
            "    # A consumed prefix that left no legal root word (verb) or no legal\n",
            "    # verb (sub, deep) is dead: every candidate is suppressed before\n",
            "    # pending values, the --format= override, or any offer can resurrect\n",
            "    # one.\n",
            "    if [[ $state == verb ]]; then\n",
            "        possible=\"{roots}\"\n",
            "{narrow_lines}\n",
            "        ((seen_mouse)) && narrow \"tui\"\n",
            "        ((seen_format)) && narrow \"{format_roots}\"\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "{verb_cases}\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    elif [[ $state == deep ]]; then\n",
            "        (({integration_bad})) || verbs=\"{integration_verb_words}\"\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == mouse ]]; then candidates=\"off on\"\n",
            "    elif [[ $value == direction ]]; then\n",
            "        case \"$family:$sub\" in\n",
            "{direction_cases}\n",
            "        esac\n",
            "    elif [[ $value == key ]]; then candidates=\"{key_values}\"\n",
            "    elif [[ $value == until ]]; then candidates=\"{until_values}\"\n",
            "    elif [[ $value == provider ]]; then candidates=\"{provider_values}\"\n",
            "    elif [[ -n $value ]]; then candidates=\"\"\n",
            "    elif [[ $cur == --format=* ]]; then\n",
            "        if ((!seen_format)); then\n",
            "            case \"$state\" in\n",
            "                verb) ((!seen_mouse)) && candidates=\"--format=text --format=json\" ;;\n",
            "                sub|deep|after|operand|operand2) candidates=\"--format=text --format=json\" ;;\n",
            "                leaf) [[ $leaf == status ]] && candidates=\"--format=text --format=json\" ;;\n",
            "            esac\n",
            "        fi\n",
            "    elif [[ $cur == --*=* ]]; then\n",
            "        # A typed `--enum=<prefix>` completes only that flag's own closed\n",
            "        # vocabulary with the `--flag=` spelling preserved; a freeform\n",
            "        # flag's value is never guessed (no resource or path probing),\n",
            "        # and a boolean or unknown name offers nothing.\n",
            "        cur_name=\"${{cur%%=*}}\"\n",
            "        case \"$cur_name\" in\n",
            "            --mouse)\n",
            "                if ((!seen_mouse)); then\n",
            "                    case \"$state\" in\n",
            "                        verb) [[ \" $possible \" == *\" tui \"* ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        leaf) [[ $leaf == tui ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "{joined_offer_cases}\n",
            "        esac\n",
            "    elif [[ $state == verb ]]; then\n",
            "        candidates=\"$possible\"\n",
            "        ((seen_format || seen_mouse)) || candidates=\"$candidates --format --format=\"\n",
            "        [[ \" $possible \" == *\" tui \"* ]] && ((!seen_mouse)) && candidates=\"$candidates --mouse\"\n",
            "        if (({no_options})); then\n",
            "            candidates=\"$candidates --help --version --skill\"\n",
            "        fi\n",
            "    elif [[ $state == sub ]]; then\n",
            "        candidates=\"$verbs\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "    elif [[ $state == deep ]]; then\n",
            "        candidates=\"$verbs\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "    elif [[ $state == leaf ]]; then\n",
            "        if [[ $leaf == tui ]]; then ((!seen_mouse)) && candidates=\"--help --mouse\"\n",
            "        elif ((!seen_format)); then candidates=\"--format --format= --help\"; fi\n",
            "    elif [[ $state == after || $state == operand || $state == operand2 ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
            "        case \"$family:$sub\" in\n",
            "{extras_cases}\n",
            "        esac\n",
            "        # Suffix `--help` is legal on every command path — including the\n",
            "        # operand-owed `get`/`api schema`/mutation paths — while no\n",
            "        # option was seen and the operand is still owed.\n",
            "        if [[ $state == after ]] && (({no_options})); then\n",
            "            candidates=\"$candidates --help\"\n",
            "        fi\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == completion ]]; then candidates=\"{shells} --help\"\n",
            "    elif [[ $state == help ]]; then\n",
            "        if [[ -z $help_a ]]; then candidates=\"{topic_words}\"\n",
            "        elif [[ -z $help_b ]]; then\n",
            "            case \"$help_a\" in\n",
            "{help_cases}\n",
            "            esac\n",
            "        elif [[ -z $help_c ]]; then\n",
            "            case \"$help_a:$help_b\" in\n",
            "{help_level3_cases}\n",
            "            esac\n",
            "        fi\n",
            "    fi\n",
            "    COMPREPLY=()\n",
            "    # shellcheck disable=SC2207\n",
            "    if [[ -n $candidates ]]; then COMPREPLY=($(compgen -W \"$candidates\" -- \"$cur\")); fi\n",
            "}}\n",
            "complete -F _orchbee orchbee\n"
        ),
        families = FAMILY_VERBS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>()
            .join("|"),
        paths = path_patterns(FAMILY_VERBS),
        operand_paths = OPERAND_PATHS.join("|"),
        topics = HELP_TOPICS.join("|"),
        topic_words = HELP_TOPICS.join(" "),
        help_paths = path_patterns(HELP_CHILDREN),
        format_values = FORMAT_VALUES.join(" "),
        key_values = KEY_VALUES.join(" "),
        until_values = UNTIL_VALUES.join(" "),
        provider_values = PROVIDER_VALUES.join(" "),
        roots = ROOT_WORDS.join(" "),
        format_roots = FORMAT_ROOT_WORDS.join(" "),
        shells = SHELLS.join(" "),
        flag_patterns = BUSINESS_FLAGS.join("|"),
        seen_decls = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("{}=0", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join(" "),
        sub_flag_cases = sub_flag_cases_bash(),
        deep_flag_patterns = deep_flag_patterns(),
        deep_verb_patterns = deep_verb_patterns(),
        integration_bad = integration_bad_bits(),
        integration_verb_words = INTEGRATION_VERBS.join(" "),
        after_flag_cases = after_flag_cases_bash(),
        seen_cases = seen_cases_bash(false),
        value_flag_patterns = value_business_flags().collect::<Vec<_>>().join("|"),
        joined_seen_cases = seen_cases_bash(true),
        joined_offer_cases = joined_offer_cases_bash(),
        scope_cases = scope_cases_bash(),
        verb_cases = verb_cases_bash(),
        narrow_lines = narrow_lines_bash(),
        extras_cases = extras_cases_bash(),
        direction_cases = direction_cases_bash(),
        no_options = no_options_bash(),
        help_cases = HELP_CHILDREN
            .iter()
            .map(|(topic, children)| format!(
                "            {topic}) candidates=\"{}\" ;;",
                children.join(" ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        help_level3_cases = help_level3_cases_bash(),
    )
}

fn zsh() -> String {
    format!(
        concat!(
            "#compdef orchbee\n",
            "# zsh completion for orchbee — a static projection of the implemented\n",
            "# grammar. No dynamic state: no Site, resource, or runtime enumeration.\n",
            "_orchbee() {{\n",
            "    local candidates=\"\" extras=\"\" verbs=\"\" possible=\"\"\n",
            "    local state=verb help_a=\"\" help_b=\"\" help_c=\"\" family=\"\" sub=\"\" leaf=\"\"\n",
            "    local seen_format=0 seen_mouse=0 {seen_decls} seen_provider2=0\n",
            "    local -a seen_flags\n",
            "    local ended=0 done=0 dead=0 value=\"\"\n",
            "    local i word cur flag_name=\"\" cur_name=\"\"\n",
            "    narrow() {{\n",
            "        local keep=\" $1 \" word out=\"\"\n",
            "        for word in ${{=possible}}; do\n",
            "            [[ $keep == *\" $word \"* ]] && out=\"$out $word\"\n",
            "        done\n",
            "        possible=\"${{out# }}\"\n",
            "    }}\n",
            "    # $1 is the resolved command's allowed flags joined by spaces; the\n",
            "    # rest are every consumed business flag. Succeeds iff each consumed\n",
            "    # flag is inside the allowed list.\n",
            "    admit() {{\n",
            "        local allowed=\" $1 \" flag\n",
            "        shift\n",
            "        for flag in \"$@\"; do [[ $allowed == *\" $flag \"* ]] || return 1; done\n",
            "    }}\n",
            "    cur=\"${{words[CURRENT]}}\"\n",
            "    for ((i = 2; i < CURRENT; i++)); do\n",
            "        word=\"${{words[i]}}\"\n",
            "        if [[ $word == -- ]]; then ended=1; continue; fi\n",
            "        if ((ended || done)); then continue; fi\n",
            "        if [[ -n $value ]]; then\n",
            "            if [[ $value == format && $word != text && $word != json ]]; then done=1; fi\n",
            "            if [[ $value == mouse && $word != off && $word != on ]]; then done=1; fi\n",
            "            value=\"\"\n",
            "            continue\n",
            "        fi\n",
            "        case \"$word\" in\n",
            "            --format|--format=*)\n",
            "                case \"$state\" in\n",
            "                    verb|sub|deep|after|operand|operand2) ;;\n",
            "                    leaf) [[ $leaf == status ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "                        --format)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; value=format; fi ;;\n",
            "                        --format=text|--format=json)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; fi ;;\n",
            "                        *) done=1 ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --mouse|--mouse=*)\n",
            "                # `--mouse` is the TUI leaf's own option: legal before the\n",
            "                # command word (only `tui` stays viable) or on that leaf.\n",
            "                # Both spellings are the grammar — the two-token form\n",
            "                # leaves a pending value, the joined form carries it and\n",
            "                # stays inside the closed {{off,on}} vocabulary.\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    leaf) [[ $leaf == tui ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    if ((seen_mouse)); then done=1; else\n",
            "                        case \"$word\" in\n",
            "                            --mouse) seen_mouse=1; value=mouse ;;\n",
            "                            --mouse=off|--mouse=on) seen_mouse=1 ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac\n",
            "                    fi\n",
            "                fi ;;\n",
            "            {flag_patterns})\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "{sub_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    deep)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "                            {deep_flag_patterns}) ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after|operand|operand2)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "{after_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "{seen_cases}\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --*=*)\n",
            "                # A joined `--flag=value` business token: the flag part\n",
            "                # drives the same state admission and bookkeeping as the\n",
            "                # two-token spelling, and the inline value never claims\n",
            "                # the next word. Booleans and unknown names stay dead.\n",
            "                flag_name=\"${{word%%=*}}\"\n",
            "                case \"$flag_name\" in\n",
            "                    {value_flag_patterns}) ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$state\" in\n",
            "                        verb) ;;\n",
            "                        sub)\n",
            "                            case \"$family:$flag_name\" in\n",
            "{sub_flag_cases}\n",
            "                                *) done=1 ;;\n",
            "                            esac ;;\n",
            "                        deep)\n",
            "                            case \"$family:$sub:$flag_name\" in\n",
            "                                {deep_flag_patterns}) ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac ;;\n",
            "                        after|operand|operand2)\n",
            "                            case \"$family:$sub:$flag_name\" in\n",
            "{after_flag_cases}\n",
            "                                *) done=1 ;;\n",
            "                            esac ;;\n",
            "                        *) done=1 ;;\n",
            "                    esac\n",
            "                fi\n",
            "                if ((!done)); then\n",
            "                    case \"$flag_name\" in\n",
            "{joined_seen_cases}\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --help|--version|--skill) done=1 ;;\n",
            "            -*) done=1 ;;\n",
            "            *)\n",
            "                case \"$state\" in\n",
            "                    verb)\n",
            "                        case \"$word\" in\n",
            "                            status)\n",
            "                                if ((${{#seen_flags[@]}} || seen_mouse)); then done=1; else leaf=status; state=leaf; fi ;;\n",
            "                            tui)\n",
            "                                if ((seen_format || ${{#seen_flags[@]}})); then done=1; else leaf=tui; state=leaf; fi ;;\n",
            "                            help|completion)\n",
            "                                if ((seen_format || ${{#seen_flags[@]}} || seen_mouse)); then done=1; else state=$word; fi ;;\n",
            "                            {families}) if ((seen_mouse)); then done=1; else family=$word; state=sub; fi ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            agent:integration)\n",
            "                                sub=integration; state=deep\n",
            "                                (({integration_bad})) && done=1 ;;\n",
            "                            {paths})\n",
            "                                sub=$word; state=after\n",
            "                                case \"$family:$sub\" in\n",
            "{scope_cases}\n",
            "                                esac ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    deep)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "                            {deep_verb_patterns})\n",
            "                                sub=\"integration:$word\"; state=after\n",
            "                                (({integration_bad})) && done=1 ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub\" in\n",
            "                            panel:process-info) if ((seen_panel || seen_current)); then done=1; else state=operand; fi ;;\n",
            "                            {operand_paths}) state=operand ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    operand) if [[ $family:$sub == agent:send-keys || $family:$sub == agent:start ]]; then state=operand; elif [[ $family:$sub == agent:rename || $family:$sub == agent:prompt ]] && ((seen_clear == 0 && seen_name == 0 && seen_session == 0)); then state=operand2; else done=1; fi ;;\n",
            "                    leaf|operand2|completion) done=1 ;;\n",
            "                    help)\n",
            "                        if [[ -z $help_a ]]; then\n",
            "                            case \"$word\" in\n",
            "                                {topics}) help_a=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        elif [[ -z $help_b ]]; then\n",
            "                            case \"$help_a:$word\" in\n",
            "                                {help_paths}) help_b=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        elif [[ -z $help_c ]]; then\n",
            "                            case \"$help_a:$help_b:$word\" in\n",
            "                                {deep_verb_patterns}) help_c=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        else done=1; fi ;;\n",
            "                esac ;;\n",
            "        esac\n",
            "    done\n",
            "    if [[ $state == verb ]]; then\n",
            "        possible=\"{roots}\"\n",
            "{narrow_lines}\n",
            "        ((seen_mouse)) && narrow \"tui\"\n",
            "        ((seen_format)) && narrow \"{format_roots}\"\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "{verb_cases}\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    elif [[ $state == deep ]]; then\n",
            "        (({integration_bad})) || verbs=\"{integration_verb_words}\"\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == mouse ]]; then candidates=\"off on\"\n",
            "    elif [[ $value == direction ]]; then\n",
            "        case \"$family:$sub\" in\n",
            "{direction_cases}\n",
            "        esac\n",
            "    elif [[ $value == key ]]; then candidates=\"{key_values}\"\n",
            "    elif [[ $value == until ]]; then candidates=\"{until_values}\"\n",
            "    elif [[ $value == provider ]]; then candidates=\"{provider_values}\"\n",
            "    elif [[ -n $value ]]; then candidates=\"\"\n",
            "    elif [[ $cur == --format=* ]]; then\n",
            "        if ((!seen_format)); then\n",
            "            case \"$state\" in\n",
            "                verb) ((!seen_mouse)) && candidates=\"--format=text --format=json\" ;;\n",
            "                sub|deep|after|operand|operand2) candidates=\"--format=text --format=json\" ;;\n",
            "                leaf) [[ $leaf == status ]] && candidates=\"--format=text --format=json\" ;;\n",
            "            esac\n",
            "        fi\n",
            "    elif [[ $cur == --*=* ]]; then\n",
            "        # A typed `--enum=<prefix>` completes only that flag's own closed\n",
            "        # vocabulary with the `--flag=` spelling preserved; a freeform\n",
            "        # flag's value is never guessed (no resource or path probing),\n",
            "        # and a boolean or unknown name offers nothing.\n",
            "        cur_name=\"${{cur%%=*}}\"\n",
            "        case \"$cur_name\" in\n",
            "            --mouse)\n",
            "                if ((!seen_mouse)); then\n",
            "                    case \"$state\" in\n",
            "                        verb) [[ \" $possible \" == *\" tui \"* ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        leaf) [[ $leaf == tui ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "{joined_offer_cases}\n",
            "        esac\n",
            "    elif [[ $state == verb ]]; then\n",
            "        candidates=\"$possible\"\n",
            "        ((seen_format || seen_mouse)) || candidates=\"$candidates --format --format=\"\n",
            "        [[ \" $possible \" == *\" tui \"* ]] && ((!seen_mouse)) && candidates=\"$candidates --mouse\"\n",
            "        if (({no_options})); then\n",
            "            candidates=\"$candidates --help --version --skill\"\n",
            "        fi\n",
            "    elif [[ $state == sub ]]; then\n",
            "        candidates=\"$verbs\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "    elif [[ $state == deep ]]; then\n",
            "        candidates=\"$verbs\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "    elif [[ $state == leaf ]]; then\n",
            "        if [[ $leaf == tui ]]; then ((!seen_mouse)) && candidates=\"--help --mouse\"\n",
            "        elif ((!seen_format)); then candidates=\"--format --format= --help\"; fi\n",
            "    elif [[ $state == after || $state == operand || $state == operand2 ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
            "        case \"$family:$sub\" in\n",
            "{extras_cases}\n",
            "        esac\n",
            "        if [[ $state == after ]] && (({no_options})); then\n",
            "            candidates=\"$candidates --help\"\n",
            "        fi\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == completion ]]; then candidates=\"{shells} --help\"\n",
            "    elif [[ $state == help ]]; then\n",
            "        if [[ -z $help_a ]]; then candidates=\"{topic_words}\"\n",
            "        elif [[ -z $help_b ]]; then\n",
            "            case \"$help_a\" in\n",
            "{help_cases}\n",
            "            esac\n",
            "        elif [[ -z $help_c ]]; then\n",
            "            case \"$help_a:$help_b\" in\n",
            "{help_level3_cases}\n",
            "            esac\n",
            "        fi\n",
            "    fi\n",
            "    [[ -n $candidates ]] && compadd -- ${{=candidates}}\n",
            "    return 0\n",
            "}}\n",
            "if (( ${{+functions[compdef]}} )); then compdef _orchbee orchbee; fi\n"
        ),
        families = FAMILY_VERBS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>()
            .join("|"),
        paths = path_patterns(FAMILY_VERBS),
        operand_paths = OPERAND_PATHS.join("|"),
        topics = HELP_TOPICS.join("|"),
        topic_words = HELP_TOPICS.join(" "),
        help_paths = path_patterns(HELP_CHILDREN),
        format_values = FORMAT_VALUES.join(" "),
        key_values = KEY_VALUES.join(" "),
        until_values = UNTIL_VALUES.join(" "),
        provider_values = PROVIDER_VALUES.join(" "),
        roots = ROOT_WORDS.join(" "),
        format_roots = FORMAT_ROOT_WORDS.join(" "),
        shells = SHELLS.join(" "),
        flag_patterns = BUSINESS_FLAGS.join("|"),
        seen_decls = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("{}=0", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join(" "),
        sub_flag_cases = sub_flag_cases_bash(),
        deep_flag_patterns = deep_flag_patterns(),
        deep_verb_patterns = deep_verb_patterns(),
        integration_bad = integration_bad_bits(),
        integration_verb_words = INTEGRATION_VERBS.join(" "),
        after_flag_cases = after_flag_cases_bash(),
        seen_cases = seen_cases_bash(false),
        value_flag_patterns = value_business_flags().collect::<Vec<_>>().join("|"),
        joined_seen_cases = seen_cases_bash(true),
        joined_offer_cases = joined_offer_cases_bash(),
        scope_cases = scope_cases_bash(),
        verb_cases = verb_cases_bash(),
        narrow_lines = narrow_lines_bash(),
        extras_cases = extras_cases_bash(),
        direction_cases = direction_cases_bash(),
        no_options = no_options_bash(),
        help_cases = HELP_CHILDREN
            .iter()
            .map(|(topic, children)| format!(
                "            {topic}) candidates=\"{}\" ;;",
                children.join(" ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        help_level3_cases = help_level3_cases_bash(),
    )
}

/// fish `seen` bookkeeping arms, one per business flag. A repeatable flag
/// still joins `seen_flags` for scope admission but never turns `done`.
/// The `joined` variant is the `--flag=value` twin: identical bookkeeping
/// except the value arrived inline, so no pending `value` is set.
fn seen_cases_fish(joined: bool) -> String {
    let flags: Vec<&&str> = if joined {
        BUSINESS_FLAGS
            .iter()
            .filter(|flag| flag_value(flag).is_some())
            .collect()
    } else {
        BUSINESS_FLAGS.iter().collect()
    };
    flags
        .into_iter()
        .map(|flag| {
            let bit = flag_bit(flag);
            let assign = match flag_value(flag) {
                Some(kind) if !joined => {
                    format!("set {bit} 1; set -a seen_flags '{flag}'; set value {kind}")
                }
                _ => format!("set {bit} 1; set -a seen_flags '{flag}'"),
            };
            if *flag == "--provider" {
                // Same contract as bash/zsh: repeatable toward `agent
                // doctor`, a dead prefix once the integration family is
                // committed, and `seen_provider2` retires `integration`
                // from the viable verbs in between.
                format!(
                    "                        case '--provider'\n                            if test $seen_provider -eq 1\n                                set seen_provider2 1\n                                switch \"$family:$sub\"\n                                    case 'agent:integration' 'agent:integration:*'\n                                        set done 1\n                                    case '*'\n                                        {assign}\n                                end\n                            else\n                                {assign}\n                            end"
                )
            } else if REPEATABLE_FLAGS.contains(flag) {
                format!("                        case '{flag}'\n                            {assign}")
            } else {
                format!("                        case '{flag}'\n                            if test ${bit} -eq 1; set done 1; else; {assign}; end")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish sub-state admission arms.
fn sub_flag_cases_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = families_of(flag)
                .iter()
                .map(|family| format!("'{family}:{flag}'"))
                .collect::<Vec<_>>()
                .join(" ");
            match *flag {
                "--panel" => format!("                            case {patterns}; if test $seen_current -eq 1; set done 1; end"),
                "--current" => format!("                            case {patterns}; if test $seen_panel -eq 1; set done 1; end"),
                "--workspace" => {
                    format!("                            case {patterns}; if test $seen_standalone -eq 1; set done 1; end")
                }
                "--standalone" => {
                    format!("                            case {patterns}; if test $seen_workspace -eq 1; set done 1; end")
                }
                "--profile" => {
                    format!("                            case {patterns}; if test $seen_exec -eq 1 -o $seen_arg -eq 1; set done 1; end")
                }
                "--exec" | "--arg" => {
                    format!("                            case {patterns}; if test $seen_profile -eq 1; set done 1; end")
                }
                _ => format!("                            case {patterns}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish after/operand admission arms.
fn after_flag_cases_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = flag_admission_patterns(flag, " ", true);
            match *flag {
                "--panel" => format!("                            case {patterns}; if test $seen_current -eq 1; or begin; test \"$family:$sub\" = panel:process-info; and test $state = operand; end; set done 1; end"),
                "--current" => format!("                            case {patterns}; if test $seen_panel -eq 1; or begin; test \"$family:$sub\" = panel:process-info; and test $state = operand; end; set done 1; end"),
                "--workspace" => {
                    format!("                            case {patterns}; if test $seen_standalone -eq 1; set done 1; end")
                }
                "--standalone" => {
                    format!("                            case {patterns}; if test $seen_workspace -eq 1; set done 1; end")
                }
                "--text" => {
                    format!("                            case {patterns}; if test $seen_key -eq 1 -o $seen_file -eq 1 -o $seen_stdin -eq 1; set done 1; end")
                }
                "--key" => {
                    format!("                            case {patterns}; if test $seen_text -eq 1 -o $seen_enter -eq 1 -o $seen_file -eq 1 -o $seen_stdin -eq 1; set done 1; end")
                }
                "--enter" => {
                    format!("                            case {patterns}; if test $seen_key -eq 1; set done 1; end")
                }
                "--file" => {
                    format!("                            case {patterns}; if test $seen_text -eq 1 -o $seen_key -eq 1 -o $seen_stdin -eq 1; set done 1; end")
                }
                "--stdin" => {
                    format!("                            case {patterns}; if test $seen_text -eq 1 -o $seen_key -eq 1 -o $seen_file -eq 1; set done 1; end")
                }
                "--profile" => {
                    format!("                            case {patterns}; if test $seen_exec -eq 1 -o $seen_arg -eq 1; set done 1; end")
                }
                "--exec" | "--arg" => {
                    format!("                            case {patterns}; if test $seen_profile -eq 1; set done 1; end")
                }
                _ => format!("                            case {patterns}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish verb-landing scope arms: the command's own declared flags admit the
/// consumed set — no per-command enumeration of the flags it rejects.
fn scope_cases_fish() -> String {
    FAMILY_VERBS
        .iter()
        .flat_map(|(family, verbs)| {
            verbs.iter().map(move |verb| {
                let command = format!("{family}:{verb}");
                let allowed = command_flags(&command).join(" ");
                let exclusive = if matches!(command.as_str(), "panel:current" | "panel:process-info") {
                    "\n                                        if test $seen_panel -eq 1 -a $seen_current -eq 1; set done 1; end".to_owned()
                } else if *family == "tab" {
                    "\n                                        if test $seen_workspace -eq 1 -a $seen_standalone -eq 1; set done 1; end".to_owned()
                } else if command == "terminal:send" {
                    "\n                                        if test (math \"$seen_text + $seen_key + $seen_file + $seen_stdin\") -ge 2 -o $seen_key -eq 1 -a $seen_enter -eq 1; set done 1; end".to_owned()
                } else {
                    String::new()
                };
                let exclusive = if command == "tab:create" || command == "panel:split" {
                    format!("{exclusive}\n                                        if test $seen_profile -eq 1 -a $seen_exec -eq 1; set done 1; end\n                                        if test $seen_profile -eq 1 -a $seen_arg -eq 1; set done 1; end")
                } else {
                    exclusive
                };
                format!("                                    case '{command}'\n                                        if not __orchbee_admit '{allowed}' $seen_flags; set done 1; end{exclusive}")
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish sub-state verb offers: a verb stays viable while every consumed flag
/// is inside that verb's declared scope. `agent` additionally offers
/// `integration` under the third level's union scope.
fn verb_cases_fish() -> String {
    FAMILY_VERBS
        .iter()
        .map(|(family, verbs)| {
            let mut lines = verbs
                .iter()
                .map(|verb| {
                    let command = format!("{family}:{verb}");
                    let allowed = command_flags(&command).join(" ");
                    // The exclusive pair must not both be seen for a tab verb
                    // to remain viable; `send` dies once `--key` stands next
                    // to a text payload bit.
                    if matches!(command.as_str(), "panel:current" | "panel:process-info") {
                        format!("                if __orchbee_admit '{allowed}' $seen_flags\n                    if test $seen_panel -eq 0 -o $seen_current -eq 0; set -a verbs {verb}; end\n                end")
                    } else if command == "tab:create" {
                        format!("                if __orchbee_admit '{allowed}' $seen_flags\n                    if test $seen_workspace -eq 0 -o $seen_standalone -eq 0\n                        if test $seen_profile -eq 0 -o $seen_exec -eq 0\n                            if test $seen_profile -eq 0 -o $seen_arg -eq 0; set -a verbs {verb}; end\n                        end\n                    end\n                end")
                    } else if *family == "tab" {
                        format!("                if __orchbee_admit '{allowed}' $seen_flags\n                    if test $seen_workspace -eq 0 -o $seen_standalone -eq 0; set -a verbs {verb}; end\n                end")
                    } else if command == "panel:split" {
                        format!("                if __orchbee_admit '{allowed}' $seen_flags\n                    if test $seen_profile -eq 0 -o $seen_exec -eq 0\n                        if test $seen_profile -eq 0 -o $seen_arg -eq 0; set -a verbs {verb}; end\n                    end\n                end")
                    } else if *verb == "send" {
                        format!("                if __orchbee_admit '{allowed}' $seen_flags\n                    if test (math \"$seen_text + $seen_key + $seen_file + $seen_stdin\") -lt 2\n                        if test $seen_key -eq 0 -o $seen_enter -eq 0; set -a verbs {verb}; end\n                    end\n                end")
                    } else {
                        format!("                if __orchbee_admit '{allowed}' $seen_flags; set -a verbs {verb}; end")
                    }
                })
                .collect::<Vec<_>>();
            if *family == "agent" {
                lines.push(format!(
                    "                if test $seen_provider2 -eq 0; and __orchbee_admit '{}' $seen_flags; set -a verbs integration; end",
                    INTEGRATION_COMMAND_FLAGS.join(" ")
                ));
            }
            format!("            case {family}\n{}", lines.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish verb-state narrowing through the `__orchbee_narrow` helper.
fn narrow_lines_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .filter_map(|flag| {
            let keep = families_of(flag).join(" ");
            match *flag {
                "--panel" => Some(format!("        if test $seen_panel -eq 1 -a $seen_current -eq 1\n            set possible\n        else if test $seen_panel -eq 1 -o $seen_current -eq 1\n            set possible (__orchbee_narrow \"{keep}\" $possible)\n        end")),
                "--current" => None,
                "--workspace" => Some(format!(
                    "        if test $seen_workspace -eq 1 -a $seen_standalone -eq 1\n            set possible\n        else if test $seen_workspace -eq 1 -o $seen_standalone -eq 1\n            set possible (__orchbee_narrow \"{keep}\" $possible)\n        end",
                )),
                "--standalone" => None,
                _ => Some(format!(
                    "        if test ${} -eq 1; set possible (__orchbee_narrow \"{keep}\" $possible); end",
                    flag_bit(flag)
                )),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish after/operand offers: unused business flags appended to candidates.
/// The four `agent:integration:*` commands share one scope — one glob arm.
fn extras_cases_fish() -> String {
    let mut integration_done = false;
    COMMAND_FLAGS
        .iter()
        .filter_map(|(command, flags)| {
            let label = if command.starts_with("agent:integration:") {
                if integration_done {
                    return None;
                }
                integration_done = true;
                "agent:integration:*"
            } else {
                command
            };
            let lines = flags
                .iter()
                .map(|flag| {
                    let cond = match *flag {
                        "--panel" | "--current" => "$seen_panel -eq 0 -a $seen_current -eq 0".to_owned(),
                        "--workspace" | "--standalone" => {
                            "$seen_workspace -eq 0 -a $seen_standalone -eq 0".to_owned()
                        }
                        "--text" => {
                            "$seen_text -eq 0 -a $seen_key -eq 0 -a $seen_file -eq 0 -a $seen_stdin -eq 0 -a $seen_paste -eq 0"
                                .to_owned()
                        }
                        "--paste" => "$seen_paste -eq 0 -a $seen_text -eq 0 -a $seen_key -eq 0".to_owned(),
                        "--key" => {
                            "$seen_text -eq 0 -a $seen_enter -eq 0 -a $seen_file -eq 0 -a $seen_stdin -eq 0 -a $seen_paste -eq 0".to_owned()
                        }
                        "--enter" => {
                            "(math \"$seen_text + $seen_file + $seen_stdin\") -ge 1 -a $seen_enter -eq 0"
                                .to_owned()
                        }
                        "--file" | "--stdin" => {
                            "$seen_text -eq 0 -a $seen_key -eq 0 -a $seen_file -eq 0 -a $seen_stdin -eq 0"
                                .to_owned()
                        }
                        // `--arg` is offered only while `--exec` stands and
                        // never beside `--profile`.
                        "--arg" if *command == "agent:start" => "0 -eq 0".to_owned(),
                        "--arg" => "$seen_exec -eq 1 -a $seen_profile -eq 0".to_owned(),
                        // `--profile` excludes the explicit launch pair.
                        "--profile" => {
                            "$seen_profile -eq 0 -a $seen_exec -eq 0 -a $seen_arg -eq 0".to_owned()
                        }
                        "--exec" => "$seen_exec -eq 0 -a $seen_profile -eq 0".to_owned(),
                        // `--until`, `--asset-root`, and doctor's
                        // `--provider` are repeatable — they stay offerable
                        // on every pass so another selector can follow.
                        // Integration's `--provider` is single-use: its seen
                        // bit gates the offer there.
                        "--provider" if command.starts_with("agent:integration:") => {
                            "$seen_provider -eq 0".to_owned()
                        }
                        "--provider" | "--asset-root" | "--until" => "0 -eq 0".to_owned(),
                        _ => format!("${} -eq 0", flag_bit(flag)),
                    };
                    if *command == "panel:process-info" && matches!(*flag, "--panel" | "--current") {
                        format!("                if test $state != operand; and test {cond}; set -a candidates {flag}; end")
                    } else { format!("                if test {cond}; set -a candidates {flag}; end") }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(format!("            case '{label}'\n{lines}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish direction-value offers.
fn direction_cases_fish() -> String {
    DIRECTION_VALUES
        .iter()
        .map(|(command, values)| {
            format!(
                "            case '{command}'; set candidates {}",
                values.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish typed `--enum=<prefix>` candidate arms — the same offer policy as
/// `joined_offer_cases_bash`: closed vocabulary only, `--flag=` spelling
/// retained, `sub` gated on the viable-verb list, `after|operand` on the
/// resolved command, `deep` on the integration union, `--direction` only
/// once its per-command vocabulary is known.
fn joined_offer_cases_fish() -> String {
    let joined = |flag: &str, values: &[&str]| {
        values
            .iter()
            .map(|value| format!("{flag}={value}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let direction = DIRECTION_VALUES
        .iter()
        .map(|(command, values)| {
            format!(
                "                    case '{command}'; set candidates {}",
                joined("--direction", values)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let key = joined("--key", KEY_VALUES);
    let until = joined("--until", UNTIL_VALUES);
    let provider = joined("--provider", PROVIDER_VALUES);
    format!(
        concat!(
            "            case '--direction'\n",
            "                if test $seen_direction -eq 0\n",
            "                    switch \"$state\"\n",
            "                        case after operand operand2\n",
            "                            switch \"$family:$sub\"\n",
            "{direction}\n",
            "                            end\n",
            "                    end\n",
            "                end\n",
            "            case '--key'\n",
            "                if test $seen_key -eq 0 -a $seen_text -eq 0 -a $seen_enter -eq 0 -a $seen_file -eq 0 -a $seen_stdin -eq 0\n",
            "                    switch \"$state\"\n",
            "                        case sub; if contains -- send $verbs; set candidates {key}; end\n",
            "                        case after operand operand2; if test \"$family:$sub\" = 'terminal:send'; set candidates {key}; end\n",
            "                    end\n",
            "                end\n",
            "            case '--until'\n",
            "                switch \"$state\"\n",
            "                    case sub; if contains -- wait $verbs; set candidates {until}; end\n",
            "                    case after operand operand2; if test \"$family:$sub\" = 'agent:wait'; set candidates {until}; end\n",
            "                end\n",
            "            case '--provider'\n",
            "                switch \"$state\"\n",
            "                    case sub; if contains -- doctor $verbs; or contains -- integration $verbs; set candidates {provider}; end\n",
            "                    case deep; if test $seen_provider -eq 0; set candidates {provider}; end\n",
            "                    case after operand operand2\n",
            "                        switch \"$family:$sub\"\n",
            "                            case 'agent:doctor'; set candidates {provider}\n",
            "                            case 'agent:integration:*'; if test $seen_provider -eq 0; set candidates {provider}; end\n",
            "                        end\n",
            "                end\n",
        ),
        direction = direction,
        key = key,
        until = until,
        provider = provider,
    )
}

/// The fish "no option was consumed" condition: the business list is empty
/// and neither orthogonal option bit stands.
fn no_options_fish() -> &'static str {
    "(count $seen_flags) -eq 0 -a $seen_format -eq 0 -a $seen_mouse -eq 0"
}

fn fish() -> String {
    format!(
        concat!(
            "# fish completion for orchbee — a static projection of the implemented\n",
            "# grammar. No dynamic state: no Site, resource, or runtime enumeration.\n",
            "function __orchbee_narrow\n",
            "    for word in $argv[2..]\n",
            "        if contains -- $word (string split ' ' -- $argv[1]); echo $word; end\n",
            "    end\n",
            "end\n",
            "# argv[1] is the resolved command's allowed flags joined by spaces;\n",
            "# argv[2..] is every consumed business flag. Succeeds iff each consumed\n",
            "# flag is inside the allowed list — both sides arrive as parameters, so\n",
            "# no caller-local variable is read implicitly.\n",
            "function __orchbee_admit\n",
            "    set -l allowed (string split ' ' -- $argv[1])\n",
            "    for f in $argv[2..]\n",
            "        contains -- $f $allowed; or return 1\n",
            "    end\n",
            "    return 0\n",
            "end\n",
            "function __orchbee_candidates\n",
            "    set -l words (commandline -opc)\n",
            "    set -l cur (commandline -ct)\n",
            "    set -e words[1]\n",
            "    set -l state verb; set -l help_a \"\"; set -l help_b \"\"; set -l help_c \"\"\n",
            "    set -l family \"\"; set -l sub \"\"; set -l leaf \"\"\n",
            "    set -l seen_format 0; set -l seen_mouse 0; {seen_decls}; set -l seen_provider2 0; set -l seen_flags\n",
            "    set -l ended 0; set -l done 0; set -l value \"\"\n",
            "    for word in $words\n",
            "        if string match -q -- \"--\" \"$word\"; set ended 1; continue; end\n",
            "        if test $ended -eq 1 -o $done -eq 1; continue; end\n",
            "        if test -n \"$value\"\n",
            // `test` reads a typed dash-word inside a `-a`/`-o` compound as an
            // operator (fish 4.2.1 rejects `--arg -c` here), so `$word`
            // comparisons stay split into standalone `test` job chains.
            "            if test \"$value\" = format; and test \"$word\" != text; and test \"$word\" != json; set done 1; end\n",
            "            if test \"$value\" = mouse; and test \"$word\" != off; and test \"$word\" != on; set done 1; end\n",
            "            set value \"\"\n",
            "            continue\n",
            "        end\n",
            "        switch \"$word\"\n",
            "            case '--format' '--format=*'\n",
            "                switch \"$state\"\n",
            "                    case verb sub deep after operand operand2\n",
            "                    case leaf; if test \"$leaf\" != status; set done 1; end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$word\"\n",
            "                        case '--format'\n",
            "                            if test $seen_format -eq 1; set done 1; else; set seen_format 1; set value format; end\n",
            "                        case '--format=text' '--format=json'\n",
            "                            if test $seen_format -eq 1; set done 1; else; set seen_format 1; end\n",
            "                        case '*'; set done 1\n",
            "                    end\n",
            "                end\n",
            "            case '--mouse' '--mouse=*'\n",
            "                # Both spellings are the grammar — the two-token form\n",
            "                # leaves a pending value, the joined form carries it and\n",
            "                # stays inside the closed {{off,on}} vocabulary.\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                    case leaf; if test \"$leaf\" != tui; set done 1; end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    if test $seen_mouse -eq 1\n",
            "                        set done 1\n",
            "                    else\n",
            "                        switch \"$word\"\n",
            "                            case '--mouse'; set seen_mouse 1; set value mouse\n",
            "                            case '--mouse=off' '--mouse=on'; set seen_mouse 1\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    end\n",
            "                end\n",
            "            case {flag_patterns}\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "{sub_flag_cases}\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case deep\n",
            "                        switch \"$family:$sub:$word\"\n",
            "{deep_flag_cases}\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case after operand operand2\n",
            "                        switch \"$family:$sub:$word\"\n",
            "{after_flag_cases}\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$word\"\n",
            "{seen_cases}\n",
            "                    end\n",
            "                end\n",
            "            case '--*=*'\n",
            "                # A joined `--flag=value` business token: the flag part\n",
            "                # drives the same state admission and bookkeeping as the\n",
            "                # two-token spelling, and the inline value never claims\n",
            "                # the next word. Booleans and unknown names stay dead.\n",
            "                set -l flag_name (string split -m1 '=' -- $word)[1]\n",
            "                switch \"$flag_name\"\n",
            "                    case {value_flag_cases}\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$state\"\n",
            "                        case verb\n",
            "                        case sub\n",
            "                            switch \"$family:$flag_name\"\n",
            "{sub_flag_cases}\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        case deep\n",
            "                            switch \"$family:$sub:$flag_name\"\n",
            "{deep_flag_cases}\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        case after operand operand2\n",
            "                            switch \"$family:$sub:$flag_name\"\n",
            "{after_flag_cases}\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        case '*'; set done 1\n",
            "                    end\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$flag_name\"\n",
            "{joined_seen_cases}\n",
            "                    end\n",
            "                end\n",
            "            case '--help' '--version' '--skill'; set done 1\n",
            "            case '-*'; set done 1\n",
            "            case '*'\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                        switch \"$word\"\n",
            "                            case status\n",
            "                                if test (count $seen_flags) -gt 0 -o $seen_mouse -eq 1\n",
            "                                    set done 1\n",
            "                                else; set leaf status; set state leaf; end\n",
            "                            case tui\n",
            "                                if test $seen_format -eq 1 -o (count $seen_flags) -gt 0\n",
            "                                    set done 1\n",
            "                                else; set leaf tui; set state leaf; end\n",
            "                            case help completion\n",
            "                                if test $seen_format -eq 1 -o (count $seen_flags) -gt 0 -o $seen_mouse -eq 1\n",
            "                                    set done 1\n",
            "                                else; set state $word; end\n",
            "                            case {families}; if test $seen_mouse -eq 1; set done 1; else; set family $word; set state sub; end\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "                            case 'agent:integration'\n",
            "                                set sub integration; set state deep\n",
            "                                if test $seen_provider2 -eq 1; or not __orchbee_admit '{integration_allowed}' $seen_flags; set done 1; end\n",
            "                            case {paths}\n",
            "                                set sub $word; set state after\n",
            "                                switch \"$family:$sub\"\n",
            "{scope_cases}\n",
            "                                end\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case deep\n",
            "                        switch \"$family:$sub:$word\"\n",
            "                            case {deep_verb_cases}\n",
            "                                set sub \"integration:$word\"; set state after\n",
            "                                if test $seen_provider2 -eq 1; or not __orchbee_admit '{integration_allowed}' $seen_flags; set done 1; end\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case after\n",
            "                        switch \"$family:$sub\"\n",
            "                            case panel:process-info; if test $seen_panel -eq 1 -o $seen_current -eq 1; set done 1; else; set state operand; end\n",
            "                            case {operand_paths}; set state operand\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case operand; if contains -- \"$family:$sub\" agent:send-keys agent:start; set state operand; else if contains -- \"$family:$sub\" agent:rename agent:prompt; and test $seen_clear -eq 0; and test $seen_name -eq 0; and test $seen_session -eq 0; set state operand2; else; set done 1; end; end\n",
            "                    case leaf operand2 completion; set done 1\n",
            "                    case help\n",
            "                        if test -z \"$help_a\"\n",
            "                            switch \"$word\"\n",
            "                                case {topics}; set help_a $word\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        else if test -z \"$help_b\"\n",
            "                            switch \"$help_a:$word\"\n",
            "                                case {help_paths}; set help_b $word\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        else if test -z \"$help_c\"\n",
            "                            switch \"$help_a:$help_b:$word\"\n",
            "                                case {deep_verb_cases}; set help_c $word\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        else; set done 1; end\n",
            "                end\n",
            "        end\n",
            "    end\n",
            "    set -l possible; set -l verbs; set -l dead 0\n",
            "    if test \"$state\" = verb\n",
            "        set possible {roots}\n",
            "{narrow_lines}\n",
            "        if test $seen_mouse -eq 1; set possible (__orchbee_narrow \"tui\" $possible); end\n",
            "        if test $seen_format -eq 1; set possible (__orchbee_narrow \"{format_roots}\" $possible); end\n",
            "        if test (count $possible) -eq 0; set dead 1; end\n",
            "    else if test \"$state\" = sub\n",
            "        switch \"$family\"\n",
            "{verb_cases}\n",
            "        end\n",
            "        if test (count $verbs) -eq 0; set dead 1; end\n",
            "    else if test \"$state\" = deep\n",
            "        if test $seen_provider2 -eq 0; and __orchbee_admit '{integration_allowed}' $seen_flags; set verbs {integration_verb_words}; end\n",
            "        if test (count $verbs) -eq 0; set dead 1; end\n",
            "    end\n",
            "    set -l candidates\n",
            "    if test $done -eq 1 -o $ended -eq 1 -o $dead -eq 1; set candidates\n",
            "    else if test \"$value\" = format; set candidates {format_values}\n",
            "    else if test \"$value\" = mouse; set candidates off on\n",
            "    else if test \"$value\" = direction\n",
            "        switch \"$family:$sub\"\n",
            "{direction_cases}\n",
            "        end\n",
            "    else if test \"$value\" = key; set candidates {key_values}\n",
            "    else if test \"$value\" = until; set candidates {until_values}\n",
            "    else if test \"$value\" = provider; set candidates {provider_values}\n",
            "    else if test -n \"$value\"; set candidates\n",
            "    else if string match -q -- '--format=*' $cur\n",
            "        if test $seen_format -eq 0\n",
            "            switch \"$state\"\n",
            "                case verb; if test $seen_mouse -eq 0; set candidates --format=text --format=json; end\n",
            "                case sub deep after operand operand2; set candidates --format=text --format=json\n",
            "                case leaf; if test \"$leaf\" = status; set candidates --format=text --format=json; end\n",
            "            end\n",
            "        end\n",
            "    else if string match -q -- '--*=*' $cur\n",
            "        # A typed `--enum=<prefix>` completes only that flag's own closed\n",
            "        # vocabulary with the `--flag=` spelling preserved; a freeform\n",
            "        # flag's value is never guessed, and a boolean or unknown name\n",
            "        # offers nothing.\n",
            "        set -l cur_name (string split -m1 '=' -- $cur)[1]\n",
            "        switch \"$cur_name\"\n",
            "            case '--mouse'\n",
            "                if test $seen_mouse -eq 0\n",
            "                    switch \"$state\"\n",
            "                        case verb; if contains -- tui $possible; set candidates --mouse=off --mouse=on; end\n",
            "                        case leaf; if test \"$leaf\" = tui; set candidates --mouse=off --mouse=on; end\n",
            "                    end\n",
            "                end\n",
            "{joined_offer_cases}\n",
            "        end\n",
            "    else if test \"$state\" = verb\n",
            "        set candidates $possible\n",
            "        if test $seen_format -eq 0 -a $seen_mouse -eq 0; set -a candidates --format --format=; end\n",
            "        if contains -- tui $possible; and test $seen_mouse -eq 0; set -a candidates --mouse; end\n",
            "        if {no_options}\n",
            "            set -a candidates --help --version --skill\n",
            "        end\n",
            "    else if test \"$state\" = sub\n",
            "        set candidates $verbs\n",
            "        if test $seen_format -eq 0; set -a candidates --format --format=; end\n",
            "    else if test \"$state\" = deep\n",
            "        set candidates $verbs\n",
            "        if test $seen_format -eq 0; set -a candidates --format --format=; end\n",
            "    else if test \"$state\" = leaf\n",
            "        if test \"$leaf\" = tui -a $seen_mouse -eq 0; set candidates --help --mouse\n",
            "        else if test $seen_format -eq 0; set candidates --format --format= --help; end\n",
            "    else if test \"$state\" = after -o \"$state\" = operand\n",
            "        if test $seen_format -eq 0; set candidates --format --format=; end\n",
            "        switch \"$family:$sub\"\n",
            "{extras_cases}\n",
            "        end\n",
            "        if test \"$state\" = after; and {no_options}\n",
            "            set -a candidates --help\n",
            "        end\n",
            "    else if test $state = completion; set candidates {shells} --help\n",
            "    else if test $state = help\n",
            "        if test -z \"$help_a\"; set candidates {topics}\n",
            "        else if test -z \"$help_b\"\n",
            "            switch \"$help_a\"\n",
            "{help_cases}\n",
            "            end\n",
            "        else if test -z \"$help_c\"\n",
            "            switch \"$help_a:$help_b\"\n",
            "{help_level3_cases}\n",
            "            end\n",
            "        end\n",
            "    end\n",
            "    printf '%s\\n' $candidates\n",
            "end\n",
            "complete -c orchbee -f -a '(__orchbee_candidates)'\n"
        ),
        families = FAMILY_VERBS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>()
            .join(" "),
        paths = path_patterns(FAMILY_VERBS).replace('|', " "),
        operand_paths = OPERAND_PATHS.join(" "),
        topics = HELP_TOPICS.join(" "),
        help_paths = path_patterns(HELP_CHILDREN).replace('|', " "),
        format_values = FORMAT_VALUES.join(" "),
        key_values = KEY_VALUES.join(" "),
        until_values = UNTIL_VALUES.join(" "),
        provider_values = PROVIDER_VALUES.join(" "),
        roots = ROOT_WORDS.join(" "),
        format_roots = FORMAT_ROOT_WORDS.join(" "),
        shells = SHELLS.join(" "),
        flag_patterns = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("'{flag}'"))
            .collect::<Vec<_>>()
            .join(" "),
        seen_decls = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("set -l {} 0", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join("; "),
        sub_flag_cases = sub_flag_cases_fish(),
        deep_flag_cases = deep_flag_cases_fish(),
        deep_verb_cases = deep_verb_cases_fish(),
        integration_allowed = INTEGRATION_COMMAND_FLAGS.join(" "),
        integration_verb_words = INTEGRATION_VERBS.join(" "),
        after_flag_cases = after_flag_cases_fish(),
        seen_cases = seen_cases_fish(false),
        value_flag_cases = value_business_flags()
            .map(|flag| format!("'{flag}'"))
            .collect::<Vec<_>>()
            .join(" "),
        joined_seen_cases = seen_cases_fish(true),
        joined_offer_cases = joined_offer_cases_fish(),
        scope_cases = scope_cases_fish(),
        verb_cases = verb_cases_fish(),
        narrow_lines = narrow_lines_fish(),
        extras_cases = extras_cases_fish(),
        direction_cases = direction_cases_fish(),
        no_options = format!("test {}", no_options_fish()),
        help_cases = HELP_CHILDREN
            .iter()
            .map(|(topic, children)| format!(
                "            case {topic}; set candidates {}",
                children.join(" ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        help_level3_cases = help_level3_cases_fish(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BUSINESS_FLAGS, COMMAND_FLAGS, DIRECTION_VALUES, FAMILY_VERBS, FORMAT_VALUES,
        HELP_CHILDREN, HELP_LEVEL3, HELP_TOPICS, INTEGRATION_COMMAND_FLAGS, INTEGRATION_VERBS,
        OPERAND_PATHS, ROOT_WORDS, command_flags, families_of, render,
    };
    use crate::invocation::CompletionShell;

    #[test]
    fn scripts_cover_the_implemented_surface() {
        for shell in [
            CompletionShell::Bash,
            CompletionShell::Zsh,
            CompletionShell::Fish,
        ] {
            let script = render(shell);
            for word in ROOT_WORDS {
                assert!(script.contains(word), "{shell:?} lacks {word}");
            }
            for (family, verbs) in FAMILY_VERBS {
                for verb in *verbs {
                    assert!(script.contains(verb), "{shell:?} lacks {family} {verb}");
                }
            }
            for flag in BUSINESS_FLAGS {
                assert!(script.contains(flag), "{shell:?} lacks {flag}");
            }
            assert!(script.contains("--format"));
            assert!(script.contains("--mouse"), "{shell:?} lacks --mouse");
            for value in FORMAT_VALUES {
                assert!(script.contains(value), "{shell:?} lacks {value}");
            }
            assert!(script.contains("off"), "{shell:?} lacks --mouse values");
            assert!(script.contains("bash"));
            assert!(script.contains("zsh"));
            assert!(script.contains("fish"));
            assert!(script.contains("--"), "{shell:?} lacks terminator handling");
        }
        // Every help child path begins with an implemented topic; `agent
        // integration` is a sub-family, not a verb, so it is validated
        // against INTEGRATION_VERBS' owner instead.
        for (topic, children) in HELP_CHILDREN {
            assert!(HELP_TOPICS.contains(topic), "{topic}");
            for child in *children {
                let implemented = FAMILY_VERBS
                    .iter()
                    .any(|(family, verbs)| family == topic && verbs.contains(child))
                    || (*topic == "agent" && *child == "integration");
                assert!(implemented, "{topic} {child}");
            }
        }
        // Third-level help paths hang off an implemented topic:child pair.
        for (path, words) in HELP_LEVEL3 {
            let (topic, child) = path.split_once(':').expect("topic:child");
            assert!(
                HELP_CHILDREN
                    .iter()
                    .any(|(t, c)| *t == topic && c.contains(&child)),
                "{path}"
            );
            assert!(!words.is_empty(), "{path}");
        }
    }

    #[test]
    fn path_tables_agree_with_the_grammar() {
        // Every operand path is an implemented verb.
        for path in OPERAND_PATHS {
            let (family, verb) = path.split_once(':').expect("family:verb");
            assert!(
                FAMILY_VERBS
                    .iter()
                    .any(|(f, verbs)| *f == family && verbs.contains(&verb)),
                "{path} is not an implemented command"
            );
        }
        assert!(OPERAND_PATHS.contains(&"api:schema"));
        // Every scoped command is an implemented verb (the third-level
        // `agent:integration:*` keys split off their last segment), and every
        // scoped flag is a declared business flag.
        for (command, flags) in COMMAND_FLAGS {
            let (family, verb) = command.rsplit_once(':').expect("family:verb");
            let implemented = FAMILY_VERBS
                .iter()
                .any(|(f, verbs)| *f == family && verbs.contains(&verb))
                || (family == "agent:integration" && INTEGRATION_VERBS.contains(&verb));
            assert!(implemented, "{command} is not an implemented command");
            for flag in *flags {
                assert!(BUSINESS_FLAGS.contains(flag), "{command} lists {flag}");
            }
        }
        // The integration union scope is exactly what each of its verbs
        // declares.
        for verb in INTEGRATION_VERBS {
            assert_eq!(
                command_flags(&format!("agent:integration:{verb}")),
                INTEGRATION_COMMAND_FLAGS,
                "agent:integration:{verb} scope"
            );
        }
        // Direction values hang off commands that actually admit --direction.
        for (command, values) in DIRECTION_VALUES {
            assert!(
                command_flags(command).contains(&"--direction"),
                "{command} lacks --direction"
            );
            assert!(!values.is_empty(), "{command} lacks direction words");
        }
        // Every flag has at least one owning family.
        for flag in BUSINESS_FLAGS {
            assert!(!families_of(flag).is_empty(), "{flag} has no owner");
        }
    }

    #[test]
    fn launch_flags_are_scoped_and_repeatable() {
        // Explicit program selection belongs only to terminal launches and stored profiles.
        for (command, flags) in COMMAND_FLAGS {
            let admits = *command == "workspace:create"
                || *command == "tab:create"
                || *command == "panel:split"
                || *command == "launch-profile:create";
            assert_eq!(flags.contains(&"--exec"), admits, "{command} --exec");
            assert_eq!(
                flags.contains(&"--arg"),
                admits || *command == "agent:start",
                "{command} --arg"
            );
        }
        for shell in [
            CompletionShell::Bash,
            CompletionShell::Zsh,
            CompletionShell::Fish,
        ] {
            let script = render(shell);
            // `--exec` is one-shot: its seen arm re-enters `done`.
            // `--arg` is repeatable: its arm consumes a value without
            // touching `done`, and the offer is gated on `--exec`.
            let (exec_arm, arg_arm, arg_offer) = match shell {
                CompletionShell::Fish => (
                    "case '--exec'\n                            if test $seen_exec -eq 1; set done 1",
                    "case '--arg'\n                            set seen_arg 1; set -a seen_flags '--arg'; set value filter",
                    "if test $seen_exec -eq 1 -a $seen_profile -eq 0; set -a candidates --arg; end",
                ),
                _ => (
                    "--exec)\n                            if ((seen_exec)); then done=1",
                    "--arg)\n                            seen_arg=1; value=filter; seen_flags+=(--arg) ;;",
                    "((!seen_exec || seen_profile)) || extras=\"$extras --arg\"",
                ),
            };
            assert!(script.contains(exec_arm), "{shell:?} --exec arm");
            assert!(script.contains(arg_arm), "{shell:?} --arg arm");
            assert!(script.contains(arg_offer), "{shell:?} --arg offer");
        }
    }

    /// fish's `test` misreads a leading-dash operand inside a compound
    /// `-a`/`-o` expression as an operator (witnessed on fish 4.2.1:
    /// `test "$value" = format -a "$word" != text` fails once `$word` is
    /// `-c`). `$word` is the only variable that can hold a typed word, so it
    /// must never appear inside a compound `test`; `-a`/`-o` compounds over
    /// fixed 0/1 `seen_*` operands are valid and remain.
    #[test]
    fn fish_compound_tests_have_no_user_word_operands() {
        let script = render(CompletionShell::Fish);
        let mut compounds = 0;
        for line in script.lines() {
            for segment in line.split(';') {
                if !segment.contains("test ")
                    || !(segment.contains(" -a ") || segment.contains(" -o "))
                {
                    continue;
                }
                compounds += 1;
                assert!(
                    !segment.contains("$word"),
                    "compound test carries typed-word operand: {line}"
                );
            }
        }
        assert!(compounds > 0, "native -a/-o compounds should remain");
    }
}
