# Native CLI Workflows

## Windows: create a usable Workspace first

```powershell
orchbee workspace create
orchbee workspace create --cwd C:/code/demo --label main --format json
orchbee workspace create-project main --label demo
orchbee workspace set-project <workspace> --project demo
orchbee workspace ungroup <workspace>
orchbee project dissolve demo
```

The first two commands each launch a Workspace and default shell in one request, with no required Project/label or automatic AI program. Placement defaults to ungrouped; only explicit `--project` selects an existing Project. Read `workspace.result` in `orchbee.workspace.creation` v2, including required-nullable `projectId`. Existing exec/profile/env/geometry and optional focus controls remain available. The last four commands change organization metadata while preserving stable IDs, directories, PTYs and global order. Projects may organize multiple repositories; ordinary and worktree Workspaces are peers.

```powershell
orchbee workspace close <workspace> --confirm
orchbee project close-all <project> --confirm
orchbee project close-status --request <operation-id>
orchbee project close-resume --request <operation-id> --generation <attempt> --confirm
```

Ordinary close affects only the selected Workspace and preserves checkout and branch. Project close-all closes captured members and retains the Project. Dissolve only ungroups, retaining Workspaces. Never replay unknown mutations automatically. The Linux examples below retain their separately declared creation flow; current Windows behavior follows the [creation contract](windows-workspace-creation.md).

Windows directory Workspace quick start and the existing Linux headless `orchbee`
CLI sequence: start the local Site, build a Project/Workspace/directory, launch and
drive a terminal, inspect worktrees, observe Agents, and close cleanly. The
command inventory lives in the [README](../README.md); the full grammar,
target rules, and failure matrix are owned by
[native CLI query](native-cli-query.md). This guide covers the supported
end-to-end path only — option and target wording matches what
`orchbee help` prints, and shell snippets are illustrative.

## Windows directory Workspace

Use a complete Windows development cohort from the [Windows quick start](../README.md#native-windows-quick-start). Replace the executable and existing directory paths below. PowerShell 5.1 and 7 can read JSON without jq.

```powershell
$cli = 'C:\path\to\cohort\orchbee.exe'
function Invoke-OrchbeeJson([string[]]$Words) {
    $output = & $cli @Words --format json
    if ($LASTEXITCODE -ne 0) { throw ($output -join "`n") }
    ($output -join "`n") | ConvertFrom-Json
}
Invoke-OrchbeeJson -Words @('site', 'start')
$project = Invoke-OrchbeeJson -Words @('project', 'create', '--label', 'demo')
$created = Invoke-OrchbeeJson -Words @('workspace', 'create', '--project', $project.result.projectId,
    '--label', 'main', '--cwd', 'C:\existing\repo', '--env', 'DEMO=hello')
$created.workspace.result
& $cli workspace get $created.workspace.result.workspaceId --format json
& $cli tui
# Close this Workspace when finished:
& $cli workspace close $created.workspace.result.workspaceId --confirm --format json
# Or stop the whole Windows Site and all its terminals:
$stopped = Invoke-OrchbeeJson -Words @('site', 'stop', '--confirm')
& $cli site stop-status $stopped.result.requestId --format json
```

Check `workspace.result.terminal.outcome` before using the terminal: `started` confirms launch, while pending, failed, unavailable or unknown results require inspection of the returned resources. Closing the TUI does not close the Workspace. The command creates the first terminal automatically and leaves client focus unchanged. Windows Codex onboarding uses `agent integration enable --provider codex`; `site diagnose` reports local Windows runtime facts without activating the Site. The numbered POSIX examples below describe the existing Linux path.

## Windows Agent diagnostics

```powershell
& $cli agent doctor --provider codex --format json
& $cli agent doctor --provider codex --asset-root 'C:\path\to\cohort\agent-observation-adapters'
```

These commands are offline and leave configuration unchanged. They inspect the native user
profile (`CODEX_HOME` when explicitly configured), provider entries on PATH, Node and declared
capsule integrity. The second command adds an explicit capsule root; it does not install it.
Version-3 JSON and localized text report `windows-endpoint-not-inspected`. Capsule 0.4.16
adds Windows Codex hooks, and Windows x64 cohorts include their required pinned ConPTY
runtime. Doctor does not exercise that transport. An installed hook or `complete:true`
does not establish Agent-board readiness. Managed enable/disable and `agent start`/`prompt` are available; see [Windows onboarding](windows-agent-integrations.md) and [startup](windows-agent-start.md).

## Prerequisites

- An unpackaged **development cohort** on Linux, built from the repository
  root — see [macOS and Linux local startup contract](../README.md#macos-and-linux-local-startup):

  ```sh
  node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
  ```

  Then run `/tmp/orchbee-native-dev/orchbee`. The staged cohort is four
  executables (CLI, Server, Worker, Host) that must come from one build plus
  the observation-adapters payload directory, all pinned by the manifest;
  these prerequisites apply to the Linux examples below; the Windows flow is above.
  This is not an installed product or a system service.

- `jq` on `PATH` if you follow the id-extraction snippets below; otherwise
  read the same fields from `--format json` output manually.
- Role split: `config check`, `site diagnose`, `api schema`,
  `agent doctor`, and the whole `agent integration` family are purely
  local (no connection, lock, or Site effect); `help`, `completion`, and
  `--version` print fixed text without touching runtime state; `site
