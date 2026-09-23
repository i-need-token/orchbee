# Orchbee native CLI Agent guide

Use `agent rename <session-id> <name>` for a unique runtime alias and `agent names` to list bindings. `agent get <name>`, `agent explain <name>` and `agent wait <name>` resolve directly; a wait pins the original session despite later name reuse. `agent resolve <name>` also exposes the full ID. `--name <alias>` explicitly selects an alias when it collides with a session ID; `--session <id>` explicitly selects the ID. `agent rename <target> --clear` releases only the name. Names disappear on session end or Site disposal. A lost rename reply is outcome-unknown; never replay it blindly.

## Discover this build

Use `orchbee --help`, `orchbee help <family>`, and `orchbee help <family> <command>` to discover exact grammar. `orchbee --version`, `orchbee --skill`, `orchbee completion powershell`, and `orchbee api schema` work offline. `orchbee api schema <schema-id>` prints an embedded result schema. The native executable and Desktop CLI have different capabilities; use the help from the executable you will invoke.

The bare `orchbee` command launches the interactive TUI. Use explicit commands for automation. Help and completion do not start a Site. `orchbee site start --format json` explicitly starts or connects to the configured local Site. Existing runtime selection uses `ORCHBEE_RUNTIME_BASE`; do not guess endpoint paths or activate a Site merely to inspect help.

## Resolve targets and inspect state

Use `orchbee status --format json` or `orchbee api snapshot` to inspect the running Site, then the appropriate `project`, `workspace`, `tab`, `panel`, `terminal`, `worktree`, or `agent` list/get command. Follow each command's declared filters and use returned stable IDs. Labels and suffix selectors are only accepted where help declares them; ambiguous targets must be resolved explicitly.

`orchbee terminal read <terminal-id> --history --rows 50 --lines 50 --unwrap --format json` reads retained output. Output can contain secrets; forward only what the task needs. Read/query commands do not create missing resources. Workspace creation, directory admission, and initial Tab creation are separate commands in this build; use their help for required parameters.

`orchbee agent read --name alpha --lines 50 --format json` reads the visible terminal
for a live named session. Add `--history --rows 50` for retained scrollback instead.
The result uses the terminal read/history schema and identifies the exact terminal.

## Send bounded input once

`orchbee terminal send <terminal-id> --text "echo ready" --enter --format json` submits text and a carriage return. Text is limited to 4096 UTF-8 bytes including the optional carriage return. Repeat `--key` to send a sequence of semantic keys; modifiers use combinations such as `ctrl+shift+a`. At most 64 keys are accepted.

Use exactly one source family: `--text`, repeated `--key`, `--file <path>`, or `--stdin`. File/stdin input preserves literal bytes, including non-UTF-8 data, with a 1 MiB bound including an appended carriage return. `--paste` is valid only with file/stdin and bounds content to 256 KiB; `--enter` adds a separate trailing Enter record after paste. The complete source is acquired and validated before sending anything. Empty input is rejected. All acquisition and transport share a five-second deadline.

Inspect the result's input and cleanup outcomes independently. Partial acceptance is not whole-request success. An unknown outcome (exit 8) may already have delivered input: inspect the terminal before taking another action and never blindly replay the request. Sending does not acquire the TUI's controller or resize ownership.

`orchbee agent send-keys --name alpha enter ctrl+c --format json` targets the Agent
terminal directly. Full session IDs also work. A missing, ended, non-navigable or
ambiguous binding refuses input. The same sequence bounds and send receipt apply.

## Observe Agents and close resources

Use `agent list/get/explain` for canonical session facts and `agent wait` with a declared condition, timeout, and optional `--after-revision` for later state. `terminal wait-output` can match existing retained output. Neither an old matching line nor an idle state proves a newly submitted task completed. Use `agent prompt <target> <text> --wait` to submit literal paste plus Enter and observe activity before matching a condition. It refuses pending attention and non-fresh/unknown state. A stalled, timed-out or cancelled wait retains its accepted submission receipt; inspect that receipt before any retry. An already-working Agent may finish its existing turn, so matching is not a prompt-specific completion guarantee. `--until` and `--timeout` require `--wait`; the default timeout is 600000 ms. This build still has no `agent start`; do not infer Agent readiness from terminal creation.

On Windows, `agent doctor` inspects local setup offline. `agent integration list/explain/enable/disable --provider codex` manages the local Codex plugin; inspect its receipt and then list the observed state. Other providers remain manual-only, and `site diagnose` remains unavailable. Other unsupported operations return a typed failure; do not replace a failure with guessed state.

Destructive close/delete commands require `--confirm`. A closed Workspace does not delete its Git checkout or branch. Use Workspace close-status/close-resume when the close result requires recovery. Exiting a TUI client does not stop the Site.

## Consume results

Use `--format json` where declared. Standard output carries one machine document; localized diagnostics use standard error. Inspect both the document and process exit code. Exit 2 is invalid usage, 4 unavailable, 7 timeout, 8 unknown outcome, and 130 cancellation. Do not parse human prose or assume a failed/cancelled wait cancelled the observed operation. Keep stable IDs and request identities needed for recovery.

## Start in the intended directory and environment

`tab create` and `panel split` accept `--cwd <absolute-path>` and repeatable `--env NAME=VALUE`,
with the default shell, an explicit program, or `--profile`. Values are literal; an empty value is
allowed. An explicit cwd overrides the accepted Workspace directory; without either, Worker home
is used. The directory must already exist. These options do not modify the stored profile.
At most 32 environment entries and 1,024 UTF-8 name/value bytes are allowed; names are ASCII
identifiers, unique ignoring case. Orchbee and terminal identity variables are reserved.
Inspect the terminal outcome in the result: committed membership alone does not prove a process
started. The native CLI help owns the complete validation rules.

## Locate the calling Panel

Inside a newly launched Windows terminal, use `orchbee panel current --format json`
to find its current Panel, Tab and Workspace. `--current` is an explicit spelling of
the same lookup. Outside that process, use `panel current --panel <id-or-selector>`.
Worker supplies `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID`; the query validates
their current binding in one snapshot. Missing or stale context is an error, never
a fallback to client focus. Explicit `--panel` ignores inherited context and cannot
be combined with `--current`. `api schema` now returns catalog envelope version 14;
the new `orchbee.panel.current` document is version 1.

Use explicit `--forever` on `agent wait` or `agent prompt --wait` for no total deadline; it conflicts with `--timeout`. The default remains 600000 ms and every read stays bounded. `--until blocked` matches fresh needs-attention; `--until done` matches fresh completed outcomes, without an unseen marker. Wait/prompt successes use version 2 with null timeoutMs for infinite mode; infinite read-timeout failures also use version 2. Windows Codex prompts add Right between paste and Enter to clear paste-burst Enter suppression.

Targeted Agent commands also accept `--panel <id|@panel:suffix>` or a positional Panel ID. Session IDs and names take precedence over positional Panel fallback. Panels select their primary terminal and require one live Agent; waits pin the selected Session.
