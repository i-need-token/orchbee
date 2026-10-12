# Orchbee native CLI Agent guide

Windows named Site lifecycle: `session list [--format json]` reads the bounded local inventory; `session attach <name> [--mouse on|off]` enters the real TUI; `session stop <name> --confirm` publishes a final checkpoint. `session delete <name> --confirm` removes only proven stopped/unused owned runtime and checkpoint state, never user project/worktree directories. It refuses default and uncertain/live state. An incomplete delete fences startup; resolve the reported cause and repeat that exact delete command. A name operand must agree with explicit `--site`. List emits `orchbee.session.list` v2 (closed v1 retained), including `recoverable`; delete retains v1 and the embedded schema catalog is v143. Startup automatically recovers complete v2–v5 registration only when all exact processes are gone and resource checks pass. New activations publish catalog v5 before marker, Server or checkpoint-store publication; stopped v1/v2 records require the primary checkpoint, while stopped v3–v5 records admit their fixed slots and exact owned corrupt-original names. Readers limited to v1–v4 refuse v5 before effects. Historical normalization that exceeds the unchanged Snapshot byte limit refuses startup and preserves the original checkpoint; it does not fall back as corruption. Background and clean-stop checkpoints restore layout, Launch Profiles, sampled cwd and unique provider resume references; corrupt bytes are retained and startup warns on fallback or an empty workspace. Legacy Active, live or uncertain owners refuse without deletion; diagnose with the same `--site` or start another named Site. Live Agent state, terminal history and screen contents are excluded; live Host adoption remains separate.

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` pastes one command and sends Enter in one input request. Supply one quoted string; text is preserved, and targets are exact IDs or `@terminal:<suffix>`. The receipt confirms input submission, not Shell readiness, command completion or exit status; never replay partial or unknown delivery. [Command submission contract](native-terminal-run.md).

Use `agent rename <session-id> <name>` for a unique runtime alias and `agent names` to list bindings. `agent get <name>`, `agent explain <name>` and `agent wait <name>` resolve directly; a wait pins the original session despite later name reuse. `agent resolve <name>` also exposes the full ID. `--name <alias>` explicitly selects an alias when it collides with a session ID; `--session <id>` explicitly selects the ID. `agent rename <target> --clear` releases only the name. Names disappear on session end or Site disposal. A lost rename reply is outcome-unknown; never replay it blindly.

Windows `terminal read` and `agent read` default to recent output (80 physical rows) and `orchbee.terminal.output` v1. `--source visible|recent|recent-unwrapped|detection` selects explicitly; bare `--unwrap` chooses recent-unwrapped. `--lines` allows zero and clamps to 1000. `--plain` exports UTF-8 directly and excludes JSON. Explicit source/plain/ANSI cannot mix with legacy history/rows/unwrap. Explicit `--history --rows` retains its history-only document. The [output contract](native-terminal-output.md) owns sample validation and migration.

Windows `terminal wait-output` defaults to recent80 and no total timeout, including explicit source selection. `--timeout` is finite; `--forever` explicitly requests the default and conflicts with timeout. `--rows` aliases recent lines (1..1000), excluding explicit source/lines. All Windows waits emit v3 with nullable timeoutMs and retain bounded I/O and Ctrl+C cleanup. Recent joins soft wraps; visible/detection match physical lines. Existing output can match without proving command completion. Use `--source visible` for a screen read or `--lines 1000 --timeout 30000` for a finite larger wait.

## Discover this build

Use `orchbee --help`, `orchbee help <family>`, and `orchbee help <family> <command>` to discover exact grammar. `orchbee --version`, `orchbee --skill`, `orchbee completion powershell`, and `orchbee api schema` work offline; Windows also provides the offline `orchbee --default-config`. `orchbee api schema <schema-id>` prints an embedded result schema. The native executable and Desktop CLI have different capabilities; use the help from the executable you will invoke.

The bare `orchbee` command launches the interactive TUI. Use explicit commands for automation. Help and completion do not start a Site. `orchbee site start --format json` explicitly starts or connects to the configured local Site. Existing runtime selection uses `ORCHBEE_RUNTIME_BASE`; do not guess endpoint paths or activate a Site merely to inspect help.

## Resolve targets and inspect state

On Windows, `--site <name>` selects a named local Site for startup, queries, input and stop.
`default` selects the original default Site; it overrides inherited `ORCHBEE_RUNTIME_SCOPE`
and `ORCHBEE_SITE`. Managed child terminals inherit their exact owning route. Names are
case-sensitive 1–64 ASCII letters, digits, dot, underscore or hyphen, excluding `.` and `..`.
Keep Agent `--session <id>` separate from the Site selector. Only activation creates a Site.

Use `orchbee status --format json` or `orchbee api snapshot` to inspect the running Site, then the appropriate `project`, `workspace`, `tab`, `panel`, `terminal`, `worktree`, or `agent` list/get command. Follow each command's declared filters and use returned stable IDs. Labels and suffix selectors are only accepted where help declares them; ambiguous targets must be resolved explicitly.

`orchbee terminal read <terminal-id> --history --rows 50 --lines 50 --unwrap --format json` reads retained output. Output may contain secrets; forward only what the task needs. Reads never create missing resources. On Windows, use `workspace create` directly for an ungrouped Workspace and default shell, without first creating a Project or supplying a label; no AI program starts automatically. Optional `--cwd`, `--label`, explicit `--project` and existing exec/profile/env/geometry controls remain available. `--no-focus` changes only focus policy. All forms return `orchbee.workspace.creation` v2; read IDs, nullable `projectId` and terminal facts from `workspace.result`, preserving partial receipts after failure. CLI has no default-Project preparation phase.

Use `workspace set-project <workspace> --project <project>` to regroup, `workspace create-project <workspace> --label <label>` to atomically create a Project and move the Workspace, and `workspace ungroup <workspace>` to remove grouping. `project dissolve <project>` removes organization and retains all Workspaces. These metadata commands preserve IDs, PTYs, cwd and Git/filesystem state. `workspace move`/`move-block` remain reorder commands. Projects may contain multiple repositories; ordinary and worktree Workspaces are peers.

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

Use `agent list/get/explain` for canonical session facts and `agent wait` with a declared condition, timeout, and optional `--after-revision` for later state. `terminal wait-output` can match existing retained output. Neither an old matching line nor an idle state proves a newly submitted task completed. Use `agent prompt <target> <text> --wait` to submit literal paste plus Enter and observe activity before matching a condition. It refuses pending attention and non-fresh/unknown state. A stalled, timed-out or cancelled wait retains its accepted submission receipt; inspect that receipt before any retry. An already-working Agent may finish its existing turn, so matching is not a prompt-specific completion guarantee. `--until` and `--timeout` require `--wait`; Windows defaults to no total timeout; use an explicit timeout for bounded automation. Use `agent start alpha --kind codex --panel <id>` to launch in an idle Windows terminal and wait for verified readiness. The default timeout is 30000 ms (3001–300000 allowed). Without `--panel`, validated caller context is required. Arguments after `--` are literal; repeated `--arg=<value>` is an alternative. Inspect pending names with `agent names`. A timeout or cancellation does not kill the process; retain submission and reservation facts and never blindly replay.

On Windows, `agent doctor` inspects local setup offline. `agent integration list/explain/enable/disable --provider <id>` manages Claude Code, Codex, Grok, GitHub Copilot, OpenCode, Kilo, Pi, OMP, Gemini and Qwen Code (`claude-code`, `codex`, `grok`, `github-copilot`, `opencode`, `kilo`, `pi`, `omp`, `gemini`, `qwen-code`); inspect its receipt and then list the observed state. Other providers remain manual-only. Windows `site diagnose` reports read-only runtime, lock and pipe-name facts using `orchbee.site.diagnose.windows` v2. Other unsupported operations return a typed failure; do not replace a failure with guessed state.

Destructive close/delete commands require `--confirm`. A closed Workspace does not delete its Git checkout or branch. Use Workspace close-status/close-resume when the close result requires recovery. Exiting a TUI client does not stop the Site.

## Consume results

Use `--format json` where declared. Standard output carries one machine document; localized diagnostics use standard error. Inspect both the document and process exit code. Exit 2 is invalid usage, 4 unavailable, 7 timeout, 8 unknown outcome, and 130 cancellation. Do not parse human prose or assume a failed/cancelled wait cancelled the observed operation. Keep stable IDs and request identities needed for recovery.

## Start in the intended directory and environment

`tab create` and `panel split` accept `--cwd <absolute-path>` and repeatable `--env NAME=VALUE`,
with the default shell, an explicit program, or `--profile`. Values are literal; an empty value is
allowed. On Windows, omitted cwd follows the resolved source Panel root process directory; an
absolute path overrides it. `--cwd home|current|workspace` selects CLI home/current or Server
Workspace/Worker-home defaults without a terminal read. An absent source retains those defaults.
The directory must already exist. These options do not modify the stored profile.
At most 32 environment entries and 1,024 UTF-8 name/value bytes are allowed; names are ASCII
identifiers, unique ignoring case. Orchbee and terminal identity variables are reserved.
Inspect the terminal outcome in the result: committed membership alone does not prove a process
started. The native CLI help owns the complete validation rules.

## Locate the calling Panel

Windows `panel current` and `panel process-info` share the
[current Panel contract](native-current-panel.md). Inside a launched terminal they validate
`ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID`; outside, with both variables absent, they
select the unique current TUI Client. Use `--client <id>` for multiple Clients, or
`--panel <id-or-selector>` for a headless explicit lookup. Those two selectors conflict and
both ignore caller environment. Partial, malformed, or stale caller metadata never falls back.
`--current` aliases the default lookup and may combine with `--client`. No focus changes.
`orchbee.panel.current` is v4 on Windows: it shares Panel Get Agent, terminal and selected-Client
focus/scroll observations. Changed Client views or canonical snapshots fail without partial
results. Queries have one five-second deadline and support Ctrl+C.

Use explicit `--forever` on `agent wait` or `agent prompt --wait` for no total deadline; it conflicts with `--timeout`. Windows defaults to no total timeout and idle/done/blocked; every read stays bounded. `--until blocked` matches fresh needs-attention; `--until done` matches fresh completed outcomes, without an unseen marker. Windows wait uses version 3 with nullable matchedChange for historical evidence; Windows prompt uses version 3 with separate activity-gate history. Both use null timeoutMs for infinite mode; infinite read-timeout failures also use version 2. Windows Codex prompts add Right between paste and Enter to clear paste-burst Enter suppression.

Targeted Agent commands also accept `--panel <id|@panel:suffix>` or a positional Panel ID. Session IDs and names take precedence over positional Panel fallback. Panels select their primary terminal and require one live Agent; waits pin the selected Session.

## Inspect processes before Agent input

`orchbee panel process-info --panel <id|@panel:suffix> --format json` reads the actual
Windows Job. Omit the target inside an Orchbee terminal to use validated caller context.
The v2 document includes live cwd, precise creation identity, Windows argv projection,
provider recognition and a foreground estimate. Null fields and ambiguous selection are
explicit; this estimate does not prove interactive readiness.

Windows `agent send-keys` and `agent prompt` check the expected running provider and
re-read the same session/presentation generation before sending. A Shell, mismatched or
ambiguous Agent, unsupported provider, or changed binding refuses without input. Prompt
also rechecks fresh known state and pending attention. Inspect process-info and Agent
facts to resolve the refusal; use `terminal send` only when explicitly targeting the
terminal itself. A process may still exit after the preflight; inspect send receipts.

## Windows Site stop

`site stop --confirm --format json` ends the managed Site and its terminal processes. Use `--detach` to return after admission, retain `requestId`, and inspect with `site stop-status <request-id> --format json`. Timeout or Ctrl+C only ends the wait. Repeat with the same `--request-id` to observe the existing attempt; only `--retry --request-id <id> --confirm` retries a failed stop. A matching stopped record is a completion receipt, consumed by a subsequent `site start`; not-recorded does not disprove an earlier stop. Never infer completion from pipe absence. Linux and macOS stop are unsupported.

## Retain Worktree operation identity

On Windows use `worktree create/open/remove` with the flags from command help. Retain the returned operation ID and query `worktree status <id>` after detach, timeout or disconnection. Do not blindly repeat uncertain Git effects. Read both the checkout result and Workspace disposition: a closed Workspace stays closed if later Git removal fails. Resume only an explicitly resumable failure using `worktree resume <id> --generation <attempt> --confirm`. Remove requires `--confirm`; `--force` permits discarding dirty checkout changes. See the [Worktree contract](windows-worktrees.md).

## Query Worktrees from a source

On Windows, `worktree list --cwd C:/code/repo --format json` needs no Project. Use `--workspace <target>` or omit both for validated caller/current-directory selection. Invalid caller context refuses without fallback. `--trust-repository` applies only to this query. `orchbee.worktree.query` v2 retains multiple Workspace associations with required-nullable `projectId`. The registered-directory form uses only `--directory <id>` and returns `orchbee.worktree.list` v2 without Project metadata; it cannot mix with source flags. Neither form activates a Site or mutates Git.

Worktree create/open accepts an ungrouped Workspace source: Server atomically creates a Project for source and new peer or reuses the current group. Opening an associated checkout reuses its Workspace; only explicit `--focus` selects it in an existing TUI. Only Directory sources may select an existing `--project`. Nullable Project metadata in `orchbee.worktree.operation` v5 is the original request, not current grouping; query and focus by stable Workspace ID.

Windows Worktree `--cwd` and create/open `--path` also accept relative paths and a leading `~`. After complete grammar and machine routing, the final executing CLI resolves these against its own current directory or home; sender and relay preserve the raw operands. Home precedence is `USERPROFILE`, `HOMEDRIVE`+`HOMEPATH`, then `HOME`; malformed selected values fail before configuration or runtime access. Paths remain literal apart from these rules, without shell or variable expansion.

## Close one Windows Project

`workspace close <target> --confirm` closes only the selected Workspace and preserves checkout and branch. Batch closure uses `project close-all <project> --confirm`, may span repositories, accepts empty groups and retains the Project. Keep the `orchbee.project.close-all` operation ID and read `project close-status --request <operation-id>`; use `project close-resume --request <operation-id> --generation <attempt> --confirm` only for a declared resumable failure. Inspect each captured member's Close identity and observation: null `close` means unsubmitted, null child `observation` means accepted but unobserved, never permission to replay. Detach, timeout and Ctrl+C stop waiting. Ordinary close status/resume needs stable `--workspace` only (plus request/generation/confirm for resume), without Project. Use `project dissolve` to ungroup while keeping Workspaces.

## Control one Windows TUI

Use `client list --format json` and retain the exact current `clientId`. With multiple views, pass `--client <id>`; never choose the first. `workspace focus <target>`, `tab focus <target>` and `panel focus [<target>]` navigate that TUI. `panel zoom [<target>] --on|--off|--toggle` changes its Tab zoom; default is toggle, resolved once by the TUI after target activation. No OS window is activated.

Retain `operationId` from `orchbee.client.control` and use `client control-status <operation-id>` after detach, timeout, Ctrl+C or unknown submission. `queued` and `delivered` are pending; only `applied`, `zoom-toggled` or the explicit single-panel `zoom-unchanged` acknowledge drawing. Incomplete may retain partial navigation. Reads never reapply a command; do not repeat an uncertain toggle. Receipts are volatile and completed ones expire after 60 seconds. An absent receipt is not proof of no effect.

## Export schemas offline

On Windows, `orchbee api schema [<schema-id>] --output <path>` writes the same JSON to a literal file path and stdout. The parent directory must exist. Existing regular files are replaced only after complete preparation; unknown IDs write nothing. Retain a reported temporary path if cleanup fails. A stdout failure after publication does not undo the file. Use `--format json` for typed export failures.

## Inspect a Windows TUI layout

Use `panel layout [<panel>] --client <id> --format json` to inspect cells without selecting the target Tab or resizing a terminal. `panel neighbor --direction left|right|up|down` returns a nullable neighbor ID; `panel edges` returns physical boundary booleans. Both accept the same Panel/Client selectors. Read `projection.sourceAvailable`: a hidden source has no neighbor/edge claim and exits 3. Pending or failed observations have null `projection`. Inactive geometry is explicitly unselected, and absent local focus stays null. All three preserve the full layout and operation identity; inspect retained results with `client control-status` after detach or interrupted waiting.

`panel focus [source] --direction left|right|up|down` finds one neighbor in the selected TUI's unzoomed cell layout and preserves zoom. No neighbor returns `direction-unchanged` without activating the source Tab. A found destination is pinned and returns `direction-focused` only after a successful draw. Receipt reads never repeat the movement.

`panel resize [panel] --direction left|right|up|down [--amount <number>]` adjusts one eligible split border, preserving focus and zoom. `layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` sets an exact binary split path. Both route to one Windows TUI and retain a `layout-adjusted` receipt. Selected changes wait for a draw; inactive changes store without navigation. Finite ratios clamp to 0.1–0.9, and finite resize amounts normalize to an absolute magnitude at most 0.5 (default 0.05). Ratios belong to that TUI session and are not restored after restart. See the [layout contract](windows-client-layout.md).

`panel swap [source] --with <target>` exchanges two Panel positions in one Tab; `--direction left|right|up|down` selects a neighbor instead. It then focuses the source in one selected Windows Client. Use exact IDs or `@panel:<suffix>` and `--client` when routing is ambiguous. Same Panel, cross-Tab and no neighbor leave positions and focus unchanged. The `orchbee.panel.swap` receipt separates canonical commit from focus acknowledgment: a nonzero exit may retain a committed exchange. Inspect the receipt and layout before acting; never blindly repeat the swap. Use the focus operation ID with `client control-status` after interrupted waiting. See the [swap contract](native-panel-swap.md).

Windows `tab create` and `panel split` resolve `--cwd follow|home|current|workspace|<absolute-path>` before creation; follow is the default and samples the resolved source root process. Their v3 receipt records the chosen directory source/path. They accept optional targets and labels, with no focus by default; `--focus` requests post-creation focus, and split accepts a Client-local `--ratio`. Explicit targets can create without a TUI. `orchbee.creation.result` v3 separates committed creation from later ratio/focus results; a later failure is not permission to repeat creation. [Native terminal creation](native-terminal-creation.md) owns target selection, total budget, concurrency guards and recovery for these Windows commands.

Windows `panel input (<panel>|--panel <panel>|--current) --right-click orchbee|panel` sets the shared right-button policy without moving focus. `--current` requires validated caller metadata; explicit targets bypass it. `panel split --right-click` commits the new Panel policy with creation. The default `orchbee` reserves right-click to the Client; `panel` permits unmodified right clicks in mouse-reporting terminal content. An admitted gesture retains its matching release after policy changes. See the [Panel input contract](native-panel-input.md) for routing, pending-input rechecks and failure recovery. This does not add a context menu. `orchbee.panel.input` v1 reports `configured` or `unchanged`; `panel list` v2 and `panel get/current` v3 expose `rightClick`. `tab get` v2 and `api snapshot` v6 admit the optional canonical field. Catalog v51 has 72 schemas and the native inventory has 86 command paths.

Windows `tab move <target> --insert-index N` moves a complete Tab to a zero-based boundary in the
original order: 0 is first, the Tab count is last, source/source+1 are unchanged, and out-of-range
values are refused. It excludes `--direction` and preserves focus and terminals. Success uses
`orchbee.tab.move` v1; runtime failures use `orchbee.cli.failure.tab-move` v1. Catalog v51/72 keeps
the legacy direction envelope unchanged.

## Automatic Tab naming

The [Tab label contract](native-tab-labels.md) owns optional `automaticLabel` metadata. New Tabs without a supplied label use their current context position; explicit numeric names and legacy Tabs remain fixed. CLI queries, label selectors and TUI display agree after move or close, and rename chooses a fixed name. Native `api snapshot` v6 exposes canonical metadata; catalog v51 contains 72 schemas. Shared protocol consumers require a matching cohort.

## Source-selected terminal output

Windows `terminal read` and `agent read` accept `--source visible|recent|recent-unwrapped|detection`.
Recent combines retained history with the live screen; detection reads the recent screen-height
range. `--plain` exports text directly for pipes, defaults to recent, and excludes JSON. Source-mode
`--lines` allows zero, clamps to 1000, and defaults to 80 for recent. It excludes the legacy
history/rows/unwrap flags, whose behavior stays unchanged. The [output contract](native-terminal-output.md)
owns selection, exact sample validation and `orchbee.terminal.output` v1; catalog v51 has 72 schemas.

Windows `terminal wait-output` accepts the same `--source` and `--lines` selection, or `--forever`
for an explicit wait without a total ceiling. Any of these chooses source mode (default recent),
excludes `--rows`, and emits `orchbee.terminal.wait-output` v3. Without `--forever`, the default
remains 30000 ms; `--forever` conflicts with `--timeout`, retains five-second I/O bounds and supports
Ctrl+C. Recent waits join soft wraps; visible/detection match physical lines. Old invocations retain
v1/v2 output. A match can come from existing output and never proves command completion.

Windows `terminal read` and `agent read --ansi` export styled snapshots, defaulting to recent output.
Use `--format json` for `orchbee.terminal.output` v2 with `result.encoding: "ansi"`; text/plain reads
retain v1. ANSI excludes `--plain` and legacy history/rows/unwrap flags. It preserves styled blanks
and Unicode, uses only generated SGR, and joins verified soft wraps under `recent-unwrapped`.
Detection remains plain text. It does not reproduce original PTY bytes, hyperlinks or graphics.
The render request and prepared output each keep their existing 8 MiB bounds; oversized render
resources fail without a fallback or retry. See the [output contract](native-terminal-output.md).

Windows `status server` reads actual Server build/protocol/cohort metadata before Client Session admission; `status client` checks this executable offline. Both accept `--format text|json` and never activate a Site. Bare `status` remains a Workbench query. The [runtime status contract](native-runtime-status.md) owns the five-second budget, unknown metadata, compatibility, schemas and failures. Update-channel management remains unavailable.

## Agent navigation

Windows `agent focus <target> [--client <id>]` resolves a name, session or Panel to its unique live primary Panel before one Client control submission. Explicit `--session`, `--name` and `--panel` selectors use the existing resolver; `--request-id`, `--detach` and the 30000 ms total default timeout retain the common navigation recovery contract. An unready Client fails with `client-not-ready` before Agent reads. The [Agent navigation contract](native-agent-navigation.md) defines primary binding, ambiguity, cancellation and retained operation status. After successful visible paint, the Client attention owner can acknowledge the observed Agent version; known host focus loss prevents acknowledgement. `agent attention <target> [--client <id>]` reads that Client independently without marking it seen.

On Windows, use `agent changes --format json` to establish a current-head cursor, then `agent changes --cursor <cursor> --limit 100 --format json` to read retained transitions. Drain pages while hasMore is true. This is a one-page read, not a subscription. Reset-required or cursor-invalid means continuity is lost: rebuild the session baseline explicitly. Never interpret a new checkpoint as delivery of missed events. Catalog v51 includes 72 schemas and 86 command paths.

Use `agent get <target> --client <id>` on Windows to inspect canonical session facts alongside that exact Client's focused/seen observation and independent state projection. A null seen value means the Client observation is missing or stale; never treat it as either seen or unseen. The read does not acknowledge anything.

Use `agent wait <target> --client <id> --until done` for an explicit completed outcome, or `--until idle` for actual idle state. Read acknowledgement does not affect either condition. Loss of the selected Client fails; missing/stale acknowledgement remains null in the separate Client projection. The v8 result retains canonical state and historical evidence independently from that projection.

For one submission followed by a specific TUI observation, use `agent prompt <target> <text> --wait --client <id> --until done`. The Client is checked before input; after Agent activity, done requires explicit outcome evidence and idle requires actual idle state, independently of seen. Client loss preserves an accepted input receipt; do not resubmit solely because observation failed. The v8 result includes the Client projection.

Windows `agent get <target>` includes terminal and runtime process information in v9; adding `--client <id>` returns v10 with that Client's projection too. Read `runtime.processInfo` for actual process facts and `runtime.promptReady` for the current submission preflight. An unavailable or changed binding never supplies an invented process. This preflight does not certify interactive UI readiness or reserve the terminal against races. Read `terminal` for raw/stripped title, cwd and frame/process provenance. Null means the Agent binding cannot safely identify one live terminal; process-unavailable retains available frame metadata. The [query contract](native-cli-query.md) owns absence and consistency.

Windows `agent list --client <id>` joins the selected TUI status in the v2 list envelope. It batches one bounded page, rereads its original cursor and checks retained changes before exposing seen/unseen. Empty pages still validate the exact Client. No-flag list retains v1. The [Client attention contract](native-client-attention.md) owns consistency and deadlines.

Windows Agent Get now emits v9 (v10 with `--client`), adding a separate `management` observation with source, revision and Server-owned startup state. Active records come only from completed startup; manually named sessions remain unmanaged. Pending startup makes promptReady false and `agent prompt` refuses before input. The bounded Status read shares the original command deadline and can fail with Agent read failure v3 (`operation: management`); it never retries or completes a launch. `agent start` v2 explicitly reports interactiveReady after its existing successful completion. The [startup contract](windows-agent-start.md) owns lifecycle and limits.

Windows `agent integration list --outdated-only` selects owned entries requiring update or repair and reports unresolved and blocked provider IDs separately. Its v2 result retains the full inspection completeness before filtering. Use `agent integration enable --provider <id>` for the explicit guarded repair; an empty filtered list alone does not prove that every provider was inspected successfully.

Windows provider selection additionally admits `cursor`, `qodercli` and `antigravity-cli`; onboarding preserves unrelated JSON hooks and reports only evidenced session identity.

The `mastracode` provider supports managed Windows lifecycle hooks and offline diagnostics. Use enable to repair missing owned event entries.

The `droid` provider supports Windows session identity onboarding and offline source-priority diagnostics.

The `kimi` provider supports Windows TOML session onboarding and offline diagnostics; legacy `.kimi` is untouched.

The `letta` provider supports Windows session onboarding with silent hooks and provider-owned HOME resolution.

The `devin` provider supports Windows session onboarding and diagnosis through its APPDATA configuration.

The `hermes` provider manages a Windows session plugin in the selected Hermes profile. Activation uses the provider CLI; offline doctor does not claim activation.

## Windows terminal observation stream

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` emits NDJSON from one existing binding without control or Host resizing. The [stream contract](windows-terminal-streams.md) owns full safe ANSI frames, resets, flush-before-ACK, deadlines and cleanup. Default viewport: 120×40; default duration: unlimited. Both output formats emit `orchbee.terminal.stream` v2 records. Catalog v68 has 76 schemas and 90 native command paths. Machine control now accepts NDJSON input through `terminal session control`; interactive `terminal attach <terminal>` or `agent attach --name <name>` requires console stdin/stdout and uses the actual console size. Both accept `--takeover`, `--mouse on|off` and an optional total `--timeout`. `Ctrl+B`, then `q`, detaches; a double prefix sends one literal prefix. Paste cannot trigger detach. Output stays interactive with either global format. No Site start, Workbench focus change, reconnect or input replay occurs.

