# Orchbee Native Entry

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` pastes one command and sends Enter in one input request. Supply one quoted string; text is preserved, and targets are exact IDs or `@terminal:<suffix>`. The receipt confirms input submission, not Shell readiness, command completion or exit status; never replay partial or unknown delivery. [Command submission contract](docs/native-terminal-run.md).

Experimental, unpackaged native Rust Client. On Windows and Linux, bare invocation connects or activates a
local Site from a verified development cohort and enters its TUI after the first validated terminal
projection frame. Native Clients can create, select, move and refresh shared Tabs while keeping independent
selection and retained terminal processes. Remote and full Workbench operations are not integrated.
The emitted `orchbee` participates in root Cargo verification; Desktop, installers, and package
managers do not consume it.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

Use `agent rename <session-id> <name>` to assign a runtime name, `agent names` to list names,
and `agent resolve <name>` to obtain the full session ID. `agent rename <target> --clear`
releases a name. `agent get/explain/wait <name>` resolve directly; a wait pins its session
even if the name is reused. Names are unique and disappear when the session ends or the Site stops.

`agent read <name>` reads the uniquely bound terminal; `agent send-keys <name> enter`
sends semantic keys through the existing terminal input receipt. Both also accept
`--session <id>` and `--name <alias>`, and refuse missing or ambiguous bindings.

`agent prompt <name> <text> --wait` submits literal paste plus Enter after a fresh,
unblocked preflight. It observes activity before waiting for a condition and retains the
input receipt on later timeout or cancellation; it does not promise turn causality.

`agent start alpha --kind codex --panel <id>` starts an Agent in an idle Windows
terminal and binds its name only after process and canonical readiness checks.
See [Windows Agent startup](docs/windows-agent-start.md) for setup, limits and recovery.

## Command surface

Bare and explicit `tui` invocation first admit the inherited interactive terminal and valid
cell geometry, then run the local startup workflow below on Windows and Linux. They are the same
Client route; `tui` does not select a different mode. Other platforms return localized unavailability. `--help`
and `--version` initialize neither TUI nor service. There are no abbreviations or remote selection;
every other, additional, or non-Unicode argument is a usage failure.

```text
orchbee                          # local terminal (TUI)
orchbee tui                      # the same terminal, explicitly
orchbee status                   # the running Site's Workbench snapshot
orchbee <family> <command>       # queries and Workbench mutations
orchbee api snapshot|schema      # machine snapshot and embedded schemas
orchbee site start               # ensure the local Site is running (no TTY)
orchbee site diagnose            # read-only local Site runtime facts
orchbee site stop --confirm      # stop the Windows Site and its terminals
orchbee site stop-status <id>    # inspect a retained stop attempt/receipt
orchbee config check             # local runtime inspection
orchbee help [<family>|<command>]
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --help | --version
```

Query families are `client`, `project`, `workspace`, `tab`, `panel`, `terminal`, `worktree`,
`launch-profile`, and
`agent` (`list`/`get`, plus `terminal read`/`wait-output`; Windows `worktree list` accepts
`--workspace <target>` or `--cwd <absolute-path>` with optional command-local trust,
or retains the legacy `--project <target>` and strict `--directory <directory-id>` pair; `launch-profile` offers `list`
and `get <selector>` — a typed read of the running Site's launch profile catalog, resolved
Server-side with no local pre-read; `agent` offers `list` with
`--include-ended`/`--limit`/`--cursor` paging, `get <session-id>`, and `explain
<session-id>` — a read-only typed view of the Server's canonical Agent sessions over OBAP,
never inferred from terminal contents — `wait <session-id> [--until <condition>]...
[--timeout <ms>]`, a held Client Session waiting for status or freshness; Windows follows
retained changes and defaults to no total timeout, with every read still bounded — plus `doctor [--provider <id>]... [--asset-root
<path>]...`, an offline bounded-filesystem diagnostic that explains from local facts alone
why Agent observations may not arrive (no provider execution, no socket connect, no writes).
Windows `agent explain` also resolves names and Panels to exact physical terminal bindings.
Canonical sessions retain `orchbee.agent.explain` v1; physical terminals use v2 with the effective
report view and a separate fresh screen-rule evaluation. The result includes manifest source/version,
the first highest-priority winner, every rule match and bounded region evidence. Live full lifecycle
authority returns an explicit screen-skipped result without a Host read. One admission and five-second
CLI budget cover resolution/read; Server queue/read work has a four-second budget. Binding or report
context changes refuse without retry. No input, controller acquisition or focus action is performed.
Windows startup selects local overrides, eligible cached remote rules, then bundled rules. Physical v3 adds actual path, cached version, shadowing and typed warnings; v2 remains closed for legacy/no-manifest/skipped results. Windows `site agent-manifests`, `site reload-agent-manifests` and `site agent-manifests-status` expose the installed snapshot and retained live-reload receipts. Explicit `site update-agent-manifests` downloads and activates validated cached rules, with separate per-provider cache and activation receipts. Managed release Sites check automatically at startup and every thirty minutes when `update.manifest_check` is enabled; debug/test and standalone execution do not schedule checks. See the [screen model](../orchbee-server/docs/agent-screen-model.md).

Mutation commands against the
running Site are `project create`/`rename`,
`workspace create`/`rename`/`accept-directory`/`close`, `tab create`/`rename`/`move`/`close`,
`panel split`/`move`/`close`, and `launch-profile create`/`delete` —
`workspace close <target> --confirm` closes every terminal in the Workspace's context under
the same tab-close lifecycle while preserving the Workspace, its checkout, branch, and
directories; its asynchronous operation stays answerable through
`workspace close-status --project <id> --workspace <id>` (strict stable IDs, tombstone
included) and resumable through
`workspace close-resume --project <id> --workspace <id> --request <close-id>
--generation <n> --confirm`.
`launch-profile create --name <name> --exec <path> [--arg <value>]...` stores one named
launch selection in the catalog and `launch-profile delete <selector>` removes it. `tab
create` and `panel split` additionally accept either an ephemeral
`--exec <path> [--arg <value>]...` launch selection or a `--profile <selector>` catalog
launch — mutually exclusive — that replaces the resolved default shell for
that one request only. `terminal send <id> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>`
delivers exactly one input record — literal text, a regular file's contents, or piped
standard input, each with an optional folded carriage return, or one of fifteen semantic
keys — to a bound terminal over the same admitted session, without
taking over TUI controller/viewer ownership; its two-axis receipt reports the Server's input
verdict and cleanup status separately. `project create` submits the existing `ProjectCreate` action —
one navigation record, no directory or filesystem effect — and returns the Server-allocated
project id for the `workspace create`/`tab create` chain; `workspace accept-directory`
submits the existing `WorkspacePrimaryDirectoryAccept` action — the Server authoritatively
probes the literal absolute path — which makes a CLI-created Workspace tab-admissible
without the TUI directory wizard. `project rename <target> --label <label>` submits the
existing `ProjectRename` action — the same one-snapshot/one-command envelope resolves the
target by exact id, an explicit `@<kind>:<suffix>` selector, or exact label first — and reports `committed`/`unchanged` under the
dedicated `orchbee.project.rename` document. `site start` is the headless activation path:
it runs the same connect-or-activate sequence the TUI startup uses — validate the cohort,
prepare the runtime directory, take the locks, commit the marker, spawn the Server when no
running Site answers — under one absolute 20-second deadline, then reports `started` only
when this invocation actually spawned the child and `already-running` when an existing Site
admitted its probe. The command is effectful even on refusal: only a grammar failure is
guaranteed zero-effect, an interrupted or timed-out start may leave committed activation
state the next invocation reports as `rejected`/`recovery-required`, and it never repairs,
stops, or deletes runtime state. `site diagnose` is the read-only complement: it reports
bounded facts about the runtime directory, marker, locks, and finite endpoint paths — each
a closed-state row at its own observation instant — under one cooperative 5-second budget,
and it never connects, creates, locks, repairs, or claims any process is alive or gone.
`agent integration` (`list`, `explain`,
`enable`, `disable`) is the local provider-onboarding family: bounded reads
and managed mutations of the invoking user's provider configuration on this
machine only — no socket, lock, or Site contact; only entries verifiably
owned by Orchbee are installed or removed. The complete grammar, target and
filter rules, mutation outcomes, and the failure matrix with exit classes are owned by
[native CLI query](docs/native-cli-query.md); `orchbee help <family>` and
`orchbee help <family> <command>` print the same grammar from the binary itself.
For an end-to-end headless sequence — start a Site, create a
Project/Workspace/directory, launch and drive a terminal, inspect worktrees,
observe Agents, and close cleanly — see [native CLI workflows](docs/cli-workflows.md).

Common command templates, in the order one session usually needs them —
follow the linked workflow to create the Workspace and terminal first, then
substitute each `<…>` placeholder with the real id or provider:

```sh
orchbee site start                        # activate the local Site once (no TTY)
orchbee status                            # is the Workbench reachable?
orchbee project create --label demo       # then workspace create / accept-directory
orchbee tab create --workspace <ws> --label shell \
  --cols 80 --rows 24 --cell-width 8 --cell-height 16
