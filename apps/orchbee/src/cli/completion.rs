//! Static completion scripts for the implemented grammar. The POSIX shells
//! run the same state machine — a faithful projection of `invocation::scan`/
//! `resolve` — and emit only the candidates the parser could still accept.
//! The scripts are emitted to stdout only; nothing is installed, no user
//! files are read or written, and no resource or Site state is ever
//! enumerated.
//!
//! Walker state (per completed word):
//! - `state` — positional position: verb | sub | deep | after | operand | help |
//!   completion (`deep` covers `agent integration` and `terminal session`)
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
//!    (verb/sub/after/operand, bare `status`); a second selector or an
//!    invalid value is usage
//! 5. `--mouse`/`--mouse=off|on` → the TUI/attach option: legal at verb
//!    (only `tui` stays viable afterward) or on the `tui` leaf; a second
//!    `--mouse` or an invalid value — joined or separate — is usage
//! 6. business flags → legal iff a command reachable here admits the flag:
//!    verb records it (checked when the command lands), sub requires the
//!    flag's owning family, deep admits its nested family's union scope,
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
//!    - verb: family→sub | `status`→leaf (optional client/server child; fails if a business flag or
//!      `--mouse` was seen) | `tui`→leaf (fails if a business flag or
//!      `--format` was seen) | `help`→help / `completion`→completion (both
//!      fail if any option was seen); only agent/terminal families admit `--mouse`
//!    - sub: family:verb → after, or a nested family → deep; the verb's
//!      declared flag scope must cover everything already seen
//!    - deep: nested family plus verb → after under its declared scope
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
//!   admitted (`--mouse=` under a viable TUI or attach; `--direction=` once the
//!   command resolves; `--key=`/`--until=`/`--provider=` inside their
//!   owning scopes); a freeform flag's value is never guessed
//! - verb → root words still viable under the seen business flags + format
//!   flags + `--help`/`--version` only when nothing was consumed
//! - sub → verbs whose flag scope covers the seen flags + format flags
//! - deep → `agent integration` verbs while the seen flags stay inside the
//!   union scope + format flags
//! - leaf → `status`: format flags + `--help` when nothing was seen; `tui`:
//!   `--help` and `--mouse` only while `--mouse` is unconsumed — a consumed
//!   `--mouse` leaves no further options on the TUI leaf
//! - after → format flags + the command's unused business flags + `--help`
//!   on every command path with no option seen; the operand-taking `get`/
//!   `api schema`/mutation paths take a legal suffix help before their
//!   operand
//! - operand → format flags + the command's unused business flags
//! - help → help topics / children
//! - completion → shells + `--help`

mod elvish;
mod grammar;
mod powershell;

#[cfg(all(test, windows))]
mod test_cases;

use crate::invocation::CompletionShell;

/// Top-level words the grammar accepts before any option.
const ROOT_WORDS: &[&str] = &[
    "layout",
    "client",
    "agent",
    "api",
    "completion",
    "config",
    "help",
    "panel",
    "project",
    #[cfg(windows)]
    "session",
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
    "layout",
    "client",
    "agent",
    "api",
    "config",
    "panel",
    "project",
    #[cfg(windows)]
    "session",
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
            "attach",
            "attention",
            "changes",
            "doctor",
            "explain",
            "focus",
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
    ("client", &["control-status", "get", "list"]),
    #[cfg(windows)]
    ("layout", &["apply", "export", "set-split-ratio"]),
    #[cfg(not(windows))]
    ("layout", &["set-split-ratio"]),
    ("launch-profile", &["create", "delete", "get", "list"]),
    ("config", &["check", "reset-keys"]),
    (
        "panel",
        &[
            #[cfg(windows)]
            "create-terminal",
            "report-agent",
            "report-agent-session",
            "clear-agent-authority",
            "release-agent",
            "close",
            "current",
            "get",
            "list",
            "move",
            "report-metadata",
            "process-info",
            "split",
            "focus",
            "zoom",
            "layout",
            "neighbor",
            "edges",
            "resize",
            "rename",
            "swap",
            "input",
        ],
    ),
    ("project", &["create", "get", "list", "rename"]),
    #[cfg(windows)]
    ("session", &["list", "attach", "stop", "delete"]),
    (
        "site",
        &[
            "diagnose",
            "start",
            "stop",
            "stop-status",
            "agent-manifests",
            "reload-agent-manifests",
            "update-agent-manifests",
            "agent-manifests-status",
        ],
    ),
    ("status", &["client", "server"]),
    (
        "tab",
        &["close", "create", "focus", "get", "list", "move", "rename"],
    ),
    (
        "terminal",
        &[
            "attach",
            "get",
            "list",
            "read",
            "run",
            "send",
            "session",
            "wait-output",
        ],
    ),
    (
        "workspace",
        &[
            "accept-directory",
            "report-metadata",
            "move",
            "move-block",
            "close",
            "close-resume",
            "close-status",
            "create",
            "focus",
            "get",
            "list",
            "rename",
        ],
    ),
    (
        "worktree",
        &["create", "list", "open", "remove", "resume", "status"],
    ),
];

/// Every business flag the grammar admits (`--format` is orthogonal), in the
/// order the state machine records them.
const BUSINESS_FLAGS: &[&str] = &[
    #[cfg(windows)]
    "--child",
    "--presentation",
    "--agent",
    "--state",
    "--message",
    "--agent-session-id",
    "--agent-session-path",
    "--session-start-source",
    "--applies-to-source",
    "--title",
    "--display-agent",
    "--state-label",
    "--clear-title",
    "--clear-display-agent",
    "--clear-state-labels",
    "--token",
    "--clear-token",
    "--seq",
    "--ttl-ms",
    "--right-click",
    "--focus",
    "--no-focus",
    "--amount",
    "--ratio",
    "--output",
    "--client",
    "--on",
    "--off",
    "--toggle",
    "--group",
    "--branch",
    "--base",
    "--force",
    "--takeover",
    "--trust-repository",
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
    "--insert-index",
    "--before",
    "--cols",
    "--rows",
    "--cell-width",
    "--cell-height",
    "--confirm",
    "--panel",
    "--current",
    "--with",
    "--target-panel",
    "--into-tab",
    "--new-tab",
    "--new-workspace",
    "--tab-label",
    "--history",
    "--path",
    "--match",
    "--regex",
    "--unwrap",
    "--source",
    "--plain",
    "--ansi",
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
    "--outdated-only",
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
    "--clear",
    "--wait",
    "--forever",
    "--adapter-source",
    "--request",
    "--generation",
];

/// The business flags every `agent integration <verb>` admits — also the
/// union scope checked when the `agent integration` sub-family itself lands.
const INTEGRATION_COMMAND_FLAGS: &[&str] = &["--provider", "--adapter-source", "--outdated-only"];

/// `agent integration`'s third-level verbs, offered at the `deep` state.
const INTEGRATION_VERBS: &[&str] = &["disable", "enable", "explain", "list"];

/// Business flags whose seen bit never makes a repeat `done` — they stay
/// offerable and keep consuming values while remaining scope-tracked.
const REPEATABLE_FLAGS: &[&str] = &[
    "--state-label",
    "--token",
    "--clear-token",
    "--env",
    "--arg",
    "--asset-root",
    "--provider",
    "--until",
    "--key",
];

