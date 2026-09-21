# Native CLI Workflows

A complete headless sequence for the experimental Linux-native `orchbee`
CLI: start the local Site, build a Project/Workspace/directory, launch and
drive a terminal, inspect worktrees, observe Agents, and close cleanly. The
command inventory lives in the [README](../README.md); the full grammar,
target rules, and failure matrix are owned by
[native CLI query](native-cli-query.md). This guide covers the supported
end-to-end path only — option and target wording matches what
`orchbee help` prints, and shell snippets are illustrative.

## Prerequisites

- An unpackaged **development cohort** on Linux, built from the repository
  root — see [Linux local startup](../README.md#linux-local-startup):

  ```sh
  node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
  ```

  Then run `/tmp/orchbee-native-dev/orchbee`. The staged cohort is four
  executables (CLI, Server, Worker, Host) that must come from one build plus
  the observation-adapters payload directory, all pinned by the manifest;
  non-Linux platforms are not admitted (`unsupported-platform`), and this
  is not an installed product or a system service.

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
No CLI command stops, repairs, or deletes a Site.

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
labels are refused with candidates. `--path` is a literal absolute POSIX
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

`send` delivers exactly one input record from exactly one source: literal
`--text`, one named `--key`, a regular `--file`, or `--stdin` — which
accepts a redirected regular file or a FIFO/pipe and refuses a terminal,
socket, or device. Text-bearing sources are capped at 4096 bytes including
the folded `--enter` carriage return. One absolute 5-second deadline
sampled at command entry covers source materialization _and_ the entire
send lifecycle. `wait-output` exits `0` on match, `7` on timeout, `130` on
signal. A send whose outcome is unknown (exit `8`) may already have been
delivered — check the terminal before resending.

## 5. Inspect worktrees

```sh
orchbee worktree list --project "$PROJECT" --directory "$DIR"
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
```

`--confirm` is required. An `accepted` receipt carries `closeRequestId`
while the Site completes the close asynchronously. Closing tabs does not
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