orchbee terminal send <terminal-id> --text 'make test' --enter
orchbee terminal wait-output <terminal-id> --match 'PASS'
orchbee agent integration enable --provider <id>   # one-time local onboarding
orchbee agent list                        # sessions the running Site observes
orchbee agent wait <session-id> --until ended
```

`orchbee help <family>` prints the same grammar from the binary itself, and
`orchbee completion <shell>` prints the matching key-driven discovery script
for your shell to source.

Help writes stdout, failures write stderr, and version writes one locale-neutral
`orchbee <version>` line. Development status is `0` for normal TUI close/help/version and for
successful queries, mutations, and checks; `2` for usage failure; the remaining failure classes
follow the owning record's failure matrix. This is not a stable public exit table.

## Native Windows quick start

With an existing Project, `workspace create --project <target> --label <label> --cwd C:\existing\repo` creates the Workspace and its first terminal in one request. Add `--env NAME=VALUE`, an explicit `--exec`/`--arg`, or `--profile` as needed. The receipt returns resource IDs and the terminal outcome; a failed launch retains the Workspace for inspection and explicit close. See the [Windows workflow](docs/cli-workflows.md#windows-directory-workspace) for PowerShell examples.

Build with the repository's pinned Rust and Zig toolchains and Node.js. Run this from the repository
root in PowerShell; the output directory must be new, and its parent must already exist:

```powershell
node scripts/stage-native-site.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
& "$env:LOCALAPPDATA\OrchbeeNative\orchbee.exe"
```

The directory contains the four exact `.exe` siblings, a hash manifest, and the observation-adapter
capsule. Keep them together. This is a native development cohort using Windows named pipes and
ConPTY. Windows Terminal is the interactive host; WSL is not required. The initial shell is the
absolute `COMSPEC` executable, falling back to `SystemRoot\System32\cmd.exe`. Run `pwsh` inside it
when PowerShell is preferred. `site start --format json` also works without an interactive console.

The prefix is `Ctrl+B`, followed by `|` for a side-by-side split, `-` for a stacked split,
`h/j/k/l` for pane focus, or `d` to detach. Splitting preserves the current focus. Run the executable
again to reattach to the retained Site. Detaching leaves shells running; an exited shell remains
observable until its Tab/Panel is explicitly closed. The runtime does not automatically recover a
crashed Site. `config check --format json` checks local cohort/runtime inputs without starting one.

Validated Windows behavior includes native startup, Unicode rendering, burst and ordinary typing,
CLI terminal send/read, split/focus/resize, detach/reattach with exact retained terminal identities,
and Git worktree discovery. Console records feed the existing key and SGR mouse decoder; the console
mode guard restores the inherited input/output modes when leaving the TUI. Large byte records use
bounded writes, and cancelled exchanges close their shared pipe without a detached flush thread.

Windows `agent doctor --provider codex --format json` now inspects local configuration, PATH,
Node and capsule integrity without starting a Site or changing provider setup. Its version-3
document reports endpoint delivery as uninspected; installed configuration is not proof of
a working Agent connection.

Windows `agent integration list/explain/enable/disable --provider <id>` manages Claude Code, Codex, Grok, GitHub Copilot, OpenCode, Kilo, Pi, OMP, Gemini and Qwen Code. See the [Windows integration contract](docs/windows-agent-integrations.md). Windows `site diagnose` reports read-only runtime, lock and pipe-name facts using `orchbee.site.diagnose.windows` v2. Capsule 0.4.16 supplies Claude Code, Codex, Grok and GitHub Copilot hooks and JavaScript modules using the native observation pipe and console correlation.
Windows x64 cohorts include the pinned ConPTY runtime needed to preserve that correlation.
Agent start remains pending; other providers retain their
existing platform limits. `terminal send --stdin` accepts redirected regular
files and Windows pipes; an owned hidden Windows PowerShell reader makes pipe input cancellable.
It is resolved from the OS system directory, never from PATH, and is terminated/reaped on failure.
Shell completion supports bash/zsh/fish/powershell/elvish. In PowerShell, load it explicitly with
`Invoke-Expression (orchbee completion powershell | Out-String)`; no profile is changed.
In interactive Elvish 0.21, use `eval (orchbee completion elvish | slurp)`; the static callback
registers both `orchbee` and `orchbee.exe` without querying a Site or modifying profiles.
For Unicode pipelines in Windows PowerShell 5.1, set `$OutputEncoding = [Text.UTF8Encoding]::new($false)` in the invoking script; PowerShell chooses the bytes sent to a native command. Orchbee preserves those bytes and does not repair prior shell encoding loss.
`orchbee --skill` prints the embedded [Agent guide](docs/cli-agent-skill.md) offline. Signing, automatic updates, crash recovery, macOS startup, and a packaged
replacement for the delivered TypeScript CLI are separate work.

## Linux local startup

Build a new development cohort from the repository root:

```sh
node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
# From a Windows host using WSL Linux:
node scripts/stage-native-site.mjs --wsl Ubuntu --output /tmp/orchbee-native-dev
```

Then run `/tmp/orchbee-native-dev/orchbee` in an interactive Linux terminal.
The script rebuilds three Rust release binaries and the Zig ReleaseSafe Host from locked inputs,
copies the canonical observation-adapters capsule as the cohort's lazily validated sidecar,
and verifies its declared payload digest before writing the manifest;
existing output directories are rejected. `--cargo-target-dir <absolute-directory>` can reuse a
Cargo build cache. This does not install a system service or replace the existing TypeScript CLI.

The default Site is `/tmp/orchbee-native-<effective-uid>`; development/tests can isolate a Site
by setting `ORCHBEE_RUNTIME_BASE` to another existing absolute directory.
The [shared runtime contract](../../crates/local-site-runtime/README.md) owns the 0700 directory,
0600 locks/sockets/immutable attempt marker, exact four-executable cohort with its declared
adapters sidecar, shared 20-second deadline,
and delayed 500-ms progress notice. Server + Worker readiness and first terminal frame readiness
are distinct phases. Initial Open, snapshot read, attach, acquire, authorized resize, and first-frame Poll all
consume the original startup deadline. Native Ready requires a drawn full frame, its exact accepted
feedback and current controlling geometry. The worker restores the ordinary request budget after
consuming the Ready reply. Undrawn FrameApplied feedback is never sent early; NeedSnapshot recovery
feedback may advance initial frame acquisition.

The Client never restarts or adopts a failed Site. Failed/residual runtimes explicitly require
recovery and are not deleted automatically. `Ctrl+B q` or `Ctrl+B d` closes only this Client;
reopening attaches to the same retained shell. Additional concurrent Clients are read-only unless
they acquire an independent controller grant. TTY admission occurs once, every duplicate is
CLOEXEC, and the prepared Session plus that same TTY are handed to the interface. Terminals must
report real valid cell/pixel dimensions; a paired `(0,0)` pixel report is admitted as an honest
unknown-pixel declaration, and missing pixel values are never guessed. When the reported
geometry is not drawable — including a temporary zero-cell report — the Client waits for a real
drawable geometry inside the original deadline instead of failing admission or inventing one;
the [native startup wait](docs/native-startup-wait.md) owns that bounded waiting model, its
cancel/timeout/fault outcomes, and its no-service-work guarantee. Linux startup has public process evidence; native Windows startup and ConPTY workflow evidence
are described above. Release performance budgets remain unclaimed.

## Native Tab interactions

The native Session renders the active context's ordered Tabs and canonical labels. Each Client
owns its own active selection, navigator and viewport; another Client's committed Tabs become
visible through the automatic publication probe, and explicit refresh remains available. These controls use the existing `Ctrl+B` prefix:

| Key                    | Connected native behavior                                                                                                                                                            |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `c`                    | Request one Server-owned Tab creation, then select its validated committed member.                                                                                                   |
| `n` / `p`              | Select the next / previous Tab in the current Client snapshot.                                                                                                                       |
| `1` … `9`              | Select that position when present.                                                                                                                                                   |
| `[` / `]`              | Select the previous / next standalone or Workspace context in Server navigation order.                                                                                               |
| `a`                    | Create a Workspace in an existing or new Project through separate explicit submissions.                                                                                              |
| `e`                    | Configure the first directory of the current Empty, unconfigured Workspace.                                                                                                          |
| `r` / `R`              | Rename the current Workspace / its Project; standalone has neither target.                                                                                                           |
| `<` / `>`              | Move the active Tab before / after its neighbor, wrapping at either boundary.                                                                                                        |
| `,`                    | Edit the selected Tab's shared label; Enter submits after its first complete draw, Escape cancels.                                                                                   |
| `x`                    | Confirm closing the selected Tab's execution for all attached Clients, or cancel.                                                                                                    |
| `w`                    | Open the navigator and refresh shared Tabs while preserving the active terminal identity.                                                                                            |
| `h` / `j` / `k` / `l`  | Move local focus to the neighboring Panel when the active Tab has a split tree.                                                                                                      |
| `-` / `_` / `\` / `\|` | Split the focused Panel below / above / left / right into a second leaf (provisional binding).                                                                                       |
| `X`                    | Request the Panel-close confirmation for the focused leaf — after the complete draw, `y` confirms and `Esc` cancels; the last leaf names the whole-Tab impact (provisional binding). |
| `H` / `J` / `K` / `L`  | Move the focused Panel leaf toward its directional neighbor (provisional binding).                                                                                                   |
| `q` / `d`              | Close this Client while retained execution continues.                                                                                                                                |

[Context selection](docs/native-context-selection.md) preserves each retained terminal while
switching location. The navigator and Workbench title show the current Project/Workspace. Empty
Workspaces can be entered and left; only explicit `c` creates a terminal, using the Workspace's
accepted directory. [Workspace setup](docs/native-workspace-setup.md) provides creation and literal
absolute-path input. Each step requires a complete first draw; cancellation preserves earlier
committed objects. Directory acceptance never starts a terminal.
[Location labels](docs/native-location-labels.md) use a prefilled editor and shared Server commands;
renaming preserves the running terminal and becomes visible to peers through the publication
probe or explicit refresh.

Creation drains earlier accepted work and preserves known commit facts through deadline and Client
retirement. Switching releases the old viewer before attaching the selected binding; it does not
stop or recreate either terminal. Moving changes shared order while preserving the active terminal
and its viewer/controller. If a committed Move receives a definite denied/unavailable snapshot result, the Client
keeps terminal interaction available and uses `w` for explicit snapshot recovery before another
structural operation.

Rename preserves exact text up to 512 Unicode scalars. Bracketed paste is atomic and lands as one
edit — prefix, bracket, and newline bytes inside the payload never escalate to commands. Backspace
removes one scalar and Ctrl+U clears the draft; unmodified Left/Right/Home/End/Delete, Ctrl+A/Ctrl+E,
and Ctrl+W reach the draft as semantic edits, while modified or unbound sequences are consumed whole
without cancelling the draft. A known rename with unavailable projection uses the same
explicit `w` recovery without resubmitting the command. The [native Session contract](docs/native-tab-session.md)
owns admission, geometry, feedback, failure, recovery and evidence bounds. Close requires a separate
lifecycle grant, including for a view-only Client. Closing the last Tab leaves Empty; explicit `c`
creates a new terminal, while ordinary Client exit preserves it for later bare Open. A peer whose
terminal was closed uses explicit `w` to recover the current membership. Linux process evidence
covers this sequence. Canonical split-panel projection, local panel focus, and `panel.split` /
`panel.close` / `panel.move` submission are integrated for ungrouped, unpinned Tabs; production
restart recovery remains unfinished. Panel operations reuse the same structural-mutation admission:
split submits the real controlling cell/pixel geometry and keeps the leaf's label, close is
leaf-scoped and commits through the ordinary publication path, and move relocates the leaf beside
its directional neighbor. Closing the Tab's last leaf removes the whole Tab — its terminal
execution is disposed and the Tab ends for every attached Client, the same consequence an explicit
`x` Tab close warns about — and the provisional `X` binding reaches that close only through the
drawn [panel-close confirmation](docs/native-panel-close-confirmation.md), which names the frozen
leaf and its impact before an exact `y` admits it. Panel commands are Workbench
mutations admitted under the Server's `WorkbenchGrant` capabilities — `mutate_navigation` and
`create_terminal` for split, `mutate_navigation` for move, `close_terminal` for close
([Workbench authority](../orchbee-server/docs/workbench-authority.md)) — a layer independent of the
terminal-side controls: a viewer attachment grants neither input nor resize authority — its
`read`/`input`/`resize` capability flags scope that attachment itself — and input and resize flow
only through the terminal's single combined input-and-resize controller grant, bound to one
Session, viewer, Worker generation, and Host terminal
([`schemas/client-server/README.md`](../../schemas/client-server/README.md)). Terminal input or
resize control never authorizes a Workbench mutation, and a read-only attachment cannot input or
resize regardless of any Workbench grant. A terminal-busy refusal from an overlapping aggregate Close is a definite
not-applied result, and a committed removal of the focused leaf recovers membership onto a
surviving leaf of the same Tab. The `-`, `_`, `\`, `|`, `X`, and `H`/`J`/`K`/`L` bindings are
provisional pending the input-design review and may be rebound; the split and close glyphs follow
the input-vocabulary draft, while `H`/`J`/`K`/`L` are interim top-level stand-ins for the draft's
`PANEL`-mode `h`/`j`/`k`/`l` move.

## Private local Client transport

On Unix, the application-private `LocalClient` accepts an endpoint and executable cohort only from
an internal caller. It delegates admission to `orchbee-local-client-protocol`, which validates the
cohort before connecting, proves the Server has the same effective operating-system user, performs
the exact `OBLC` handshake with two-second read/write timeouts, and returns the accepted generation
plus the same unbuffered stream. Those existing handshake timeouts remain a separate OBLC boundary.
On Linux, private `LocalClient::connect_before` instead passes the caller's absolute monotonic
deadline through connection, peer proof, hello, response and final admission. Its
[owning adapter and correctness record](../../crates/local-client-protocol/README.md#deadline-bounded-linux-admission)
define failure cleanup and the unchanged-deadline guarantee. Both paths switch the admitted stream
to nonblocking I/O before `OBCS`. Ordinary requests have an independent budget; initial local
requests are clamped to the original startup deadline. macOS deadline admission remains unfinished.

`submit_terminal_open`, `submit_terminal_create`, `submit_terminal_start`, `submit_terminal_viewer`, and `submit_terminal_control` each take ownership of one already validated
typed request and encode its complete OBCS frame before submission. Encoding failure is definite
and leaves the Session available. A successful encoding starts one fifty-second monotonic deadline
covering the complete frame write and the one Server-frame read; readiness waits recompute the
remaining budget, so partial progress cannot renew it. Failure before the first write syscall is
definite. Once that syscall begins, any write or read failure or deadline expiry, malformed or
incompatible result, wrong result kind, or wrong correlation makes the effect outcome typed
`OutcomeUnknown` and retires the connection. The transport never retries or returns the request for
replay. A correctly correlated Server `outcome-unknown` result remains a valid OBCS result rather
than a transport ambiguity.

Bare Linux startup consumes this transport for standalone Open, viewer, and control intent.
Its close-only `LocalClientShutdown` handle stops a blocked exchange without acquiring another
stream reader or writer. Reconnection and reconciliation remain separate boundaries.

The fifty-second submission bound accommodates Server's one queued and one in-flight Worker
exchange (each at most two seconds writing plus twenty seconds reading) and Client result delivery.
Handshake limits stay separate. Viewer submission validates action/outcome compatibility and exact
correlation, and preserves opaque OBSR bytes for the connected projection consumer; it grants no
controller authority and performs no terminal presentation decoding. Controller submission validates
exact grant acquisition, input acknowledgement, and complete resize geometry against its action.
Server-reported unknown control outcome is returned without retry; mismatched outcomes retire the
transport. The caller owns fresh request identities, increasing grant-scoped sequences, and any
explicit later acquisition. Cleanup after Session loss may outlive a submission deadline.

All existing-viewer actions and all controller actions carry the exact Server-issued `attachmentId`.
The native executor retains that identity immediately after Attach, before setup can fail or be
superseded. Poll, control, projection feedback and cleanup use that captured identity; bounded busy
retries preserve it while refreshing only the permitted request identity and control sequence.
Projection feedback is checked against its owning attachment before submission, never retargeted.
`LocalClient` stores no implicit current viewer. In a connected native Session every visible leaf
of the active Tab's canonical panel tree owns its own viewer attachment — a stable logical
`PaneKey` ({selection anchor, `WorkbenchPanelId`}) whose physical attachment carries a `PaneRef`
incarnation minted per record. Input and resize admit only the focused pane's exact incarnation,
while polls continue for every admitted pane incarnation so background leaves keep rendering their
own frames. Same-Tab `h`/`j`/`k`/`l` focus movement is a local selection change — it never
detaches, re-attaches, or re-tags work already accepted — and cross-Tab selection still runs the
existing switch cleanup/attach chain. A detached-and-rebound pane fences every late input, frame,
feedback, and settlement tagged with its retired incarnation, and viewer capacity stays bounded by
the shared HostViewers limit (8).

### Submission correctness record

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                  |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | An internal Client with an already admitted same-user Session must submit one typed Create, Start, viewer, or controller request without duplicating an effect when transport completion is ambiguous. Endpoint acquisition, process activation, and presentation are excluded.                                                                                           |
| Inputs       | Trusted typed `TerminalCreateRequest`, `TerminalStartRequest`, `TerminalViewerRequest`, or `TerminalControlRequest`, the admitted `ClientSessionConnection`, its accepted generation, a finite request budget, and monotonic time. Server bytes and readiness outcomes are untrusted. An absent connection means `SessionUnavailable`.                                    |
| Data         | One invocation owns one `requestId`, encoded frame, absolute deadline, connection state, possible-handoff boundary, expected result kind, and decoded result class. The `LocalClient` exclusively owns its Session.                                                                                                                                                       |
| Outputs      | Exact correlated typed result; definite `RequestEncoding` or `RequestNotSubmitted`; typed `OutcomeUnknown` with the submitted identity; or `SessionUnavailable`. There is no cancellation, retry, replay return, or optimistic terminal mutation.                                                                                                                         |
| Invariants   | Encoding precedes the deadline and transport; one invocation begins at most one frame handoff; only exact kind and identity complete; every ambiguity after possible handoff retires the Session; a retired Session sends no bytes.                                                                                                                                       |
| Decisions    | Before versus after first possible handoff; write incomplete versus complete; result before versus at/after deadline; exact versus wrong kind or identity; valid versus malformed frame; known versus Server-reported unknown outcome. Deadline observation wins if acceptance has not completed.                                                                         |
| Model        | `LocalClientSubmission` is the bounded one-invocation state refinement. `LocalClient::exchange` is the implementation workflow; its deadline-aware nonblocking stream wrapper is the time and I/O adapter.                                                                                                                                                                |
| Transitions  | The transition record below defines the serializable abstract phases and guards. Runtime phase is call-local; only the live-or-retired connection state persists between calls.                                                                                                                                                                                           |
| Dependencies | One already admitted Unix stream, monotonic time, readiness polling, OBCS encoding/decoding, and same-stream OBLC admission. The OBCS workflow has no filesystem, process, endpoint-selection, randomness, localization, or UI capability.                                                                                                                                |
| Ownership    | `LocalClient` is the sole stream and retirement owner. OBCS owns frame and result semantics; OBLC owns admission, credentials, cohort, generation, and its existing handshake timeouts.                                                                                                                                                                                   |
| Verification | Fake-Server tests cover exact Create/Start, large OBSR records, incompatible viewer outcomes, exact control outcomes and resize geometry, correlation, protocol and EOF failures, silent timeout, deterministic partial read/write deadline exhaustion, pre-write expiry, retirement, and no duplicate send. TLC requires a witness for every registered semantic action. |
| Correctness  | Claims LC-01 through LC-05 below refine implementation branches and deterministic tests to the registered model. TLC exhausts 49 bounded states for one request and one Session; it does not prove OS scheduling, codec correctness, OBLC connect timing, activation, reconnection, reconciliation, or future TUI behavior.                                               |

### Submission transition record

| Current phase                      | Event and guard                                                   | Next phase         | Result and connection effect                                           |
| ---------------------------------- | ----------------------------------------------------------------- | ------------------ | ---------------------------------------------------------------------- |
| `ready`                            | Submit while a Session exists                                     | `encoding`         | No transport effect                                                    |
| `encoding`                         | Frame encoding fails                                              | `definite-failure` | `RequestEncoding`; Session retained                                    |
| `encoding`                         | Frame encoding succeeds and creates one absolute deadline         | `write-ready`      | Encoded frame is private to this invocation                            |
| `write-ready`                      | Deadline or I/O setup fails before the first write syscall        | `definite-failure` | `RequestNotSubmitted`; Session retained                                |
| `write-ready`                      | First write syscall may hand bytes to the kernel                  | `write-begun`      | Handoff count becomes one                                              |
| `write-begun`                      | The one complete frame is written before expiry                   | `awaiting-result`  | Wait for exactly one Server frame under the same deadline              |
| `write-begun` or `awaiting-result` | Deadline, I/O, timeout, or EOF wins                               | `retired`          | `OutcomeUnknown`; connection closed                                    |
| `awaiting-result`                  | Decoded frame is malformed, wrong-kind, or wrong-ID before expiry | `retired`          | `OutcomeUnknown`; connection closed                                    |
| `awaiting-result`                  | Exact correlated result is decoded and deadline is still open     | `completed`        | Return typed known or Server-reported-unknown result; Session retained |
| `retired`                          | Any later submit call                                             | `retired`          | `SessionUnavailable`; no write and no retry                            |

### Correctness traceability

| Claim                                                                                                                              | Implementation and deterministic witness                                                                                                                                                                  | TLA+ evidence                                                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| LC-01: encoding and every failure before the first write are definite.                                                             | `exchange` encodes before starting `SubmissionDeadline`; `deadline_expiry_before_the_first_write_is_a_definite_failure` observes zero Server bytes and a retained generation.                             | `EncodingFails`, `PreWriteDeadline`, `PreWriteIoFails`; `DefiniteFailurePrecedesHandoff`.                      |
| LC-02: one invocation has at most one request-frame handoff and never retries it.                                                  | One encoded buffer is passed once to `write_all`; slow-write and silent-timeout tests reject a second request and subsequent calls return `SessionUnavailable`.                                           | `WriteBegins`; `AtMostOneHandoff`; `NoSendAfterRetirement`.                                                    |
| LC-03: every failure after possible handoff is unknown and retires the Session.                                                    | Write/read, EOF, incompatible frame, wrong kind, and wrong identity branches all call `retire_with_unknown`; focused tests assert the generation disappears.                                              | `WriteFails`, `WriteDeadlineExpires`, `ReadFails`, all retire actions; `UnknownRequiresHandoffAndRetirement`.  |
| LC-04: only exact kind and identity complete, while a correlated Server `outcome-unknown` is data rather than transport ambiguity. | Exact Create/Start, wrong-kind, wrong-ID, and correlated-unknown fake-Server tests exercise the result classifier.                                                                                        | Matching and wrong-result arrival actions; `AcceptedResultIsExactAndBeforeDeadline`.                           |
| LC-05: one monotonic deadline covers the full write and read and is not renewed by progress.                                       | `SubmissionDeadline` is created once; every readiness boundary recomputes its remainder. Stepping-clock tests force sub-deadline byte progress across both read and write until the total budget expires. | `WriteDeadlineExpires`, `ResultDeadlineExpires`, `RetireTimedOutResult`; `TimeoutCannotProduceAcceptedResult`. |

## TUI shell foundation

Application-private shell adapters own the following terminal and presentation boundaries.
Bare Linux startup uses the prepared Session; unconnected `run` and explicit `run_connected`
remain test and composition seams.

The input and transition tables below describe the unconnected fallback. Connected context and Tab
actions are handled by the native Session described above.

| Concern     | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Admission   | The staged Unix adapter requires inherited stdin and stdout to be TTYs with the same terminal-device identity, duplicates only those admitted descriptors, and reads cell and pixel dimensions from the duplicated stdout descriptor. It has no `/dev/tty`, global Crossterm size, or process fallback.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Lifecycle   | One staged guard owns raw mode, the alternate screen, and cursor visibility. It marks cleanup responsibility before an operation may partially succeed, retains each flag until its inverse succeeds, and attempts each failed inverse at most twice before returning one aggregate error. A process-lifetime singleton panic hook dispatches only when the panicking thread is the active guard's owner, so another thread cannot clear a setup lease before its terminal state is saved; guard drop does not remove a later hook. Normal return, initialization/event/draw failure, owner-thread panic, and each of `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` have deterministic restoration or signal-observation evidence.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Client View | One invocation owns prefix mode, viewport class, sidebar projection, active overlay, and transient notice. It owns no shared Tab, Panel, Workspace, Site, or Terminal fact and persists nothing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Layout      | At 96 columns or wider, a 30-column Orchbee business sidebar is docked beside the Workbench region. Narrower viewports keep the Workbench full width and expose the sidebar only as a left overlay. The connected Workbench projection contains a Tab strip and canonical Panel tree; split leaves retain identity and layout, while the active leaf receives the current terminal frame.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Input       | The staged adapter polls and reads at most 4,096 raw bytes at a time from the admitted stdin descriptor. A bounded legacy decoder turns the stream into typed items — a UTF-8 scalar, a CSI/SS3 or kitty-`u` key, an atomic bracketed-paste run, or one non-decision sequence. Every open frame (ESC wait, CSI/SS3, UTF-8 scalar, paste run) owns its bytes across reads under a bounded injected wait; a frame opened inside a captured packet keeps that owned provenance even when it completes in a later read, an over-bound or timed-out frame drains to a reliable boundary instead of leaking its tail into bindings, and a paste run pairs only with the terminator matching its opener's notation. Every completed read is dispatched in order before any subsequent signal, viewport query, or resize decision can terminate that loop iteration. `Ctrl+B` enters visible prefix mode. `b`, `w`, `?`, `q`, and `d` drive local shell state or exit; connected `c`, Tab selection, and `h`/`j`/`k`/`l` actions are handled by the Native Session. Outside prefix mode, all bytes—including `Ctrl+C`, `Ctrl+D`, escape sequences, UTF-8, and input while an overlay is visible—reach an injectable terminal-input port byte-for-byte in order; a lone `Esc` forwards only after a bounded wait, and prefix then `Ctrl+B` forwards one literal byte. |
| Outcome     | The shell consumes its prepared Session. Bootstrap owns activation; Server owns terminal identity; Host owns terminal lifetime. The shell does not mutate Workbench state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

The deterministic Client View transition model is:

| State and event                                  | Next state, command, and visible result                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Normal + `Ctrl+B`                                | Enter Prefix; emit no terminal input; render the prefix indicator.                                                                                                                                                                                                                                                                                                                                                                               |
| Normal + other bytes, with or without an overlay | Preserve local view state and offer the exact ordered bytes to the terminal-input port. The unavailable port rejects them without side effects and the staged renderer reports that input was not sent.                                                                                                                                                                                                                                          |
| Prefix + `b`                                     | Leave Prefix and toggle the docked wide sidebar or narrow sidebar overlay for this Client View.                                                                                                                                                                                                                                                                                                                                                  |
| Prefix + `w` or `?`                              | Leave Prefix and open the Workbench navigator or shortcut-help overlay locally; repeating the same binding closes that overlay again.                                                                                                                                                                                                                                                                                                            |
| Prefix + `PageUp` or `PageDown`                  | Leave Prefix and, only while the shortcut-help overlay is open, turn its page; the page index is Client-local, bounded by the registry row count, reset to 0 when help closes and reopens, and settled each draw from the renderer's clamped effective page (e.g. after a resize). With help closed the chord is inert and reports the same unknown-prefix notice as any unbound key; without Prefix the keys reach the terminal port unchanged. |
| Prefix + any other complete key or sequence      | Leave Prefix and report an unknown-prefix notice; a complete unknown sequence (a CSI tail, a UTF-8 scalar, a modified key) is exactly one unbound event and can never leave bytes in the pane.                                                                                                                                                                                                                                                   |
| Prefix + `c`, `n`, `p`, `1` … `9`, or `h/j/k/l`  | Leave Prefix; a connected Native Session executes the corresponding Tab or local Panel-focus action, while the unconnected fallback preserves shared state and reports the action as unavailable. No shell-fallback command is submitted.                                                                                                                                                                                                        |
| Prefix + `m`                                     | Leave Prefix and release or restore host pointer capture through the leased mode pair; while released the status line shows a persistent indicator and host-native selection and scrolling are unimpaired. Mouse capture, hit testing, pane passthrough, and the wheel fallback are governed by [native mouse input](docs/native-mouse-input.md).                                                                                                |
| Prefix + `Ctrl+B`                                | Leave Prefix and offer one literal `Ctrl+B` event to the terminal-input port.                                                                                                                                                                                                                                                                                                                                                                    |
| Prefix + `q` or `d`                              | Leave Prefix and close this Client shell.                                                                                                                                                                                                                                                                                                                                                                                                        |
| Prefix + `Esc`                                   | Cancel Prefix without changing an overlay. Outside Prefix, `Esc` and its following bytes are ordinary terminal input.                                                                                                                                                                                                                                                                                                                            |
| Resize                                           | Query the admitted stdout descriptor, update the fixed backend viewport, and recompute only the Client-local viewport class. Crossing into narrow mode hides the docked sidebar; returning wide closes a sidebar overlay and restores the prior wide-sidebar preference.                                                                                                                                                                         |
| Event/draw error, panic, or termination signal   | Stop the loop and attempt cursor, alternate-screen, and raw-mode restoration without starting or stopping another process.                                                                                                                                                                                                                                                                                                                       |

The invariant is that every prefix sequence produces at most one typed command, non-prefix bytes
remain ordered and byte-for-byte identical, a complete unknown sequence is one non-decision event
rather than a tail leak, the unconnected adapter never changes shared Workbench or terminal state, and every
possibly acquired terminal mode retains one bounded restoration owner. Unit tests cover state,
input-port, fixed-backend, layout, renderer, real-PTY same-terminal admission, an actual panic hook,
all four signals, cross-thread setup interleaving, post-read viewport/resize failures, and lifecycle
fault injection. Native PTY process tests separately prove that missing-cohort bare and help/version paths
leave terminal modes untouched, that Linux PTY-master `EIO` after the final slave closes is a normal
hangup, and that the linked public binary contains no `/dev/tty` or `tput` fallback token. These process witnesses describe the Unix adapter. The Windows adapter and its
separate console and ConPTY evidence are described above.

## Connected single-terminal TUI refinement

The accepted local-terminal milestone gives the private TUI an actual consumer of `LocalClient`:
`run_connected` receives an already admitted Client and explicit create-or-attach intent. It does
not select an endpoint, activate services, change public grammar, or publish shared Workbench facts.
A real process/PTY witness renders a Server/Worker/Host terminal through this adapter;
bare Linux startup enters the same prepared shell after bounded Site activation.

The following record retains the explicit private Create/Attach adapter and its regression tests.
Standalone startup uses the native Tab Session rather than this adapter's single-terminal queue.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | The shell has input and lifecycle ports but cannot display or control a retained terminal. Connect one Session while preserving responsive exit, bounded queues, exact feedback, and terminal lifetime.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Inputs       | Admitted LocalClient, optional default-shell cwd or an accepted create reference, inherited terminal geometry and raw input, untrusted OBCS/OBSR responses, and termination signals.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Data         | One connection-owning task, eight queued commands of at most 4,096 input bytes each, one response slot, one pending frame feedback, one bounded text projection, fresh request IDs, and one controller sequence. Host alone owns VT and history. Acquiring an existing terminal also submits its initial panel geometry; input and subsequent resize share the same increasing sequence.                                                                                                                                                                                                                                                                                                                                |
| Outputs      | Create/attach and explicit acquire, ordered admitted raw input and geometry, text/style/cursor rendering, exact applied feedback or NeedSnapshot, localized connecting/read-only/blocked/failure states, and Session-only cleanup.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Invariants   | No UI thread performs Server I/O. The task serializes every exchange. It never retries an effect. A queue-full input is explicitly unsubmitted. Unknown outcome closes the task and discards unsent commands. No task survives its TUI owner.                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Model        | Opening -> standalone snapshot selection where applicable -> attached controller or read-only viewer -> polling -> received record -> atomic text projection commit -> draw -> feedback -> polling. Control intent is queued separately from the single projection flight. Closing shuts down the admitted socket and joins the task; Server owns generation-scoped detach cleanup.                                                                                                                                                                                                                                                                                                                                     |
| Decisions    | Snapshot replaces all rows without moving the committed revision backwards, including after a projection reset; delta needs the exact attachment, epoch, sequence, generation, base revision, screen, and geometry. Invalid content requests a new snapshot without applying any row. Wrong envelope or scope retires the Session. ProjectionBlocked clears presentation until a later-revision snapshot. No controller auto-promotion, reconnect, or unknown-input replay.                                                                                                                                                                                                                                             |
| Dependencies | Existing OBCS and Rust Terminal Protocol bindings, bounded channels, POSIX socket shutdown, the admitted descriptors, existing Ratatui renderer, and localized copy. No new third-party dependency, direct Host path, or VT emulator.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Ownership    | `terminal_session` owns the connection task and input admission; `terminal_projection` owns Client text state and feedback; the Terminal Protocol binding owns bounded OBTF text decoding; the shell owns modes and draw ordering.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Verification | Canonical OBTF fixtures, malformed/boundary partitions, deterministic snapshot/delta and queue/retirement traces, render buffer assertions, and real process/PTY witnesses. Existing LocalClientSubmission and LocalTerminalRelay map transport retirement and projection/control ordering; their finite bounds are not performance guarantees.                                                                                                                                                                                                                                                                                                                                                                         |
| Limits       | Text cells and basic styles/cursor are projected; graphics, hyperlink activation, history, selection, terminal interaction-mode negotiation, cursor-shape parity, and full Workbench behavior are excluded. Unsupported cell widths use a replacement glyph. Missing or invalid inherited cell/pixel geometry rejects before Create or initial attach, or ends an active TUI through its restoration boundary; no pixel size is invented. Optional extension contents remain opaque and are not retained by the text decoder. The projection counts shared text allocations and rejects a retained text budget above 36 MiB before commit. Control characters in displayed cells never become terminal escape commands. |
| Review       | Checks cover memory amplification, at-most-once effects, failed draw feedback, and joined cancellation. The former authority-model timeout is resolved; startup has separate independent review and Linux process evidence.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |

`terminal_text` owns the forbidden display-scalar rule: Unicode control characters and
U+2028/U+2029. Terminal-cell rendering uses this rule with its existing width checks, and the final
backend `Print` independently replaces forbidden scalars with U+FFFD. Safe text is borrowed without
allocation; canonical strings remain unchanged. Captured-output tests verify that cell payloads
cannot inject terminal commands while preserving the backend's own cursor and style commands.

## Known-unchanged Client backpressure

The carrier retains one queued request and one wire owner; even an idle wire may briefly have a
full queue. Terminal Session owns a bounded response to exact viewer/control ServerBusy results:
each is a completed, correlated request that dispatched no Host action. It waits 10 ms, then makes
a fresh request with a fresh ID. Carrier Backpressure precedes OBCN dispatch; an Attach Worker
capacity rejection can follow an OBCN exchange but precedes Host attachment creation. Input and Resize also use a fresh increasing controller sequence,
because Server consumes the earlier sequence before queue admission. The logical operation retains
one absolute deadline, clamped to the original startup deadline before the first frame and to the
ordinary 50-second budget afterward. Each underlying LocalClient submission still writes once.

Create, Open and Start are never resubmitted. Generic rejection, controller contention, unavailable
runtime, malformed responses, transport failures and unknown outcomes are final. Feedback remains
the same exact owed feedback until accepted; this policy cannot acknowledge an undrawn frame.
Client cancellation closes the stream, interrupts an exchange, and joins after at most one 10-ms
backoff under normal scheduling. Expiry or identity exhaustion ends the operation.

| Claim                                                       | Refinement and evidence                                                                                                                                                                    |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Only known-unchanged capacity results allow another attempt | Server viewer/control ServerBusy producers establish unchanged Host effects; Client matches only that exact result. Fake Server partitions check busy, rejection, unavailable and unknown. |
| No possible effect is replayed                              | LocalClientSubmission still owns each once-only wire exchange; LocalClientBackpressure permits another submission only after a known-unchanged response.                                   |
| Deadline and identity do not reset                          | One operation deadline covers all attempts; request IDs and controller sequence increase; bounded fake Server traces exercise expiry and busy Resize.                                      |
| Retained feedback and cancellation stay bounded             | The Session keeps one active command and the existing eight-slot queue; fake Server tests and real concurrent Client traces cover preservation and joined drop.                            |

LocalClientBackpressure abstracts one viewer/control operation, up to three fresh request identities,
and a finite deadline. It trusts the ServerBusy-to-no-dispatch classification, exact response
validation and the existing transport model; it does not prove OS scheduling or grant new retry
authority to other callers or protocols.

## Localization and authority

Authored help and diagnostics ship in `en-US` and `zh-CN`. Locale selection checks nonempty
`LC_ALL`, `LC_MESSAGES`, then `LANG`; a `zh` language prefix selects `zh-CN`, and every other value
falls back to canonical `en-US`. Locale data affects presentation only and is never persisted or
used as authority.

Only help/version/usage one-shot routes use standard-library arguments, environment, and streams alone.
Bare Windows and Linux invocation composes the shared runtime, private Client transport, and TUI;
platform adapters own console and transport effects. The delivered TypeScript [`apps/cli`](../cli/README.md) remains
the shipped CLI authority until a separately accepted packaged cutover.

## POSIX local Site rendezvous foundation

Shared ownership and the complete primitive modeling record are maintained by
[local-site-runtime](../../crates/local-site-runtime/README.md); the application keeps no duplicate implementation.

## Accepted local Site activation modeling record

[`LocalSiteActivation`](formal/tla/LocalSiteActivation.tla) is maintained design evidence for the
accepted connect-first and lazy-activation workflow. This abstract model is independent of its later runtime refinement; the shared runtime record
owns concrete attempt markers, processes, lifetime locks, and first-frame deadlines.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Two concurrent local `orchbee` TUI invocations must coordinate through one Site activation attempt without either becoming the Server or Worker's lifetime supervisor. A present compatible Site should require no launch; an absent Site should start at most one exact installed cohort; every non-success outcome must be explicit.                                                                                                                                                    |
| Inputs       | Two Client invocation slots; TUI, help, and version intent; an initial Site observation of absent, compatible-ready, incompatible, or partial; one activation-lock result; the exact installed Server-plus-Worker cohort capability; typed readiness, incompatibility, partial-cohort, and launcher-failure observations; and an abstract monotonic total-deadline expiry. External process outcomes are untrusted and may never arrive.                                                  |
| Data         | Per-Client phase, typed result, and connection path; one optional activation-lock owner; a zero-or-one launch count and exact-installed cohort tag; Site and typed-readiness state; and a Site-owned ready-service lifetime marker. No PID, path, runtime-base, scope, ACL, process tree, cleanup record, recovery identity, service-manager unit, or numeric time value exists.                                                                                                          |
| Outputs      | A compatible present Site connects directly. An absent Site lets one lock owner start the exact installed cohort while a contender waits for typed readiness. Success connects and releases the caller's activation resource. Incompatible Site, partial cohort, launcher failure, and deadline expiry are distinct failures. Help and version complete without discovery or activation. Client exit ends only that Client.                                                               |
| Invariants   | Every TUI path attempts connection before activation; the activation owner is singular; the launch count is at most one and every launch is the exact installed cohort under the lock; a contender becomes a readiness waiter; only a ready compatible Site can produce a connection; settled Clients own no activation resource; ready-service lifetime belongs to the Site; failures and Client exit never stop or rewrite Site state; and help/version preserve every activation fact. |
| Decisions    | Invocation kind determines whether Site work is permitted. Connection observation distinguishes direct success, absent activation, incompatible failure, and partial-cohort failure. After absence, lock ownership distinguishes launcher from waiter; a lock winner rechecks Site state before launching. Typed readiness, typed failure, or the Client-owned total deadline settles each waiter. Deadline observation may win until connection completes.                               |
| Model        | A bounded workflow over exactly two Client slots and four supported initial Site classes. One scalar `activationOwner` models `activation.lock`; `launchCount` and `launchedCohort` expose singleton exact-cohort launch; `serviceLifetimeOwner` distinguishes the ready Site from every Client. External launcher/readiness actions are nondeterministic and have no fairness assumption.                                                                                                |
| Transitions  | `idle -> connecting -> connected` for a present compatible Site; `connecting -> failed` for present incompatible or partial state; `connecting -> lock-ready -> lock-held -> waiting-readiness` for the launch winner; `lock-ready -> waiting-readiness` for contention; waiting or recheck states settle as connected or one typed failure; `connected -> exited` preserves Site state. Help/version go directly from `idle` to information completion.                                  |
| Dependencies | The model composes the accepted command contract with the current private `LocalClient` admission boundary and `ModeCheckedLocalSiteRendezvous` lock semantics only at their named abstractions. Future runtime refinement must supply installation-owned cohort selection, typed readiness, and one monotonic total deadline. This slice adds no process, socket, filesystem, localization, or public-command implementation.                                                            |
| Ownership    | `apps/orchbee` owns the Client bootstrap workflow, deadline, activation guard, and typed Client result. Installation composition owns the exact cohort capability. Server and Worker own their readiness and service lifetimes after success. The model grants no Client authority to supervise, stop, replace, downgrade, clean up, or recover a Site.                                                                                                                                   |
| Verification | The registered TLC configuration explores four initial Site classes and exactly two Client identities, reaches every maintained semantic action, and checks safety plus Client-local deadline progress. It checks 850 distinct states to depth 13. Symmetry reduction is intentionally disabled because TLC liveness checking with symmetry can miss violations. Runtime refinement evidence is N/A for this model-only slice and remains required before wiring.                         |
| Correctness  | LA-01 through LA-05 below map the accepted claims to actions, invariants, and progress properties. `WF_vars(DeadlineExpires(client))` is the only fairness clause: it represents the contract-guaranteed local total deadline. `CohortBecomesReady`, typed cohort failures, and `LauncherFails` are intentionally unfair, so a permanently silent external process still yields a bounded Client failure rather than an assumed response.                                                 |
| Limits       | The abstraction proves only the checked two-Client, one-Site state space. It does not prove runtime refinement, operating-system scheduling, process creation or detachment, daemonization, service-manager behavior, runtime base or scope, ACLs, socket metadata, cleanup, recovery, retry, update, a concrete deadline or progress threshold, localization, TUI behavior, packaging, public wiring, or external process progress.                                                      |

| Transition owner     | Event and guard                                                      | Result                                                                                                                          |
| -------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Invocation           | Help or version                                                      | Information completion; Site, lock, launch, readiness, and service-lifetime facts are unchanged                                 |
| Client connection    | Compatible Site is already ready                                     | Direct Client connection; no activation resource or launch                                                                      |
| Client connection    | Site is incompatible or partial                                      | Corresponding typed failure; no activation or Site mutation                                                                     |
| Activation contender | Initial connection finds no ready Site and `activation.lock` is held | Wait for the same typed readiness result under the caller's total deadline                                                      |
| Activation winner    | Lock acquired, recheck still finds absence, no launch exists         | Start exactly one installed Server-plus-Worker cohort and wait while retaining the guard                                        |
| Activation winner    | Recheck finds ready, in-progress, incompatible, partial, or failed   | Connect, release and wait, or fail according to the exact observation; never start a second cohort                              |
| External cohort      | Launch is pending                                                    | May publish ready, incompatible, partial, launcher failure, or no result at all; no external completion is fair                 |
| Pending Client       | Typed result or total deadline is observed                           | Connect or return `incompatible`, `partial-cohort`, `launcher-failure`, or `timeout`; release any caller-owned activation guard |
| Connected Client     | Client exits                                                         | End only the Client Session; preserve Site, cohort, readiness, launch, and service-lifetime state                               |

This model abstracts runtime refinement; `LocalSiteStartup` supplies the concrete startup safety case.
Future wiring must add deterministic process tests before claiming the model maps to implementation.

| Claim                                                                                                    | TLA+ evidence                                                                                                                                                                                                                               | Bound                                                                                                                                                       |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LA-01: every activating TUI connects first; present compatible, help, and version paths never launch.    | `ConnectFindsCompatible`, all connection-observation actions, `CompleteHelp`, `CompleteVersion`; `ConnectionProbeNeverActivates`, `InformationInvocationHasNoActivationEffect`.                                                             | Models command intent and abstract observations, not parser, transport, or process refinement.                                                              |
| LA-02: concurrent absence produces at most one exact installed-cohort launch under one lock.             | `AcquireActivationLock`, `ActivationLockContended`, `LaunchExactInstalledCohort`; `ActivationOwnerIsUniqueAndPending`, `LaunchIsSingletonAndExact`, `LaunchStartsOnlyUnderTheActivationLock`, `ContenderWaitsForTypedReadiness`.            | Exactly two Client slots and one activation round; no runtime-base, lock-file, or process implementation claim.                                             |
| LA-03: incompatibility, partial cohort, launcher failure, and timeout are explicit and do not stop Site. | Direct, recheck, and readiness failure actions plus `DeadlineExpires`; `ClientShapeIsExact`, `ExplicitFailureHasNoServiceStopEffect`.                                                                                                       | Failure presentation, cleanup, recovery, retry, and stale-artifact policy remain excluded.                                                                  |
| LA-04: connection releases Client activation ownership; Client exit does not supervise or stop Site.     | `RecheckCompatible`, `ObserveReady`, `ExitClient`; `SettledClientReleasesItsActivationResource`, `ReadySiteOwnsItsServiceLifetime`, `NoClientSupervisesServiceLifetime`, `ClientExitPreservesSite`, `ActivationResourceEventuallyReleases`. | Models ready-service ownership only; it does not select how processes detach or how service lifetime is implemented.                                        |
| LA-05: every started TUI settles without assuming that an external process responds.                     | `EveryPendingTuiEventuallySettles` under only per-Client weak fairness for `DeadlineExpires`; external cohort and launcher completions have no fairness clause.                                                                             | Assumes the future adapter actually guarantees observation of one finite monotonic total deadline; no numerical value, OS scheduling, or external liveness. |

## Measurement

Collect the reference release baseline from the repository root:

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

The script builds the exact locked release package, warms and repeatedly invokes
`orchbee --version`, reports artifact bytes and warm latency percentiles, and reports peak resident
memory where `/usr/bin/time` exposes it. Results describe only the printed host, toolchain, profile,
and invocation. They are not acceptance budgets, cold-cache guarantees, or evidence for future TUI
performance.

The current Linux reference on 2026-09-07 used source commit `ac620e35`, x86_64 WSL2,
Linux 6.18.33.2, Rust 1.97.0, the default Cargo release profile and the same-cohort Zig ReleaseSafe
Host. Python 3.14.4 drove 5 version warmups and 50 version samples, plus a 120×32-cell,
960×640-pixel PTY over 5 fresh Sites and 3 reopens per Site. The isolated invocation environment
contained only account HOME/USER/LOGNAME, Cargo and standard executable paths, repository-root PWD,
`LANG=LC_ALL=en_US.UTF-8`, `SHELL=/usr/bin/zsh`, `TERM=xterm-256color`, and a unique
`ORCHBEE_RUNTIME_BASE`. The filesystem cache is warm; a fresh Site does not imply a cold OS cache.

Interface timing spans Client spawn to a complete draw containing `[*1 1]` and the bottom-row
`Tab 1 · Ready · controlling` footer, downstream of the full-frame draw, exact FrameApplied /
FeedbackAccepted exchange and latest-geometry UI Ready publication. It does not prove that the
worker has consumed the Ready reply or cleared the startup ceiling. Client VmHWM is sampled at
that endpoint: it measures peak residence through Ready, not the entire TUI lifetime.
Every close verifies exact terminal-mode restoration and alternate-screen exit. Before each warm
reopen, a condition wait outside the timed interval verifies the previous Server Session task and
socket have retired. Queued viewer Detach has no public completion witness; any remaining detach
work can enter the next attachment's measured latency. Earlier single-terminal references used
different readiness witnesses; these observations are not a strict before/after comparison.

| Measurement                                                      | Reference observation            |
| ---------------------------------------------------------------- | -------------------------------- |
| Client executable size                                           | 2,989,768 bytes                  |
| Version startup min / p50 / p95 / max                            | 0.791 / 0.834 / 1.176 / 1.406 ms |
| Version peak resident memory                                     | 2,818,048 bytes                  |
| Fresh Site complete Ready frame p50 / p95 (5 samples)            | 485.526 / 501.903 ms             |
| Warm Site complete Ready frame p50 / p95 (15 samples)            | 439.502 / 486.007 ms             |
| Client peak resident memory through Ready p50 / max (20 samples) | 4,792,320 / 4,894,720 bytes      |

These are host observations, not performance promises or direct comparisons with another OS,
driver or the old one-shot artifact. Remeasure and replace the reference when reachable dependencies
or startup paths change; supported-platform release acceptance budgets remain reserved.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-server -p orchbee-worker --locked
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
node scripts/tla/check-inventory.mjs
node scripts/tla/run-models.mjs --models local-client-admission,local-client-submission,local-client-backpressure,local-site-activation,local-site-startup,workbench-terminal-selection
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```

