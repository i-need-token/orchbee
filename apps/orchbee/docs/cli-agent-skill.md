# Orchbee native CLI Agent guide

This guide describes user workflows for the Windows native CLI. Discover exact syntax and platform support from the installed build before acting.

## Discover this build

Start with `orchbee --help`, then `orchbee <family> --help` and `orchbee <family> <command> --help`. A single help flag before `--` also works with already supplied operands and options, without executing them. Unknown options, missing values and duplicates still fail. `help <path>` accepts an exact help path. A bare `orchbee` starts the TUI.

`--version`, `--skill`, `completion <shell>`, `api schema`, and Windows `--default-config` work offline. Completion supports Bash, Zsh, Fish, PowerShell and Elvish. `api schema` lists the schema IDs; use `api schema <schema-id>` for the same build's machine contract, or add `--output <path>` to export it. Read the emitted `schema` and `version`; this guide does not pin catalog counts or historical versions.

For PowerShell, `orchbee completion powershell` prints the completion script.

## Select and start a Session

Use `config check` to inspect local configuration, `status client` to inspect this executable, and `status server` to inspect the selected Server. These commands do not start a Session. Use `server diagnose` for read-only runtime facts and `server start` for headless startup.

`--session <name>` selects a local named Session and may appear before or after the command. The default is `default`. Use the same selection on subsequent commands and diagnostics. `session list` shows the local inventory; `session attach <name>` enters its TUI. Exiting a TUI leaves the Session running.

```text
orchbee --session work server start
orchbee --session work status --format json
orchbee --session work tui
```

A missing runtime base is an environment check failure; inspect `config check` before retrying. Recovery-required means startup could not establish safe ownership. Diagnose that exact Session or use another name. Do not delete runtime directories manually.

## Resolve targets and inspect state

A Workspace has a directory and may belong to a Project. Project grouping is optional. A Workspace contains Tabs; Tabs contain Panels; a Panel has a primary terminal and may have child terminals. TUI Clients retain their own selection and view.

Use `api snapshot`, then the relevant `project`, `workspace`, `tab`, `panel`, `terminal`, `launch-profile`, `client` or `agent` list/get command. Reuse returned IDs. Prefer printed stable handles such as `@panel:0x1`: they preserve the exact ID with leading zeros omitted and cannot change when other resources come or go. Quote handles in PowerShell. Legacy suffix selectors still work but are momentary; never save them as identities. IDs and compact handles are scoped to the selected Session and underlying identity lifetime; rediscover runtime terminal IDs after Server restart.

Inside an Orchbee terminal, caller metadata can select its Panel. Outside, `panel current` can use the unique current TUI Client; pass `--client <id>` when several exist or `--panel <id>` for a headless lookup. `agent start` without `--panel` specifically requires valid caller metadata. Partial or stale caller metadata never authorizes a fallback.

## Create and arrange terminals

```text
orchbee workspace create --cwd . --label work
orchbee tab create --workspace work --label shell
orchbee panel list --format json
orchbee panel split --panel <panel-id> --direction right
```

Inspect each creation receipt: committed layout membership does not prove a process started. Commands that need a Client refuse missing, ambiguous or unready Clients; use `client list` and an explicit `--client`.

`tab create`, `panel split` and `panel create-terminal` support the declared launch choices, cwd and literal environment entries. Use `--profile` for a saved Launch Profile or `--exec` with repeatable `--arg` for an explicit program. The directory must already exist. Inspect help for `--cwd home|current|workspace`, absolute overrides, complete terminal geometry and the bounds on `--env NAME=VALUE`. These overrides do not modify a saved profile.

`panel create-terminal --child` creates a child terminal. Use `workspace`/`tab` move commands, `panel move`/`swap`, `layout`, `panel resize`, `panel zoom` and focus commands for arrangement. Client focus controls TUI selection; it does not activate an OS window. Read the command's help before choosing current targets or layout paths.

## Send input and read output

`terminal run <terminal-id> "git status"` submits one literal command plus Enter. `terminal send` accepts exactly one of `--text`, `--file` or `--stdin`, with optional `--paste`/`--enter`, or declared keys. Keys are ordered; at most 64 are accepted. File/stdin input is read locally, including when routing to a saved machine. The source read is capped at 1 MiB; the resulting paste and argv retain their smaller declared bounds.

A receipt proves accepted input records, not Shell readiness, command completion or exit status. Never replay partial or unknown delivery. Inspect the receipt and terminal before deciding what to send next.

`terminal read` and `agent read` default on Windows to recent output. Select `visible`, `recent`, `recent-unwrapped` or `detection` with `--source`; `--plain` emits UTF-8 text and excludes JSON. Legacy `--history --rows` has a separate history contract. Do not mix incompatible output modes; inspect help for limits, wrapping and ANSI.

