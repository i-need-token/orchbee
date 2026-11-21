# Native CLI Workflows

## Windows: create a usable Workspace first

```powershell
orchbee workspace create
orchbee workspace create --cwd C:/code/demo --label main --format json
orchbee project create --label demo
orchbee workspace set-project <workspace> --project demo
orchbee project dissolve demo
```

The first command starts the background Session when it is not running and says so on standard error. The two Workspace creation commands each launch a Workspace and default shell in one request, with no required Project/label or automatic AI program. Placement defaults to ungrouped; only explicit `--project` selects an existing Project. Read `workspace.result` in `orchbee.workspace.creation` v1, including required-nullable `projectId`. Existing exec/profile/env/geometry and optional focus controls remain available. The last four commands change organization metadata while preserving stable IDs, directories, PTYs and global order. Projects may organize multiple repositories; ordinary and worktree Workspaces are peers.

```powershell
orchbee workspace close <workspace> --confirm
orchbee project close-all <project> --confirm
orchbee operation status <operation-id>
orchbee operation resume <operation-id> --generation <attempt> --confirm
```

Ordinary close affects only the selected Workspace and preserves checkout and branch. Project close-all closes captured members and retains the Project. Dissolve only ungroups, retaining Workspaces. Never replay unknown mutations automatically. The Linux and macOS examples below retain their separately declared creation flow; current Windows behavior follows the [creation contract](windows-workspace-creation.md).

Windows directory Workspace quick start and the existing Linux and macOS headless `orchbee`
CLI sequence: start the local Session, build a Project/Workspace/directory, launch and
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
$project = Invoke-OrchbeeJson -Words @('project', 'create', '--label', 'demo')
$created = Invoke-OrchbeeJson -Words @('workspace', 'create', '--project', $project.result.projectId,
    '--label', 'main', '--cwd', 'C:\existing\repo', '--env', 'DEMO=hello')