/// `<family>:<verb>` (or `agent:integration:<verb>`) → the business flags
/// that command admits. Commands not listed here admit none.
const CONTROL_FLAGS: &[&str] = &["--cols", "--rows", "--timeout", "--takeover"];
const OBSERVE_FLAGS: &[&str] = &["--cols", "--rows", "--timeout"];
const COMMAND_FLAGS: &[(&str, &[&str])] = &[
    #[cfg(windows)]
    (
        "layout:apply",
        &[
            "--file",
            "--stdin",
            "--tab",
            "--confirm",
            "--workspace",
            "--standalone",
            "--label",
            "--client",
            "--focus",
            "--no-focus",
            "--timeout",
        ],
    ),
    #[cfg(windows)]
    (
        "layout:export",
        &["--tab", "--panel", "--current", "--client", "--timeout"],
    ),
    #[cfg(windows)]
    ("session:attach", &["--mouse"]),
    #[cfg(windows)]
    ("session:delete", &["--confirm"]),
    #[cfg(windows)]
    (
        "session:stop",
        &[
            "--confirm",
            "--request-id",
            "--retry",
            "--detach",
            "--timeout",
        ],
    ),
    ("terminal:attach", &["--takeover", "--timeout", "--mouse"]),
    (
        "agent:attach",
        &[
            "--takeover",
            "--timeout",
            "--mouse",
            "--name",
            "--session",
            "--panel",
        ],
    ),
    ("terminal:session", CONTROL_FLAGS),
    ("terminal:session:observe", OBSERVE_FLAGS),
    ("terminal:session:control", CONTROL_FLAGS),
    (
        "panel:resize",
        &[
            "--panel",
            "--current",
            "--direction",
            "--amount",
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
        ],
    ),
    (
        "layout:set-split-ratio",
        &[
            "--tab",
            "--panel",
            "--current",
            "--path",
            "--ratio",
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
        ],
    ),
    (
        "panel:layout",
        &[
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
            "--panel",
            "--current",
        ],
    ),
    (
        "panel:neighbor",
        &[
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
            "--panel",
            "--current",
            "--direction",
        ],
    ),
    (
        "panel:edges",
        &[
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
            "--panel",
            "--current",
        ],
    ),
    ("api:schema", &["--output"]),
    ("client:list", &[]),
    ("client:get", &[]),
    ("client:control-status", &[]),
    (
        "workspace:focus",
        &["--client", "--request-id", "--detach", "--timeout"],
    ),
    (
        "tab:focus",
        &["--client", "--request-id", "--detach", "--timeout"],
    ),
    (
        "panel:focus",
        &[
            "--direction",
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
            "--panel",
            "--current",
        ],
    ),
    (
        "panel:zoom",
        &[
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
            "--panel",
            "--current",
            "--on",
            "--off",
            "--toggle",
        ],
    ),
    ("site:reload-agent-manifests", &["--request-id"]),
    ("site:update-agent-manifests", &["--request-id"]),
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
    (
        "agent:list",
        &["--include-ended", "--limit", "--cursor", "--client"],
    ),
    ("agent:changes", &["--limit", "--cursor"]),
    ("agent:get", &["--session", "--name", "--panel", "--client"]),
    (
        "agent:focus",
        &[
            "--session",
            "--name",
            "--panel",
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
        ],
    ),
    (
        "agent:attention",
        &[
            "--session",
            "--name",
            "--panel",
            "--client",
            "--request-id",
            "--detach",
            "--timeout",
        ],
    ),
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
            "--source",
            "--plain",
            "--ansi",
        ],
    ),
    ("agent:send-keys", &["--session", "--name", "--panel"]),
    (
        "agent:prompt",
        &[
            "--session",
            "--panel",
            "--name",
            "--client",
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
            "--client",
            "--panel",
            "--name",
            "--until",
            "--after-revision",
            "--forever",
            "--timeout",
        ],
    ),
    (
        "agent:integration:list",
        &["--provider", "--adapter-source", "--outdated-only"],
    ),
    (
        "agent:integration:explain",
        &["--provider", "--adapter-source"],
    ),
    (
        "agent:integration:enable",
        &["--provider", "--adapter-source"],
    ),
    (
        "agent:integration:disable",
        &["--provider", "--adapter-source"],
    ),
    ("project:create", &["--label"]),
    ("project:rename", &["--label"]),
    ("workspace:accept-directory", &["--path"]),
    #[cfg(not(windows))]
    ("workspace:list", &["--project"]),
    #[cfg(windows)]
    ("workspace:list", &["--project", "--client"]),
    #[cfg(windows)]
    ("workspace:get", &["--client"]),
    (
        "workspace:create",
        &[
            "--workspace",
            "--client",
            "--focus",
            "--no-focus",
            "--timeout",
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
    (
        "workspace:report-metadata",
        &["--source", "--token", "--clear-token", "--seq", "--ttl-ms"],
    ),
    ("workspace:move", &["--insert-index"]),
    ("workspace:move-block", &["--before"]),
    (
        "workspace:close",
        &[
            "--group",
            "--confirm",
            "--request-id",
            "--detach",
            "--timeout",
        ],
    ),
    (
        "workspace:close-status",
        &["--project", "--workspace", "--group", "--request"],
    ),
    (
        "workspace:close-resume",
        &[
            "--group",
            "--detach",
            "--timeout",
            "--project",
            "--workspace",
            "--request",
            "--generation",
            "--confirm",
        ],
    ),
    #[cfg(not(windows))]
    ("tab:list", &["--workspace", "--standalone"]),
    #[cfg(windows)]
    ("tab:list", &["--workspace", "--standalone", "--client"]),
    #[cfg(windows)]
    ("tab:get", &["--client"]),
    (
        "tab:create",
        &[
            "--focus",
            "--no-focus",
            "--client",
            "--timeout",
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
    #[cfg(windows)]
    (
        "panel:create-terminal",
        &[
            "--panel",
            "--current",
            "--client",
            "--child",
            "--presentation",
            "--label",
            "--exec",
            "--arg",
            "--profile",
            "--cwd",
            "--env",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
            "--timeout",
        ],
    ),
    ("panel:input", &["--panel", "--current", "--right-click"]),
    (
        "panel:report-agent",
        &[
            "--presentation",
            "--source",
            "--agent",
            "--state",
            "--message",
            "--seq",
            "--agent-session-id",
            "--agent-session-path",
        ],
    ),
    (
        "panel:report-agent-session",
        &[
            "--presentation",
            "--source",
            "--agent",
            "--seq",
            "--agent-session-id",
            "--agent-session-path",
            "--session-start-source",
        ],
    ),
    (
        "panel:clear-agent-authority",
        &["--presentation", "--source", "--seq"],
    ),
    (
        "panel:release-agent",
        &["--presentation", "--source", "--agent", "--seq"],
    ),
    (
        "panel:report-metadata",
        &[
            "--presentation",
            "--source",
            "--agent",
            "--applies-to-source",
            "--title",
            "--display-agent",
            "--state-label",
            "--clear-title",
            "--clear-display-agent",
            "--clear-state-labels",
            "--token",
            "--clear-token",
            "--seq",
            "--ttl-ms",
        ],
    ),
    ("panel:rename", &["--label", "--clear"]),
    (
        "panel:swap",
        &[
            "--with",
            "--direction",
            "--panel",
            "--current",
            "--client",
            "--timeout",
        ],
    ),
    ("tab:move", &["--direction", "--insert-index"]),
    ("tab:close", &["--confirm"]),
    ("panel:current", &["--panel", "--current", "--client"]),
    ("panel:process-info", &["--panel", "--current", "--client"]),
    #[cfg(not(windows))]
    ("panel:list", &["--tab"]),
    #[cfg(windows)]
    ("panel:list", &["--tab", "--client"]),
    #[cfg(windows)]
    ("panel:get", &["--client"]),
    (
        "panel:split",
        &[
            "--right-click",
            "--panel",
            "--current",
            "--ratio",
            "--focus",
            "--no-focus",
            "--client",
            "--timeout",
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
            "--new-workspace",
            "--tab-label",
            "--direction",
            "--label",
            "--workspace",
            "--standalone",
            "--project",
            "--ratio",
            "--focus",
            "--no-focus",
            "--client",
            "--timeout",
        ],
    ),
    (
        "terminal:read",
        &[
            "--history",
            "--lines",
            "--rows",
            "--unwrap",
            "--source",
            "--plain",
            "--ansi",
        ],
    ),
    (
        "terminal:send",
        &["--text", "--enter", "--key", "--file", "--stdin", "--paste"],
    ),
    (
        "terminal:wait-output",
        &[
            "--match",
            "--regex",
            "--rows",
            "--timeout",
            "--source",
            "--lines",
            "--forever",
        ],
    ),
    (
        "worktree:list",
        &[
            "--project",
            "--directory",
            "--workspace",
            "--cwd",
            "--client",
            "--trust-repository",
        ],
    ),
    (
        "worktree:create",
        &[
            "--project",
            "--workspace",
            "--cwd",
            "--client",
            "--branch",
            "--base",
            "--path",
            "--label",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
            "--trust-repository",
            "--request-id",
            "--detach",
            "--focus",
            "--no-focus",
            "--timeout",
        ],
    ),
    (
        "worktree:open",
        &[
            "--project",
            "--workspace",
            "--cwd",
            "--client",
            "--branch",
            "--path",
            "--label",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
            "--trust-repository",
            "--request-id",
            "--detach",
            "--focus",
            "--no-focus",
            "--timeout",
        ],
    ),
    (
        "worktree:remove",
        &[
            "--project",
            "--workspace",
            "--force",
            "--trust-repository",
            "--confirm",
            "--request-id",
            "--detach",
            "--timeout",
        ],
    ),
    (
        "worktree:resume",
        &["--generation", "--confirm", "--detach", "--timeout"],
    ),
    ("launch-profile:create", &["--name", "--exec", "--arg"]),
];

/// `<family>:<verb>` → the words its `--direction` flag accepts. Only
/// direction-admitting commands appear.
const DIRECTION_VALUES: &[(&str, &[&str])] = &[
    ("panel:focus", &["left", "right", "up", "down"]),
    ("panel:resize", &["left", "right", "up", "down"]),
    ("panel:swap", &["left", "right", "up", "down"]),
    ("panel:neighbor", &["left", "right", "up", "down"]),
    ("tab:move", &["first", "last", "next", "previous"]),
    ("panel:split", &["above", "below", "left", "right"]),
    ("panel:move", &["above", "below", "left", "right"]),
];

const CWD_VALUES: &[&str] = &["follow", "home", "current", "workspace"];
const OUTPUT_SOURCES: &[&str] = &["visible", "recent", "recent-unwrapped", "detection"];
const RIGHT_CLICK_VALUES: &[&str] = &["orchbee", "panel"];

/// Discoverable examples; arbitrary printable scalar keys are also accepted.
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
    "space",
    "insert",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
    "f13",
    "f14",
    "f15",
    "f16",
    "f17",
    "f18",
    "f19",
    "f20",
    "f21",
    "f22",
    "f23",
    "f24",
    "f25",
    "plus",
    "minus",
    "semicolon",
    "slash",
    "ctrl+shift+a",
    "super+c",
    "alt+enter",
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
    "kilo",
    "cursor",
    "qodercli",
    "antigravity-cli",
    "mastracode",
    "droid",
    "kimi",
    "letta",
    "devin",
    "hermes",
    "pi",
    "omp",
    "qwen-code",
];

/// `<family>:<verb>` paths that consume one positional operand.
const OPERAND_PATHS: &[&str] = &[
    #[cfg(windows)]
    "panel:create-terminal",
    "panel:report-metadata",
    "panel:report-agent",
    "panel:report-agent-session",
    "panel:clear-agent-authority",
    "panel:release-agent",
    "terminal:attach",
    "agent:attach",
    "terminal:session:observe",
    "terminal:session:control",
    "panel:input",
    "panel:resize",
    "panel:swap",
    "client:get",
    "client:control-status",
    "workspace:focus",
    "tab:focus",
    "panel:focus",
    "panel:zoom",
    "panel:layout",
    "panel:neighbor",
    "panel:edges",
    "worktree:resume",
    "worktree:status",
    #[cfg(windows)]
    "session:attach",
    #[cfg(windows)]
    "session:stop",
    #[cfg(windows)]
    "session:delete",
    "site:stop-status",
    "site:agent-manifests-status",
    "agent:start",
    "agent:explain",
    "agent:get",
    "agent:focus",
    "agent:attention",
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
    "panel:rename",
    "terminal:get",
    "terminal:read",
    "terminal:send",
    "terminal:run",
    "terminal:wait-output",
    "workspace:accept-directory",
    "workspace:close",
    "workspace:get",
    "workspace:rename",
    "workspace:report-metadata",
    "workspace:move",
    "workspace:move-block",
];

/// Help topics that lead anywhere (`help <word>`); leaf topics accept no
/// deeper word.
const HELP_TOPICS: &[&str] = &[
    "layout",
    "client",
    "agent",
    "api",
    "completion",
    "config",
    "panel",
    "project",
    #[cfg(windows)]
    "session",
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
            "attach",
            "attention",
            "changes",
            "doctor",
            "explain",
            "focus",
            "get",
            "integration",
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
    ("client", &["control-status", "get", "list"]),
    #[cfg(windows)]
    ("layout", &["apply", "export", "set-split-ratio"]),
    #[cfg(not(windows))]
    ("layout", &["set-split-ratio"]),
    ("launch-profile", &["create", "delete", "get", "list"]),
    ("config", &["check", "reset-keys"]),
    (
        "panel",
        &[
            #[cfg(windows)]
            "create-terminal",
            "report-agent",
            "report-agent-session",
            "clear-agent-authority",
            "release-agent",
            "close",
            "current",
            "get",
            "list",
            "move",
            "report-metadata",
            "process-info",
            "split",
            "focus",
            "zoom",
            "layout",
            "neighbor",
            "edges",
            "resize",
            "rename",
            "swap",
            "input",
        ],
    ),
    ("project", &["create", "get", "list", "rename"]),
    #[cfg(windows)]
    ("session", &["list", "attach", "stop", "delete"]),
    (
        "site",
        &[
            "diagnose",
            "start",
            "stop",
            "stop-status",
            "agent-manifests",
            "reload-agent-manifests",
            "update-agent-manifests",
            "agent-manifests-status",
        ],
    ),
    ("status", &["client", "server"]),
    (
        "tab",
        &["close", "create", "focus", "get", "list", "move", "rename"],
    ),
    (
        "terminal",
        &[
            "attach",
            "get",
            "list",
            "read",
            "run",
            "send",
            "session",
            "wait-output",
        ],
    ),
    (
        "workspace",
        &[
            "accept-directory",
            "move",
            "move-block",
            "close",
            "close-resume",
            "close-status",
            "create",
            "focus",
            "get",
            "list",
            "rename",
        ],
    ),
    (
        "worktree",
        &["create", "list", "open", "remove", "resume", "status"],
    ),
];

/// `help <topic> <child>` → third-level words. Nested command families
/// reach a third help level.
const HELP_LEVEL3: &[(&str, &[&str])] = &[
    ("agent:integration", INTEGRATION_VERBS),
    ("terminal:session", &["observe", "control"]),
];

const SHELLS: &[&str] = &["bash", "elvish", "fish", "powershell", "zsh"];
const FORMAT_VALUES: &[&str] = &["text", "json"];

pub(crate) fn render(shell: CompletionShell) -> String {
    match shell {
        CompletionShell::Bash => bash(),
        CompletionShell::Zsh => zsh(),
        CompletionShell::Fish => fish(),
        CompletionShell::PowerShell => powershell::render(),
        CompletionShell::Elvish => elvish::render(),
    }
}

/// `flag` → the walker's one-shot seen bit for it.
fn flag_bit(flag: &str) -> &'static str {
    match flag {
        "--child" => "seen_child",
        "--mouse" => "seen_mouse",
        "--output" => "seen_output",
        "--client" => "seen_client",
        "--on" => "seen_zoom_on",
        "--off" => "seen_zoom_off",
        "--toggle" => "seen_zoom_toggle",
        "--branch" => "seen_branch",
        "--base" => "seen_base",
        "--focus" => "seen_focus",
        "--no-focus" => "seen_no_focus",
        "--force" => "seen_force",
        "--takeover" => "seen_takeover",
        "--trust-repository" => "seen_trust_repository",
        "--group" => "seen_group",
        "--kind" => "seen_kind",
        "--project" => "seen_project",
        "--panel" => "seen_panel",
        "--current" => "seen_current",
        "--workspace" => "seen_workspace",
        "--tab" => "seen_tab",
        "--standalone" => "seen_standalone",
        "--label" => "seen_label",
        "--right-click" => "seen_right_click",
        "--source" => "seen_output_source",
        "--state" => "seen_panel_agent_state",
        "--message" => "seen_panel_agent_message",
        "--agent-session-id" => "seen_panel_agent_agent_session_id",
        "--agent-session-path" => "seen_panel_agent_agent_session_path",
        "--session-start-source" => "seen_panel_agent_session_start_source",
        "--agent" => "seen_panel_metadata_agent",
        "--applies-to-source" => "seen_panel_metadata_applies_to_source",
        "--title" => "seen_panel_metadata_title",
        "--display-agent" => "seen_panel_metadata_display_agent",
        "--presentation" => "seen_report_presentation",
        "--state-label" => "seen_panel_metadata_state_label",
        "--clear-title" => "seen_panel_metadata_clear_title",
        "--clear-display-agent" => "seen_panel_metadata_clear_display_agent",
        "--clear-state-labels" => "seen_panel_metadata_clear_state_labels",
        "--token" => "seen_metadata_token",
        "--clear-token" => "seen_metadata_clear_token",
        "--seq" => "seen_metadata_sequence",
        "--ttl-ms" => "seen_metadata_ttl",
        "--plain" => "seen_plain",
        "--ansi" => "seen_ansi",
        "--direction" => "seen_direction",
        "--insert-index" => "seen_insert_index",
        "--before" => "seen_before",
        "--amount" => "seen_amount",
        "--ratio" => "seen_ratio",
        "--cols" => "seen_cols",
        "--rows" => "seen_rows",
        "--cell-width" => "seen_cell_width",
        "--cell-height" => "seen_cell_height",
        "--confirm" => "seen_confirm",
        "--with" => "seen_with",
        "--target-panel" => "seen_target_panel",
        "--into-tab" => "seen_into_tab",
        "--new-tab" => "seen_new_tab",
        "--new-workspace" => "seen_new_workspace",
        "--tab-label" => "seen_tab_label",
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
        "--outdated-only" => "seen_outdated_only",
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
        "--on" | "--off" | "--toggle" | "--focus" | "--no-focus" => None,
        "--cwd" => Some("cwd"),
        "--right-click" => Some("right_click"),
        "--source" => Some("output_source"),
        "--token" | "--clear-token" | "--seq" | "--ttl-ms" => Some("filter"),
        "--clear-title"
        | "--clear-display-agent"
        | "--clear-state-labels"
        | "--plain"
        | "--ansi" => None,
        "--direction" => Some("direction"),
        "--key" => Some("key"),
        "--until" => Some("until"),
        "--provider" => Some("provider"),
        "--standalone" | "--confirm" | "--new-tab" | "--new-workspace" | "--history"
        | "--enter" | "--stdin" | "--outdated-only" | "--include-ended" | "--unwrap"
        | "--paste" | "--current" | "--clear" | "--wait" | "--forever" | "--retry" | "--detach"
        | "--force" | "--takeover" | "--trust-repository" | "--group" => None,
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
    bits.push("seen_mouse");
    bits.join(" || ")
}

/// `agent:integration:<flag>` patterns — the flags admitted at the `deep`
/// state, where the resolved command is not yet chosen.
fn deep_flag_patterns() -> String {
    HELP_LEVEL3
        .iter()
        .flat_map(|(path, verbs)| {
            verbs.iter().flat_map(move |verb| {
                command_flags(&format!("{path}:{verb}"))
                    .iter()
                    .map(move |flag| format!("{path}:{flag}"))
            })
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|")
}
fn deep_verb_patterns() -> String {
    HELP_LEVEL3
        .iter()
        .flat_map(|(path, verbs)| verbs.iter().map(move |verb| format!("{path}:{verb}")))
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
    deep_flag_patterns()
        .split('|')
        .map(|path| format!("                            case '{path}'"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn deep_verb_cases_fish() -> String {
    deep_verb_patterns()
        .split('|')
        .map(|path| format!("'{path}'"))
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
            let mut arm = match *flag {
                "--client" => format!("                            {patterns}) if [[ $family:$sub == panel:process-info && $state == operand ]]; then done=1; fi ;;"),

                "--project" | "--directory" => format!("                            {patterns}) if [[ $family:$sub == worktree:list ]] && ((seen_workspace || seen_cwd || seen_trust_repository)); then done=1; fi ;;"),
                "--trust-repository" => format!("                            {patterns}) if [[ $family:$sub == worktree:list ]] && ((seen_project || seen_directory)); then done=1; fi ;;"),
                "--cwd" => format!("                            {patterns}) if [[ $family == worktree && $seen_workspace == 1 ]] || {{ [[ $family:$sub == worktree:list ]] && ((seen_project || seen_directory)); }}; then done=1; fi ;;"),
                "--path" => format!("                            {patterns}) if [[ $family:$sub == worktree:open && $seen_branch == 1 ]]; then done=1; fi ;;"),
                "--branch" => format!("                            {patterns}) if [[ $family:$sub == worktree:open && $seen_path == 1 ]]; then done=1; fi ;;"),
                "--panel" => format!("                            {patterns}) if ((seen_current)) || [[ ( $family:$sub == panel:process-info || $family:$sub == panel:focus || $family:$sub == panel:zoom || $family:$sub == panel:layout || $family:$sub == panel:neighbor || $family:$sub == panel:edges || $family:$sub == panel:resize || $family:$sub == panel:swap || $family:$sub == panel:input || $family:$sub == panel:split || $family:$sub == panel:create-terminal ) && $state == operand ]]; then done=1; fi ;;"),
                "--current" => format!("                            {patterns}) if ((seen_panel)) || [[ ( $family:$sub == panel:process-info || $family:$sub == panel:focus || $family:$sub == panel:zoom || $family:$sub == panel:layout || $family:$sub == panel:neighbor || $family:$sub == panel:edges || $family:$sub == panel:resize || $family:$sub == panel:swap || $family:$sub == panel:input || $family:$sub == panel:split || $family:$sub == panel:create-terminal ) && $state == operand ]]; then done=1; fi ;;"),
                "--workspace" => {
                    format!(
                        "                            {patterns}) if ((seen_standalone)) || [[ $family == worktree && $seen_cwd == 1 ]] || {{ [[ $family:$sub == worktree:list ]] && ((seen_project || seen_directory)); }}; then done=1; fi ;;"
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
            };
            arm.truncate(arm.len() - " ;;".len());
            if arm.ends_with(')') { arm.push_str(" :"); }
            for command in COMMAND_FLAGS.iter().map(|(command, _)| *command) {
                let conflicts = command_flag_conflicts(command, flag);
                if !conflicts.is_empty() {
                    let bits = conflicts.iter().map(|flag| flag_bit(flag)).collect::<Vec<_>>().join(" || ");
                    arm.push_str(&format!("; if [[ $family:$sub == {command} ]] && (({bits})); then done=1; fi"));
                }
            }
            arm.push_str(" ;;");
            arm
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
                for (flag, conflict) in command_conflict_pairs(&command) {
                    exclusive.push_str(&format!("(({} && {})) && done=1\n                                        ", flag_bit(flag), flag_bit(conflict)));
                }
                if matches!(command.as_str(), "panel:current" | "panel:process-info" | "panel:focus" | "panel:zoom" | "panel:layout" | "panel:neighbor" | "panel:edges" | "panel:resize" | "panel:swap" | "panel:input" | "panel:split" | "panel:create-terminal") {
                    exclusive.push_str("((seen_panel && seen_current)) && done=1\n                                        ");
                }
                if *family == "worktree" {
                    exclusive.push_str("((seen_workspace && seen_cwd)) && done=1\n                                        ");
                    if command == "worktree:list" { exclusive.push_str("(((seen_project || seen_directory) && (seen_workspace || seen_cwd || seen_trust_repository))) && done=1\n                                        "); }
                    if command == "worktree:open" { exclusive.push_str("((seen_path && seen_branch)) && done=1\n                                        "); }
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
                    let guard = if command == "worktree:list" {
                        " && ((!seen_workspace || !seen_cwd)) && ((!seen_project && !seen_directory || !seen_workspace && !seen_cwd && !seen_trust_repository))"
                    } else if command == "worktree:open" {
                        " && ((!seen_workspace || !seen_cwd)) && ((!seen_path || !seen_branch))"
                    } else if *family == "worktree" {
                        " && ((!seen_workspace || !seen_cwd))"
                    } else if matches!(command.as_str(), "panel:current" | "panel:process-info" | "panel:focus" | "panel:zoom" | "panel:layout" | "panel:neighbor" | "panel:edges" | "panel:resize" | "panel:swap" | "panel:input" | "panel:split" | "panel:create-terminal") {
                        " && ((!seen_panel || !seen_current))"
                    } else if command == "tab:create" {
                        " && ((!seen_workspace || !seen_standalone)) && ((!seen_profile || (!seen_exec && !seen_arg)))"
                    } else if *family == "tab" {
                        " && ((!seen_workspace || !seen_standalone))"
                    } else if command == "panel:split" {
                        " && ((!seen_profile || (!seen_exec && !seen_arg)))"
                    } else if *verb == "send" {
                        " && (( (seen_text + seen_key + seen_file + seen_stdin) < 2 && (!seen_key || !seen_enter) ))"
                    } else {
                        ""
                    };
                    let mut guard = guard.to_owned();
                    for (flag, conflict) in command_conflict_pairs(&command) {
                        guard.push_str(&format!(" && ((!{} || !{}))", flag_bit(flag), flag_bit(conflict)));
                    }
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
                        "--workspace" | "--cwd" if *command == "worktree:list" => "seen_workspace || seen_cwd || seen_project || seen_directory".to_owned(),
                        "--trust-repository" if *command == "worktree:list" => "seen_trust_repository || seen_project || seen_directory".to_owned(),
                        "--project" | "--directory" if *command == "worktree:list" => format!("{} || seen_workspace || seen_cwd || seen_trust_repository", flag_bit(flag)),
                        "--workspace" | "--cwd" if command.starts_with("worktree:") => "seen_workspace || seen_cwd".to_owned(),
                        "--path" | "--branch" if *command == "worktree:open" => "seen_path || seen_branch".to_owned(),
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
                        "--provider" | "--asset-root" | "--until" | "--token" | "--clear-token" | "--state-label" => "0".to_owned(),
                        _ => flag_bit(flag).to_owned(),
                    };
                    let conflicts = command_flag_conflicts(command, flag).iter().map(|flag| flag_bit(flag)).collect::<Vec<_>>();
                    let cond = if conflicts.is_empty() { cond } else { format!("{cond} || {}", conflicts.join(" || ")) };
                    if matches!(*command, "panel:process-info" | "panel:focus" | "panel:zoom" | "panel:layout" | "panel:neighbor" | "panel:edges" | "panel:resize" | "panel:swap" | "panel:input" | "panel:split" | "panel:create-terminal") && matches!(*flag, "--panel" | "--current") || (*command == "panel:process-info" && *flag == "--client") || (*command == "agent:attach" && matches!(*flag, "--name" | "--session" | "--panel")) {
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
    let cwd = joined("--cwd", CWD_VALUES);
    let right_click = joined("--right-click", RIGHT_CLICK_VALUES);
    let output_source = joined("--source", OUTPUT_SOURCES);
    let key = joined("--key", KEY_VALUES);
    let until = joined("--until", UNTIL_VALUES);
    let provider = joined("--provider", PROVIDER_VALUES);
    format!(
        concat!(
            "            --cwd) ((!seen_cwd)) && [[ $family:$sub == tab:create || $family:$sub == panel:split || $family:$sub == workspace:create ]] && candidates=\"{cwd}\" ;;\n",
            "            --source)\n",
            "                if ((!seen_output_source)); then\n",
            "                    case \"$state\" in\n",
            "                        verb) candidates=\"{output_source}\" ;;\n",
            "                        sub) [[ ( $family == terminal || $family == agent ) && ( \" $verbs \" == *\" read \"* || \" $verbs \" == *\" wait-output \"* ) ]] && candidates=\"{output_source}\" ;;\n",
            "                        after|operand|operand2) [[ $family:$sub == terminal:read || $family:$sub == terminal:wait-output || $family:$sub == agent:read ]] && candidates=\"{output_source}\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --right-click)\n",
            "                if ((!seen_right_click)); then\n",
            "                    case \"$state\" in\n",
            "                        verb) candidates=\"{right_click}\" ;;\n",
            "                        sub) [[ $family == panel && ( \" $verbs \" == *\" input \"* || \" $verbs \" == *\" split \"* ) ]] && candidates=\"{right_click}\" ;;\n",
            "                        after|operand|operand2) [[ $family:$sub == panel:input || $family:$sub == panel:split ]] && candidates=\"{right_click}\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --direction)\n",
            "                if ((!seen_direction && !seen_insert_index)); then\n",
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
        cwd = cwd,
        right_click = right_click,
        output_source = output_source,
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
            "    local metadata_source_only=0 ended=0 done=0 dead=0 value=\"\"\n",
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
            "            if [[ $value == output_source && $word != visible && $word != recent && $word != recent-unwrapped && $word != detection ]]; then metadata_source_only=1; [[ $word =~ ^[[:space:]]*[A-Za-z0-9:._-]{{1,80}}[[:space:]]*$ ]] || done=1; fi\n",
            "            if [[ $value == right_click && $word != orchbee && $word != panel ]]; then done=1; fi\n",
            "            value=\"\"\n",
            "            continue\n",
            "        fi\n",
            "        if [[ $family:$sub == agent:attach ]]; then\n",
            "            if [[ $state == operand && ${{word%%=*}} == --name || $state == operand && ${{word%%=*}} == --session || $state == operand && ${{word%%=*}} == --panel ]]; then done=1; fi\n",
            "            if [[ $state == after && $word != -* ]] && ((seen_name || seen_session || seen_panel)); then done=1; fi\n",
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
            "                # `--mouse` is owned by TUI and attach: legal before the\n",
            "                # command word, in attach scope, or on the TUI leaf.\n",
            "                # Both spellings are the grammar — the two-token form\n",
            "                # leaves a pending value, the joined form carries it and\n",
            "                # stays inside the closed {{off,on}} vocabulary.\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub) [[ $family == terminal || $family == agent ]] || done=1 ;;\n",
            "                    after|operand|operand2) [[ $family:$sub == terminal:attach || $family:$sub == agent:attach ]] || done=1 ;;\n",
            "                    leaf) [[ $leaf == tui ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    if ((seen_mouse)); then done=1; else\n",
            "                        case \"$word\" in\n",
            "                            --mouse) seen_mouse=1; value=mouse; seen_flags+=(--mouse) ;;\n",
            "                            --mouse=off|--mouse=on) seen_mouse=1; seen_flags+=(--mouse) ;;\n",
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
            "                if [[ $word == --source=* && $word != --source=visible && $word != --source=recent && $word != --source=recent-unwrapped && $word != --source=detection ]]; then metadata_source_only=1; [[ ${{word#*=}} =~ ^[[:space:]]*[A-Za-z0-9:._-]{{1,80}}[[:space:]]*$ ]] || done=1; fi\n",
            "                if [[ $word == --right-click=* && $word != --right-click=orchbee && $word != --right-click=panel ]]; then done=1; fi\n",
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
            "                                if ((seen_format)) || ! admit '--mouse' \"${{seen_flags[@]}}\"; then done=1; else leaf=tui; state=leaf; fi ;;\n",
            "                            help|completion)\n",
            "                                if ((seen_format || ${{#seen_flags[@]}} || seen_mouse)); then done=1; else state=$word; fi ;;\n",
            "                            {families}) if ((seen_mouse)) && [[ $word != terminal && $word != agent ]]; then done=1; else family=$word; state=sub; fi ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            terminal:session) sub=session; state=deep; admit '--cols --rows --timeout --takeover' \"${{seen_flags[@]}}\" || done=1 ;;\n",
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
            "                                sub=\"$sub:$word\"; state=after\n",
            "                                # Every `agent integration` verb shares the\n",
            "                                # same flag scope — the union check.\n",
            "                                if [[ $family == agent ]]; then (({integration_bad})) && done=1; elif [[ $sub == session:observe ]] && ((seen_takeover)); then done=1; fi ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub\" in\n",
            "                            panel:process-info|panel:focus|panel:zoom|panel:layout|panel:neighbor|panel:edges|panel:resize|panel:swap|panel:input|panel:split|panel:create-terminal) if ((seen_panel || seen_current)) || [[ $family:$sub == panel:process-info && $seen_client == 1 ]]; then done=1; else state=operand; fi ;;\n",
            "                            {operand_paths}) state=operand ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    operand) if [[ $family:$sub == agent:send-keys || $family:$sub == agent:start ]]; then state=operand; elif [[ $family:$sub == terminal:run || $family:$sub == agent:rename || $family:$sub == agent:prompt ]] && ((seen_clear == 0 && seen_name == 0 && seen_session == 0)); then state=operand2; else done=1; fi ;;\n",
            "                    leaf) if [[ $leaf == status && ( $word == client || $word == server ) ]]; then family=status; sub=$word; state=after; else done=1; fi ;;\n",
            "                    operand2|completion) done=1 ;;\n",
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
            "    if ((metadata_source_only)) && [[ $state != verb && $family != workspace ]]; then done=1; fi\n",
            "    if [[ $state == verb ]]; then\n",
            "        possible=\"{roots}\"\n",
            "{narrow_lines}\n",
            "        ((metadata_source_only)) && narrow \"workspace\"\n",
            "        ((seen_mouse)) && narrow \"tui agent terminal\"\n",
            "        ((seen_format)) && narrow \"{format_roots}\"\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "{verb_cases}\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    elif [[ $state == deep ]]; then\n",
            "        if [[ $family == terminal ]]; then verbs=control; ((seen_takeover == 0)) && verbs=\"$verbs observe\"; else (({integration_bad})) || verbs=\"{integration_verb_words}\"; fi\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead || seen_zoom_on + seen_zoom_off + seen_zoom_toggle > 1)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == mouse ]]; then candidates=\"off on\"\n",
            "    elif [[ $value == direction ]]; then\n",
            "        case \"$family:$sub\" in\n",
            "{direction_cases}\n",
            "        esac\n",
            "    elif [[ $value == cwd ]]; then [[ $family:$sub == tab:create || $family:$sub == panel:split || $family:$sub == workspace:create ]] && candidates=\"{cwd_values}\"\n",
            "    elif [[ $value == output_source ]]; then [[ $family != workspace ]] && candidates=\"{output_source_values}\"\n",
            "    elif [[ $value == right_click ]]; then candidates=\"{right_click_values}\"\n",
            "    elif [[ $value == key ]]; then candidates=\"{key_values}\"\n",
            "    elif [[ $value == until ]]; then candidates=\"{until_values}\"\n",
            "    elif [[ $value == provider ]]; then candidates=\"{provider_values}\"\n",
            "    elif [[ -n $value ]]; then candidates=\"\"\n",
            "    elif [[ $cur == --format=* ]]; then\n",
            "        if ((!seen_format)); then\n",
            "            case \"$state\" in\n",
            "                verb) candidates=\"--format=text --format=json\" ;;\n",
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
            "                        verb) [[ \" $possible \" == *\" tui \"* || \" $possible \" == *\" agent \"* || \" $possible \" == *\" terminal \"* ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        sub) [[ $family == terminal || $family == agent ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        after|operand|operand2) [[ $family:$sub == terminal:attach || $family:$sub == agent:attach ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        leaf) [[ $leaf == tui ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "{joined_offer_cases}\n",
            "        esac\n",
            "    elif [[ $state == verb ]]; then\n",
            "        candidates=\"$possible\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "        [[ \" $possible \" == *\" tui \"* || \" $possible \" == *\" agent \"* || \" $possible \" == *\" terminal \"* ]] && ((!seen_mouse)) && candidates=\"$candidates --mouse\"\n",
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
            "        [[ $leaf == status ]] && candidates=\"client server\"\n",
            "        if [[ $leaf == tui ]]; then ((!seen_mouse)) && candidates=\"--help --mouse\"\n",
            "        elif ((!seen_format)); then candidates=\"$candidates --format --format= --help\"; fi\n",
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
        cwd_values = CWD_VALUES.join(" "),
        right_click_values = RIGHT_CLICK_VALUES.join(" "),
        output_source_values = OUTPUT_SOURCES.join(" "),
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
            "    local metadata_source_only=0 ended=0 done=0 dead=0 value=\"\"\n",
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
            "            if [[ $value == output_source && $word != visible && $word != recent && $word != recent-unwrapped && $word != detection ]]; then metadata_source_only=1; [[ $word =~ ^[[:space:]]*[A-Za-z0-9:._-]{{1,80}}[[:space:]]*$ ]] || done=1; fi\n",
            "            if [[ $value == right_click && $word != orchbee && $word != panel ]]; then done=1; fi\n",
            "            value=\"\"\n",
            "            continue\n",
            "        fi\n",
            "        if [[ $family:$sub == agent:attach ]]; then\n",
            "            if [[ $state == operand && ${{word%%=*}} == --name || $state == operand && ${{word%%=*}} == --session || $state == operand && ${{word%%=*}} == --panel ]]; then done=1; fi\n",
            "            if [[ $state == after && $word != -* ]] && ((seen_name || seen_session || seen_panel)); then done=1; fi\n",
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
            "                # `--mouse` is owned by TUI and attach: legal before the\n",
            "                # command word, in attach scope, or on the TUI leaf.\n",
            "                # Both spellings are the grammar — the two-token form\n",
            "                # leaves a pending value, the joined form carries it and\n",
            "                # stays inside the closed {{off,on}} vocabulary.\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub) [[ $family == terminal || $family == agent ]] || done=1 ;;\n",
            "                    after|operand|operand2) [[ $family:$sub == terminal:attach || $family:$sub == agent:attach ]] || done=1 ;;\n",
            "                    leaf) [[ $leaf == tui ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    if ((seen_mouse)); then done=1; else\n",
            "                        case \"$word\" in\n",
            "                            --mouse) seen_mouse=1; value=mouse; seen_flags+=(--mouse) ;;\n",
            "                            --mouse=off|--mouse=on) seen_mouse=1; seen_flags+=(--mouse) ;;\n",
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
            "                if [[ $word == --source=* && $word != --source=visible && $word != --source=recent && $word != --source=recent-unwrapped && $word != --source=detection ]]; then metadata_source_only=1; [[ ${{word#*=}} =~ ^[[:space:]]*[A-Za-z0-9:._-]{{1,80}}[[:space:]]*$ ]] || done=1; fi\n",
            "                if [[ $word == --right-click=* && $word != --right-click=orchbee && $word != --right-click=panel ]]; then done=1; fi\n",
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
            "                                if ((seen_format)) || ! admit '--mouse' \"${{seen_flags[@]}}\"; then done=1; else leaf=tui; state=leaf; fi ;;\n",
            "                            help|completion)\n",
            "                                if ((seen_format || ${{#seen_flags[@]}} || seen_mouse)); then done=1; else state=$word; fi ;;\n",
            "                            {families}) if ((seen_mouse)) && [[ $word != terminal && $word != agent ]]; then done=1; else family=$word; state=sub; fi ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            terminal:session) sub=session; state=deep; admit '--cols --rows --timeout --takeover' \"${{seen_flags[@]}}\" || done=1 ;;\n",
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
            "                                sub=\"$sub:$word\"; state=after\n",
            "                                if [[ $family == agent ]]; then (({integration_bad})) && done=1; elif [[ $sub == session:observe ]] && ((seen_takeover)); then done=1; fi ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub\" in\n",
            "                            panel:process-info|panel:focus|panel:zoom|panel:layout|panel:neighbor|panel:edges|panel:resize|panel:swap|panel:input|panel:split|panel:create-terminal) if ((seen_panel || seen_current)) || [[ $family:$sub == panel:process-info && $seen_client == 1 ]]; then done=1; else state=operand; fi ;;\n",
            "                            {operand_paths}) state=operand ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    operand) if [[ $family:$sub == agent:send-keys || $family:$sub == agent:start ]]; then state=operand; elif [[ $family:$sub == terminal:run || $family:$sub == agent:rename || $family:$sub == agent:prompt ]] && ((seen_clear == 0 && seen_name == 0 && seen_session == 0)); then state=operand2; else done=1; fi ;;\n",
            "                    leaf) if [[ $leaf == status && ( $word == client || $word == server ) ]]; then family=status; sub=$word; state=after; else done=1; fi ;;\n",
            "                    operand2|completion) done=1 ;;\n",
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
            "    if ((metadata_source_only)) && [[ $state != verb && $family != workspace ]]; then done=1; fi\n",
            "    if [[ $state == verb ]]; then\n",
            "        possible=\"{roots}\"\n",
            "{narrow_lines}\n",
            "        ((metadata_source_only)) && narrow \"workspace\"\n",
            "        ((seen_mouse)) && narrow \"tui agent terminal\"\n",
            "        ((seen_format)) && narrow \"{format_roots}\"\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "{verb_cases}\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    elif [[ $state == deep ]]; then\n",
            "        if [[ $family == terminal ]]; then verbs=control; ((seen_takeover == 0)) && verbs=\"$verbs observe\"; else (({integration_bad})) || verbs=\"{integration_verb_words}\"; fi\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead || seen_zoom_on + seen_zoom_off + seen_zoom_toggle > 1)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == mouse ]]; then candidates=\"off on\"\n",
            "    elif [[ $value == direction ]]; then\n",
            "        case \"$family:$sub\" in\n",
            "{direction_cases}\n",
            "        esac\n",
            "    elif [[ $value == cwd ]]; then [[ $family:$sub == tab:create || $family:$sub == panel:split || $family:$sub == workspace:create ]] && candidates=\"{cwd_values}\"\n",
            "    elif [[ $value == output_source ]]; then [[ $family != workspace ]] && candidates=\"{output_source_values}\"\n",
            "    elif [[ $value == right_click ]]; then candidates=\"{right_click_values}\"\n",
            "    elif [[ $value == key ]]; then candidates=\"{key_values}\"\n",
            "    elif [[ $value == until ]]; then candidates=\"{until_values}\"\n",
            "    elif [[ $value == provider ]]; then candidates=\"{provider_values}\"\n",
            "    elif [[ -n $value ]]; then candidates=\"\"\n",
            "    elif [[ $cur == --format=* ]]; then\n",
            "        if ((!seen_format)); then\n",
            "            case \"$state\" in\n",
            "                verb) candidates=\"--format=text --format=json\" ;;\n",
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
            "                        verb) [[ \" $possible \" == *\" tui \"* || \" $possible \" == *\" agent \"* || \" $possible \" == *\" terminal \"* ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        sub) [[ $family == terminal || $family == agent ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        after|operand|operand2) [[ $family:$sub == terminal:attach || $family:$sub == agent:attach ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                        leaf) [[ $leaf == tui ]] && candidates=\"--mouse=off --mouse=on\" ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "{joined_offer_cases}\n",
            "        esac\n",
            "    elif [[ $state == verb ]]; then\n",
            "        candidates=\"$possible\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "        [[ \" $possible \" == *\" tui \"* || \" $possible \" == *\" agent \"* || \" $possible \" == *\" terminal \"* ]] && ((!seen_mouse)) && candidates=\"$candidates --mouse\"\n",
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
            "        [[ $leaf == status ]] && candidates=\"client server\"\n",
            "        if [[ $leaf == tui ]]; then ((!seen_mouse)) && candidates=\"--help --mouse\"\n",
            "        elif ((!seen_format)); then candidates=\"$candidates --format --format= --help\"; fi\n",
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
        cwd_values = CWD_VALUES.join(" "),
        right_click_values = RIGHT_CLICK_VALUES.join(" "),
        output_source_values = OUTPUT_SOURCES.join(" "),
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
            let mut arm = match *flag {
                "--client" => format!("                            case {patterns}; if test \"$family:$sub\" = panel:process-info; and test $state = operand; set done 1; end"),

                "--project" | "--directory" => format!("                            case {patterns}; if test \"$family:$sub\" = worktree:list; and test $seen_workspace -eq 1 -o $seen_cwd -eq 1 -o $seen_trust_repository -eq 1; set done 1; end"),
                "--trust-repository" => format!("                            case {patterns}; if test \"$family:$sub\" = worktree:list; and test $seen_project -eq 1 -o $seen_directory -eq 1; set done 1; end"),
                "--cwd" => format!("                            case {patterns}; if test $family = worktree; and test $seen_workspace -eq 1; set done 1; end; if test \"$family:$sub\" = worktree:list; and test $seen_project -eq 1 -o $seen_directory -eq 1; set done 1; end"),
                "--path" => format!("                            case {patterns}; if test \"$family:$sub\" = worktree:open; and test $seen_branch -eq 1; set done 1; end"),
                "--branch" => format!("                            case {patterns}; if test \"$family:$sub\" = worktree:open; and test $seen_path -eq 1; set done 1; end"),
                "--panel" => format!("                            case {patterns}; if test $seen_current -eq 1; or begin; contains -- \"$family:$sub\" panel:process-info panel:focus panel:zoom panel:layout panel:neighbor panel:edges panel:resize panel:swap panel:input panel:split panel:create-terminal; and test $state = operand; end; set done 1; end"),
                "--current" => format!("                            case {patterns}; if test $seen_panel -eq 1; or begin; contains -- \"$family:$sub\" panel:process-info panel:focus panel:zoom panel:layout panel:neighbor panel:edges panel:resize panel:swap panel:input panel:split panel:create-terminal; and test $state = operand; end; set done 1; end"),
                "--workspace" => {
                    format!("                            case {patterns}; if test $seen_standalone -eq 1; or begin; test $family = worktree; and test $seen_cwd -eq 1; end; set done 1; end; if test \"$family:$sub\" = worktree:list; and test $seen_project -eq 1 -o $seen_directory -eq 1; set done 1; end")
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
            };
            for command in COMMAND_FLAGS.iter().map(|(command, _)| *command) {
                let conflicts = command_flag_conflicts(command, flag);
                if !conflicts.is_empty() {
                    let bits = conflicts.iter().map(|flag| format!("${} -eq 1", flag_bit(flag))).collect::<Vec<_>>().join(" -o ");
                    arm.push_str(&format!("; if test \"$family:$sub\" = {command}; and test {bits}; set done 1; end"));
                }
            }
            arm
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish verb-landing scope arms: the command's own declared flags admit the
/// consumed set — no per-command enumeration of the flags it rejects.
fn scope_cases_fish() -> String {
    let arms = FAMILY_VERBS
        .iter()
        .flat_map(|(family, verbs)| {
            verbs.iter().map(move |verb| {
                let command = format!("{family}:{verb}");
                let allowed = command_flags(&command).join(" ");
                let exclusive = if *family == "worktree" {
                    let mut guard = "\n                                        if test $seen_workspace -eq 1 -a $seen_cwd -eq 1; set done 1; end".to_owned();
                    if command == "worktree:list" { guard.push_str("\n                                        if test $seen_project -eq 1 -o $seen_directory -eq 1; and test $seen_workspace -eq 1 -o $seen_cwd -eq 1 -o $seen_trust_repository -eq 1; set done 1; end"); }
                    if command == "worktree:open" { guard.push_str("\n                                        if test $seen_path -eq 1 -a $seen_branch -eq 1; set done 1; end"); }
                    guard
                } else if matches!(command.as_str(), "panel:current" | "panel:process-info" | "panel:focus" | "panel:zoom" | "panel:layout" | "panel:neighbor" | "panel:edges" | "panel:resize" | "panel:swap" | "panel:input" | "panel:split" | "panel:create-terminal") {
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
                let mut exclusive = exclusive;
                for (flag, conflict) in command_conflict_pairs(&command) {
                    exclusive.push_str(&format!("\n                                        if test ${} -eq 1 -a ${} -eq 1; set done 1; end", flag_bit(flag), flag_bit(conflict)));
                }
                (command, format!("                                        if not __orchbee_admit '{allowed}' $seen_flags; set done 1; end{exclusive}"))
            })
        })
        .collect::<Vec<_>>();
    grouped_fish_cases(arms)
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
                    let offer = if *family == "worktree" {
                        let selector = if command == "worktree:list" { "if begin; test $seen_project -eq 0 -a $seen_directory -eq 0; or test $seen_workspace -eq 0 -a $seen_cwd -eq 0 -a $seen_trust_repository -eq 0; end; " } else if command == "worktree:open" { "if test $seen_path -eq 0 -o $seen_branch -eq 0; " } else { "" };
                        let close = if matches!(command.as_str(), "worktree:open" | "worktree:list") { " end;" } else { "" };
                        format!("                if __orchbee_admit '{allowed}' $seen_flags\n                    if test $seen_workspace -eq 0 -o $seen_cwd -eq 0; {selector}set -a verbs {verb};{close} end\n                end")
                    } else if matches!(command.as_str(), "panel:current" | "panel:process-info" | "panel:focus" | "panel:zoom" | "panel:layout" | "panel:neighbor" | "panel:edges" | "panel:resize" | "panel:swap" | "panel:input" | "panel:split" | "panel:create-terminal") {
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
                    };
                    let mut conflicts = Vec::new();
                    for (flag, conflict) in command_conflict_pairs(&command) {
                        conflicts.push(format!("test ${} -eq 0 -o ${} -eq 0", flag_bit(flag), flag_bit(conflict)));
                    }
                    if conflicts.is_empty() { offer } else { format!("                if {}\n{offer}\n                end", conflicts.join("; and ")) }
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
    let arms = COMMAND_FLAGS
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
                        "--workspace" | "--cwd" if *command == "worktree:list" => "$seen_workspace -eq 0 -a $seen_cwd -eq 0 -a $seen_project -eq 0 -a $seen_directory -eq 0".to_owned(),
                        "--trust-repository" if *command == "worktree:list" => "$seen_trust_repository -eq 0 -a $seen_project -eq 0 -a $seen_directory -eq 0".to_owned(),
                        "--project" | "--directory" if *command == "worktree:list" => format!("${} -eq 0 -a $seen_workspace -eq 0 -a $seen_cwd -eq 0 -a $seen_trust_repository -eq 0", flag_bit(flag)),
                        "--workspace" | "--cwd" if command.starts_with("worktree:") => "$seen_workspace -eq 0 -a $seen_cwd -eq 0".to_owned(),
                        "--path" | "--branch" if *command == "worktree:open" => "$seen_path -eq 0 -a $seen_branch -eq 0".to_owned(),
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
                        "--provider" | "--asset-root" | "--until" | "--token" | "--clear-token" | "--state-label" => "0 -eq 0".to_owned(),
                        _ => format!("${} -eq 0", flag_bit(flag)),
                    };
                    let conflicts = command_flag_conflicts(command, flag).iter().map(|flag| format!("${} -eq 0", flag_bit(flag))).collect::<Vec<_>>();
                    let cond = if conflicts.is_empty() { cond } else { format!("{cond} -a {}", conflicts.join(" -a ")) };
                    if matches!(*command, "panel:process-info" | "panel:focus" | "panel:zoom" | "panel:layout" | "panel:neighbor" | "panel:edges" | "panel:resize" | "panel:swap" | "panel:input" | "panel:split" | "panel:create-terminal") && matches!(*flag, "--panel" | "--current") || (*command == "panel:process-info" && *flag == "--client") || (*command == "agent:attach" && matches!(*flag, "--name" | "--session" | "--panel")) {
                        format!("                if test $state != operand; and test {cond}; set -a candidates {flag}; end")
                    } else { format!("                if test {cond}; set -a candidates {flag}; end") }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some((label.to_owned(), lines))
        })
        .collect::<Vec<_>>();
    grouped_fish_cases(arms)
}

fn grouped_fish_cases(arms: Vec<(String, String)>) -> String {
    // Commands with identical offer predicates share one case body. Group only the fully
    // rendered predicates, so command-specific conflicts and operand guards remain intact.
    let mut groups: Vec<(Vec<String>, String)> = Vec::new();
    for (label, body) in arms {
        if let Some((labels, _)) = groups.iter_mut().find(|(_, existing)| *existing == body) {
            labels.push(label);
        } else {
            groups.push((vec![label], body));
        }
    }
    groups
        .into_iter()
        .map(|(labels, body)| {
            let labels = labels
                .iter()
                .map(|label| format!("'{label}'"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("            case {labels}\n{body}")
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
    let cwd = joined("--cwd", CWD_VALUES);
    let right_click = joined("--right-click", RIGHT_CLICK_VALUES);
    let output_source = joined("--source", OUTPUT_SOURCES);
    let key = joined("--key", KEY_VALUES);
    let until = joined("--until", UNTIL_VALUES);
    let provider = joined("--provider", PROVIDER_VALUES);
    format!(
        concat!(
            "            case '--cwd'; if test $seen_cwd -eq 0; and contains -- \"$family:$sub\" tab:create panel:split panel:create-terminal workspace:create; set candidates {cwd}; end\n",
            "            case '--source'\n",
            "                if test $seen_output_source -eq 0\n",
            "                    switch \"$state\"\n",
            "                        case verb; set candidates {output_source}\n",
            "                        case sub; if contains -- $family terminal agent; and begin; contains -- read $verbs; or contains -- wait-output $verbs; end; set candidates {output_source}; end\n",
            "                        case after operand operand2; if contains -- \"$family:$sub\" terminal:read terminal:wait-output agent:read; set candidates {output_source}; end\n",
            "                    end\n",
            "                end\n",
            "            case '--right-click'\n",
            "                if test $seen_right_click -eq 0\n",
            "                    switch \"$state\"\n",
            "                        case verb; set candidates {right_click}\n",
            "                        case sub; if test $family = panel; and begin; contains -- input $verbs; or contains -- split $verbs; end; set candidates {right_click}; end\n",
            "                        case after operand operand2; if contains -- \"$family:$sub\" panel:input panel:split panel:create-terminal; set candidates {right_click}; end\n",
            "                    end\n",
            "                end\n",
            "            case '--direction'\n",
            "                if test $seen_direction -eq 0 -a $seen_insert_index -eq 0\n",
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
        cwd = cwd,
        right_click = right_click,
        output_source = output_source,
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
    let script = format!(
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
            "    set -l metadata_source_only 0; set -l ended 0; set -l done 0; set -l value \"\"\n",
            "    for word in $words\n",
            "        if string match -q -- \"--\" \"$word\"; set ended 1; continue; end\n",
            "        if test $ended -eq 1 -o $done -eq 1; continue; end\n",
            "        if test -n \"$value\"\n",
            // `test` reads a typed dash-word inside a `-a`/`-o` compound as an
            // operator (fish 4.2.1 rejects `--arg -c` here), so `$word`
            // comparisons stay split into standalone `test` job chains.
            "            if test \"$value\" = format; and test \"$word\" != text; and test \"$word\" != json; set done 1; end\n",
            "            if test \"$value\" = mouse; and test \"$word\" != off; and test \"$word\" != on; set done 1; end\n",
            "            if test \"$value\" = output_source; and not contains -- \"$word\" visible recent recent-unwrapped detection; set metadata_source_only 1; if not string match -rq \"^[[:space:]]*[A-Za-z0-9:._-]{{1,80}}[[:space:]]*$\" -- \"$word\"; set done 1; end; end\n",
            "            if test \"$value\" = right_click; and not contains -- \"$word\" orchbee panel; set done 1; end\n",
            "            set value \"\"\n",
            "            continue\n",
            "        end\n",
            "        if test \"$family:$sub\" = agent:attach\n",
            "            if test \"$state\" = operand; and contains -- (string split -m1 '=' -- $word)[1] --name --session --panel; set done 1; end\n",
            "            if test \"$state\" = after; and not string match -q -- '-*' $word; and test (math \"$seen_name + $seen_session + $seen_panel\") -gt 0; set done 1; end\n",
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
            "                    case sub; if not contains -- $family terminal agent; set done 1; end\n",
            "                    case after operand operand2; if not contains -- \"$family:$sub\" terminal:attach agent:attach; set done 1; end\n",
            "                    case leaf; if test \"$leaf\" != tui; set done 1; end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    if test $seen_mouse -eq 1\n",
            "                        set done 1\n",
            "                    else\n",
            "                        switch \"$word\"\n",
            "                            case '--mouse'; set seen_mouse 1; set value mouse; set -a seen_flags --mouse\n",
            "                            case '--mouse=off' '--mouse=on'; set seen_mouse 1; set -a seen_flags --mouse\n",
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
            "                if string match -q -- '--source=*' $word; and not contains -- $word --source=visible --source=recent --source=recent-unwrapped --source=detection; set metadata_source_only 1; if not string match -rq '^[[:space:]]*[A-Za-z0-9:._-]{{1,80}}[[:space:]]*$' -- (string sub -s 10 -- $word); set done 1; end; end\n",
            "                if string match -q -- '--right-click=*' $word; and not contains -- $word --right-click=orchbee --right-click=panel; set done 1; end\n",
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
            "                                if test $seen_format -eq 1; or not __orchbee_admit '--mouse' $seen_flags\n",
            "                                    set done 1\n",
            "                                else; set leaf tui; set state leaf; end\n",
            "                            case help completion\n",
            "                                if test $seen_format -eq 1 -o (count $seen_flags) -gt 0 -o $seen_mouse -eq 1\n",
            "                                    set done 1\n",
            "                                else; set state $word; end\n",
            "                            case {families}; if test $seen_mouse -eq 1; and not contains -- $word terminal agent; set done 1; else; set family $word; set state sub; end\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "                            case 'terminal:session'\n",
            "                                set sub session; set state deep; __orchbee_admit '--cols --rows --timeout --takeover' $seen_flags; or set done 1\n",
            "                            case 'agent:integration'\n",
            "                                set sub integration; set state deep\n",
            "                                if test $family = agent; and begin; test $seen_provider2 -eq 1; or not __orchbee_admit '{integration_allowed}' $seen_flags; end; set done 1; end\n",
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
            "                                set sub \"$sub:$word\"; set state after\n",
            "                                if test $family = agent; if test $seen_provider2 -eq 1; or not __orchbee_admit '{integration_allowed}' $seen_flags; set done 1; end; else if test $sub = session:observe; and test $seen_takeover -eq 1; set done 1; end\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case after\n",
            "                        switch \"$family:$sub\"\n",
            "                            case panel:process-info panel:focus panel:zoom panel:layout panel:neighbor panel:edges panel:resize panel:swap panel:input panel:split panel:create-terminal; if test $seen_panel -eq 1 -o $seen_current -eq 1; or begin; test \"$family:$sub\" = panel:process-info; and test $seen_client -eq 1; end; set done 1; else; set state operand; end\n",
            "                            case {operand_paths}; set state operand\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case operand; if contains -- \"$family:$sub\" agent:send-keys agent:start; set state operand; else if contains -- \"$family:$sub\" terminal:run agent:rename agent:prompt; and test $seen_clear -eq 0; and test $seen_name -eq 0; and test $seen_session -eq 0; set state operand2; else; set done 1; end; end\n",
            "                    case leaf\n",
            "                        if test \"$leaf\" = status; and contains -- $word client server; set family status; set sub $word; set state after; else; set done 1; end\n",
            "                    case operand2 completion; set done 1\n",
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
            "    if test $metadata_source_only -eq 1; and test \"$state\" != verb; and test \"$family\" != workspace; set done 1; end\n",
            "    set -l possible; set -l verbs; set -l dead 0\n",
            "    if test \"$state\" = verb\n",
            "        set possible {roots}\n",
            "{narrow_lines}\n",
            "        if test $metadata_source_only -eq 1; set possible (__orchbee_narrow workspace $possible); end\n",
            "        if test $seen_mouse -eq 1; set possible (__orchbee_narrow \"tui agent terminal\" $possible); end\n",
            "        if test $seen_format -eq 1; set possible (__orchbee_narrow \"{format_roots}\" $possible); end\n",
            "        if test (count $possible) -eq 0; set dead 1; end\n",
            "    else if test \"$state\" = sub\n",
            "        switch \"$family\"\n",
            "{verb_cases}\n",
            "        end\n",
            "        if test (count $verbs) -eq 0; set dead 1; end\n",
            "    else if test \"$state\" = deep\n",
            "        if test $family = terminal; set verbs control; if test $seen_takeover -eq 0; set -a verbs observe; end; else if test $seen_provider2 -eq 0; and __orchbee_admit '{integration_allowed}' $seen_flags; set verbs {integration_verb_words}; end\n",
            "        if test (count $verbs) -eq 0; set dead 1; end\n",
            "    end\n",
            "    set -l candidates\n",
            "    if test $done -eq 1 -o $ended -eq 1 -o $dead -eq 1; or test (math \"$seen_zoom_on + $seen_zoom_off + $seen_zoom_toggle\") -gt 1; set candidates\n",
            "    else if test \"$value\" = format; set candidates {format_values}\n",
            "    else if test \"$value\" = mouse; set candidates off on\n",
            "    else if test \"$value\" = direction\n",
            "        switch \"$family:$sub\"\n",
            "{direction_cases}\n",
            "        end\n",
            "    else if test \"$value\" = cwd; if contains -- \"$family:$sub\" tab:create panel:split panel:create-terminal workspace:create; set candidates {cwd_values}; end\n",
            "    else if test \"$value\" = output_source; if test \"$family\" != workspace; set candidates {output_source_values}; end\n",
            "    else if test \"$value\" = right_click; set candidates {right_click_values}\n",
            "    else if test \"$value\" = key; set candidates {key_values}\n",
            "    else if test \"$value\" = until; set candidates {until_values}\n",
            "    else if test \"$value\" = provider; set candidates {provider_values}\n",
            "    else if test -n \"$value\"; set candidates\n",
            "    else if string match -q -- '--format=*' $cur\n",
            "        if test $seen_format -eq 0\n",
            "            switch \"$state\"\n",
            "                case verb; set candidates --format=text --format=json\n",
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
            "                        case verb; if begin; contains -- tui $possible; or contains -- agent $possible; or contains -- terminal $possible; end; set candidates --mouse=off --mouse=on; end\n",
            "                        case sub; if contains -- $family terminal agent; set candidates --mouse=off --mouse=on; end\n",
            "                        case after operand operand2; if contains -- \"$family:$sub\" terminal:attach agent:attach; set candidates --mouse=off --mouse=on; end\n",
            "                        case leaf; if test \"$leaf\" = tui; set candidates --mouse=off --mouse=on; end\n",
            "                    end\n",
            "                end\n",
            "{joined_offer_cases}\n",
            "        end\n",
            "    else if test \"$state\" = verb\n",
            "        set candidates $possible\n",
            "        if test $seen_format -eq 0; set -a candidates --format --format=; end\n",
            "        if begin; contains -- tui $possible; or contains -- agent $possible; or contains -- terminal $possible; end; and test $seen_mouse -eq 0; set -a candidates --mouse; end\n",
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
            "        if test \"$leaf\" = status; set candidates client server; end\n",
            "        if test \"$leaf\" = tui -a $seen_mouse -eq 0; set candidates --help --mouse\n",
            "        else if test $seen_format -eq 0; set -a candidates --format --format= --help; end\n",
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
        cwd_values = CWD_VALUES.join(" "),
        right_click_values = RIGHT_CLICK_VALUES.join(" "),
        output_source_values = OUTPUT_SOURCES.join(" "),
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
    );
    // This generated Fish program has no multiline literals or heredocs.
    // Remove one formatting indent, keeping grammar growth within the fixed-body budget.
    script
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Alternative retained group selectors cannot be mixed with ordinary scope IDs.
fn command_conflict_pairs(
    command: &str,
) -> std::collections::BTreeSet<(&'static str, &'static str)> {
    command_flags(command)
        .iter()
        .flat_map(|flag| {
            command_flag_conflicts(command, flag)
                .iter()
                .map(move |other| {
                    if flag < other {
                        (*flag, *other)
                    } else {
                        (*other, *flag)
                    }
                })
        })
        .collect()
}

fn command_flag_conflicts(command: &str, flag: &str) -> &'static [&'static str] {
    match (command, flag) {
        ("panel:create-terminal", "--child") => &["--presentation"],
        ("panel:create-terminal", "--presentation") => &["--child", "--label"],
        ("panel:create-terminal", "--label") => &["--presentation"],
        ("panel:create-terminal", "--panel") => &["--current", "--client"],
        ("panel:create-terminal", "--current" | "--client") => &["--panel"],
        ("panel:create-terminal", "--profile") => &["--exec", "--arg"],
        ("panel:create-terminal", "--exec" | "--arg") => &["--profile"],
        ("panel:report-metadata", "--title") => &["--clear-title"],
        ("panel:report-metadata", "--clear-title") => &["--title"],
        ("panel:report-metadata", "--display-agent") => &["--clear-display-agent"],
        ("panel:report-metadata", "--clear-display-agent") => &["--display-agent"],
        ("panel:report-metadata", "--state-label") => &["--clear-state-labels"],
        ("panel:report-metadata", "--clear-state-labels") => &["--state-label"],

        ("layout:apply", "--file") => &["--stdin"],
        ("layout:apply", "--stdin") => &["--file"],
        ("layout:apply", "--tab") => &["--workspace", "--standalone"],
        ("layout:apply", "--workspace") => &["--tab", "--standalone", "--confirm"],
        ("layout:apply", "--standalone") => &["--tab", "--workspace", "--confirm"],
        ("layout:apply", "--confirm") => &["--workspace", "--standalone"],
        ("layout:apply", "--focus") => &["--no-focus"],
        ("layout:apply", "--no-focus") => &["--focus"],
        ("layout:export", "--tab") => &["--panel", "--current"],
        ("layout:export", "--panel") => &["--tab", "--current"],
        ("layout:export", "--current") => &["--tab", "--panel"],
        ("panel:move", "--into-tab") => &[
            "--new-tab",
            "--new-workspace",
            "--workspace",
            "--standalone",
            "--project",
            "--label",
            "--tab-label",
        ],
        ("panel:move", "--new-tab") => &[
            "--into-tab",
            "--new-workspace",
            "--target-panel",
            "--direction",
            "--ratio",
            "--project",
            "--tab-label",
        ],
        ("panel:move", "--new-workspace") => &[
            "--into-tab",
            "--new-tab",
            "--target-panel",
            "--direction",
            "--ratio",
            "--workspace",
            "--standalone",
        ],
        ("panel:move", "--target-panel" | "--direction" | "--ratio") => {
            &["--new-tab", "--new-workspace"]
        }
        ("panel:move", "--label") => &["--into-tab"],
        ("panel:move", "--workspace") => &[
            "--into-tab",
            "--new-workspace",
            "--standalone",
            "--project",
            "--tab-label",
        ],
        ("panel:move", "--standalone") => &[
            "--into-tab",
            "--new-workspace",
            "--workspace",
            "--project",
            "--tab-label",
        ],
        ("panel:move", "--project" | "--tab-label") => {
            &["--into-tab", "--new-tab", "--workspace", "--standalone"]
        }
        ("panel:move", "--focus") => &["--no-focus"],
        ("panel:move", "--no-focus") => &["--focus"],
        ("agent:attach", "--name") => &["--session", "--panel"],
        ("agent:attach", "--session") => &["--name", "--panel"],
        ("agent:attach", "--panel") => &["--name", "--session"],
        ("worktree:create" | "worktree:open", "--focus") => &["--no-focus", "--detach"],
        ("worktree:create" | "worktree:open", "--no-focus" | "--detach") => &["--focus"],
        ("worktree:list", "--client") => &["--project", "--directory"],
        ("worktree:list", "--project" | "--directory") => &["--client"],
        ("panel:current" | "panel:process-info", "--panel") => &["--current", "--client"],
        ("panel:current" | "panel:process-info", "--client" | "--current") => &["--panel"],
        ("terminal:wait-output", "--source" | "--lines") => &["--rows"],
        ("terminal:wait-output", "--forever") if cfg!(windows) => &["--timeout"],
        ("terminal:wait-output", "--rows") if cfg!(windows) => &["--source", "--lines"],
        ("terminal:wait-output", "--forever") => &["--rows", "--timeout"],
        ("terminal:wait-output", "--timeout") => &["--forever"],
        ("terminal:wait-output", "--rows") => &["--source", "--lines", "--forever"],

        ("terminal:read" | "agent:read", "--source") => &["--history", "--rows", "--unwrap"],
        ("terminal:read" | "agent:read", "--history" | "--rows" | "--unwrap") => {
            &["--source", "--plain", "--ansi"]
        }
        ("terminal:read" | "agent:read", "--plain") => {
            &["--ansi", "--history", "--rows", "--unwrap"]
        }
        ("terminal:read" | "agent:read", "--ansi") => {
            &["--plain", "--history", "--rows", "--unwrap"]
        }
        ("tab:create" | "panel:split" | "workspace:create", "--focus") => &["--no-focus"],
        ("tab:create" | "panel:split" | "workspace:create", "--no-focus") => &["--focus"],
        ("tab:create" | "panel:split", "--profile") => &["--exec", "--arg"],
        ("tab:create" | "panel:split", "--exec" | "--arg") => &["--profile"],
        ("panel:input" | "panel:split", "--panel") => &["--current"],
        ("panel:input" | "panel:split", "--current") => &["--panel"],
        ("tab:move", "--direction") => &["--insert-index"],
        ("tab:move", "--insert-index") => &["--direction"],
        ("panel:swap", "--with") => &["--direction"],
        ("panel:swap", "--direction") => &["--with"],
        ("panel:rename", "--label") => &["--clear"],
        ("panel:rename", "--clear") => &["--label"],
        ("layout:set-split-ratio", "--tab") => &["--panel", "--current"],
        ("layout:set-split-ratio", "--panel") => &["--tab", "--current"],
        ("layout:set-split-ratio", "--current") => &["--tab", "--panel"],
        ("panel:zoom", "--on") => &["--off", "--toggle"],
        ("panel:zoom", "--off") => &["--on", "--toggle"],
        ("panel:zoom", "--toggle") => &["--on", "--off"],

        ("workspace:close-status", "--group" | "--request")
        | ("workspace:close-resume", "--group" | "--detach" | "--timeout") => {
            &["--project", "--workspace"]
        }
        ("workspace:close-status", "--project" | "--workspace") => &["--group", "--request"],
        ("workspace:close-resume", "--project" | "--workspace") => {
            &["--group", "--detach", "--timeout"]
        }
        _ => &[],
    }
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
        assert_eq!(
            BUSINESS_FLAGS.len(),
            BUSINESS_FLAGS
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            "duplicate flags multiply every shell projection"
        );
        // Every operand path is an implemented verb.
        for path in OPERAND_PATHS {
            let (family, verb) = path.rsplit_once(':').expect("family:verb");
            assert!(
                FAMILY_VERBS
                    .iter()
                    .any(|(f, verbs)| *f == family && verbs.contains(&verb))
                    || HELP_LEVEL3
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
                || HELP_LEVEL3
                    .iter()
                    .any(|(f, verbs)| *f == family && verbs.contains(&verb));
            assert!(implemented, "{command} is not an implemented command");
            for flag in *flags {
                assert!(
                    *flag == "--mouse" || BUSINESS_FLAGS.contains(flag),
                    "{command} lists {flag}"
                );
            }
        }
        // Every integration verb is contained in its union; only list admits maintenance filtering.
        for verb in INTEGRATION_VERBS {
            assert_eq!(
                command_flags(&format!("agent:integration:{verb}")),
                if *verb == "list" {
                    INTEGRATION_COMMAND_FLAGS
                } else {
                    &["--provider", "--adapter-source"]
                },
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
                || *command == "panel:create-terminal"
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
            // These assertions own control flow, not generated indentation.
            let unindented = script
                .lines()
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n");
            let normalized = |text: &str| {
                text.lines()
                    .map(str::trim_start)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            assert!(
                unindented.contains(&normalized(exec_arm)),
                "{shell:?} --exec arm"
            );
            assert!(
                unindented.contains(&normalized(arg_arm)),
                "{shell:?} --arg arm"
            );
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