Set `ORCHBEE_TEST_NATIVE_COHORT` to an absolute staged directory to run the same real-process
acceptance suite against the actual release artifacts. Fixtures validate that cohort and copy it
into isolated directories; tests do not change the source cohort or default Site.

```sh
ORCHBEE_TEST_NATIVE_COHORT=/tmp/orchbee-native-dev cargo test -p orchbee --test local_startup_process --locked
```

### Agent integration conformance fixtures

`tests/fixtures/agent-integrations/` holds the corpora replayed by the
`agent_integrations` unit harnesses:

- `provider-conformance/` — `payloads.json` plus `cases/<provider>.cases.json`:
  per-provider inspection, enable, and disable cases pinning scripted command
  replies, expected argv, filesystem effects, and the rendered CLI verdict.
- `provider-effects/` — `cases.json`: filesystem-effect machine cases —
  capsule publish, move-aside, backup digests, remove walks, cohort and
  version gates, and budget or cancellation frontiers.

The harnesses resolve both corpora through `CARGO_MANIFEST_DIR`, so any
checkout replays them with no environment override or machine-local path.
A case edit is a semantic contract change and belongs in the same reviewed
change as the engine behavior it pins; a new provider adds one file under
`provider-conformance/cases/`.

## Standalone terminal intent