start` performs the one effectful activation; every other command —
  and the bare/`tui` entry's connect-or-activate — talks to the local
  Site.
- Agent rows appear only when the running Site observes sessions from a
  supported Agent integration that is already enabled. Provider onboarding
  is the `agent integration` family in section 6 — local, bounded, and
  never Site-touching — and there is no `agent launch`; an enabled
  provider's installed executable is launched like any other program —
  through the generic `--exec` route in section 3.

## 1. Check local inputs and start the Site

```sh
orchbee config check   # local inputs only; exits 1 on any failed check
orchbee site start     # one 20-second budget; "started" or "already-running"
orchbee status         # the running Site's Workbench snapshot
```

`site start` runs the same connect-or-activate sequence the TUI uses. An
interrupted or timed-out start can leave committed activation state behind;
a later invocation then answers `rejected` with a recovery-required reason.
Linux has no Site stop, repair or deletion command. Windows supports `site stop --confirm` and `site stop-status <id>`; a completed stop allows a later `site start`. Stop ends all terminal processes and discards the Site's in-memory state. A timeout or cancellation ends the CLI wait only: retain the request ID and query status. Use `--retry --request-id <id>` only to explicitly retry a failed stop.

## 2. Create a Project, Workspace, and directory

Each command prints a receipt; verify `outcome` is `committed` before
continuing — a refused or failed receipt means stop and read the failure
envelope instead of chaining ids. `jq -e` below exits nonzero on a missing
field, so `set -e` or per-step checking halts the sequence.

```sh
PROJECT=$(orchbee project create --label demo --format json | jq -er .result.projectId)
WS=$(orchbee workspace create --project "$PROJECT" --label main --format json | jq -er .result.workspaceId)
# Replace with an existing absolute project directory on this machine:
DIR=$(orchbee workspace accept-directory "$WS" --path /absolute/path/to/existing-repo --format json | jq -er .result.directoryId)
```

A target is an exact 32-hex id or an exact case-sensitive label; ambiguous
labels are refused with candidates. `--path` is a literal absolute local-platform
path — the CLI never expands `~`/`$`, creates, or probes it; the Server
probes authoritatively and rejects a Workspace that already has a
directory. `accept-directory` returns `result.directoryId` (plus
`workerId`): a Workbench identity, not the path.

## 3. Launch a terminal

```sh
TAB=$(orchbee tab create --workspace "$WS" --label shell \
  --format json | jq -er .result.tabId)
