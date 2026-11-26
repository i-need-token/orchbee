# Orchbee Native Entry

## Sessions

A Session contains your Workspaces, terminals and Agents; its background service keeps programs
running when you leave the interface. Start with the default Session and use Workspaces for daily
organization. Windows and macOS support separate named Sessions through `--session <name>` and
`session attach <name>`; `server` commands manage the selected Session's background service. The
[Session contract](docs/native-cli-query.md#sessions-and-background-services) defines terminology,
supported boundaries and unchanged storage/protocol identities.

Windows and macOS refuse a TUI launched from a terminal already inside the selected Session before
opening another Client. The localized message returns to the current Shell; CLI subcommands and explicit
routes to another Session remain usable. Failed and ended terminals retain navigation, creation
and close paths; [terminal settlement](docs/native-panel-lifecycle.md#terminal-settlement) preserves
final output and known exit codes. An interactive Shell that exits with code 0 instead closes its
Panel, and its Tab when it was the last Panel.

On Windows and macOS, `events subscribe` streams bounded observations from an existing Session: one
text line per record, or NDJSON records under `--format json`. Run `orchbee events --help` for
bounds and conditions; the [event contract](../orchbee-server/docs/windows-events.md) owns behavior
and evidence.

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` pastes one command and sends
Enter in one input request. The receipt confirms input submission, not Shell readiness, command
completion or exit status; never replay partial or unknown delivery. [Command submission
contract](docs/native-terminal-run.md).

On Windows and Linux, and in the Apple Silicon macOS development package, bare invocation connects
or activates a local Session from a verified executable cohort and enters its TUI after the first
validated terminal projection frame or a stable navigation view for an unavailable terminal. Native
Clients operate shared Workbench state while keeping independent selection and retained terminal
processes.

`agent prompt <target> <text> --wait` submits literal paste plus Enter after a fresh,
unblocked preflight. It observes activity before waiting for a condition and retains the
input receipt on later timeout or cancellation; it does not promise turn causality.

`agent start codex` starts an Agent on Windows or macOS in a new Tab beside the caller's Panel, or
with `--split right|down` in a new split, or with `--here --panel <id>` at the confirmed prompt of
that idle terminal. See [Windows Agent startup](docs/windows-agent-start.md) for setup, limits and
recovery.

Windows `channel show [--format text|json]` reads the strict stable/preview preference from the shared user configuration without activating a Session. See [configuration and channel provenance](docs/windows-cli-configuration.md#rule-update-policy).

Windows, macOS and Linux `machine list|add|rename|remove|enable|disable` manage saved remote machines, named by ID or exact label; `add` checks the new machine through the background machine hub and saves it only once it is online, keeping that connection; `remove --stop-session --confirm` stops the remote Session before removing the record; `machine connect|disconnect` connect a saved machine through the hub and end that connection, and `connect --remember` remembers a typed sign-in answer in Credential Manager on Windows and in the login Keychain on macOS; `add --no-install` is parsed and refused as not yet available, and help and completion leave it out. `workspace set-machine <workspace> --to <machine|local>` moves a Workspace to another machine by creating it there and closing it where it ran; files are not moved. See [remote machines](docs/machines.md) for the catalog, partial outcomes, command routing, changing a Workspace's machine, and live TUI switching.

Windows `install` publishes the signed release directory the running Client sits in into the store's
`versions\<version>` and makes it the active version through `active.json`, then performs the PATH
step unless `--no-path` is given; an install made with `--no-path` is recorded in the store, so a
later `update` also leaves the PATH step off. Help and completion leave `install` out; it stays
callable. `update` reads the signed Orchbee update feed, or the one `ORCHBEE_UPDATE_FEED` names,
says it is up to date when the channel offers nothing newer before fetching anything large, and
otherwise downloads the one archive the feed names, checks its signed size and SHA-256, unpacks it
and installs it beside the running one with the same verification, immutable publication and
activation; `channel set` saves a channel first. See [Windows release
installation](docs/windows-cli-configuration.md#windows-release-installation). A release build
withholds the whole `channel` family; see [release-withheld surfaces](#release-withheld-surfaces).

On macOS and Linux `install` and `uninstall` work the same way, with the command link `~/.local/bin/orchbee` as the PATH step (on macOS also two lines in the shell's startup file that put `~/.local/bin` on `PATH`) and the store in `~/.local/share/orchbee`. macOS also runs `update` and the automatic helper, downloading with the system `curl` and unpacking with the system `tar`; nothing downloads on Linux yet, and `channel set` runs only on Windows. See [macOS and Linux release installation](docs/windows-cli-configuration.md#macos-and-linux-release-installation).

The hidden `--create-window-folder`, which Desktop Main runs through its bundled Client on every platform while `<data root>/internal/desktop/` is missing, creates the data root, `internal/` and that folder with `create_private_directory`, the rule by which every Client creates its folders, so the window never makes a folder its Session would refuse. After the data root's layout admission it prints nothing and exits `0`, or answers the shared JSON failure document `check-failed` `{stage:"data-root"}`, whose `refusal` names the item and the rule it breaks when the Client refused one. Help, completion and the `--skill` guide leave it out, and its grammar never changes.

Windows `uninstall [--delete-data]` is the inverse: it stops the environment's Sessions, disables
every Agent integration, removes the user `Path` entry, the installer entries and the store root,
and removes logs, caches and runtime state from the data root; settings and Sessions stay unless
`--delete-data` is given. See [Windows release
uninstall](docs/windows-cli-configuration.md#windows-release-uninstall).

The [TUI platform boundary](docs/tui-platforms.md) defines the shared Client implementation,
optional OS adapters, localized unsupported outcomes and native-platform evidence. State, input,
menus and rendering compile together on Windows, Linux and macOS; optional Windows host effects
remain explicitly unavailable until their platform adapter exists.

## Command surface

Every `ORCHBEE_*` variable Orchbee reads or sets, with its level, is in the
[environment variable table](docs/environment-variables.md).

Bare and explicit `tui` invocation first admit the inherited interactive terminal and valid cell
geometry, then run the local startup workflow below on Windows, Linux, and macOS Apple Silicon. They
are the same Client route; `tui` does not select a different mode. `--help` and `--version`
initialize neither TUI nor service. Commands use the exact grammar below without abbreviations;
unknown commands, undeclared options, extra operands, and non-Unicode arguments are usage failures.
Windows, macOS and Linux remote selection follows the [saved-machine routing
contract](docs/machines.md#command-routing-contract). A release build withholds some surfaces; see
[release-withheld surfaces](#release-withheld-surfaces).

```text
orchbee                          # local terminal (TUI)
orchbee tui                      # the same terminal, explicitly
orchbee status                   # the running Session's counts and processes
orchbee <family> <command>       # queries and Workbench mutations
orchbee api snapshot|schema      # machine snapshot and embedded schemas
orchbee server start               # ensure the local Session is running (no TTY)
orchbee server stop --confirm      # stop the Windows Session (release: type its name instead)
orchbee doctor                   # check whether Orchbee works here
orchbee operation status <id>    # inspect a retained operation by its request ID
orchbee config check             # local runtime inspection
orchbee diagnostics create --output <path>  # diagnostic bundle to send for help (Windows, macOS)
orchbee help [<family>|<command>]
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --help | --version
```

Windows and macOS `panel create-terminal` starts an unbound primary or exact member, or appends a
child without splitting the Panel. See [Panel terminal creation](docs/windows-panel-terminals.md).

`server start` is the headless activation path: it runs the same connect-or-activate sequence the
TUI startup uses — validate the cohort, prepare the runtime directory, take the locks, commit the
marker, spawn the Server when no running Session answers — under one absolute 20-second deadline,
then reports `started` only when this invocation, or the release store's Client it handed the start
to on Windows and macOS ([store
start](docs/windows-cli-configuration.md#windows-release-installation)), actually spawned the child
and `already-running` when an existing Session admitted its probe. `workspace create`, `project
create`, `worktree create` and `worktree open` run the same start first when no Session answers and
print its started line on standard error. The command is effectful even on refusal: only a grammar
failure is guaranteed zero-effect, an interrupted or timed-out start may leave committed activation
state. Recovery preserves checkpoint data and never terminates old processes. The [Windows recovery
contract](../../crates/local-runtime/docs/windows-crash-recovery.md) owns those boundaries. `doctor`
is the read-only complement: its runtime folder check reports bounded facts about the runtime
directory, marker, locks, and finite endpoint paths — each a closed-state row at its own observation
instant — under one cooperative 5-second budget, and it never creates, locks, repairs, or claims any
process is alive or gone. `server connect` is the byte relay for a Client that cannot own local IPC,
such as the Desktop shell: it runs the same connect-or-activate sequence, admits this process
through OBLC, then copies the connection's bytes between its standard streams until either side
ends. Both streams must be pipes, and it takes no `--format` because stdout carries raw protocol
bytes. It neither decodes those bytes nor reconnects; the parent owns the protocol and any new
connection. Help and completion leave it out because Client shells, not people, run it.

The complete grammar, target and
filter rules, mutation outcomes, and the failure matrix with exit classes are owned by
[native CLI query](docs/native-cli-query.md); `orchbee help <family>` and
`orchbee help <family> <command>` print the same grammar from the binary itself.
For an end-to-end headless sequence — start a Session, create a
Project/Workspace/directory, launch and drive a terminal, inspect worktrees,
observe Agents, and close cleanly — see [native CLI workflows](docs/cli-workflows.md).

Common command templates, in the order one session usually needs them —
follow the linked workflow to create the Workspace and terminal first, then
substitute each `<…>` placeholder with the real id or provider:

```sh
orchbee server start                        # activate the local Session once (no TTY)
orchbee status                            # is the Workbench reachable?
orchbee project create --label demo       # then workspace create / set-directory
orchbee tab create --workspace <ws> --label shell
orchbee terminal send <terminal-id> --text 'make test' --enter
orchbee terminal wait-output <terminal-id> --match 'PASS'
orchbee agent list                        # sessions the running Session observes
orchbee agent wait <session-id> --until ended
```

The Apple Silicon macOS development cohort runs the same Session-backed Agent commands as Linux:
`agent list`, `get`, `read` and `wait`, and `agent prompt`. macOS also runs `agent start`, which Linux refuses as unsupported. Like Windows,
macOS reads them through the unified Agent projection, so physical terminal Agents, `@agent:`
selectors, the per-Client projection (`--client`) and `agent focus` work there; Linux keeps the
canonical sessions only.
`SIGINT` and `SIGTERM` cancel these commands as on Linux, with the typed cancellation document at
exit 130, and `terminal send --file` and `--stdin` read their declared source on macOS too.

Session concepts and onboarding stay in this guide rather than introducing a separate help section.

Help writes stdout, failures write stderr, and version writes one locale-neutral `orchbee <version>`
line.

## Native Windows quick start

`workspace create --cwd C:\existing\repo` creates an ungrouped Workspace and its first terminal in
one request. The receipt returns resource IDs and the terminal outcome; a failed launch retains the
Workspace for inspection and explicit close. See the [Windows
workflow](docs/cli-workflows.md#windows-directory-workspace) for PowerShell examples.

Build with the repository's pinned Rust and Zig toolchains and Node.js. Run this from the repository
root in PowerShell; the output directory must be new, and its parent must already exist:

```powershell
node scripts/stage-local-runtime.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
```

Staging prints the Client path, for example `...\OrchbeeNative\ob-dev-<16 lowercase hex>.exe`; run
it in Windows Terminal. Development and test Clients stay off `PATH`, since staging changes no
`PATH`: run one by the path staging prints, or let `pnpm desktop` stage and start its own. The
directory contains the four exact `.exe` siblings, the Windows launcher `<stem>-launcher.exe` that a
release installs as `bin\<stem>.exe`, the window launcher `<stem>-desktop-launcher.exe` that a
window version carries for its entry ([its contract](../orchbee-desktop-launcher/README.md)), a hash
manifest, and the observation-adapter capsule. Native Windows staging defaults to this checkout's
development stem, `ob-dev-<16 lowercase hex>`, the same one `pnpm desktop` uses, so a development
cohort never shares image names with the installed release. `--artifact-stem orchbee` stages the
release names and `ob-test-<16 lowercase hex>` a test cohort. A plain `cargo build` or `cargo test`
compiles this checkout's test stem, `ob-test-<same 16 hex>`, on every platform, so its binaries
never act on the release installation; on Windows Cargo still writes them as
`target\...\orchbee*.exe`, and they find their siblings only in a cohort staged under the stem's
names. Windows Terminal is the interactive host; WSL is not required. A Session started from an
administrator terminal admits only administrator Clients. Orchbee in a standard terminal says so and
names the way out, `orchbee server stop` in an administrator terminal; see the [failure
matrix](docs/native-cli-query.md#failure-matrix).

On a Session with no Workspace or terminal content, `cd <project>; orchbee` creates
an ungrouped Workspace named after the current directory and its first terminal in that directory.
Later launches restore the last displayed Workspace, Tab and Panel on Windows and open the first
Workspace on Linux and macOS; launching from another directory does not add a Workspace. See [directory startup](docs/native-directory-startup.md)
and [startup position](docs/native-context-selection.md#windows-startup-preference).

The prefix is `Ctrl+b`, followed by `|` for a side-by-side split, `-` for a stacked split, `h/j/k/l`
for Panel focus, or `d` to detach. Run the executable again to reattach to the retained Session.
Detaching leaves shells running. Continuous Workbench checkpoints retain layout changes and sampled
terminal directories without screen contents on Windows and macOS; Linux keeps none yet. See the
[Workbench checkpoint contract](../orchbee-server/docs/workbench-checkpoint.md).

After attachment, the TooSmall recovery surface withholds Panel geometry observations instead of
reporting that every Panel departed. Terminal viewers, controller grants, selection and the last
valid child geometry survive widths below 20 or heights below 5. Restoring a drawable viewport
resumes normal observation and navigation without detaching; the 20×5 Minimal boundary still
reports real Panel geometry. This applies with split Panels and notification cards as well.

The Agent integration writers for Windows and macOS are dormant in this release: no command or
setting enables them, an integration an earlier version enabled stays as written, and uninstall
still removes the entries Orchbee wrote. See the [Windows integration
contract](docs/windows-agent-integrations.md) and the [macOS integration
contract](docs/macos-agent-integrations.md). Windows x64 cohorts include the pinned ConPTY runtime
needed to preserve console correlation. `terminal send --stdin` accepts redirected regular files and
pipes; on Windows an owned hidden Windows PowerShell reader makes pipe input cancellable. It is
resolved from the OS system directory, never from PATH, and is terminated/reaped on failure. Shell
completion supports bash/zsh/fish/powershell/elvish. In PowerShell, load it explicitly with
`Invoke-Expression (orchbee completion powershell | Out-String)`; no profile is changed. In
interactive Elvish 0.21, use `eval (orchbee completion elvish | slurp)`; the static callback
registers the running executable's own name, with and without `.exe`, without querying a Session or
modifying profiles. A development or test Client therefore registers its own `ob-dev-` or `ob-test-`
name and never takes over the release `orchbee` completer. For Unicode pipelines in Windows
PowerShell 5.1, set `$OutputEncoding = [Text.UTF8Encoding]::new($false)` in the invoking script;
PowerShell chooses the bytes sent to a native command. Orchbee preserves those bytes and does not
repair prior shell encoding loss. `orchbee --skill` prints the embedded [Agent
guide](docs/cli-agent-skill.md) offline, or its [`zh-CN` product
copy](docs/cli-agent-skill.zh-CN.md) under that locale.

## macOS and Linux local startup

Build a new development cohort from the repository root:

```sh
node scripts/stage-local-runtime.mjs --output /tmp/orchbee-native-dev
# From a Windows host using WSL Linux:
node scripts/stage-local-runtime.mjs --wsl Ubuntu --output /tmp/orchbee-native-dev
# On Apple Silicon macOS:
node scripts/stage-local-runtime.mjs --output /private/tmp/orchbee-native-dev
```

Then run the staged Client, named by the environment stem, in an interactive terminal; the script
prints its path, for example `/tmp/orchbee-native-dev/ob-dev-<16 lowercase hex>` on Linux.
The script rebuilds three Rust release binaries and the Zig ReleaseSafe Host from locked inputs,
on macOS also the window launcher `<stem>-desktop-launcher`, which no `cohort.json` role lists
([its contract](../orchbee-desktop-launcher/README.md#macos)),
copies the canonical observation-adapters capsule as the cohort's lazily validated sidecar,
and verifies its declared payload digest before writing the manifest;
existing output directories are rejected. `--cargo-target-dir <absolute-directory>` can reuse a
Cargo build cache. This stages a development cohort without installing a system service.
The binaries compile this checkout's development stem, `ob-dev-<16 lowercase hex>`, as their
environment, name their files `ob-dev-<16 lowercase hex>`, `ob-dev-<16 lowercase hex>-server`
and so on, and keep their state under `~/.ob-dev-<16 lowercase hex>`; `--artifact-stem` selects
`orchbee` or an `ob-test-` stem as on Windows.

The default Session is `default`, kept in `<data root>/internal/runtime/default/`, with its sockets
in `/tmp/<stem>-<effective-uid>/` on Linux and `/private/tmp/<stem>-<effective-uid>/` on macOS.
Development/tests can isolate a Session by setting `ORCHBEE_RUNTIME_BASE` to another existing
absolute directory. A folder that breaks a privacy rule stops startup with a message naming it and
how to fix it. The [shared runtime contract](../../crates/local-runtime/README.md) owns the 0700
directory, 0600 locks/sockets/immutable attempt marker, exact four-executable cohort with its
declared adapters sidecar, shared 20-second deadline, and delayed 500-ms progress notice.

The Client never restarts or adopts a failed Session. Failed/residual runtimes explicitly require
recovery and are not deleted automatically. `Ctrl+b q` or `Ctrl+b d` closes only this Client;
reopening attaches to the same retained shell. Additional concurrent Clients are read-only unless
they acquire an independent controller grant. On Windows, the Panel menu's take-control row
explicitly takes control of the current Panel and supplies this window's Panel size; ordinary typing
and clicks never take over. The previous Client keeps viewing and confirms read-only on its next
input or resize interaction; see [native controller takeover](docs/native-control-takeover.md).
Terminals must report real valid cell/pixel dimensions; a paired `(0,0)` pixel report is admitted as
an honest unknown-pixel declaration, and missing pixel values are never guessed. When the reported
geometry is not drawable — including a temporary zero-cell report — the Client waits for a real
drawable geometry inside the original deadline instead of failing admission or inventing one; the
[native startup wait](docs/native-startup-wait.md) owns that bounded waiting model, its
cancel/timeout/fault outcomes, and its no-service-work guarantee.

## Native Tab interactions

The Workbench Client renders the active context's ordered Tabs and canonical labels. Each Client
owns its own active selection, navigator and viewport; another Client's committed Tabs become
visible through the automatic publication probe, and explicit refresh remains available.

[Context selection](docs/native-context-selection.md) preserves each retained terminal while
switching location. Workspace identity is independent of its optional Project. [Workspace
setup](docs/native-workspace-setup.md) immediately creates an ungrouped Workspace and its first
shell in the current directory of the focused Workspace's first terminal, else that Workspace's
directory, or the launch directory when there is none; Choose directory for new Workspace… opens the
editable picker instead. Known partial results remain inspectable with `orchbee workspace list`;
unknown outcomes never replay, and late completion cannot override a newer selection. [Worktree
launch](docs/native-worktrees.md) adds a peer Workspace, reuses the source's current Project or
atomically groups an ungrouped source with its new peer.

Windows `Ctrl+b Space` opens [action menus](docs/native-action-menus.md). Git discovery uses the
first terminal's root cwd; a Project derives that source from its first canonical member rather than
owning a directory. Only authoritative never-created/empty runtime proof skips confirmation.

`Ctrl+b Shift+D` opens the Windows [Workspace Close confirmation](docs/native-workspace-close.md).
[Location labels](docs/native-location-labels.md) use a prefilled editor and shared Server commands;
renaming preserves the running terminal and becomes visible to peers through the publication probe
or explicit refresh.

On Windows, drag a visible Panel divider with the plain left button to resize Panels and controlled
terminal viewports as the mouse moves, then release to apply one shared, saved ratio. [Divider
resizing](docs/native-divider-resize.md) shares the keyboard adjustment's acknowledgement and leaves
terminal control unchanged.

On Windows, unmodified dragging in a Panel without application mouse tracking selects live or
retained-history text; double press selects a word and release copies the complete Host export to
the system clipboard. [Selection and effects](docs/native-terminal-selection-effects.md) owns the
clipboard dependency and lifecycle; Panel-program writes, OSC 52 mode, and clipboard configuration
remain to be connected.

[Custom commands](docs/custom-commands.md), on Windows and macOS, share one installed catalog between
`command list`, `command invoke`, configured shortcuts and shortcut discovery. Shell jobs, Panel
creation, independent popups and plugin actions retain their existing execution owners and receipts.

Bracketed paste is atomic and lands as one edit — prefix, bracket, and newline bytes inside the
payload never escalate to commands. The [Workbench Client runtime
contract](docs/workbench-client-runtime.md) owns admission, geometry, feedback, failure, recovery
and evidence bounds. Panel commands are Workbench mutations admitted under the Server's
`WorkbenchGrant` capabilities — `mutate_navigation` and `create_terminal` for split,
`mutate_navigation` for move, `close_terminal` for close ([Workbench
authority](../orchbee-server/docs/workbench-authority.md)) — a layer independent of the
terminal-side controls: a viewer attachment grants neither input nor resize authority — its
`read`/`input`/`resize` capability flags scope that attachment itself — and input and resize flow
only through the terminal's single combined input-and-resize controller grant, bound to one Session,
viewer, Worker generation, and Host terminal
([`schemas/client-server/README.md`](../../schemas/client-server/README.md)). Terminal input or
resize control never authorizes a Workbench mutation, and a read-only attachment cannot input or
resize regardless of any Workbench grant.

## Application crates

The `orchbee` binary is the entry layer: process entry, command-line grammar and output, the
`doctor` checks, startup and release installation, and the terminal shell loop and backend.
Two application-private crates sit below it and never depend on it:

- [Workbench Client](crates/workbench-client/README.md) is the Client runtime: Session activation,
  projected Workbench state, views, input dispatch, terminal presentation, Agent and machine
  workflows, and localization.
- [Local client](crates/local-client/README.md) is the Client Connection transport beneath that
  runtime.

Modules inside the binary follow the [application module layers](docs/module-layers.md), which
apply the Workbench Client layer model.

## Private local Client transport

On Unix, the application-private `LocalClient` accepts an endpoint and executable cohort only from
an internal caller. It delegates admission to `orchbee-local-client-protocol`, which validates the
cohort before connecting, proves the Server has the same effective operating-system user, performs
the exact `OBLC` handshake with two-second read/write timeouts, and returns the accepted generation
plus the same unbuffered stream. On Linux and macOS, private `LocalClient::connect_before` instead
passes the caller's absolute monotonic deadline through connection, peer proof, hello, response and
final admission. Its [owning adapter and correctness
record](../../crates/local-client-protocol/README.md#deadline-bounded-posix-admission) define
failure cleanup and the unchanged-deadline guarantee. Both paths switch the admitted stream to
nonblocking I/O before `OBCS`. Ordinary requests have an independent budget; initial local requests
are clamped to the original startup deadline.

`submit_terminal_open`, `submit_terminal_create`, `submit_terminal_start`, `submit_terminal_viewer`, and `submit_terminal_control` each take ownership of one already validated
typed request and encode its complete OBCS frame before submission. Encoding failure is definite
and leaves the Client Connection available. A successful encoding starts one fifty-second monotonic deadline
covering the complete frame write and the one Server-frame read; readiness waits recompute the
remaining budget, so partial progress cannot renew it. Failure before the first write syscall is
definite. Once that syscall begins, any write or read failure or deadline expiry, malformed or
incompatible result, wrong result kind, or wrong correlation makes the effect outcome typed
`OutcomeUnknown` and retires the connection. The one exception is a multiplexed lane whose Server
closed that request's stream as `outcome-unknown`: the outcome is still typed `OutcomeUnknown`, but
the doubt stays with that request and the lane keeps serving. The transport never retries or
returns the request for replay. A correctly correlated Server `outcome-unknown` result remains a
valid OBCS result rather than a transport ambiguity.

Bare Linux and macOS startup consume this transport for canonical Workspace selection, viewer and
control intent. Its close-only `LocalClientShutdown` handle stops a blocked exchange without
acquiring another stream reader or writer.

The fifty-second submission bound accommodates Server's one queued and one in-flight Worker exchange
(each at most two seconds writing plus twenty seconds reading) and Client result delivery. Viewer
submission validates action/outcome compatibility and exact correlation, and preserves opaque OBSR
bytes for the connected projection consumer; it grants no controller authority and performs no
terminal presentation decoding. Controller submission validates exact grant acquisition, input
acknowledgement, and complete resize geometry against its action. Server-reported unknown control
outcome is returned without retry; mismatched outcomes retire the transport. The Workbench Client
reads a control request the Server closed alone as `outcome-unknown` as that unknown answer, so
initialization controls (`acquire`, `host-colors`, setup/catch-up `resize`) degrade to view-only
without retiring and ready `input`, `resize` and `clear` settle at their pane; shared `LocalClient`
workbench/agent helpers likewise preserve a multiplexed carrier when only one request's stream
closes as `outcome-unknown`. The caller owns fresh request identities, increasing grant-scoped
sequences, and any explicit later acquisition. Cleanup after Client Connection loss may outlive a
submission deadline.

All existing-viewer actions and all controller actions carry the exact Server-issued `attachmentId`.
The native executor retains that identity immediately after Attach, before setup can fail or be
superseded. Poll, control, projection feedback and cleanup use that captured identity; bounded busy
retries preserve it while refreshing only the permitted request identity and control sequence.
Projection feedback is checked against its owning attachment before submission, never retargeted.
`LocalClient` stores no implicit current viewer. In a connected Workbench Client every visible leaf
of the active Tab's canonical panel tree owns its own viewer attachment — a stable logical
`PaneKey` ({selection anchor, `WorkbenchPanelId`}) whose physical attachment carries a `PaneRef`
incarnation minted per record. Input and resize admit only the focused pane's exact incarnation,
while polls continue for every admitted pane incarnation so background leaves keep rendering their
own frames. Same-Tab `h`/`j`/`k`/`l` focus movement is a local selection change — it never
detaches, re-attaches, or re-tags work already accepted — and cross-Tab selection still runs the
existing switch cleanup/attach chain. A detached-and-rebound pane fences every late input, frame,
feedback, and settlement tagged with its retired incarnation, and viewer capacity stays bounded by
the shared HostViewers limit (8).

Worker completion wakes the active shell's input wait, and a Server hint polls a pane at once.
Revisiting a Tab can immediately paint its own retained live text at the same
cell dimensions from an 8-image / 16 MiB accounted cache, while fresh attachment readiness still
gates input and feedback. First visits and cache/geometry misses keep the normal first-frame path.
See [presentation scheduling and retained pixels](docs/workbench-client-runtime.md#presentation-scheduling-and-retained-pixels)
for identity, lifetime, cache accounting and controlled timing evidence.

## TUI shell foundation

Application-private shell adapters own the following terminal and presentation boundaries.
Bare Linux and macOS startup use the prepared Workbench Client; unconnected `run` and explicit `run_connected`
remain test and composition seams.

| Concern     | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admission   | The staged Unix adapter requires inherited stdin and stdout to be TTYs with the same terminal-device identity, duplicates only those admitted descriptors, and reads cell and pixel dimensions from the duplicated stdout descriptor. It has no `/dev/tty`, global Crossterm size, or process fallback.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Lifecycle   | One staged guard owns raw mode, the alternate screen, and cursor visibility. It marks cleanup responsibility before an operation may partially succeed, retains each flag until its inverse succeeds, and attempts each failed inverse at most twice before returning one aggregate error. A process-lifetime singleton panic hook dispatches only when the panicking thread is the active guard's owner, so another thread cannot clear a setup lease before its terminal state is saved; guard drop does not remove a later hook. Normal return, initialization/event/draw failure, owner-thread panic, and each of `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` have deterministic restoration or signal-observation evidence.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Client View | One invocation owns prefix mode, viewport class, sidebar projection, active overlay, and transient notice. It owns no shared Tab, Panel, Workspace, Session, or Terminal fact and persists nothing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Layout      | The [shell layout](docs/tui-shell-layout.md) owns viewport tiers, bounded sidebar width and terminal geometry. On Windows, the sidebar nests Agents under their exact Workspace owner, including nonempty legacy content in one scrollable tree; disclosure preserves terminal selection and keyboard ownership. Tab-strip and sidebar `+` controls reuse existing create actions and menus. Canonical Panel identities and terminal ownership remain unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Input       | The staged adapter polls and reads at most 4,096 raw bytes at a time from the admitted stdin descriptor. A bounded legacy decoder turns the stream into typed items — a UTF-8 scalar, a CSI/SS3 or kitty-`u` key, an atomic bracketed-paste run, or one non-decision sequence. Each collecting frame (ESC wait, CSI/SS3, UTF-8 scalar, paste run) owns its bytes across reads under a bounded injected wait; a frame opened inside a captured packet keeps that owned provenance even when it completes in a later read, an over-bound or timed-out frame drains to a reliable boundary instead of leaking its tail into bindings, and a paste run pairs only with the terminator matching its opener's notation. The legacy decoder emits exactly one rejection for invalid UTF-8, over-bound and timed-out pasted runs with no retained raw payload, then drains silently to that terminator; the pane receives no rejected fragments and an armed prefix remains armed. Attach and text editors retain their existing refusal feedback; ordinary pane and prefix dispatch add no global notice. Every completed read is dispatched in order before any subsequent signal, viewport query, or resize decision can terminate that loop iteration. `Ctrl+b` enters visible prefix mode. `b`, `w`, `q`, and `d` drive local shell state or exit, `Space` and `?` open the action menu; connected `c`, Tab selection, and `h`/`j`/`k`/`l` actions are handled by the Workbench Client. When no named owner or exclusive capture owns input, ordinary bytes—including `Ctrl+C`, `Ctrl+D`, escape sequences, UTF-8—reach an injectable terminal-input port byte-for-byte in order; a lone `Esc` forwards only after a bounded wait, and prefix then `Ctrl+b` forwards one literal byte when the current owner admits it. Captured overlays may admit configured prefix bindings to close or switch surfaces after modal and confirmation owners. |
| Outcome     | The shell consumes its prepared Workbench Client. Bootstrap owns activation; Server owns terminal identity; Host owns terminal lifetime. The shell does not mutate Workbench state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

The invariant is that every prefix sequence produces at most one typed command, non-prefix bytes
remain ordered and byte-for-byte identical, a complete unknown sequence is one non-decision event
rather than a tail leak, the unconnected adapter never changes shared Workbench or terminal state,
and every possibly acquired terminal mode retains one bounded restoration owner.

## Shared TUI connection

`WorkbenchClient::start` promotes the bootstrap OBLC admission into one shared OBWP v1 carrier.
All feature origins refer to its connection actor. Creating a logical lane allocates a resource
scope and cancellation handle, never another Server socket or OBLC identity. The actor owns
reconnect and restores subscriptions; one-shot mutations are never replayed after possible write.
Its decoder admits the `outcome-unknown` stream close only when the carrier's handshake answer
listed `stream-outcome-unknown`; such a close fails that one request as submitted with an unknown
outcome, never as not submitted, and the carrier keeps serving.
A `capacity` close before a stream's first frame left its request without effect. The actor opens
a subscription, or a request whose caller waits for it alone, again under a fresh stream ID after
pauses from 10 to 640 ms, at most seven times; a request whose caller may have sent others behind
it fails at once. A stream that finally fails, fails alone with a typed `ResourceBusy` refusal, a
request as not submitted, and the carrier keeps serving.
A local wake stream makes command/cancel admission interrupt the same readiness wait as incoming
frames. No channel polling timer is used for delivery. The [Server delivery contract](../orchbee-server/README.md#multiplexed-client-delivery)
owns framing, bounds, gap recovery and the finite model's exact claims and exclusions.

Saved-machine observers promote their bootstrap connection before publishing an origin. Discovery
keeps one state subscription; Agent/Workspace collectors register invalidations before their first
read and retire prior jobs on reset. Metadata reads follow commits and owner expiry, while Host
process titles and requested external Git state retain sampling. One carrier-wide application
progress probe bounds idle freshness; SSH keepalive is insufficient. Failed registration or reads
use bounded failure backoff, not successful-state polling. A failed remote observer retains its
background guard until both collectors have withdrawn cached sources and subscriptions.

| Consumer                                                                                              | Shared-carrier behavior                                                                                                                                                                                                   |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Window title, configuration, Settings                                                                 | UI-owned state subscriptions; settings edits are asynchronous one-shot requests. No forwarding thread.                                                                                                                    |
| Notifications                                                                                         | UI-owned bounded event subscription; cancellation stops only that listener.                                                                                                                                               |
| Client control                                                                                        | Stable commit invalidations plus one-shot Synchronize/completion, configured-command and layout requests. Its domain worker coordinates snapshot/control ordering; it owns no socket, admission retry or heartbeat timer. |
| Workbench membership, Worktree operations, Agent registry/retained physical reports, graphics catalog | Commit invalidations schedule existing bounded reads. Empty/unbound views no longer issue periodic membership reads.                                                                                                      |
| Worktree discovery, popup, Plugin, copy, link and Agent read jobs                                     | Existing bounded domain jobs use logical scopes on the same carrier. Scope cancellation releases only their own terminal resources.                                                                                       |

Host terminal output, title/process diagnostics, Plugin/Popup process exit, close disposal and
external Git/filesystem state still require their existing bounded samples where no event source
exists. The reason is recorded at the sampling site. Startup preference lock contention and owned
OS executable lookup are finite local jobs, not Server subscription polling. UI rendering cadence
and semantic recovery deadlines do not imply additional network reads.

Measure performance separately on the same Windows host and private development runtime: one idle
Workspace/PowerShell, ten-second warm-up, then three thirty-second samples of Client/Server/Worker
CPU time, thread count, Server connection-handler counts and thread context switches. Pair each
required role by PID and process creation time throughout the run, normalize CPU by the actual
monotonic sample interval, and retain unmatched thread-lifetime counts. Clear inherited Session
selection and verify the private runtime before sampling. Named Server handlers are an idle
admission proxy, not an OBLC handshake counter. If thread names cannot be read, report that proxy as
unavailable; zero matching names does not establish zero connections. Freeze a compiling baseline
and candidate, workload, toolchain and sampling script; exclude builds and other measurements from
those windows. Context switches are a scheduling proxy, not hardware wakeups or ReadyThread events.
A direct Windows readiness measurement requires an authorized ETW capture and filtering the
[ReadyThread payload's target thread
identity](https://learn.microsoft.com/en-us/windows/win32/etw/readythread) against process/thread
lifetimes; retain trace-loss counts. If capture permissions are unavailable, record the concrete
limitation and a native manual handoff without claiming that metric was collected.

## Native terminal projection

`run_connected` receives an admitted `LocalClient` and starts the Native workbench session. The
shell owns terminal modes and drawing; the session owns its mailbox, connection task and admitted
input. `terminal_projection` validates terminal frames and produces exact feedback. Terminal Host
retains VT and history authority.

A refused record returns `ProjectionRejection`: the record family (`operation`), the validation
`stage`, and the static protocol decoder error as its `source`, never record bytes or terminal text.
CLI reads keep their external classification, `protocol-invalid`; the Workbench Client contains
the refusal to its pane ([refused projection records](docs/workbench-client-runtime.md#refused-projection-records)).
Neither prints any of it, because the Client's standard error and machine documents are product
surfaces. The Workbench Client's only diagnostic exits are the `client-retired`,
`client-fault-contained` and `client-record-contained` records of the daily log described under
the [diagnostic bundle](#diagnostic-bundle).

`terminal_text` owns the forbidden display-scalar rule: Unicode control characters and
U+2028/U+2029. Terminal-cell rendering uses this rule with its existing width checks, and the final
backend `Print` independently replaces forbidden scalars with U+FFFD. Safe text is borrowed without
allocation; canonical strings remain unchanged.

## Known-unchanged Client backpressure

The carrier retains one queued request and one wire owner; even an idle wire may briefly have a
full queue. Workbench Client owns a bounded response to exact viewer/control ServerBusy results:
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

## Localization and authority

Authored help and diagnostics ship in `en-US` and `zh-CN`. Locale selection checks nonempty
`LC_ALL`, `LC_MESSAGES`, then `LANG`; a `zh` language prefix selects `zh-CN`, and every other value
falls back to canonical `en-US`. Locale data affects presentation only and is never persisted or
used as authority.

`crates/workbench-client/src/localization.rs` owns locale selection and shared text over plain presentation parameters;
feature copy lives in `crates/workbench-client/src/localization/<feature>.rs`. Features select status, menu, shortcut and
Settings descriptions before calling this lower layer; it does not inspect Workbench views,
Settings drafts, CLI commands or input dispatch. CLI copy and command/result formatting live in
`src/cli_text/<feature>.rs`, with `CliText` retaining the selected `Locale`. Add messages to their
owning text module; domain modules continue returning typed outcomes.

Only help/version/usage one-shot routes use standard-library arguments, environment, and streams
alone. Bare Windows, Linux, and macOS invocation composes the shared runtime, private Client
transport, and TUI; platform adapters own console and transport effects.

## POSIX local Session rendezvous foundation

Shared ownership and the complete primitive modeling record are maintained by
[local-runtime](../../crates/local-runtime/README.md); the application keeps no duplicate implementation.

## Accepted local Session activation modeling record

Two concurrent local `orchbee` TUI invocations coordinate through one Session activation attempt
without either becoming the Server or Worker's lifetime supervisor. `apps/orchbee` owns the Client
bootstrap workflow, deadline, activation guard, and typed Client result. Installation composition
owns the exact cohort capability. Server and Worker own their readiness and service lifetimes after
success. No Client has authority to supervise, stop, replace, downgrade, clean up, or recover a
Session.

| Transition owner     | Event and guard                                                         | Result                                                                                                                          |
| -------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Invocation           | Help or version                                                         | Information completion; Session, lock, launch, readiness, and service-lifetime facts are unchanged                              |
| Client connection    | Compatible Session is already ready                                     | Direct Client connection; no activation resource or launch                                                                      |
| Client connection    | Session is incompatible or partial                                      | Corresponding typed failure; no activation or Session mutation                                                                  |
| Activation contender | Initial connection finds no ready Session and `activation.lock` is held | Wait for the same typed readiness result under the caller's total deadline                                                      |
| Activation winner    | Lock acquired, recheck still finds absence, no launch exists            | Start exactly one installed Server-plus-Worker cohort and wait while retaining the guard                                        |
| Activation winner    | Recheck finds ready, in-progress, incompatible, partial, or failed      | Connect, release and wait, or fail according to the exact observation; never start a second cohort                              |
| External cohort      | Launch is pending                                                       | May publish ready, incompatible, partial, launcher failure, or no result at all; no external completion is fair                 |
| Pending Client       | Typed result or total deadline is observed                              | Connect or return `incompatible`, `partial-cohort`, `launcher-failure`, or `timeout`; release any caller-owned activation guard |
| Connected Client     | Client exits                                                            | End only the Client Connection; preserve Session, cohort, readiness, launch, and service-lifetime state                         |

## Measurement

For an actual TUI invocation, `ORCHBEE_TUI_MEASUREMENTS` may select one new absolute diagnostic
file in an existing owned directory. Help, version, queries and other non-TUI commands do not read
it or create diagnostic files. The opt-in collector records bounded timing/count samples in memory
and writes them after terminal restoration; it records no input bytes or Panel text. The
[timing contract](docs/native-terminal-environment.md#tui-timing-observations) defines the schema,
failure and truncation rules, packet correlation, and the limits of internal completion times.
Compare enabled and disabled runs before attributing delay to the measured runtime. The variable
and the file's format are unstable diagnostics and may change in any release without migration.
Per-stage CPU costs without terminal I/O come from the
[frame-pipeline benchmarks](crates/workbench-client/README.md#benchmarks).

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
960×640-pixel PTY over 5 fresh Sessions and 3 reopens per Session. The isolated invocation environment
contained only account HOME/USER/LOGNAME, Cargo and standard executable paths, repository-root PWD,
`LANG=LC_ALL=en_US.UTF-8`, `SHELL=/usr/bin/zsh`, `TERM=xterm-256color`, and a unique
`ORCHBEE_RUNTIME_BASE`. The filesystem cache is warm; a fresh Session does not imply a cold OS cache.

The recorded interface timing spans Client spawn to a complete draw containing `[*1 1]` and the former bottom-row
`Tab 1 · Ready · controlling` footer, downstream of the full-frame draw, exact FrameApplied /
FeedbackAccepted exchange and latest-geometry UI Ready publication. It does not prove that the
worker has consumed the Ready reply or cleared the startup ceiling. Client VmHWM is sampled at
that endpoint: it measures peak residence through Ready, not the entire TUI lifetime.
The current observer explicitly opens and dismisses the prefix status overlay before sampling;
new timings include this input round trip and are not directly comparable to these reference figures.
Every close verifies exact terminal-mode restoration and alternate-screen exit. Before each warm
reopen, a condition wait outside the timed interval verifies the previous Server connection handler and
socket have retired. Queued viewer Detach has no public completion witness; any remaining detach
work can enter the next attachment's measured latency. Earlier single-terminal references used
different readiness witnesses; these observations are not a strict before/after comparison.

| Measurement                                                      | Reference observation            |
| ---------------------------------------------------------------- | -------------------------------- |
| Client executable size                                           | 2,989,768 bytes                  |
| Version startup min / p50 / p95 / max                            | 0.791 / 0.834 / 1.176 / 1.406 ms |
| Version peak resident memory                                     | 2,818,048 bytes                  |
| Fresh Session complete Ready frame p50 / p95 (5 samples)         | 485.526 / 501.903 ms             |
| Warm Session complete Ready frame p50 / p95 (15 samples)         | 439.502 / 486.007 ms             |
| Client peak resident memory through Ready p50 / max (20 samples) | 4,792,320 / 4,894,720 bytes      |

These are host observations, not performance promises or direct comparisons with another OS,
driver or the old one-shot artifact. Remeasure and replace the reference when reachable dependencies
or startup paths change; supported-platform release acceptance budgets remain reserved.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-server -p orchbee-worker --locked
cargo test -p orchbee -p orchbee-workbench-client --all-targets --all-features --locked
cargo clippy -p orchbee -p orchbee-workbench-client --all-targets --all-features --locked -- -D warnings
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```

Each Windows CLI peer fixture owns its runtime endpoints and settings; tests that change cohort files use
`Fixture::private_cohort()`.

## Workspace terminal startup

The connected TUI reads a correlated OBWP snapshot before choosing a location. Selection uses an exact Workspace/Context pair, remembered selection when available, or the first canonical Workspace in display order, including an empty one. Eligible directory startup may explicitly launch the first Workspace; other empty Workbenches remain empty. There is no `TerminalIntent::Standalone`, `terminal.open` request or Workspace-ID-less selection.

An identity-less `NoWorkspace` is only the Client's empty landing state: it owns no Context, creates no terminal, appears in no navigation row and is never saved as a Workspace position. Workspace Project membership remains nullable. Empty Workspaces preserve their identity and support explicit configuration/creation.

The Client attaches only the snapshot's exact Session/Worker/Terminal binding through `attach-binding`. Known pending, failed, unavailable or outcome-unknown creation provenance cannot be replaced by attachment permission. View-only, controller, Resize and first-frame readiness keep their existing authority and deadlines. Client exit preserves shared membership and live terminals; last-Tab Close leaves an empty Workspace and later startup does not recreate it. [Context selection](docs/native-context-selection.md) and [terminal creation](docs/native-terminal-creation.md) own the detailed rules.

## Windows explicit Session stop

`server stop --confirm` ends the existing managed Session and its terminal processes. Use `--detach` for admission only and retain the returned ID for `operation status <id>`. Explicit retry requires `--retry --request-id <id> --confirm`. Cancellation or timeout does not cancel an accepted stop. A matching stopped record and free lifetime lock prove completion; the next `server start` consumes that record and creates a fresh Session. See the [CLI stop contract](docs/native-cli-query.md#windows-session-stop). macOS stops the same way and publishes a final checkpoint, so the next start relaunches its terminals in their saved directories; Linux has no `server stop` yet. The release build replaces `--confirm` with the [release confirmation](#release-destructive-confirmation).

## Release destructive confirmation

The Windows and macOS release builds guard `server stop`, `--machine <selector> server stop`, `machine remove <selector> --stop-session`, `session stop <name>` and `session delete <name>`. Each runs only when standard input and standard output are both consoles, terminals on macOS, and the person types the target Session's name at the prompt on standard error exactly as the prompt shows it, letter case included, although letter case does not otherwise tell Sessions apart; surrounding whitespace is ignored. `server stop` names the Session the invocation selects: `--session`, an inherited Session route, `ORCHBEE_SESSION` or `default`. `--machine <selector> server stop` and `machine remove <selector> --stop-session` name the selected saved machine's remote Session, and their prompt names the machine and says that every terminal in that Session stops; see [remote Session stop](docs/machines.md#remote-session-stop) and [removing a machine](docs/machines.md#removing-a-machine). The prompt of `server stop` and `session stop <name>` first says what stops: the Session and its terminal count from the Session's handshake `query`, asked within two seconds, or every terminal in it when the Session cannot say; a Session with no terminal keeps the bare request for its name. Release refuses `--confirm`, a stream that is not a console, a target it cannot name and a mismatched name with `denied` (exit 3), details `{stage:"confirmation",reason}` where `reason` is `confirm-option`, `not-interactive`, `session-unknown` or `name-mismatch`, and a localized message naming the command to run by hand. Ctrl+C at the prompt cancels with exit 130. Development and test builds require `--confirm` as before and never prompt; TUI and Desktop stop actions keep their own confirmation dialogs. Desktop's Quit All, in the menu of its notification-area or menu-bar icon, confirms once in a native dialog and records the stop through its admitted Client connection, not through this command, so the release prompt and its log do not apply to it; `operation status <id>` reads that stop as any other. The guard slows unintended Agent actions and is not a security boundary. On macOS `server stop` names `ORCHBEE_RUNTIME_SCOPE`, which a managed terminal inherits, or `default`. Linux keeps `--confirm` and writes no request log until ORC-328.

Before the command runs, every Windows and macOS invocation of these commands that passes grammar, accepted or refused, in every environment, appends one JSON line to `logs/destructive-commands.jsonl` in the environment's data root. A grammar failure, such as a development `server stop` without `--confirm`, is not a request and is not logged, nor is a remote stop whose machine cannot be selected. Each record has:

| Field         | Value                                                                                                                                                                                                                                                                                             |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `version`     | `1`                                                                                                                                                                                                                                                                                               |
| `time`        | UTC RFC 3339 with milliseconds, or `null` when the clock cannot be read                                                                                                                                                                                                                           |
| `environment` | The compiled environment stem                                                                                                                                                                                                                                                                     |
| `command`     | `server stop`, `machine server stop` for a remote stop, `session stop`, `session delete` or `uninstall`                                                                                                                                                                                           |
| `session`     | The target Session name, or `null` when it cannot be named                                                                                                                                                                                                                                        |
| `interactive` | `{stdin, stdout}`: whether each stream is a console, or on macOS a terminal                                                                                                                                                                                                                       |
| `parent`      | `{pid, image}` from a process snapshot; `image` is `null` when the parent has exited or its PID now names a newer process. On macOS `image` is the start path `ps` reports, which names a program a shell started by name only by that name, and `null` when the parent changed while it was read |
| `outcome`     | `accepted`, `refused-confirm-option`, `refused-not-interactive`, `refused-session-unknown`, `refused-name-mismatch` or `cancelled`                                                                                                                                                                |

`outcome` is the admission decision; the command's own result stays in its output. A failed append never blocks or changes the command. The [local runtime guide](../../crates/local-runtime/README.md#destructive-request-log) owns the file's location, privacy and rotation.

## Release-withheld surfaces

A release build removes the entry points of features that are not yet polished and keeps their
code; development and test builds offer every entry point. `orchbee_local_runtime::is_release()`,
derived from the compiled environment stem, is the only owner of the decision.

| Surface                       | What a release withholds                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `plugin` and `popup` families | Help, completion and the `--skill` guide leave them out. Any invocation that reaches one, including `help plugin`, a family without a verb or a usage error, is refused before grammar or runtime work with `stage: "release"`, `reason: "withheld-in-release"`, `feature: "plugin"` or `"popup"`, exit `4`.                                                                                                |
| `channel` family              | Withheld the same way, `channel show` included, with `feature: "channel"`. `update` is offered in every Windows and macOS build; 0.1.0-beta.1 withheld it with `feature: "update"`, and a Linux release build withholds it the same way until its download is built. A development build's `update` and `channel set` refuse with `non-release-build` before saving a channel or touching the installation. |

Sidebar Search is withheld in every build, not only in a release, until it is finished (ORC-424):
the TUI leaves out the Search row and its `/` key, and its code stays in place. The Machines action
(`Ctrl+b Shift+M`) opens Settings › Machines, because remote machines ship. See
[shell layout](docs/tui-shell-layout.md#withheld-search).

The `--skill` guide encloses withheld passages between `<!-- unreleased -->` and
`<!-- /unreleased -->` lines; a release leaves the passages out and every build drops the marker
lines. Unit tests describe the development grammar on every host. The Desktop and Web Client
withhold their Search too and keep the Actions-only palette, as the TUI keeps its action
menu; see the [Web Workbench](../web/docs/workbench.md#release-withheld-search).

## Windows and macOS Worktree operations

The [Worktree contract](docs/windows-worktrees.md) owns source selection, defaults, confirmation,
partial outcomes and recovery. These commands do not activate a Session.

## Windows and macOS Workspace group Close

`project close-all <target> --confirm` closes the captured Workspaces in that Project while
retaining the Project record and Git checkouts. Ordinary `workspace close` always closes only its
target. See the [Project close-all contract](docs/windows-worktrees.md#project-close-all).

Windows CLI Panel/Tab/Workspace Close captures and settles every bound primary and child Presentation, including grouped/pinned Tabs. Canonical removal waits for all target disposal/cleanup and exact membership revalidation; unknown effects keep their retained receipts and recovery constraints. Unbound Presentations, child navigation and other structural operations retain their separate contracts; see the [Close model](../orchbee-server/docs/terminal-tab-close.md#complete-presentation-membership).

## Windows and macOS Client navigation

[Windows Client view control](docs/windows-client-control.md) owns `client list/get/status`,
`workspace focus`, `tab focus`, `panel focus` and `panel maximize`. Navigation targets one current
TUI via `--client` or unambiguous validated caller context. No operating-system window activation is
added. macOS admits the same controls; Linux admits only the read-only `client list` and `client
get`.

## Schema export

`api schema [<schema-id>] --output <path>` exports the embedded JSON document offline and also emits
it on stdout. The [export contract](docs/native-cli-query.md#offline-schema-export) owns failure
stages, temporary cleanup and post-publication stdout failures. It runs on Windows and macOS; Linux
answers the typed unsupported-platform failure.

## Diagnostic bundle

`diagnostics create --output <path> [--format text|json]` writes one stored ZIP archive to a literal
path the person chooses and creates nothing in the data root; nothing is uploaded. It runs offline,
never starts a Session, is not release-withheld, and runs on Windows and macOS; Linux answers the
typed unsupported-platform failure. Publication shares the [schema
export](docs/native-cli-query.md#offline-schema-export) steps with a
`.orchbee-diagnostics-<nonce>.tmp` sibling: the parent must exist and only a regular file is
replaced. On macOS the bundle is created private to the user (mode `0600`).

| Entry                     | Contents                                                                                                                         |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `manifest.json`           | `orchbee.diagnostics.bundle` v1, creation time, each section's `included`, `absent` or `unavailable` state, omitted log lines    |
| `versions.json`           | Product version, environment stem, release flag, cohort identity and artifact digests, and whether each Session runs this Server |
| `system.json`             | Operating system, architecture, Windows product and build or macOS version, locale and its variable, and a closed terminal token |
| `config-check.json`       | The `config check` document                                                                                                      |
| `sessions.json`           | The `session list` document with states                                                                                          |
| `config.toml`             | The settings file, redacted                                                                                                      |
| `logs/<YYYY-MM-DD>.jsonl` | Well-formed records of today's daily log and the six before it                                                                   |

Every carried document passes the
[redaction rules](../../crates/local-runtime/README.md#diagnostic-bundle-redaction): commands,
arguments, environment values, credentials and unrecognized settings values become placeholders,
and home paths, data-root variables, URL credentials and URL queries are scrubbed from every string.
The bundle never reads checkpoints, layouts, terminal output, plugin data, saved machines or the
destructive request log. The Client writes `tui-started` and `tui-exited` with a closed outcome,
`server-start` and `server-start-finished`, `server-restart` and `server-restart-finished` with the
exit status, and `relay-started` and
`relay-exited` to the
[daily log](../../crates/local-runtime/README.md#diagnostic-log), the remote link helper
writes `link-rights` with the rights its token runs with or why it refused, its
`ensure-session` writes the [Session start's records](docs/machines.md#link-helper),
`session-start-source` and `server-job`, and the machine hub (`--machine-hub`) writes `panicked`
with its thread role token, `file` stem and `line` before aborting; other commands write none.
The TUI also writes `machine-hub-loss` (when a protocol violation ends a hub connection, with `loss`,
`stage` and `class`), `machine-hub-ended` (when attachment ends permanently, with `failure`), and
`client-retired` when its connection retires on a failure: the failure's
token, the kind of command the connection worker held then, such as `poll` or `panel`, and the
source directory, file stem and line of the mailbox check or Client step that closed it, never the
path. A process exit's close writes nothing. When the TUI contains one request's fault to its
Panel instead, such as a refused input, resize, poll or frame acknowledgement, it writes
`client-fault-contained`: the command, the Server's answer token and the number of records
suppressed since, at most once a minute for each command and answer. When it refuses one
projection record and contains it to the Panel, it writes `client-record-contained`: the record
family, the validation stage and whether the record was dropped, the Panel resynced or the Panel
stopped. A stop always writes; the others write at most once a minute for each stage. The TUI has
no entry of its own: the command is one line in any terminal, and Desktop offers it as an Action.

## Windows Panel layout queries

`panel layout <panel>` without `--client` reads canonical structure without a TUI. With `--client`,
it reads that Client’s cell geometry; `panel neighbor --direction left|right|up|down` and `panel
edges` also require Client geometry. See the [geometry contract](docs/windows-client-layout.md) for
structural and rendered receipts.

Server owns shared ratios; Clients project them at their own viewport sizes and retain local focus,
maximized state and control. See the [layout contract](docs/windows-client-layout.md).

## Shared Panel titles

`panel rename <panel-id> --label <text>` sets the shared primary Presentation override; `--clear`
(or blank text) restores its creation label. The [Panel label contract](docs/native-panel-labels.md)
owns normalization, revision/replay rules and coordinated cohort compatibility.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf
exchange, preserving Split/slot and terminal identities. [Native Panel
swap](docs/native-panel-swap.md) owns the workflow, receipt and evidence.

On Windows and macOS, `tab create` and `panel split` resolve `--cwd
follow|home|current|workspace|<absolute-path>` before creation; follow is the default and samples
the resolved source root process. `orchbee.creation.result` separates committed creation from later
ratio/focus results; a later failure is not permission to repeat creation. [Native terminal
creation](docs/native-terminal-creation.md) owns target selection, total budget, concurrency guards
and recovery for these Windows commands.

On Windows and macOS, `tab move <target> --insert-index N` moves a complete Tab to a zero-based
boundary in the original order: 0 is first, the Tab count is last, source/source+1 are unchanged,
and out-of-range values are refused. It excludes `--order` and preserves focus and terminals.

## Automatic Tab naming

The [Tab label contract](docs/native-tab-labels.md) owns optional `automaticLabel` metadata. New
Tabs without a supplied label use their current context position; explicit numeric names and legacy
Tabs remain fixed. CLI queries, label selectors and TUI display agree after move or close, and
rename chooses a fixed name.

## Source-selected terminal output

Linux, macOS and Windows `terminal read` and `agent read` accept `--source
visible|recent|recent-unwrapped|detection`, defaulting to recent output. The [output
contract](docs/native-terminal-output.md) owns selection, exact sample validation and
`orchbee.terminal.output` v3, including its nullable output cursor.

`terminal wait-output` uses the same sources and line limits, with a default total timeout of 30000
ms. A match does not prove command completion; managed-shell completion is observed by `terminal run
--wait`.

`terminal read` and `agent read --ansi` export styled snapshots. Original PTY bytes, hyperlinks and
graphics are not reproduced. See the [output contract](docs/native-terminal-output.md).

Bare `status` reads the Workbench and then lists the Session's processes and their versions, one line each, with one handshake `query`. `doctor` checks this executable offline and folds the same Session read with the settings, saved data and runtime folder checks into five lines and exits 1 only when a check failed. The [runtime status contract](docs/native-runtime-status.md) owns the five-second budget, refusals, schemas and failures. Release preference and installation are owned by the explicit Windows update commands.

## Agent navigation

Windows and macOS `agent focus <target> [--client <id>]` resolves a name, session or Panel to its
unique live primary Panel before one Client control submission, which a definite `stale-view`
refusal repeats on a fresh read up to three times while the target resolves the same way. The [Agent
navigation contract](docs/native-agent-navigation.md) defines primary binding, ambiguity,
cancellation and retained operation status.

Windows host focus reporting is owned by [native Client attention](docs/native-client-attention.md);
focus reports are Client metadata and never terminal text.

The Windows provider hook writers for Cursor, Qoder, Antigravity, Mastracode, Droid, Kimi, Letta, Devin
and Hermes are dormant with the rest of the integration family: no command or setting enables them in this
release. Their contracts remain in the [Windows integration contract](docs/windows-agent-integrations.md).

On Windows and macOS, `workspace create` accepts omitted Project, label and cwd, resolves source
terminal/Workspace/home defaults, creates an ungrouped Workspace by default, and optionally focuses
an existing TUI with `--focus`. Workspace and focus retain separate outcomes; optional Project
placement commits atomically with Workspace creation, without a separate Project preparation step.
See the [Workspace creation contract](docs/windows-workspace-creation.md); `orchbee api schema`
supplies the current command and schema catalog.

## Windows CLI configuration

Shared application and terminal themes apply to the native TUI from the same Server settings as Desktop. See [native themes](docs/native-themes.md).

`config check` also validates `config.toml` in the data root, `%USERPROFILE%/.orchbee` for release,
which `ORCHBEE_HOME` moves, with an absolute `ORCHBEE_CONFIG_FILE` override. `[terminal] new_cwd`
sets creation cwd defaults; `[worktrees] directory` selects the checkout root. See [configuration
and precedence](docs/windows-cli-configuration.md). Windows CLI creation reads these defaults from
the running Session's published configuration, so file errors are reported by the Server and `config
check`, not by creation; common TUI/CLI activation reads startup policy before commitment.

## Windows terminal observation stream

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` emits NDJSON from
one existing binding without control or Host resizing. The [stream
contract](docs/windows-terminal-streams.md) owns full safe ANSI frames, resets, flush-before-ACK,
deadlines and cleanup. `Ctrl+b q` detaches; a double prefix sends one literal prefix. No Session
start, Workbench focus change, reconnect or input replay occurs.

## Workspace metadata integration

The [Workspace metadata contract](docs/windows-workspace-metadata.md) defines report normalization,
source sequence fences, TTL and the consistent read join. Its Server and shared wire owners are
separate from canonical topology and terminal OSC metadata.

On Windows and macOS, `panel report-metadata` supports source-owned presentation fields, guards, token patches, sequence and TTL with a binding-fenced, once-only receipt. See the [Panel metadata contract](docs/windows-panel-metadata.md).

On Windows and macOS, `panel report-agent`, `clear-agent-authority` and `release-agent` submit explicit source-owned reports and return correlated dispositions; Linux returns `unsupported-platform` for them and for `panel report-metadata`. See the [Agent report contract](docs/windows-panel-agent-reports.md).

## CLI notifications

On Windows and macOS, `orchbee notification show "Build complete" --body "All checks passed"
--position bottom-right --sound done` queues a custom notification for accepting main-TUI Clients on
the running Session. See [delivery, limits and lifecycle](docs/windows-notifications.md); acceptance
is queue admission, not a display or sound acknowledgement.

## Outer terminal title

On Windows and macOS, `orchbee terminal title set "Build workspace"` publishes an override for the
foreground Client, a main TUI or a Desktop window, which shows it as its window title; `orchbee
terminal title clear` returns to the live `appearance.window_title` template (fallback `Orchbee`).
See [title ownership and restoration](docs/windows-terminal-title.md) for configured templates and
physical-host limits.

Windows `plugin link/unlink/enable/disable/list/config-dir` and `plugin action list` manage the shared user catalog offline. `plugin action invoke` submits owned commands to an existing Session; `plugin log list` reads retained results. See [Windows plugins](docs/windows-plugins.md) for context, process ownership, bounded output, startup and event hooks, managed install/update/uninstall and Pane integration. A start receipt confirms admission; logs report execution results.

## Panel graphics commands

On Windows and macOS, `panel graphics set|clear|info|stream` admits closed image inputs, file/pipe
acquisition and bounded raw frame streams, with one submission through existing Workbench ownership;
Linux returns `unsupported-platform`. See [Panel graphics CLI](docs/windows-panel-graphics.md) for
parameters, unknown receipts and producer cleanup. The [native TUI
renderer](docs/native-panel-graphics.md) stages complete images and publishes successful draw facts;
it draws Kitty images only in an outer terminal that answers the Kitty query, such as Ghostty or
kitty, and the cell raster elsewhere, including iTerm2 and Terminal.app.

## Native Settings

Press Ctrl+b , in the Windows, Linux or macOS TUI to open the Settings page; Windows also exposes
Settings through navigation. All common settings are the same `config.toml` keys Desktop edits
through the Server, including locale, application and terminal themes, selection, Agent tail
display, notifications, sound, scrollback and launch profiles. See the [native Settings
contract](docs/native-settings.md) and the [settings
registry](../../crates/local-runtime/README.md#user-settings).