The private connected TUI accepts `TerminalIntent::Standalone`. It submits one
`submit_terminal_open` and requires Opened, then reads a correlated OBWP snapshot on that same
admitted connection. It uses the snapshot's exact Site/Worker/Server-terminal binding to submit
OBCS `attach-binding`; the Open create reference remains an explicit legacy value and does not
select this viewer. Bare Linux invocation consumes this path through verified local Site
activation and Worker lazy Host startup. Explicit Create and Attach intents keep their existing
Create-reference attachment behavior.

The pure projector requires exactly one context matching the standalone context ID, no groups,
and zero through 256 ordered, ungrouped, unpinned Tabs. Every root must be a Panel or a recursively
binary Panel split tree with no child Presentations and a bound primary Presentation at every leaf.
It retains both labels, all membership identities, split/slot topology and each binding exactly; one
unsupported member rejects the entire projection. Cold startup separately requires the first ordered
Tab and uses its binding. Canonical OBWP validation owns global uniqueness and reference integrity.
Pure refresh and created-member validators enforce revision, immutable member and correlated creation
facts. Interactive creation, switching, refresh, split-tree rendering and local Panel focus are
integrated; the native Session keeps the active leaf as its one attached viewer.

The terminal projection adapter also provides the native Session's controlling full-frame geometry
gate. A supplied expected geometry must match all four fields; mismatch clears the visible frame
and requests a snapshot without resetting sequence, revision, or blocked-revision fences. Native
controlling setup consumes this gate; the retained explicit Create/Attach adapter passes no expected
geometry. Focused tests cover each field and recovery with a fresh matching full frame.