`terminal wait-output` can match output that already exists. Windows defaults to no total timeout; use `--timeout 30000` for bounded automation, or explicit `--forever`. `--timeout` and `--forever` conflict. A matched line alone does not prove a newly submitted command finished.

## Install and operate Agents

`agent doctor` inspects local setup offline. Use `agent integration list`, then `explain --provider <id>` to discover each provider's actual availability and management mode. `enable`/`disable --provider <id>` perform explicit guarded changes. `list --outdated-only` shows owned repair candidates and separately reports blocked or unresolved providers; an empty list does not establish complete inspection.

Windows Codex inspection and installation use the same configuration root: nonempty `CODEX_HOME`, otherwise the native user profile's `.codex`. Invalid overrides refuse without fallback. Provider exit alone does not prove installation; read the observed state in the mutation receipt.

`agent start review --kind codex --panel <panel-id>` launches in an observed idle cmd, PowerShell or pwsh terminal and waits for verified readiness. Default timeout: 30000 ms; allowed range: 3001–300000. Literal provider arguments follow `--`, or use repeatable `--arg=<value>`. A timeout or cancellation can leave the provider running; inspect the submission and pending reservation with `agent names` before retrying.

`agent list/get/explain` inspect current Agent state. On Windows, `agent list` joins canonical sessions with physical terminal Agents and reports each row's source, semantic status and placement; `--client <id>` adds separate Client observations without changing status. Pages contain 1–100 rows (default 50). Windows JSON uses `orchbee.agent.list` v2 and `orchbee.agent.get` v19. List cursors use `u1.<base64url>` with canonical JSON encoded as base64url, are at most 1024 characters, and bind to the Site, source revisions, filters and Client selection. A recognized legacy-format cursor is rejected before live source reads; determining whether a `u1` cursor is stale requires reading the current sources. Restart from the first page after `cursor-restart`. Optional cwd enrichment is limited to 64 rows per page and may be null.

On Windows, `@agent:<kind>` selects a unique provider and `@agent:<n>` selects a one-based ordinal in the complete current list, not the current page. Bare names keep Server alias precedence before provider matching. `agent get`, `read`, `prompt`, `send-keys`, `wait` and `focus` use the same resolver; an incomplete projection cannot establish a unique provider match. Version 19 `agent get` keeps ordinary Agent identity and status in `result.agent`, with facet-specific detail in `result.inspection`.

`agent rename <target> <name>` assigns a runtime alias; `agent resolve` and `agent names` inspect it, and `agent rename <target> --clear` releases it. Aliases expire with their sessions. Waits remain pinned to their original target despite later alias reuse.

`agent prompt <target> <text> --wait` submits literal input and observes activity before checking a condition. `agent wait` only observes. Windows defaults to no total timeout and idle/done/blocked; specify a finite timeout for automation. Freshness, attention and existing work affect admission. A matched state does not identify a prompt-specific turn. Cancelling a wait does not undo accepted input or stop the Agent.

Retain observed revisions and inspect `agent wait --help` before using `--after-revision` for later state.

`--until`, `--timeout` and `--forever` require `agent prompt --wait`; `--forever`
conflicts with `--timeout`. Windows Codex input includes a Right key between paste and
Enter. Linux retains a 600000 ms default wait deadline. A wait failure retains any
accepted input receipt, so never replay uncertain input.

## Events and terminal streams

`events subscribe --event workspace.created --timeout 30000` prints one line per event; add `--format json` for NDJSON records. `orchbee help events subscribe` lists every event name. `events wait` waits for one filter and defaults to 30000 ms; timeout `0` checks the admitted baseline once. Retained cursors include an epoch and decimal-string sequence; a gap requires rebuilding the baseline.

For a typed filter, inspect `api schema orchbee.events.stream` at `$defs/filter`. A minimal valid input is:

```json
{
	"kind": "event",
	"event": "workspace.created",
	"scope": { "kind": "all" },
	"condition": { "kind": "any" }
}
```

Typed filters own their scopes and conditions and cannot mix with shorthand selectors. Use the simple `--event` form unless you need the typed surface.

`terminal session observe` emits bounded NDJSON frames without control. `terminal session control` accepts NDJSON input; inspect its declared input schema before writing records. `terminal attach` and `agent attach` require real console stdin/stdout. `Ctrl+B`, then `q`, detaches; a doubled prefix sends one literal prefix. Attachment does not start a Session, reconnect or replay input.

## Plugins and saved machines