## Report Workspace display data

`orchbee workspace report-metadata <workspace> --source hook:build --token summary=testing --seq 42 --ttl-ms 60000 --format json` reports transient values. Use `--clear-token summary` to remove a key. Keys share one Workspace map across sources; sequence freshness is per source. Stale/equal sequence is a successful no-op and does not refresh TTL. Reads through `workspace list/get` return `tokens` and `metadataSource`. Site restart clears reported data. An unknown result (exit 8) may have applied: keep its request identity, inspect current values and never blindly replay to extend TTL. Bounds and normalization are in the [Workspace metadata contract](windows-workspace-metadata.md).

Windows `panel report-metadata <panel> --source <id>` reports display fields and token patches with optional sequence/TTL. Use `--title`, `--display-agent`, repeated `--state-label STATE=TEXT` or explicit clear flags. A report follows the exact primary terminal binding and does not confer Agent authority or change OSC title. Keep unknown exit-8 receipts; never retry them automatically. See [Panel metadata](windows-panel-metadata.md) for guards and replacement semantics. Panel list/get/current and Agent Get now expose source-attributed `metadata`; TUI consumption and actual Agent authority guard context remain required.

On Windows, use `config import-herdr --input-file SOURCE --output-file DEST --dry-run --format json` to inspect an explicitly selected Herdr conversion. Only this preview includes candidate values; handle them as source data. Omit `--dry-run` to create a new file; never retry after creation or a lost receipt. Existing targets refuse. Inspect a retained recovery file before deciding its disposition. Import starts no Site, discovers no config, and rejects `--site`/`--machine`. Select the new configuration explicitly for a new Client, keeping the old file for rollback.