Open, snapshot read, attach, controller acquisition, initial controller-authorized Resize and
the first validated terminal projection frame share the existing startup ceiling. An observer
does not resize. The snapshot exchange keeps one carrier and the existing nonblocking deadline
I/O owner; Workbench request IDs remain distinct from OBCS correlation and logical terminal IDs.

| Condition                                                                                                      | Carrier state and standalone behavior                                                                                    |
| -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Local encoding or definite pre-first-write failure                                                             | Retain the carrier and return a definite not-submitted failure; the workflow exits unavailable without retry.            |
| I/O after possible handoff; malformed, excessive, truncated, wrong-kind or wrong-correlation snapshot response | Retire the mixed carrier and report snapshot-read failure; an already successful Open does not become an unknown launch. |
| Valid denied or unavailable snapshot result                                                                    | Return the typed result without protocol retirement; the workflow exits unavailable and drops its Session.               |
| Valid snapshot with unsupported standalone shape                                                               | Exit unavailable without another selection or fallback to the Open reference.                                            |
| Known-unchanged viewer server-busy result                                                                      | Existing retry retains the exact binding tuple, uses a fresh OBCS request ID and keeps the same logical deadline.        |
| Unknown Open/submission or ambiguous attachment result                                                         | Stop without replaying Open, substituting Create or falling back to Create-reference attachment.                         |