```

The four geometry flags are an all-or-none group: with all four absent the
command requests an 80x24 logical grid with unknown cell pixels (0,0) — a
request policy, not a measured display fact; supplying any of
`--cols`/`--rows`/`--cell-width`/`--cell-height` requires all four, e.g.
`--cols 80 --rows 24 --cell-width 8 --cell-height 16` to override the
default. Add `--exec /abs/path [--arg v]...`
to replace the default shell for that one request — repeat `--arg` once
per verbatim argument, with no shell splitting; this is also how an
installed provider executable is launched, with whatever arguments that
program already accepts. Use `--standalone` instead of `--workspace` for
the standalone context, or `panel split <panel-id> --direction right ...`
for a sibling panel; both accept the same `--exec`/`--arg` selection.

## 4. Read, wait, and send input

Find the terminal bound under the new tab — `terminal list` rows carry
`boundTo.tabId`, `boundTo.panelId`, and `boundTo.presentationId`:

```sh
TERM_ID=$(orchbee terminal list --format json | jq -er --arg tab "$TAB" \
  '.result.terminals[] | select(.boundTo.tabId == $tab) | .terminalId')
```

A deterministic send-then-wait pair — the `wait-output` text is the one
the previous `send` provably produced:

```sh
orchbee terminal send "$TERM_ID" --text 'echo READY-MARKER' --enter
orchbee terminal wait-output "$TERM_ID" --match 'READY-MARKER' --timeout 30000
```

Independent reads and the remaining send forms:

```sh
orchbee terminal read "$TERM_ID"                          # visible frame
orchbee terminal read "$TERM_ID" --history --rows 50      # retained scrollback
orchbee terminal read "$TERM_ID" --unwrap --lines 20      # last 20 logical lines
orchbee terminal wait-output "$TERM_ID" --regex 'build (ok|FAILED)'  # alternate search
orchbee terminal send "$TERM_ID" --key ctrl-c
printf 'summarize this\n' | orchbee terminal send "$TERM_ID" --stdin --enter
orchbee terminal send "$TERM_ID" --file prompt.txt --enter
```

`send` submits one input sequence from exactly one source family: literal
`--text`, repeatable `--key`, a regular `--file`, or `--stdin`. Stdin accepts
redirected regular files and pipes, including Windows pipes. Text is limited
to 4096 UTF-8 bytes; file/stdin preserves raw bytes up to 1 MiB. These limits
include a folded `--enter` carriage return. File/stdin with `--paste` accepts
256 KiB of content and an optional separate Enter record. Keys accept named
keys and modifier combinations, with at most 64 records per sequence.
One absolute five-second deadline covers source acquisition and delivery.
`wait-output` exits `0` on match, `7` on timeout, `130` on cancellation.
An unknown send outcome (exit `8`) may already have delivered input; inspect
the terminal before another action. Partial input and cleanup outcomes are
reported independently. Use `orchbee --skill` for the embedded Agent guide.

## 5. Inspect worktrees

```sh
orchbee worktree list --directory "$DIR"
```

`--directory` takes the strict nonzero 32-lowercase-hex Workbench directory
id from `accept-directory` or `workspace get` — never a path or label. The
listing is read-only: it never creates, moves, locks, or prunes a worktree.
Branch names keep their full `refs/heads/…` form; an all-zero `head` is an
unborn repository and `null` marks the bare entry.

## 6. Onboard a provider and observe Agents

`agent integration` manages the local onboarding of a supported provider —
inspect its state, explain the enable plan, and enable or disable the
managed integration. These commands run on the invoking user's machine and
PATH only: they never contact a Site, and every mutation stays bounded and
non-interactive.

```sh
orchbee agent integration list                     # every provider's local state
orchbee agent integration explain --provider codex # the plan enable would run
orchbee agent integration enable --provider codex
orchbee agent integration disable --provider codex
```

Only entries verifiably owned by Orchbee are installed or removed; a
foreign or newer-versioned entry is refused before any effect.

With a supported integration enabled and the Site observing sessions —
nothing below performs provider lifecycle or control:

```sh
orchbee agent list                          # live sessions only
orchbee agent list --include-ended --limit 20
orchbee agent get <session-id>
orchbee agent explain <session-id>          # how status/freshness were derived
orchbee agent wait <session-id>             # poll until a condition matches
orchbee agent wait <session-id> --until ended --until failed
```

A session id is the opaque identity printed by `agent list`, never a label;
a page's `nextCursor` token continues with `--cursor`. These reads submit
no Workbench mutation and no provider command — nothing is created,
activated, or mutated — and an ended session stays queryable only while
the Site retains it. A terminal binding that has closed is not navigable.
`agent wait` polls one session on a held Client Session until an `--until`
condition matches or `--timeout` lands (default 600000 ms): status
conditions need `fresh` freshness, `fresh`/`stale`/`disconnected` match the
observation directly, and `--after-revision` fences against the first
returned global revision. It samples state at each poll — a transition
that begins and ends between two completed observations is missed.

## 7. Close cleanly

```sh
orchbee tab close "$TAB" --confirm          # accepted; the Site finishes async
orchbee panel close <panel-id> --confirm
orchbee workspace close "$WS" --confirm     # closes the whole context's terminals
```

`--confirm` is required. An `accepted` receipt carries `closeRequestId`
while the Site completes the close asynchronously. `workspace close` reuses
the same lifecycle at the Workspace's context scope, removes the Workspace
and its terminal context after cleanup, and preserves its checkout,
branch, and directories. The retained operation stays answerable by strict stable
IDs even after the Workspace leaves the active projection:

```sh
orchbee workspace close-status --workspace <workspace-id>
orchbee workspace close-resume --workspace <workspace-id> \
  --request <close-request-id> --generation <n> --confirm