$created.result.workspace.result
& $cli workspace get $created.result.workspace.result.workspaceId --format json
& $cli tui
# Close this Workspace when finished:
& $cli workspace close $created.result.workspace.result.workspaceId --confirm --format json
# Or stop the whole Windows Session and all its terminals (development and test cohorts):
$stopped = Invoke-OrchbeeJson -Words @('server', 'stop', '--confirm')
& $cli operation status $stopped.result.requestId --format json
```

Check `result.workspace.result.terminal.outcome` before using the terminal: `started` confirms launch, while pending, failed, unavailable or unknown results require inspection of the returned resources. Closing the TUI does not close the Workspace. The command creates the first terminal automatically and leaves client focus unchanged. `doctor` checks the installation, settings, saved data, runtime folder and Session without activating the Session. The numbered POSIX examples below describe the Linux and macOS path.

## Windows checks

```powershell
& $cli doctor
& $cli doctor --format json
```

`doctor` is local and leaves everything unchanged: it prints a verdict, one line per check and a
command to run for a warning or failure, and exits 1 only when a check failed. `agent start` and
`agent prompt` remain available; see [runtime status and doctor](native-runtime-status.md#doctor)
and [startup](windows-agent-start.md).

## Prerequisites

- An unpackaged **development cohort** on Linux or Apple Silicon macOS, built from the repository
  root — see [macOS and Linux local startup contract](../README.md#macos-and-linux-local-startup):

  ```sh
  node scripts/stage-local-runtime.mjs --output /tmp/orchbee-native-dev
  ```

  Then run the Client the script names, `/tmp/orchbee-native-dev/ob-dev-<16 lowercase hex>`
  for this checkout's development stem. The staged cohort is four
  executables (CLI, Server, Worker, Host) that must come from one build plus
  the observation-adapters payload directory, all pinned by the manifest;
  these prerequisites apply to the Linux and macOS examples below; the Windows flow is above.
  This is not an installed product or a system service.

- `jq` on `PATH` if you follow the id-extraction snippets below; otherwise
  read the same fields from `--format json` output manually.
- Role split: `config check` and `api schema` are purely
  local (no connection, lock, or Session effect), and `doctor` adds only the Session check's handshake `query`;
  `help`, `completion`, and `--version` print fixed text without touching
  runtime state; `server start`,
  and the creating commands `workspace create`, `project create`, `worktree create` and
  `worktree open` declare activation and start the Session when it is not running; every other
  command — and the bare/`tui` entry's connect-or-activate — talks to the local Session.
- Agent rows appear when the running Session observes an Agent, on screen or through hooks an earlier
  version installed. Orchbee no longer installs provider hooks, and there is no `agent launch`; an
  Agent's installed executable is launched like any other program, through the generic `--exec`
  route in section 3.

## 1. Check local inputs and start the Session

```sh
orchbee config check   # local inputs only; exits 1 on any failed check
orchbee server start     # one 20-second budget; "started" or "already-running"
orchbee status         # the running Session's Workbench counts and processes
```

`server start` runs the same connect-or-activate sequence the TUI uses. An
interrupted or timed-out start can leave committed activation state behind;
a later invocation then answers `rejected` with a recovery-required reason.
Linux has no Session stop, repair or deletion command. Windows and macOS support `server stop --confirm`, and `operation status <id>` reads the stop; a completed stop allows a later `server start`. Stop ends all terminal processes and discards the Session's in-memory state. A timeout or cancellation ends the CLI wait only: retain the request ID and query status. Use `--retry --request-id <id>` only to explicitly retry a failed stop. The release build refuses `--confirm` and scripted stops; a person runs `server stop` in a console and types the Session name, as the [release confirmation](../README.md#release-destructive-confirmation) describes.

### Restart the Server (Windows)

`server restart` hands this Session to a new Server and Worker of this build while every terminal
keeps running. It runs only on Windows; other platforms refuse it as unsupported. The declared
workflow, in phase order:

1. Reserve: holding the Session's activation lock until it answers, so that no start runs beside
   the restart, the command creates this build's Server suspended outside the caller's Job, records
   it in the Session record as the restart's successor with a fresh ticket and activation
   identifier, then resumes it.
2. The successor checks the running Server, its connected Clients and its Terminal Hosts; asks the
   running Server to prepare (it holds new requests, records the Session exactly, releases the
   Session store and ends its Worker while every Host keeps its terminals); takes the store and
   every Host; and commits by publishing the Session record that names it and its Worker. The
   running Server then ends, and the successor binds the Session's endpoints.
3. The command succeeds once the successor answers on the Session's Client endpoint.

- Commit point: the successor's Session record publish.
- A running Server whose restart cannot keep terminals answers the `incompatible` capability
  failure before anything starts.
- A refusal (exit 3, or 4 when no Server runs) leaves nothing changed: the command removes its
  successor entry and ends the processes it names.
- A failure (exit 9) names where the restart ended once the command has cleared its entry: `start`
  when the successor could not be started, `check` when it ended in its Check before starting its
  Worker, `takeover` when it ended later and the running Server keeps or took back the Session, and
  `open` with no Server serving the Session, after the commit or because the running Server ended
  too. Every terminal keeps running, and after `open` the next start brings the Session back. The
  phase comes from the successor entry the command clears, the only trace a successor that was
  ended leaves: an entry that names a Worker means the running Server may have held the Session.
- The fixed deadline covers the whole command; a timed-out wait (exit 7) leaves the restart running.
- Ctrl+C stops the wait only (exit 130); the restart continues by itself, and its successor or the
  running Server clears its entry.
- Until Clients reconnect to a new Server by themselves (a later release), a connected Client
  refuses the restart (`clients`).

## 2. Create a Project, Workspace, and directory

Each command prints a receipt; verify `outcome` is `committed` before
continuing — a refused or failed receipt means stop and read the failure
envelope instead of chaining ids. `jq -e` below exits nonzero on a missing
field, so `set -e` or per-step checking halts the sequence.

```sh
PROJECT=$(orchbee project create --label demo --format json | jq -er .result.projectId)
WS=$(orchbee workspace create --project "$PROJECT" --label main --format json | jq -er .result.workspace.result.workspaceId)
# Replace with an existing absolute project directory on this machine:
DIR=$(orchbee workspace set-directory "$WS" --path /absolute/path/to/existing-repo --format json | jq -er .result.directoryId)
```

A target is an exact 32-hex id or an exact case-sensitive label; ambiguous
labels are refused with candidates. `--path` is a literal absolute local-platform
path — the CLI never expands `~`/`$`, creates, or probes it; the Server
probes authoritatively and rejects a Workspace that already has a
directory. `set-directory` returns `result.directoryId` (plus
`workerId`): a Workbench identity, not the path.

## 3. Launch a terminal

```sh
TAB=$(orchbee tab create --workspace "$WS" --label shell \
  --format json | jq -er .result.tabId)