There is no snapshot-read retry. Existing controller and projection admission still follow
attachment, and binding possession grants no controller authority. Equal encoded bytes across
distinct Site/Worker/Terminal identity domains remain valid; each field is independently typed
and validated.

The [Server-owned record](../orchbee-server/docs/workbench-authority.md#terminal-selection-from-workbench-bindings)
defines exact membership lookup, failure precedence, retained Worker generation and attachment
linearization. This Client neither owns the standalone terminal nor writes Workbench membership.
Session traces cover the mixed request order, exact selected tuple, structural refusal, bounded
I/O and absence of fallback; fresh release PTY tests cover first open and retained-shell reentry.
The Server's WorkbenchTerminalSelection model proves bounded selection safety, while existing
carrier and Client deadline models and runtime tests own progress. Remote access, restart
adoption, persistence and a full Workbench Client remain outside this slice.

The accepted [native Tab Session contract](docs/native-tab-session.md) owns the multi-Tab
creation, local selection and explicit refresh refinement, its shared-carrier barriers, and
the required implementation and process evidence. Its pure projection and classified TabCreate
adapter are implemented. The adapter distinguishes definite precommit, committed and unknown
outcomes; caller-deadline methods use the earliest ordinary, caller and startup deadline from one
shared injected clock. The deterministic Session state and bounded mailbox implement atomic
selection admission, Create/refresh barriers, exact event settlement, geometry gating, and carrier
retirement. Retirement preserves the first cause and distinguishes explicitly unknown outcomes
from definite failures. These kernels are integrated with the native connection executor and UI
consumers. Generated barrier tests, scripted-carrier traces, and the Linux two-Client Tab/refresh
process witness provide implementation evidence; the contract's
[claim/evidence matrix](docs/native-tab-session.md#claimevidence-matrix) records the checked claims
and their limits. This process evidence does not establish other operating systems, installation,
restart recovery or remote access.

`tab create` and `panel split` also accept per-launch `--cwd <absolute-path>` and repeated
`--env NAME=VALUE`, including with `--profile` or the default shell. An explicit cwd overrides
the Workspace directory. Windows omission follows the resolved source root process when available;
`--cwd workspace` keeps Workspace/Worker-home defaults. Environment values are literal
and bounded; see command help and [the native CLI contract](docs/native-cli-query.md).

Targeted Agent commands also accept `--panel <id|@panel:suffix>` or a positional Panel ID. Session IDs and names take precedence over positional Panel fallback. Panels select their primary terminal and require one live Agent; waits pin the selected Session.

## Windows explicit Site stop

`site stop --confirm` ends the existing managed Windows Site and its terminal processes. Use `--detach` for admission only and retain the returned ID for `site stop-status <id>`. Explicit retry requires `--retry --request-id <id> --confirm`. Cancellation or timeout does not cancel an accepted stop. A matching stopped record and free lifetime lock prove completion; the next `site start` consumes that record and creates a fresh Site. See the [CLI stop contract](docs/native-cli-query.md#windows-site-stop). No Linux/macOS stop runtime is implemented.

## Windows Worktree operations

`worktree create/open/remove/status/resume` now uses retained Server operations and Windows Worker Git execution. The [Worktree contract](docs/windows-worktrees.md) owns source selection, defaults, confirmation, partial outcomes and recovery. These commands do not activate a Site. Create/Open accept optional `--focus` and exact `--client`; default `--no-focus` keeps the view. List/Create/Open follow the validated caller or selected TUI context when source is omitted.

## Windows Workspace group Close

`workspace close <target> --group --confirm` closes the captured worktree group, source last, without deleting Git checkouts. Known main Workspaces with associated members require `--group`; linked Workspaces close only themselves. Use `workspace close-status --group --request <operation-id>` after members disappear, and `workspace close-resume --group --request <operation-id> --generation <attempt> --confirm` only for a declared resumable failure. Group mode supports detached/bounded waiting and reports each child Close identity and observed state. See the [owning model](docs/windows-worktrees.md#workspace-group-closure). `api schema` catalog v68 contains 76 documents including `orchbee.workspace.group-close` v1.

Windows CLI Panel/Tab/Workspace Close captures and settles every bound primary and child Presentation, including grouped/pinned Tabs. Canonical removal waits for all target disposal/cleanup and exact membership revalidation; unknown effects keep their retained receipts and recovery constraints. Unbound Presentations, child navigation and other structural operations retain their separate contracts; see the [Close model](../orchbee-server/docs/terminal-tab-close.md#complete-presentation-membership).

## Windows Client navigation

[Windows Client view control](docs/windows-client-control.md) owns `client list/get/control-status`, `workspace focus`, `tab focus`, `panel focus` and `panel zoom`. Navigation targets one current TUI via `--client` or unambiguous validated caller context. It submits once and waits for a drawn acknowledgement; timeout or Ctrl+C stops waiting only. Single-panel zoom has an explicit no-op receipt. Catalog v68 includes 76 schemas, adding `orchbee.client.view`, `orchbee.client.control` and `orchbee.cli.failure.client-control`, all v1. No operating-system window activation or Mac/Linux control runtime is added.

Windows process tests execute the emitted completion in Git for Windows Bash, found relative to `git --exec-path`; that installed Bash is required for this witness. Conflicting flags are checked before and after their command verb without contacting a Site. PowerShell has a separate real-shell grammar witness.

## Windows schema export

`api schema [<schema-id>] --output <path>` exports the embedded JSON document offline and also emits it on stdout. The destination parent must exist; a complete sibling file replaces an existing regular target. The [export contract](docs/native-cli-query.md#windows-offline-schema-export) owns failure stages, temporary cleanup and post-publication stdout failures. Catalog v68 contains 76 documents, including `orchbee.cli.failure.schema-export` v1.

## Windows Panel layout queries

`panel layout`, `panel neighbor --direction left|right|up|down` and `panel edges` inspect an exact Windows TUI Client. They accept a positional Panel, `--panel` or `--current`, and `--client` when routing is ambiguous. Reads preserve focus and terminal sizes, including inactive or unvisited targets. `orchbee.panel.layout` v1 retains cell geometry, nullable hidden rectangles, unzoomed neighbors, physical-edge results and the operation receipt. See the [geometry contract](docs/windows-client-layout.md). Catalog v68 contains 76 documents; the native command inventory contains 88 paths.

`panel focus [source] --direction left|right|up|down` finds one neighbor in the selected TUI's unzoomed cell layout and preserves zoom. No neighbor returns `direction-unchanged` without activating the source Tab. A found destination is pinned and returns `direction-focused` only after a successful draw. Receipt reads never repeat the movement.

`panel resize [panel] --direction left|right|up|down [--amount <number>]` adjusts one eligible split border, preserving focus and zoom. `layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` sets an exact binary split path. Both route to one Windows TUI and retain a `layout-adjusted` receipt. Selected changes wait for a draw; inactive changes store without navigation. Finite ratios clamp to 0.1–0.9, and finite resize amounts normalize to an absolute magnitude at most 0.5 (default 0.05). Ratios belong to that TUI session and are not restored after restart. See the [layout contract](docs/windows-client-layout.md).

## Shared Panel titles

`panel rename <panel-id> --label <text>` sets the shared primary Presentation override; `--clear` (or blank text) restores its creation label. It accepts an exact ID or `@panel:<suffix>`, with no focus or terminal effect. The [Panel label contract](docs/native-panel-labels.md) owns normalization, revision/replay rules and coordinated cohort compatibility. `orchbee.panel.rename` v1 distinguishes `renamed` from `unchanged`; its nullable `customLabel` echoes normalized request metadata. `panel get` and `panel current` v3 expose immutable `label`, nullable `customLabel` and effective `displayLabel`. `api snapshot` v4 preserves the canonical optional override. Catalog v68 contains 76 schemas and the native command inventory contains 88 paths.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf exchange, preserving Split/slot and terminal identities. The Windows CLI separately acknowledges source focus through the original Client; known canonical commits survive later failure or unknown focus. [Native Panel swap](docs/native-panel-swap.md) owns the workflow, receipt and evidence.

Windows `tab create` and `panel split` resolve `--cwd follow|home|current|workspace|<absolute-path>` before creation; follow is the default and samples the resolved source root process. Their v2 receipt records the chosen directory source/path. They accept optional targets and labels, with no focus by default; `--focus` requests post-creation focus, and split accepts a Client-local `--ratio`. Explicit targets can create without a TUI. `orchbee.creation.result` v2 separates committed creation from later ratio/focus results; a later failure is not permission to repeat creation. [Native terminal creation](docs/native-terminal-creation.md) owns target selection, total budget, concurrency guards and recovery for these Windows commands.

Windows `panel input (<panel>|--panel <panel>|--current) --right-click orchbee|panel` sets the shared right-button policy without moving focus. `--current` requires validated caller metadata; explicit targets bypass it. `panel split --right-click` commits the new Panel policy with creation. The default `orchbee` reserves right-click to the Client; `panel` permits unmodified right clicks in mouse-reporting terminal content. An admitted gesture retains its matching release after policy changes. See the [Panel input contract](docs/native-panel-input.md) for routing, pending-input rechecks and failure recovery. This does not add a context menu. `orchbee.panel.input` v1 reports `configured` or `unchanged`; `panel list` v2 and `panel get/current` v3 expose `rightClick`. `tab get` v2 and `api snapshot` v4 admit the optional canonical field. Catalog v68 has 76 schemas and the native inventory has 88 command paths.

Windows `tab move <target> --insert-index N` moves a complete Tab to a zero-based boundary in the
original order: 0 is first, the Tab count is last, source/source+1 are unchanged, and out-of-range
values are refused. It excludes `--direction` and preserves focus and terminals. Success uses
`orchbee.tab.move` v1; runtime failures use `orchbee.cli.failure.tab-move` v1. Catalog v68/76 keeps
the legacy direction envelope unchanged.

## Automatic Tab naming

The [Tab label contract](docs/native-tab-labels.md) owns optional `automaticLabel` metadata. New Tabs without a supplied label use their current context position; explicit numeric names and legacy Tabs remain fixed. CLI queries, label selectors and TUI display agree after move or close, and rename chooses a fixed name. Native `api snapshot` v4 exposes canonical metadata; catalog v68 contains 76 schemas. Shared protocol consumers require a matching cohort.

## Source-selected terminal output

Windows `terminal read` and `agent read` accept `--source visible|recent|recent-unwrapped|detection`.
Recent combines retained history with the live screen; detection reads the recent screen-height
range. `--plain` exports text directly for pipes, defaults to recent, and excludes JSON. Source-mode
`--lines` allows zero, clamps to 1000, and defaults to 80 for recent. It excludes the legacy
history/rows/unwrap flags, whose behavior stays unchanged. The [output contract](docs/native-terminal-output.md)
owns selection, exact sample validation and `orchbee.terminal.output` v1; catalog v68 has 76 schemas.

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
resources fail without a fallback or retry. See the [output contract](docs/native-terminal-output.md).

Windows `status server` reads actual Server build/protocol/cohort metadata before Client Session admission; `status client` checks this executable offline. Both accept `--format text|json` and never activate a Site. Bare `status` remains a Workbench query. The [runtime status contract](docs/native-runtime-status.md) owns the five-second budget, unknown metadata, compatibility, schemas and failures. Update-channel management remains unavailable.

## Agent navigation

Windows `agent focus <target> [--client <id>]` resolves a name, session or Panel to its unique live primary Panel before one Client control submission. Explicit `--session`, `--name` and `--panel` selectors use the existing resolver; `--request-id`, `--detach` and the 30000 ms total default timeout retain the common navigation recovery contract. Starting or Unavailable Clients fail with `client-not-ready` before Agent reads; Busy permits the Client-owned cancellable-mode preflight. The [Agent navigation contract](docs/native-agent-navigation.md) defines primary binding, ambiguity, cancellation and retained operation status. Successful explicit focus acknowledges current known Agent versions in the selected Tab even when the outer terminal is unfocused. Ordinary paint still requires host focus. `agent attention <target> [--client <id>]` reads that Client independently without marking it seen.

Windows host focus reporting is owned by [native Client attention](docs/native-client-attention.md); focus reports are Client metadata and never terminal text. The attention/CLI acknowledgement integration remains under development.

Windows `agent changes [--cursor <c>] [--limit <n>]` returns a bounded canonical change page or current-head checkpoint. The default limit is 50 (1–100); reuse the returned cursor. `reset-required` and `cursor-invalid` require explicit baseline recovery. See the [query contract](docs/native-cli-query.md#windows-agent-change-pages). `orchbee.agent.changes` v1 is available in catalog v68 (76 schemas, 88 command paths).

Cursor, Qoder and Antigravity session identity hooks are managed on Windows with `agent integration enable --provider cursor`, `agent integration enable --provider qodercli` or `agent integration enable --provider antigravity-cli`. Existing hooks and other configuration values are preserved.

Windows `agent integration enable --provider mastracode` installs session, run, tool, permission and subagent observations; partial owned hook sets can be repaired.

Windows `agent integration enable --provider droid` manages session identity hooks in the effective user configuration without shadowing existing fallback hooks.

Windows `agent integration enable --provider kimi` manages current Kimi TOML session hooks; working/blocked/idle parity remains open.

Windows `agent integration enable --provider letta` installs a silent SessionStart hook and keeps default conversations distinct per Agent.

Windows `agent integration enable --provider devin` manages six session identity hook groups in APPDATA/devin/config.json.

Windows `agent integration enable --provider hermes` publishes two owned Python plugin entries and enables them through the provider CLI; disable retains the plugin and its cache.

Windows `workspace create` accepts omitted Project, label and cwd, resolves source terminal/Workspace/home defaults, creates a Default Project only when none exist, and optionally focuses an existing TUI with `--focus`. Every Windows form launches its first terminal; explicit operands and no-focus use the same workflow and receipt. Project, Workspace and focus return separate receipts; Worktree Create/Open shares first-Project setup. See the [Workspace creation contract](docs/windows-workspace-creation.md). The current API catalog is v84 with 88 schemas and 99 command paths.

## Windows CLI configuration

`config check` also validates `APPDATA/Orchbee/config.toml`, with an absolute `ORCHBEE_CONFIG_FILE` override. `[terminal] new_cwd` sets creation cwd defaults; `[worktrees] directory` selects the checkout root. `[session] resume_agents_on_restore` controls Agent resume on the next Site start, default true; the exact true/false environment override wins. `config check` JSON v3 reports its effective value and source. Explicit command arguments override valid settings. See [configuration and precedence](docs/windows-cli-configuration.md). Windows CLI reads each invocation; common TUI/CLI activation reads startup policy before commitment. Direct TUI creation and Server reload are separate capabilities.

## Windows terminal observation stream

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` emits NDJSON from one existing binding without control or Host resizing. The [stream contract](docs/windows-terminal-streams.md) owns full safe ANSI frames, resets, flush-before-ACK, deadlines and cleanup. Default viewport: 120×40; default duration: unlimited. Both output formats emit `orchbee.terminal.stream` v2 records. Catalog v68 has 76 schemas and 90 native command paths. Machine control now accepts NDJSON input through `terminal session control`; interactive `terminal attach <terminal>` or `agent attach --name <name>` requires console stdin/stdout and uses the actual console size. Both accept `--takeover`, `--mouse on|off` and an optional total `--timeout`. `Ctrl+B`, then `q`, detaches; a double prefix sends one literal prefix. Paste cannot trigger detach. Output stays interactive with either global format. No Site start, Workbench focus change, reconnect or input replay occurs.

## Workspace metadata integration

The [Workspace metadata contract](docs/windows-workspace-metadata.md) defines report normalization, source sequence fences, TTL and the consistent read join. Its Server and shared wire owners are separate from canonical topology and terminal OSC metadata. `workspace report-metadata` and Workspace list/get consume that owner with bounded, correlated reads and unknown-outcome receipts; general events and configurable TUI token templates require their own integration.

Windows `panel report-metadata` supports source-owned presentation fields, guards, token patches, sequence and TTL with a binding-fenced, once-only receipt. See the [Panel metadata contract](docs/windows-panel-metadata.md).

Windows `panel report-agent`, `report-agent-session`, `clear-agent-authority` and `release-agent` submit explicit source-owned reports and return correlated dispositions. See the [Agent report contract](docs/windows-panel-agent-reports.md).