```

`close-status` reports the scope lifecycle plus every captured member's
status, ending in a `closed` tombstone revision; `close-resume` opens one
fresh attempt window only on a resumable retained operation — a stale
`--generation` answers `rejected`/`stale-generation`, and unknown outcomes
are non-resumable. Closing tabs does not
stop the Site, and nothing in this guide deletes runtime state.

## JSON and automation notes

- `--format json` selects the machine envelope on the commands that accept
  it — the families above, `status`, `site start`, and `config check`;
  `help`, `completion`, and `--version` do not take it. A failure envelope
  likewise needs an explicit `--format json` for its stdout document.
- u64 fields serialize as decimal strings (for example `"revision": "9"`).
- For every target rule, source bound, deadline, and exit class, the
  authoritative reference is [native CLI query](native-cli-query.md) and
  `orchbee help <family> <command>`.

For native Windows Cursor, use `agent integration enable --provider cursor`, then `agent doctor --provider cursor`. This configures a session identity hook; working/idle recognition is a separate capability.

## Edit the focused terminal's scrollback

`orchbee terminal edit-scrollback <terminal> [--client <id>] [--timeout <ms>] [--format json|text]` asks one current Windows TUI to open its focused bound primary or child terminal's complete retained plain UTF-8 scrollback in the selected editor. The terminal target uses the normal full ID, typed suffix selector or exact-label resolver. The existing Site and a Ready TUI are required; the CLI does not start either or switch focus to make a target eligible. Explicit `--client` selects exactly that Client; otherwise the existing caller-context/unique-current-Client rule applies. A target that differs from the selected native member fails before editor submission.

One invocation deadline covers snapshot, exact target/Client selection, passive current-Presentation read, unchanged Client-view check, one editor control submission and retained status reads. `--timeout` defaults to 30000 ms and accepts 1-3600000 ms. No stdin, executable, cwd, environment, dimensions or controller flags are accepted. The CLI never attaches, controls or resizes the source terminal: the TUI owns its separate read-only capture connection. It sends no source keystrokes or focus acknowledgement.

The Worker selects its inherited nonblank VISUAL, then EDITOR, otherwise the Windows system-directory notepad.exe. Changing a CLI process's environment does not change a running Worker's preference. Arguments remain literal and the Host appends the exclusive temporary document path. The capture uses the active primary/alternate screen, unwraps soft lines and refuses the whole result above 32 MiB. The configured editor may be a console editor in the popup or a GUI editor with its own window.

Known editor operation answers are `orchbee.client.control` v10 and preserve the exact operation/source/member, observed view and wait status. Text uses the localized popup/control result. `popup-opened` requires an installed editor frame; retained queued/delivered, refusal, timeout, cancellation and unknown results never claim that the editor is ready. Unknown submission is never automatically replayed; retain its operation ID for `client control-status`. Ctrl+C stops waiting with exit130 and does not undo a submitted request. Close through the existing popup close control/button; return to normal source input requires known settlement or the modeled connection cleanup path. Physical editor/GUI acceptance remains part of final Windows validation.