```

Add `--exec /abs/path [--arg v]...`
to replace the default shell for that one request — repeat `--arg` once
per verbatim argument, with no shell splitting; this is also how an
installed provider executable is launched, with whatever arguments that
program already accepts. Use `--workspace` for the exact Workspace, or `panel split <panel-id> --direction right ...` for a sibling panel; both accept the same `--exec`/`--arg` selection.

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
orchbee terminal read "$TERM_ID" --source visible                    # visible frame
orchbee terminal read "$TERM_ID" --source recent --lines 50          # recent physical lines
orchbee terminal read "$TERM_ID" --source recent-unwrapped --lines 20 # last 20 logical lines
orchbee terminal wait-output "$TERM_ID" --regex 'build (ok|FAILED)'  # alternate search
orchbee terminal send "$TERM_ID" --key ctrl+c
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
id from `set-directory` or `workspace get` — never a path or label. The
listing is read-only: it never creates, moves, locks, or prunes a worktree.
Branch names keep their full `refs/heads/…` form; an all-zero `head` is an
unborn repository and `null` marks the bare entry.

## 6. Observe Agents

The commands below observe Agents and send them input; none changes a provider's configuration:

```sh
orchbee agent list                          # live sessions only
orchbee agent list --include-ended --limit 20
orchbee agent get <session-id>
orchbee agent get <session-id> --format json  # adds how status/freshness were derived
orchbee agent wait <session-id>             # poll until a condition matches
orchbee agent wait <session-id> --until ended --until failed
```

A session id is the opaque identity printed by `agent list`, never a label;
a page's `nextCursor` token continues with `--cursor`. These reads submit
no Workbench mutation and no provider command — nothing is created,
activated, or mutated — and an ended session stays queryable only while
the Session retains it. A terminal binding that has closed is not navigable.
`agent wait` polls one session on a held Client Connection until an `--until`
condition matches or `--timeout` lands (default 600000 ms): status
conditions need `fresh` freshness, `fresh`/`stale`/`disconnected` match the
observation directly, and `--after-revision` fences against the first
returned global revision. It samples state at each poll — a transition
that begins and ends between two completed observations is missed.

## 7. Close cleanly

```sh
orchbee tab close "$TAB" --confirm          # accepted; the Session finishes async
orchbee panel close <panel-id> --confirm
orchbee workspace close "$WS" --confirm     # closes the whole context's terminals
```

`--confirm` is required. An `accepted` receipt carries `closeRequestId`
while the Session completes the close asynchronously. `workspace close` reuses
the same lifecycle at the Workspace's context scope, removes the Workspace
and its terminal context after cleanup, and preserves its checkout,
branch, and directories. The retained operation stays answerable by its close request ID
even after the Workspace leaves the active projection:

```sh
orchbee operation status <close-request-id>
orchbee operation resume <close-request-id> --generation <n> --confirm
```

`operation status` reports the scope lifecycle, ending in a `closed`
tombstone revision; `operation resume` opens one
fresh attempt window only on a resumable retained operation — a stale
`--generation` answers `rejected`/`stale-generation`, and unknown outcomes
are non-resumable. Closing tabs does not
stop the Session, and nothing in this guide deletes runtime state.

## JSON and automation notes

- `--format json` selects the machine envelope on the commands that accept
  it — the families above, `status`, `server start`, and `config check`;
  `help`, `completion`, and `--version` do not take it. A failure envelope
  likewise needs an explicit `--format json` for its stdout document.
- u64 fields serialize as decimal strings (for example `"revision": "9"`).
- For every target rule, source bound, deadline, and exit class, the
  authoritative reference is [native CLI query](native-cli-query.md) and
  `orchbee help <family> <command>`.

The provider hook writers, Cursor's included, are dormant in this release; see the [Windows integration contract](windows-agent-integrations.md).