Use `plugin list`, `plugin action list` and `plugin pane open --help` to discover installed capabilities. `plugin link` registers a local manifest; `plugin install` supports `--yes` for the declared noninteractive trust flow. Registry commands do not start a Session; action invocation and pane commands require an existing Session.

`plugin action invoke <action-id> --context <json>` accepts display overrides such as `{"selectedText":"example"}`. Inspect `api schema orchbee.plugin.runtime` at `$defs/context`; fields use camelCase and unknown fields are refused. Accepted plugin work can continue after the CLI exits. Use `plugin log list` for retained logs.

Popup placement requires a current ready Client and focus. Remove `--no-focus`, or choose tab/split placement. `popup close` disposes of the popup terminal. Normal pane and popup lifetime rules differ; inspect the relevant receipt.

`machine list/add/rename/enable/disable/remove` manage saved SSH profiles. The routing option must be first: `orchbee --machine <name-or-id> <command>`. It conflicts with local `--session`; TUI and terminal streams are not routed. Remote cwd names remote paths, while file/stdin input is local. Removing a profile does not stop remote sessions.

## Close, stop and recover

Destructive commands require their declared `--confirm`. Workspace close does not delete its Git checkout or branch. Use close-status/close-resume or worktree status/resume with the retained request ID and generation when a receipt requires recovery. Do not start a second operation as an implicit retry.

`server stop --confirm` or `session stop <name> --confirm` ends the Session after its final checkpoint. Startup restores eligible layout and launch facts, not live screen history or active Agent state. A failed/cancelled CLI wait can leave the stop in progress; query its retained identity.

`session delete <name> --confirm` removes only proven stopped, unused owned state. It refuses the default Session and live or uncertain ownership, and does not delete project directories. Repeat an interrupted delete only after resolving its reported cause.

## Consume results and preserve UTF-8

Use `--format json` where declared. Ordinary queries emit one document; event/terminal streams emit NDJSON and attachment is interactive. Localized diagnostics go to stderr. Inspect the document and exit code together: 2 usage, 3 target/refusal, 4 unavailable, 5 denied, 6 incompatible/protocol, 7 timeout, 8 unknown outcome, 9 integration refusal/failure, and 130 cancellation. Read `schema`, `version`, completeness and the operation's own outcome. Human tables exceeding 80 cells use labeled records so full IDs stay copyable.

New managed PowerShell terminals initialize native pipeline encoding as UTF-8. In an externally started PowerShell, set its decoding and outgoing pipeline encoding before consuming native output:

```powershell
$cliUtf8 = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $cliUtf8
$OutputEncoding = $cliUtf8
orchbee workspace list --format json | ConvertFrom-Json
```

## Further capabilities

Use help for metadata reporting, Agent view filters, attention/acknowledgement, terminal selection/copy/links/graphics, layout export/apply, notifications, configured commands, manifest maintenance, configuration reload/import/reset, update and channel selection. Some are Windows-only; the installed grammar owns admission and effect details. `channel set` saves its selection and starts an update attempt, so inspect both the saved setting and update result.

Detailed behavior and maintenance contracts are indexed in [the native CLI reference](native-cli-query.md) and [the application README](../README.md). Prefer the installed build's help and schemas for executable discovery.

## Wait for a shell command or new output

For an idle, empty command line in a newly initialized managed PowerShell terminal, use `terminal run <target> <command> --wait --timeout 30000 --format json`. It submits once and waits for that invocation's shell markers. Inspect `submission`, `completion`, `failure` and `cleanup` separately. `powershell-success` reports Shell success (0/1), not a native process's numeric exit code. Timeout/cancellation never retracts accepted input; do not replay it.

To exclude retained output, save `terminal read --format json`'s `result.cursor` and pass `--after <cursor>` to `terminal wait-output`, or use `--after now` before new work. Only later physical output rows match. The current row and prompt/input rows are excluded. Reset/eviction/window gaps reject the cursor explicitly; increase `--lines` in advance and use a finite `--timeout` for automation. See [command submission](native-terminal-run.md).

## Headless layout operations

Explicit targets let shared layout operations run without a TUI. Use `panel layout <panel>`,
`layout export --tab <tab>`, `layout set-split-ratio --tab <tab> --path root --ratio 0.6`,
and `panel swap <panel> --with <other-panel>` without `--client`. The shared receipts retain
canonical identities and outcomes; they make no viewport or focus claim. `layout apply --file
layout.json --standalone` also runs headlessly; explicit Workspace and confirmed Tab destinations
are supported. Add `--client` for Client geometry/observations, or `--focus` when applying to a
Client. Directional neighbors, resize and zoom require Client geometry. Read commands do not
accept `--request-id` or `--detach`. Inspect unknown mutation receipts before another submission.
