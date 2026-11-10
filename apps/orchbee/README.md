# Orchbee Native Entry

## Sessions

A Session contains your Workspaces, terminals and Agents; its background service keeps programs
running when you leave the interface. Start with the default Session and use Workspaces for daily
organization. Windows supports separate named Sessions through `--session <name>` and
`session attach <name>`; `server` commands manage the selected Session's background service.
Agent IDs use `--agent-session <id>`. The [Session contract](docs/native-cli-query.md#sessions-and-background-services)
defines terminology, supported boundaries and unchanged storage/protocol identities.

Windows refuses a TUI launched from a terminal already inside the selected Session before opening
another Client. The localized message returns to the current Shell; CLI subcommands and explicit
routes to another Session remain usable. Failed and ended terminals retain navigation, creation
and close paths; [terminal settlement](docs/native-panel-lifecycle.md#terminal-settlement) preserves
final output and known exit codes. An interactive Shell that exits with code 0 instead closes its
pane, and its Tab when it was the last pane.

Windows `events subscribe` emits bounded NDJSON observations from an existing Session. `events wait`
retains one connection and filter through wait slices, distinguishing match, known timeout,
history loss, refusal and interruption. Use `--event tab.renamed`, a resource selector or a closed
`--filter` JSON; `--after-cursor` pins retained journal history. Output/scroll probes preserve actual
terminal and Client viewport ownership. Run `orchbee events --help` for bounds and conditions;
the [event contract](../orchbee-server/docs/windows-events.md) owns behavior and final Windows evidence.

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` pastes one command and sends Enter in one input request. Supply one quoted string; text is preserved, and targets are exact IDs or `@terminal:<suffix>`. The receipt confirms input submission, not Shell readiness, command completion or exit status; never replay partial or unknown delivery. [Command submission contract](docs/native-terminal-run.md).

Experimental native Rust CLI and TUI Client. On Windows and Linux, and in the Apple Silicon macOS
development package, bare invocation connects or activates a local Session from a verified executable
cohort and enters its TUI after the first validated terminal
projection frame or a stable navigation view for an unavailable terminal. Native Clients operate shared Workbench state while keeping independent selection and
retained terminal processes. Windows also supports saved-machine SSH command routing and live TUI
switching, user configuration, and complete-cohort release installation and updates through the contracts
linked below. The emitted `orchbee` participates in root Cargo verification. macOS packaging is an
unsigned, per-user development cohort for Apple Silicon; it does not include release signing or installation.

Use `agent rename <session-id> <name>` to assign a runtime name, `agent names` to list names,
and `agent resolve <name>` to obtain the full session ID. `agent rename <target> --clear`
releases a name. `agent get/explain/wait <name>` resolve directly; a wait pins its session
even if the name is reused. Names are unique and disappear when the session ends or the Session stops.

`agent read <name>` reads the uniquely bound terminal; `agent send-keys <name> enter`
sends semantic keys through the existing terminal input receipt. Both also accept
`--agent-session <id>` and `--name <alias>`, and refuse missing or ambiguous bindings.

`agent prompt <name> <text> --wait` submits literal paste plus Enter after a fresh,
unblocked preflight. It observes activity before waiting for a condition and retains the
input receipt on later timeout or cancellation; it does not promise turn causality.

`agent start alpha --kind codex --panel <id>` starts an Agent in an idle Windows
terminal. Its integration's canonical Session confirms readiness and binds the name; without an
integration the Agent observed ready in its Panel confirms it and no name is bound.
See [Windows Agent startup](docs/windows-agent-start.md) for setup, limits and recovery.

Windows `channel show [--format text|json]` reads the strict stable/preview preference from the shared user configuration without activating a Session. See [configuration and channel provenance](docs/windows-cli-configuration.md#rule-update-policy).

Windows `machine list|add|rename|remove|enable|disable` manages saved SSH profiles with remote Windows readiness checks before add. See [saved Windows machines](docs/machines.md) for persistence, partial outcomes, command routing, and live TUI switching.

Windows `update` and `channel set` share complete-cohort installation, explicit release sources, immutable publication and separate user PATH activation. See [Windows release installation](docs/windows-cli-configuration.md#windows-release-installation). A release build withholds `update` and the whole `channel` family; see [release-withheld surfaces](#release-withheld-surfaces).

The [TUI platform boundary](docs/tui-platforms.md) defines the shared Client implementation,
optional OS adapters, localized unsupported outcomes and native-platform evidence. State, input,
menus and rendering compile together on Windows, Linux and macOS; optional Windows host effects
remain explicitly unavailable until their platform adapter exists.

## Command surface

Bare and explicit `tui` invocation first admit the inherited interactive terminal and valid
cell geometry, then run the local startup workflow below on Windows, Linux, and macOS Apple Silicon.
They are the same Client route; `tui` does not select a different mode. Other platforms return localized unavailability. `--help`
and `--version` initialize neither TUI nor service. Commands use the exact grammar below without
abbreviations; unknown commands, undeclared options, extra operands, and non-Unicode arguments are usage
failures. Windows remote selection follows the [saved-machine routing contract](docs/machines.md#command-routing-contract).
A release build withholds some surfaces; see [release-withheld surfaces](#release-withheld-surfaces).

```text
orchbee                          # local terminal (TUI)
orchbee tui                      # the same terminal, explicitly
orchbee status                   # the running Session's Workbench snapshot
orchbee <family> <command>       # queries and Workbench mutations
orchbee api snapshot|schema      # machine snapshot and embedded schemas
orchbee server start               # ensure the local Session is running (no TTY)
orchbee server connect             # relay a Client connection over stdin and stdout (pipes only)
orchbee server diagnose            # read-only local Session runtime facts
orchbee server stop --confirm      # stop the Windows Session (release: type its name instead)
orchbee server stop-status <id>    # inspect a retained stop attempt/receipt
orchbee config check             # local runtime inspection
orchbee diagnostics create --output <path>  # Windows diagnostic bundle to send for help
orchbee help [<family>|<command>]
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --help | --version
```

Query families are `client`, `project`, `workspace`, `tab`, `panel`, `terminal`, `worktree`,
`launch-profile`, and
`agent` (`list`/`get`, plus `terminal read`/`wait-output`; Windows `worktree list` accepts
`--workspace <target>` or `--cwd <path>` with optional command-local trust,
or retains the strict `--directory <directory-id>` form; `launch-profile` offers `list`
and `get <selector>` — a typed read of the running Session's launch profile catalog, resolved
Server-side with no local pre-read; `agent` offers `list` with
`--include-ended`/`--limit`/`--cursor` paging, `get <session-id>`, and `explain
<session-id>` — a read-only typed view of the Server's canonical Agent sessions over OBAP,
never inferred from terminal contents — `wait <session-id> [--until <condition>]...
[--timeout <ms>]`, a held Client Connection waiting for status or freshness; Windows follows
retained changes and defaults to no total timeout, with every read still bounded — plus `doctor [--provider <id>]... [--asset-root
<path>]...`, an offline bounded-filesystem diagnostic that explains from local facts alone
why Agent observations may not arrive (no provider execution, no socket connect, no writes).
Windows `agent explain` also resolves names and Panels to exact physical terminal bindings.
Canonical sessions retain `orchbee.agent.explain` v1; physical terminals use v2 with the effective
report view and a separate fresh screen-rule evaluation. The result includes manifest source/version,
the first highest-priority winner, every rule match and bounded region evidence. One admission and five-second
CLI budget cover resolution/read; Server queue/read work has a four-second budget. Binding or report
context changes refuse without retry. No input, controller acquisition or focus action is performed.
Windows startup selects local overrides, eligible cached remote rules, then bundled rules; cached remote rules take part only while `ORCHBEE_AGENT_DETECTION_MANIFEST_CATALOG_URL` names a valid catalog, and Orchbee ships no default catalog. Physical diagnostics include actual path, cached version, shadowing and typed warnings. Windows `server agent-manifests`, `server reload-agent-manifests` and `server agent-manifests-status` expose the installed snapshot and retained live-reload receipts. Explicit `server update-agent-manifests` downloads and activates validated cached rules, with separate per-provider cache and activation receipts. With a catalog, managed release Sessions check automatically at startup and every thirty minutes when `update.manifest_check` is enabled; without one, no check runs and Update is unavailable; debug/test and standalone execution do not schedule checks. See the [screen model](../orchbee-server/docs/agent-screen-model.md).

Windows `panel create-terminal` starts an unbound primary or exact member, or appends a child
without splitting the Panel. It preserves literal launch options, returns retained pending/final
receipts and never changes Client focus. See [Panel terminal creation](docs/windows-panel-terminals.md).

Mutation commands against the
running Session include `project create`/`rename`/`dissolve`/`close-all`,
`workspace create`/`rename`/`set-project`/`create-project`/`ungroup`/`accept-directory`/`close`, `tab create`/`rename`/`move`/`close`,
`panel split`/`move`/`close`, and `launch-profile create`/`delete` —
`workspace close <target> --confirm` closes every terminal in the Workspace's context under
the same tab-close lifecycle, then removes that Workspace while preserving its checkout, branch,
and directories; its asynchronous operation stays answerable through
`workspace close-status --workspace <id>` (strict stable IDs, tombstone
included) and resumable through
`workspace close-resume --workspace <id> --request <close-id>
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
dedicated `orchbee.project.rename` document. `server start` is the headless activation path:
it runs the same connect-or-activate sequence the TUI startup uses — validate the cohort,
prepare the runtime directory, take the locks, commit the marker, spawn the Server when no
running Session answers — under one absolute 20-second deadline, then reports `started` only
when this invocation actually spawned the child and `already-running` when an existing Session
admitted its probe. The command is effectful even on refusal: only a grammar failure is
guaranteed zero-effect, an interrupted or timed-out start may leave committed activation
state. On Windows, the next invocation automatically recovers a complete v2–v5 registered generation only after
proving every old exact process gone under all required guards; live, incomplete and legacy Active
records remain `rejected`/`recovery-required`. Recovery preserves checkpoint data and never terminates
old processes. A fresh activation publishes catalog v5 before marker, Server or checkpoint-store
publication; readers limited to v1–v4 refuse v5 without changing its bytes. The
[Windows recovery contract](../../crates/local-runtime/docs/windows-crash-recovery.md)
owns those boundaries. `server diagnose` is the read-only complement: it reports
bounded facts about the runtime directory, marker, locks, and finite endpoint paths — each
a closed-state row at its own observation instant — under one cooperative 5-second budget,
and it never connects, creates, locks, repairs, or claims any process is alive or gone.
`server connect` is the byte relay for a Client that cannot own local IPC, such as the Desktop
shell: it runs the same connect-or-activate sequence, admits this process through OBLC, then copies
the connection's bytes between its standard streams until either side ends. Both streams must be
pipes, and it takes no `--format` because stdout carries raw protocol bytes. It neither decodes
those bytes nor reconnects; the parent owns the protocol and any new connection.
`agent integration` (`list`, `explain`,
`enable`, `disable`) is the local provider-onboarding family: bounded reads
and managed mutations of the invoking user's provider configuration on this
machine only — no socket, lock, or Session contact; only entries verifiably
owned by Orchbee are installed or removed. The complete grammar, target and
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
orchbee project create --label demo       # then workspace create / accept-directory
orchbee tab create --workspace <ws> --label shell \
  --cols 80 --rows 24 --cell-width 8 --cell-height 16
orchbee terminal send <terminal-id> --text 'make test' --enter
orchbee terminal wait-output <terminal-id> --match 'PASS'
orchbee agent integration enable --provider <id>   # one-time local onboarding
orchbee agent list                        # sessions the running Session observes
orchbee agent wait <session-id> --until ended
```

The Apple Silicon macOS development cohort supports the Session-backed `agent list` query and its
bounded page options. The per-Client projection (`--client`) remains Windows-only; other Agent
observation commands retain their documented platform limits.

`orchbee help <family>` prints the same grammar from the binary itself, and
`orchbee completion <shell>` prints the matching key-driven discovery script
for your shell to source.

Help uses shared section headings and ordering at each level. Root help provides quick start,
command groups and global options; family help lists direct children with a one-line synopsis each; command help describes
arguments, options, examples and operational constraints. Session concepts and onboarding stay
in this guide rather than introducing a separate help section.

Help writes stdout, failures write stderr, and version writes one locale-neutral
`orchbee <version>` line. Development status is `0` for normal TUI close/help/version and for
successful queries, mutations, and checks; `2` for usage failure; the remaining failure classes
follow the owning record's failure matrix. This is not a stable public exit table.

## Native Windows quick start

`workspace create --cwd C:\existing\repo` creates an ungrouped Workspace and its first terminal in one request. An explicit `--project <target>` adds optional organization; label and directory have defaults. Add `--env NAME=VALUE`, an explicit `--exec`/`--arg`, or `--profile` as needed. The receipt returns resource IDs and the terminal outcome; a failed launch retains the Workspace for inspection and explicit close. See the [Windows workflow](docs/cli-workflows.md#windows-directory-workspace) for PowerShell examples.

Build with the repository's pinned Rust and Zig toolchains and Node.js. Run this from the repository
root in PowerShell; the output directory must be new, and its parent must already exist:

```powershell
node scripts/stage-local-runtime.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
```

Staging prints the Client path, for example `...\OrchbeeNative\ob-dev-<16 lowercase hex>.exe`; run it in
Windows Terminal. Development and test Clients stay off `PATH`, since staging changes no `PATH`:
run one by the path staging prints, or let `pnpm desktop` stage and start its own. The directory
contains the four exact `.exe` siblings, a hash manifest, and the observation-adapter capsule. Keep them together. The file names start with the artifact stem. Native
Windows staging defaults to this checkout's development stem, `ob-dev-<16 lowercase hex>`, the same
one `pnpm desktop` uses, so a development cohort never shares image names with the installed release.
`--artifact-stem orchbee` stages the release names and `ob-test-<16 lowercase hex>` a test cohort.
A plain `cargo build` or `cargo test` compiles this checkout's test stem, `ob-test-<same 16 hex>`,
on every platform, so its binaries never act on the release installation; on Windows Cargo still
writes them as `target\...\orchbee*.exe`, and they find their
siblings only in a cohort staged under the stem's names. This is a native development cohort using Windows named pipes and
ConPTY. Windows Terminal is the interactive host; WSL is not required. New terminals capture the shared launch profile and scrollback default. The built-in System shell
selects the first compatible PowerShell 7 on absolute PATH or system Windows PowerShell.
See [Windows configuration](docs/windows-cli-configuration.md#automatic-windows-shell)
for candidate selection. `server start --format json` also works without an interactive console.

On a Windows Session with no Workspace or terminal content, `cd <project>; orchbee` creates
an ungrouped Workspace named after the current directory and its first terminal in that directory.
Later launches restore the last displayed Workspace, Tab and Panel; launching from
another directory does not add a Workspace. See [directory startup](docs/native-directory-startup.md)
and [startup position](docs/native-context-selection.md#windows-startup-preference).

The prefix is `Ctrl+B`, followed by `|` for a side-by-side split, `-` for a stacked split,
`h/j/k/l` for pane focus, or `d` to detach. Splitting focuses the new pane by default; `terminal.focus_new_panel = false` preserves the current focus once the Session publishes it. Run the executable
again to reattach to the retained Session. Detaching leaves shells running. A shell that exits with
code 0 closes its pane; any other exit remains observable until its Tab/Panel is explicitly closed.
If every registered Session process has exited,
running the executable again automatically starts a fresh generation and restores the existing
checkpoint. Live or uncertain old owners block startup; current recovery does not adopt a live Host.
`session list` v2 reports guarded cold eligibility as `recoverable`. Continuous Workbench checkpoints
retain layout changes and sampled terminal directories without screen
contents; see the [Windows Workbench checkpoint contract](../orchbee-server/docs/workbench-checkpoint.md).
`config check --format json` checks local cohort/runtime inputs without starting one.

After attachment, the TooSmall recovery surface withholds pane geometry observations instead of
reporting that every pane departed. Terminal viewers, controller grants, selection and the last
valid child geometry survive widths below 20 or heights below 5. Restoring a drawable viewport
resumes normal observation and navigation without detaching; the 20×5 Minimal boundary still
reports real pane geometry. This applies with split panes and notification cards as well.

Validated Windows behavior includes native startup, Unicode rendering, burst and ordinary typing,
CLI terminal send/read, split/focus/resize, detach/reattach with exact retained terminal identities,
and Git worktree discovery. Console records feed the existing key and SGR mouse decoder; the console
mode guard restores the inherited input/output modes when leaving the TUI. Large byte records use
bounded writes, and cancelled exchanges close their shared pipe without a detached flush thread.

Windows `agent doctor --provider codex --format json` now inspects local configuration, PATH,
Node and capsule integrity without starting a Session or changing provider setup. Its version-3
document reports endpoint delivery as uninspected; installed configuration is not proof of
a working Agent connection.

Windows `agent integration list/explain/enable/disable --provider <id>` manages Claude Code, Codex, Grok, GitHub Copilot, OpenCode, Kilo, Pi, OMP, Gemini and Qwen Code. See the [Windows integration contract](docs/windows-agent-integrations.md). Windows `server diagnose` reports read-only runtime, lock and pipe-name facts using `orchbee.server.diagnose.windows` v2. Capsule 0.4.16 supplies Claude Code, Codex, Grok and GitHub Copilot hooks and JavaScript modules using the native observation pipe and console correlation.
Windows x64 cohorts include the pinned ConPTY runtime needed to preserve that correlation.
Other providers retain their
existing platform limits. `terminal send --stdin` accepts redirected regular
files and Windows pipes; an owned hidden Windows PowerShell reader makes pipe input cancellable.
It is resolved from the OS system directory, never from PATH, and is terminated/reaped on failure.
Shell completion supports bash/zsh/fish/powershell/elvish. In PowerShell, load it explicitly with
`Invoke-Expression (orchbee completion powershell | Out-String)`; no profile is changed.
In interactive Elvish 0.21, use `eval (orchbee completion elvish | slurp)`; the static callback
registers the running executable's own name, with and without `.exe`, without querying a Session or
modifying profiles. A development or test Client therefore registers its own `ob-dev-` or
`ob-test-` name and never takes over the release `orchbee` completer.
For Unicode pipelines in Windows PowerShell 5.1, set `$OutputEncoding = [Text.UTF8Encoding]::new($false)` in the invoking script; PowerShell chooses the bytes sent to a native command. Orchbee preserves those bytes and does not repair prior shell encoding loss.
`orchbee --skill` prints the embedded [Agent guide](docs/cli-agent-skill.md) offline, or its [`zh-CN` product copy](docs/cli-agent-skill.zh-CN.md) under that locale. Operating-system service
installation, release signing, automatic application updates, and live Host adoption remain outside these commands.

## macOS and Linux local startup

Build a new development cohort from the repository root:

```sh
node scripts/stage-local-runtime.mjs --output /tmp/orchbee-native-dev
# From a Windows host using WSL Linux:
node scripts/stage-local-runtime.mjs --wsl Ubuntu --output /tmp/orchbee-native-dev
# On Apple Silicon macOS:
node scripts/stage-local-runtime.mjs --output /private/tmp/orchbee-native-dev
```

Then run the staged `orchbee` in an interactive terminal (`/tmp/orchbee-native-dev/orchbee` on
Linux, `/private/tmp/orchbee-native-dev/orchbee` on macOS).
The script rebuilds three Rust release binaries and the Zig ReleaseSafe Host from locked inputs,
copies the canonical observation-adapters capsule as the cohort's lazily validated sidecar,
and verifies its declared payload digest before writing the manifest;
existing output directories are rejected. `--cargo-target-dir <absolute-directory>` can reuse a
Cargo build cache. This stages a development cohort without installing a system service.
The binaries compile this checkout's development stem, `ob-dev-<16 lowercase hex>`, as their
environment, so they keep their state under `~/.ob-dev-<16 lowercase hex>`; `--artifact-stem`
selects `orchbee` or an `ob-test-` stem as on Windows. File names, sockets, and runtime directories
keep the `orchbee` names in every environment until ORC-328.

The default Session is `/tmp/orchbee-native-<effective-uid>` on Linux and
`/private/tmp/orchbee-native-<effective-uid>` on macOS. Development/tests can isolate a Session by
setting `ORCHBEE_RUNTIME_BASE` to another existing absolute directory on an admitted local filesystem.
The [shared runtime contract](../../crates/local-runtime/README.md) owns the 0700 directory,
0600 locks/sockets/immutable attempt marker, exact four-executable cohort with its declared
adapters sidecar, shared 20-second deadline,
and delayed 500-ms progress notice. Server + Worker readiness and first terminal frame readiness
are distinct phases. Initial Open, snapshot read, attach, acquire, authorized resize, and first-frame Poll all
consume the original startup deadline. Native Ready requires a drawn full frame, its exact accepted
feedback and current controlling geometry. The worker restores the ordinary request budget after
consuming the Ready reply. Undrawn FrameApplied feedback is never sent early; NeedSnapshot recovery
feedback may advance initial frame acquisition.

The Client never restarts or adopts a failed Session. Failed/residual runtimes explicitly require
recovery and are not deleted automatically. `Ctrl+B q` or `Ctrl+B d` closes only this Client;
reopening attaches to the same retained shell. Additional concurrent Clients are read-only unless
they acquire an independent controller grant. On Windows, `Ctrl+B t` explicitly takes control of
the current pane and supplies this window's pane size; ordinary typing and clicks never take over.
The previous Client keeps viewing and confirms read-only on its next input or resize interaction;
see [native controller takeover](docs/native-control-takeover.md). TTY admission occurs once, every duplicate is
CLOEXEC, and the prepared Workbench Client plus that same TTY are handed to the interface. Terminals must
report real valid cell/pixel dimensions; a paired `(0,0)` pixel report is admitted as an honest
unknown-pixel declaration, and missing pixel values are never guessed. When the reported
geometry is not drawable — including a temporary zero-cell report — the Client waits for a real
drawable geometry inside the original deadline instead of failing admission or inventing one;
the [native startup wait](docs/native-startup-wait.md) owns that bounded waiting model, its
cancel/timeout/fault outcomes, and its no-service-work guarantee. Linux startup has public process evidence;
macOS Apple Silicon staging and TUI startup are verified against APFS/ACL admission; native Windows startup
and ConPTY workflow evidence are described above. Release performance budgets remain unclaimed.

## Native Tab interactions

The Workbench Client renders the active context's ordered Tabs and canonical labels. Each Client
owns its own active selection, navigator and viewport; another Client's committed Tabs become
visible through the automatic publication probe, and explicit refresh remains available. These controls use the existing `Ctrl+B` prefix:

| Key                    | Connected native behavior                                                                                                                                                                |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `c`                    | Request one Server-owned Tab creation, then select its validated committed member. With no open Workspace on Windows, use explicit Workspace creation instead.                           |
| `n` / `p`              | Select the next / previous Tab in the current Client snapshot.                                                                                                                           |
| `1` … `9`              | Select that position when present.                                                                                                                                                       |
| `[` / `]`              | Select the previous / next visible Workspace, including nonempty legacy content; skip the empty compatibility slot.                                                                      |
| `Shift+N`              | On Windows, create an ungrouped Workspace with its first shell using name/directory defaults, then enter it.                                                                             |
| `a`                    | Use the same one-step Workspace creation.                                                                                                                                                |
| `e`                    | Configure the first directory of the current Empty, unconfigured Workspace.                                                                                                              |
| `r` / `R`              | Rename the current canonical Workspace / its Project; legacy content retains terminal actions with a localized restriction.                                                              |
| `<` / `>`              | Move the active Tab before / after its neighbor, wrapping at either boundary.                                                                                                            |
| `,`                    | Edit the selected Tab's shared label; Enter submits after its first complete draw, Escape cancels.                                                                                       |
| `x`                    | Confirm closing the selected Tab's execution for all attached Clients, or cancel.                                                                                                        |
| `w`                    | Open the navigator and refresh shared Tabs while preserving the active terminal identity. On Windows, select with arrows/Tab and Enter; Esc returns to the terminal.                     |
| `h` / `j` / `k` / `l`  | Move local focus to the neighboring Panel when the active Tab has a split tree.                                                                                                          |
| `-` / `_` / `\` / `\|` | Split the focused Panel below / above / left / right into a second leaf (provisional binding).                                                                                           |
| `X`                    | Request the Panel-close confirmation for the focused leaf — after the complete draw, `Enter` confirms and `Esc` cancels; the last leaf names the whole-Tab impact (provisional binding). |
| `H` / `J` / `K` / `L`  | Move the focused Panel leaf toward its directional neighbor (provisional binding).                                                                                                       |
| `P`                    | On Windows, enter [keyboard resize mode](docs/native-keyboard-resize.md): arrows move a divider 5%, Shift+arrows 1%, Esc exits.                                                          |
| `q` / `d`              | Close this Client while retained execution continues.                                                                                                                                    |

[Context selection](docs/native-context-selection.md) preserves each retained terminal while
switching location. Workspace identity is independent of its optional Project. The sidebar lists
Agents directly under each Workspace and supports several expanded Workspace lists at once.
[Workspace setup](docs/native-workspace-setup.md) immediately creates an ungrouped Workspace and its
first shell in the current directory of the focused Workspace's first terminal, else that Workspace's
directory, or the launch directory when there is none; New
Workspace in directory… opens the editable picker instead. A Project-menu creation explicitly targets
that Project. Rename and organization are available afterwards; directory acceptance never starts
a terminal. Known partial results remain inspectable with `orchbee workspace list`; unknown
outcomes never replay, and late completion cannot override a newer selection.
The Windows TUI retains the sidebar and displays the chosen name in a stable pending row after
submission while terminal startup continues asynchronously.
`Ctrl+B W` creates a Git checkout with a generated branch and default base after source discovery;
`Ctrl+B O` selects an existing checkout by exact path. [Worktree launch](docs/native-worktrees.md)
adds a peer Workspace, reuses the source's current Project or atomically groups an ungrouped source
with its new peer. Other Clients keep their selection. Escape hides observation after submission
and retains the operation ID.

Windows `Ctrl+B F2` opens [action menus](docs/native-action-menus.md).
Right-click a pane, Tab, Workspace, Project or Agent row for its existing actions; an Agent row
offers Rename and Close. Opening and cancelling preserve focus; addressed organization and Close actions also preserve
the current terminal. A pointer-opened menu starts at the pressed column below its trigger row,
else above it, and shifts left to stay inside the viewport; a keyboard-opened row menu opens beside
its row. Hover moves the candidate without executing it; click or Enter executes, and Esc or an
outside click cancels. Context menus use a rectangular border titled with their target, action rows
and an optional disabled reason, without X, footer hints or selection counter. Submenus cascade
beside their parent; Left, Esc or a click on the parent returns to it. The global action menu
retains its discovery controls. A Tab's menu keeps Rename, Pin and group membership above its red
close rows; New Tab to right, duplication, selection and movement are in the global action menu.
Right-clicking the new-Tab button lists the saved launch profiles when there is more than one.
Project Rename opens the editor for that exact Project, including empty and noncurrent Projects,
without entering a child Workspace. The current terminal stays selected through rename or cancel.
A Project's New Workspace submenu offers New Worktree…, its Workspaces' folders and its Git
Worktrees; a Workspace's Move to Project submenu lists the other Projects, New Project… and, when
grouped, Remove from Project. A Workspace's menu also offers New Worktree… and Open Worktree ▸,
which lists its repository's other checkouts; an ungrouped Workspace and the Worktree either adds
land in one new Project. Git discovery uses the first terminal's root cwd; a Project derives
that source from its first canonical member rather than owning a directory. These Worktree rows stay
disabled until that discovery succeeds. The red Close Workspace… action closes that Workspace;
Dissolve Project removes only organization and preserves all Workspaces and programs.
Only authoritative never-created/empty runtime proof skips confirmation. Runtime-backed scope needs
review; pending or unavailable authority retains its precise refusal. Close All Workspaces remains a
separate compatible action.

`Ctrl+B Shift+D` opens the Windows [Workspace Close confirmation](docs/native-workspace-close.md).
After a complete draw, unmodified `Enter` closes only this Workspace's terminals for every Client;
Esc cancels. Git directories remain, and canonical removal selects a surviving Workspace or shows the explicit no-Workspace creation entry.
[Location labels](docs/native-location-labels.md) use a prefilled editor and shared Server commands;
renaming preserves the running terminal and becomes visible to peers through the publication
probe or explicit refresh.

On Windows, drag a visible pane divider with the plain left button to resize panes and controlled
terminal viewports as the mouse moves, then release to apply one shared, saved ratio. Sidebar
borders also resize their actual layout while held. Esc, navigation or external viewport changes
cancel the local draft; Shift stays with the outer terminal. Further drags during submission are not queued.
[Divider resizing](docs/native-divider-resize.md) shares the keyboard adjustment's acknowledgement
and leaves terminal control unchanged.

On Windows, unmodified dragging in a pane without application mouse tracking selects live or
retained-history text; double press selects a word and release copies the complete Host export to
the system clipboard. `Shift` dragging stays with the outer terminal. Copy failure is localized
and never replayed. [Selection and effects](docs/native-terminal-selection-effects.md) owns the
clipboard dependency and lifecycle; pane-program writes, OSC 52 mode, and clipboard configuration
remain to be connected.

[Windows custom commands](docs/windows-custom-commands.md) share one installed catalog between
`command list`, `command invoke`, configured shortcuts and shortcut discovery. Shell jobs, Panel
creation, independent popups and plugin actions retain their existing execution owners and receipts.

Creation drains earlier accepted work and preserves known commit facts through deadline and Client
retirement. Switching releases the old viewer before attaching the selected binding; it does not
stop or recreate either terminal. Moving changes shared order while preserving the active terminal
and its viewer/controller. If a committed Move receives a definite denied/unavailable snapshot result, the Client
keeps terminal interaction available and uses `w` for explicit snapshot recovery before another
structural operation.

Rename preserves exact text up to 512 Unicode scalars. Bracketed paste is atomic and lands as one
edit — prefix, bracket, and newline bytes inside the payload never escalate to commands. Backspace
removes one scalar and Ctrl+U clears the draft from the end; unmodified Left/Right/Home/End/Delete,
Ctrl+A/Ctrl+E, and the shared readline keys (Ctrl+W/K/Y, Alt+B/F/D; see
[native line editor](docs/native-line-editor.md)) reach the draft as semantic edits, while modified or unbound sequences are consumed whole
without cancelling the draft. A known rename with unavailable projection uses the same
explicit `w` recovery without resubmitting the command. The [Workbench Client runtime contract](docs/workbench-client-runtime.md)
owns admission, geometry, feedback, failure, recovery and evidence bounds. Close requires a separate
lifecycle grant, including for a view-only Client. Closing the last Tab leaves Empty; explicit `c`
creates a new terminal, while ordinary Client exit preserves it for later binding attachment. A peer whose
terminal was closed uses explicit `w` to recover the current membership. Linux process evidence
covers this sequence. Canonical split-panel projection, local panel focus, and `panel.split` /
`panel.close` / `panel.move` submission preserve canonical Tab grouping and pinned metadata; production
restart recovery remains unfinished. Panel operations reuse the same structural-mutation admission:
split submits the real controlling cell/pixel geometry and keeps the leaf's label, close is
leaf-scoped and commits through the ordinary publication path, and move relocates the leaf beside
its directional neighbor. Closing the Tab's last leaf removes the whole Tab — its terminal
execution is disposed and the Tab ends for every attached Client, the same consequence an explicit
`x` Tab close warns about — and the provisional `X` binding reaches that close only through the
drawn [panel-close confirmation](docs/native-panel-close-confirmation.md), which names the frozen
leaf and its impact before an unmodified `Enter` admits it. Panel commands are Workbench
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

## Application crates

The `orchbee` binary is the entry layer: process entry, command-line grammar and output, Agent
doctor diagnostics, startup and release installation, and the terminal shell loop and backend.
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
plus the same unbuffered stream. Those existing handshake timeouts remain a separate OBLC boundary.
On Linux and macOS, private `LocalClient::connect_before` instead passes the caller's absolute monotonic
deadline through connection, peer proof, hello, response and final admission. Its
[owning adapter and correctness record](../../crates/local-client-protocol/README.md#deadline-bounded-posix-admission)
define failure cleanup and the unchanged-deadline guarantee. Both paths switch the admitted stream
to nonblocking I/O before `OBCS`. Ordinary requests have an independent budget; initial local
requests are clamped to the original startup deadline.

`submit_terminal_open`, `submit_terminal_create`, `submit_terminal_start`, `submit_terminal_viewer`, and `submit_terminal_control` each take ownership of one already validated
typed request and encode its complete OBCS frame before submission. Encoding failure is definite
and leaves the Client Connection available. A successful encoding starts one fifty-second monotonic deadline
covering the complete frame write and the one Server-frame read; readiness waits recompute the
remaining budget, so partial progress cannot renew it. Failure before the first write syscall is
definite. Once that syscall begins, any write or read failure or deadline expiry, malformed or
incompatible result, wrong result kind, or wrong correlation makes the effect outcome typed
`OutcomeUnknown` and retires the connection. The transport never retries or returns the request for
replay. A correctly correlated Server `outcome-unknown` result remains a valid OBCS result rather
than a transport ambiguity.

Bare Linux and macOS startup consume this transport for canonical Workspace selection, viewer and control intent.
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
explicit later acquisition. Cleanup after Client Connection loss may outlive a submission deadline.

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

### Submission correctness record

| Field        | Contract                                                                                                                                                                                                                                                                                                                           |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | An internal Client with an already admitted same-user Client Connection must submit one typed Create, Start, viewer, or controller request without duplicating an effect when transport completion is ambiguous. Endpoint acquisition, process activation, and presentation are excluded.                                          |
| Inputs       | Trusted typed `TerminalCreateRequest`, `TerminalStartRequest`, `TerminalViewerRequest`, or `TerminalControlRequest`, the admitted `ClientConnection`, its accepted generation, a finite request budget, and monotonic time. Server bytes and readiness outcomes are untrusted. An absent connection means `ConnectionUnavailable`. |
| Data         | One invocation owns one `requestId`, encoded frame, absolute deadline, connection state, possible-handoff boundary, expected result kind, and decoded result class. The `LocalClient` exclusively owns its Client Connection.                                                                                                      |
| Outputs      | Exact correlated typed result; definite `RequestEncoding` or `RequestNotSubmitted`; typed `OutcomeUnknown` with the submitted identity; or `ConnectionUnavailable`. There is no cancellation, retry, replay return, or optimistic terminal mutation.                                                                               |
| Invariants   | Encoding precedes the deadline and transport; one invocation begins at most one frame handoff; only exact kind and identity complete; every ambiguity after possible handoff retires the Client Connection; a retired Client Connection sends no bytes.                                                                            |
| Decisions    | Before versus after first possible handoff; write incomplete versus complete; result before versus at/after deadline; exact versus wrong kind or identity; valid versus malformed frame; known versus Server-reported unknown outcome. Deadline observation wins if acceptance has not completed.                                  |
| Model        | One invocation submits serial requests at most once under one total deadline. `LocalClient::exchange` is the implementation workflow; its deadline-aware nonblocking stream wrapper is the time and I/O adapter.                                                                                                                   |
| Transitions  | The transition record below defines the serializable abstract phases and guards. Runtime phase is call-local; only the live-or-retired connection state persists between calls.                                                                                                                                                    |
| Dependencies | One already admitted Unix stream, monotonic time, readiness polling, OBCS encoding/decoding, and same-stream OBLC admission. The OBCS workflow has no filesystem, process, endpoint-selection, randomness, localization, or UI capability.                                                                                         |
| Ownership    | `LocalClient` is the sole stream and retirement owner. OBCS owns frame and result semantics; OBLC owns admission, credentials, cohort, generation, and its existing handshake timeouts.                                                                                                                                            |
| Verification | Fake-Server tests cover exact Create/Start, large OBSR records, incompatible viewer/control outcomes, resize geometry, correlation, protocol/EOF failures, silent timeout, partial I/O deadline exhaustion, pre-write expiry, retirement and no duplicate send.                                                                    |
| Correctness  | Claims LC-01 through LC-05 map to implementation branches and deterministic tests below. No separate submission TLA model is retained. These cases do not prove OS scheduling, arbitrary codec input, activation, reconnection or reconciliation.                                                                                  |

### Submission transition record

| Current phase                      | Event and guard                                                   | Next phase         | Result and connection effect                                                     |
| ---------------------------------- | ----------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------------- |
| `ready`                            | Submit while a Client Connection exists                           | `encoding`         | No transport effect                                                              |
| `encoding`                         | Frame encoding fails                                              | `definite-failure` | `RequestEncoding`; Client Connection retained                                    |
| `encoding`                         | Frame encoding succeeds and creates one absolute deadline         | `write-ready`      | Encoded frame is private to this invocation                                      |
| `write-ready`                      | Deadline or I/O setup fails before the first write syscall        | `definite-failure` | `RequestNotSubmitted`; Client Connection retained                                |
| `write-ready`                      | First write syscall may hand bytes to the kernel                  | `write-begun`      | Handoff count becomes one                                                        |
| `write-begun`                      | The one complete frame is written before expiry                   | `awaiting-result`  | Wait for exactly one Server frame under the same deadline                        |
| `write-begun` or `awaiting-result` | Deadline, I/O, timeout, or EOF wins                               | `retired`          | `OutcomeUnknown`; connection closed                                              |
| `awaiting-result`                  | Decoded frame is malformed, wrong-kind, or wrong-ID before expiry | `retired`          | `OutcomeUnknown`; connection closed                                              |
| `awaiting-result`                  | Exact correlated result is decoded and deadline is still open     | `completed`        | Return typed known or Server-reported-unknown result; Client Connection retained |
| `retired`                          | Any later submit call                                             | `retired`          | `ConnectionUnavailable`; no write and no retry                                   |

### Correctness traceability

| Claim                                                                                                                              | Implementation and deterministic witness                                                                                                                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LC-01: encoding and every failure before the first write are definite.                                                             | `exchange` encodes before starting `SubmissionDeadline`; `deadline_expiry_before_the_first_write_is_a_definite_failure` observes zero Server bytes and a retained generation.                             |
| LC-02: one invocation has at most one request-frame handoff and never retries it.                                                  | One encoded buffer is passed once to `write_all`; slow-write and silent-timeout tests reject a second request and subsequent calls return `ConnectionUnavailable`.                                        |
| LC-03: every failure after possible handoff is unknown and retires the Client Connection.                                          | Write/read, EOF, incompatible frame, wrong kind, and wrong identity branches all call `retire_with_unknown`; focused tests assert the generation disappears.                                              |
| LC-04: only exact kind and identity complete, while a correlated Server `outcome-unknown` is data rather than transport ambiguity. | Exact Create/Start, wrong-kind, wrong-ID, and correlated-unknown fake-Server tests exercise the result classifier.                                                                                        |
| LC-05: one monotonic deadline covers the full write and read and is not renewed by progress.                                       | `SubmissionDeadline` is created once; every readiness boundary recomputes its remainder. Stepping-clock tests force sub-deadline byte progress across both read and write until the total budget expires. |

## TUI shell foundation

Application-private shell adapters own the following terminal and presentation boundaries.
Bare Linux and macOS startup use the prepared Workbench Client; unconnected `run` and explicit `run_connected`
remain test and composition seams.

The input and transition tables below describe the unconnected fallback. Connected context and Tab
actions are handled by the Workbench Client described above.

| Concern     | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admission   | The staged Unix adapter requires inherited stdin and stdout to be TTYs with the same terminal-device identity, duplicates only those admitted descriptors, and reads cell and pixel dimensions from the duplicated stdout descriptor. It has no `/dev/tty`, global Crossterm size, or process fallback.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Lifecycle   | One staged guard owns raw mode, the alternate screen, and cursor visibility. It marks cleanup responsibility before an operation may partially succeed, retains each flag until its inverse succeeds, and attempts each failed inverse at most twice before returning one aggregate error. A process-lifetime singleton panic hook dispatches only when the panicking thread is the active guard's owner, so another thread cannot clear a setup lease before its terminal state is saved; guard drop does not remove a later hook. Normal return, initialization/event/draw failure, owner-thread panic, and each of `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` have deterministic restoration or signal-observation evidence.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Client View | One invocation owns prefix mode, viewport class, sidebar projection, active overlay, and transient notice. It owns no shared Tab, Panel, Workspace, Session, or Terminal fact and persists nothing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Layout      | The [shell layout](docs/tui-shell-layout.md) owns viewport tiers, bounded sidebar width and terminal geometry. On Windows, the sidebar nests Agents under their exact Workspace owner, including nonempty legacy content in one scrollable tree; disclosure preserves terminal selection and keyboard ownership. Tab-strip and sidebar `+` controls reuse existing create actions and menus. Canonical Panel identities and terminal ownership remain unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Input       | The staged adapter polls and reads at most 4,096 raw bytes at a time from the admitted stdin descriptor. A bounded legacy decoder turns the stream into typed items — a UTF-8 scalar, a CSI/SS3 or kitty-`u` key, an atomic bracketed-paste run, or one non-decision sequence. Each collecting frame (ESC wait, CSI/SS3, UTF-8 scalar, paste run) owns its bytes across reads under a bounded injected wait; a frame opened inside a captured packet keeps that owned provenance even when it completes in a later read, an over-bound or timed-out frame drains to a reliable boundary instead of leaking its tail into bindings, and a paste run pairs only with the terminator matching its opener's notation. The legacy decoder emits exactly one rejection for invalid UTF-8, over-bound and timed-out pasted runs with no retained raw payload, then drains silently to that terminator; the pane receives no rejected fragments and an armed prefix remains armed. Attach and text editors retain their existing refusal feedback; ordinary pane and prefix dispatch add no global notice. Every completed read is dispatched in order before any subsequent signal, viewport query, or resize decision can terminate that loop iteration. `Ctrl+B` enters visible prefix mode. `b`, `w`, `?`, `q`, and `d` drive local shell state or exit; connected `c`, Tab selection, and `h`/`j`/`k`/`l` actions are handled by the Workbench Client. When no named owner or exclusive capture owns input, ordinary bytes—including `Ctrl+C`, `Ctrl+D`, escape sequences, UTF-8—reach an injectable terminal-input port byte-for-byte in order; a lone `Esc` forwards only after a bounded wait, and prefix then `Ctrl+B` forwards one literal byte when the current owner admits it. Shortcut Help captures ordinary input while open, and `Esc` closes it without forwarding; captured overlays may admit configured prefix bindings to close or switch surfaces after modal and confirmation owners. |
| Outcome     | The shell consumes its prepared Workbench Client. Bootstrap owns activation; Server owns terminal identity; Host owns terminal lifetime. The shell does not mutate Workbench state.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

The deterministic Client View transition model is:

| State and event                                 | Next state, command, and visible result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Normal + `Ctrl+B`                               | Enter Prefix; emit no terminal input; render the prefix indicator.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Normal + other bytes, with no input owner       | Preserve local view state and offer the exact ordered bytes to the terminal-input port. The unavailable port rejects them without side effects and the staged renderer reports that input was not sent.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Prefix + `b`                                    | Leave Prefix and toggle the docked wide sidebar or narrow sidebar overlay for this Client View; repeating the binding closes it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Prefix + `w` or `?`                             | Leave Prefix and open the Workbench navigator or shortcut Help. Windows navigator captures keyboard/paste: arrows and Tab browse, Enter opens the painted target, Esc or its toggle closes it, and global prefix actions switch through the binding owner. Help captures ordinary input; Esc closes it, `?` closes it when repeated, and other global prefix actions switch surfaces through the same action owner.                                                                                                                                                                                                                                                                                                                        |
| Prefix + `PageUp` or `PageDown`                 | Leave Prefix and scroll Help by eight rendered lines while its overlay is open; Up/Down scroll one line. Help presents effective global bindings plus local bindings for an active copy, resize, or sidebar-navigation mode in localized action groups, and captures printable input to fuzzy-filter action names and key labels; Backspace edits, Ctrl+U clears, and Esc closes without reaching the terminal. Scroll position is Client-local and settles from the exact rendered viewport. With Help closed, Windows Prefix + PageUp enters retained history; PageDown remains unbound. Other platforms retain the unbound behavior. Outside exclusive capture and retained history, unprefixed keys reach the terminal port unchanged. |
| Prefix + `g`                                    | Toggle the Agent Board; repeating the binding closes it. Other configured prefix actions switch through the normal Client action owner after modal and confirmation captures keep first refusal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Prefix + `P` on Windows                         | Enter Client-local resize mode. Arrows and Shift+arrows adjust shared ratios; unknown keys and paste are swallowed, `?` shows mode help, and Esc returns input to the pane unless Help is open, where Esc closes Help.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Prefix + any other complete key or sequence     | Leave Prefix and report an unknown-prefix notice; a complete unknown sequence (a CSI tail, a UTF-8 scalar, a modified key) is exactly one unbound event and can never leave bytes in the pane.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Prefix + `c`, `n`, `p`, `1` … `9`, or `h/j/k/l` | Leave Prefix; a connected Workbench Client executes the corresponding Tab or local Panel-focus action, while the unconnected fallback preserves shared state and reports the action as unavailable. No shell-fallback command is submitted.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Prefix + `m`                                    | Leave Prefix and release or restore host pointer capture through the leased mode pair; while released the status line shows a persistent indicator and host-native selection and scrolling are unimpaired. Mouse capture, hit testing, pane passthrough, and the wheel fallback are governed by [native mouse input](docs/native-mouse-input.md).                                                                                                                                                                                                                                                                                                                                                                                          |
| Prefix + `Ctrl+B`                               | Leave Prefix and offer one literal `Ctrl+B` event to the terminal-input port.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Prefix + `q` or `d`                             | Leave Prefix and close this Client shell.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Prefix + `Esc`                                  | Cancel Prefix without changing an overlay. With Help open, `Esc` closes it; native captures use their local close route. Only when the pane owns input does `Esc` reach the terminal as ordinary input.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Resize                                          | Query the admitted stdout descriptor, update the fixed backend viewport, and recompute only the Client-local viewport class. Crossing into narrow mode hides the docked sidebar; returning wide closes a sidebar overlay and restores the prior wide-sidebar preference.                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Event/draw error, panic, or termination signal  | Stop the loop and attempt cursor, alternate-screen, and raw-mode restoration without starting or stopping another process.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

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

## Shared TUI connection

`WorkbenchClient::start` promotes the bootstrap OBLC admission into one shared OBWP v1 carrier.
All feature origins refer to its connection actor. Creating a logical lane allocates a resource
scope and cancellation handle, never another Server socket or OBLC identity. The actor owns
reconnect and restores subscriptions; one-shot mutations are never replayed after possible write.
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
| Window title, configuration, Settings                                                                 | UI-owned state subscriptions; configuration reload and preference writes are asynchronous one-shot requests. No forwarding thread.                                                                                        |
| Notifications                                                                                         | UI-owned bounded event subscription; cancellation stops only that listener.                                                                                                                                               |
| Client control                                                                                        | Stable commit invalidations plus one-shot Synchronize/completion, configured-command and layout requests. Its domain worker coordinates snapshot/control ordering; it owns no socket, admission retry or heartbeat timer. |
| Workbench membership, Worktree operations, Agent registry/retained physical reports, graphics catalog | Commit invalidations schedule existing bounded reads. Empty/unbound views no longer issue periodic membership reads.                                                                                                      |
| Worktree discovery, popup, Plugin, copy, link and Agent read jobs                                     | Existing bounded domain jobs use logical scopes on the same carrier. Scope cancellation releases only their own terminal resources.                                                                                       |

Host terminal output, title/process diagnostics, Plugin/Popup process exit, close disposal and
external Git/filesystem state still require their existing bounded samples where no event source
exists. The reason is recorded at the sampling site. Startup preference lock contention and owned
OS executable lookup are finite local jobs, not Server subscription polling. UI rendering cadence
and semantic recovery deadlines do not imply additional network reads.

Correctness evidence includes actual carrier traces for cancellation versus late reply, event
overflow versus resnapshot, pending mutation versus another stream, and configuration recovery
without replay. Measure performance separately on the same Windows host and private development
runtime: one idle Workspace/PowerShell, ten-second warm-up, then three thirty-second samples of
Client/Server/Worker CPU time, thread count, Server connection-handler counts and thread context
switches. Pair each required role by PID and process creation time throughout the run, normalize
CPU by the actual monotonic sample interval, and retain unmatched thread-lifetime counts. Clear
inherited Session selection and verify the private runtime before sampling. Named Server handlers
are an idle admission proxy, not an OBLC handshake counter. If thread names cannot be read,
report that proxy as unavailable; zero matching names does not establish zero connections. Freeze a compiling baseline and
candidate, workload, toolchain and sampling script; exclude builds and other measurements from
those windows. Context switches are a scheduling proxy, not hardware wakeups or ReadyThread
events. A direct Windows readiness measurement requires an authorized ETW capture and filtering
the [ReadyThread payload's target thread identity](https://learn.microsoft.com/en-us/windows/win32/etw/readythread)
against process/thread lifetimes; retain trace-loss counts. If capture permissions are unavailable,
record the concrete limitation and a native manual handoff without claiming that metric was
collected. These commands and source changes alone establish no CPU improvement or macOS/Linux result.

## Native terminal projection

`run_connected` receives an admitted `LocalClient` and starts the Native workbench session. The
shell owns terminal modes and drawing; the session owns its mailbox, connection task and admitted
input. `terminal_projection` validates terminal frames and produces exact feedback. Terminal Host
retains VT and history authority.

A refused record returns `ProjectionRejection`: the record family (`operation`), the validation
`stage`, and the static protocol decoder error as its `source`, never record bytes or terminal text.
Callers keep their external classification — `protocol-invalid` for CLI reads and the protocol
carrier failure for the Workbench Client — and print none of it, because the Client's standard
error and machine documents are product surfaces. The Workbench Client's only diagnostic exit is
the `client-retired` record of the daily log described under the
[diagnostic bundle](#windows-diagnostic-bundle).

[Workbench Client lifecycle regressions](crates/workbench-client/src/workbench_client/tests.rs) cover interrupted Drop/join,
unknown-input retirement and read-only input refusal. [Native transport regressions](crates/workbench-client/src/workbench_client/connection/native/tests.rs)
and [mailbox traces](crates/workbench-client/src/workbench_client/mailbox/tests.rs) cover bounded admission, projection/feedback
ordering and retirement. [Linux public startup/PTY witnesses](tests/local_startup_process.rs) and
[native pointer witnesses](tests/local_startup_process/native_pointer.rs) cover retained shells,
resize, pointer routing and terminal-mode restoration. Their finite deadlines are test bounds,
not performance guarantees.

`terminal_text` owns the forbidden display-scalar rule: Unicode control characters and
U+2028/U+2029. Terminal-cell rendering uses this rule with its existing width checks, and the final
backend `Print` independently replaces forbidden scalars with U+FFFD. Safe text is borrowed without
allocation; canonical strings remain unchanged. Captured-output tests verify that cell payloads
cannot inject terminal commands while preserving the backend's own cursor and style commands.

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

| Claim                                                       | Refinement and evidence                                                                                                                                                                                                                                                                                                                                                            |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Only known-unchanged capacity results allow another attempt | Server viewer/control ServerBusy producers establish unchanged Host effects; Client matches only that exact result. Fake Server partitions check busy, rejection, unavailable and unknown.                                                                                                                                                                                         |
| No possible effect is replayed                              | [LocalClient tests](crates/local-client/src/local_client/tests.rs) check timeout and EOF after one exchange without resubmission; [native command traces](crates/workbench-client/src/workbench_client/connection/native/tests/commands.rs) check busy input/resize/feedback and retirement after ambiguous input.                                                                 |
| Deadline and identity do not reset                          | One operation deadline covers all attempts; request IDs and controller sequence increase; bounded fake Server traces exercise expiry and busy Resize.                                                                                                                                                                                                                              |
| Retained feedback and cancellation stay bounded             | [Mailbox tests](crates/workbench-client/src/workbench_client/mailbox/tests.rs) check the eight-record capacity; [native transport tests](crates/workbench-client/src/workbench_client/connection/native/tests.rs) check exact drawn feedback; [Workbench Client lifecycle tests](crates/workbench-client/src/workbench_client/tests.rs) check joined drop of the retained adapter. |

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

Only help/version/usage one-shot routes use standard-library arguments, environment, and streams alone. Bare Windows, Linux, and macOS invocation composes the shared runtime, private Client transport, and TUI; platform adapters own console and transport effects. The native application owns the current development CLI/TUI surface.

## POSIX local Session rendezvous foundation

Shared ownership and the complete primitive modeling record are maintained by
[local-runtime](../../crates/local-runtime/README.md); the application keeps no duplicate implementation.

## Accepted local Session activation modeling record

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Two concurrent local `orchbee` TUI invocations must coordinate through one Session activation attempt without either becoming the Server or Worker's lifetime supervisor. A present compatible Session should require no launch; an absent Session should start at most one exact installed cohort; every non-success outcome must be explicit.                                                                                                                                                    |
| Inputs       | Two Client invocation slots; TUI, help, and version intent; an initial Session observation of absent, compatible-ready, incompatible, or partial; one activation-lock result; the exact installed Server-plus-Worker cohort capability; typed readiness, incompatibility, partial-cohort, and launcher-failure observations; and an abstract monotonic total-deadline expiry. External process outcomes are untrusted and may never arrive.                                                        |
| Data         | Per-Client phase, typed result, and connection path; one optional activation-lock owner; a zero-or-one launch count and exact-installed cohort tag; Session and typed-readiness state; and a Session-owned ready-service lifetime marker. No PID, path, runtime-base, scope, ACL, process tree, cleanup record, recovery identity, service-manager unit, or numeric time value exists.                                                                                                             |
| Outputs      | A compatible present Session connects directly. An absent Session lets one lock owner start the exact installed cohort while a contender waits for typed readiness. Success connects and releases the caller's activation resource. Incompatible Session, partial cohort, launcher failure, and deadline expiry are distinct failures. Help and version complete without discovery or activation. Client exit ends only that Client.                                                               |
| Invariants   | Every TUI path attempts connection before activation; the activation owner is singular; the launch count is at most one and every launch is the exact installed cohort under the lock; a contender becomes a readiness waiter; only a ready compatible Session can produce a connection; settled Clients own no activation resource; ready-service lifetime belongs to the Session; failures and Client exit never stop or rewrite Session state; and help/version preserve every activation fact. |
| Decisions    | Invocation kind determines whether Session work is permitted. Connection observation distinguishes direct success, absent activation, incompatible failure, and partial-cohort failure. After absence, lock ownership distinguishes launcher from waiter; a lock winner rechecks Session state before launching. Typed readiness, typed failure, or the Client-owned total deadline settles each waiter. Deadline observation may win until connection completes.                                  |
| Model        | A bounded workflow over exactly two Client slots and four supported initial Session classes. One scalar `activationOwner` models `activation.lock`; `launchCount` and `launchedCohort` expose singleton exact-cohort launch; `serviceLifetimeOwner` distinguishes the ready Session from every Client. External launcher/readiness actions are nondeterministic and have no fairness assumption.                                                                                                   |
| Transitions  | `idle -> connecting -> connected` for a present compatible Session; `connecting -> failed` for present incompatible or partial state; `connecting -> lock-ready -> lock-held -> waiting-readiness` for the launch winner; `lock-ready -> waiting-readiness` for contention; waiting or recheck states settle as connected or one typed failure; `connected -> exited` preserves Session state. Help/version go directly from `idle` to information completion.                                     |
| Dependencies | The model composes the accepted command contract with the current private `LocalClient` admission boundary and `ModeCheckedLocalSessionRendezvous` lock semantics only at their named abstractions. Future runtime refinement must supply installation-owned cohort selection, typed readiness, and one monotonic total deadline. This slice adds no process, socket, filesystem, localization, or public-command implementation.                                                                  |
| Ownership    | `apps/orchbee` owns the Client bootstrap workflow, deadline, activation guard, and typed Client result. Installation composition owns the exact cohort capability. Server and Worker own their readiness and service lifetimes after success. The model grants no Client authority to supervise, stop, replace, downgrade, clean up, or recover a Session.                                                                                                                                         |
| Verification | Startup state, deadline and real-process tests cover supported initial Session classes, competing Clients, exact-cohort launch, existing service preservation and failure settlement. The shared LocalSessionStartup core supplies only its separately declared bounded protocol evidence.                                                                                                                                                                                                         |
| Correctness  | Claims LA-01 through LA-05 remain behavioral requirements for startup and admission. Controlled deadline and process traces must demonstrate settlement without assuming an external process responds. The retired activation workflow supplies no current TLC proof.                                                                                                                                                                                                                              |
| Limits       | The abstraction proves only the checked two-Client, one-Session state space. It does not prove runtime refinement, operating-system scheduling, process creation or detachment, daemonization, service-manager behavior, runtime base or scope, ACLs, socket metadata, cleanup, recovery, retry, update, a concrete deadline or progress threshold, localization, TUI behavior, packaging, public wiring, or external process progress.                                                            |

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

This model abstracts runtime refinement; `LocalSessionStartup` supplies the concrete startup safety case.
Future wiring must add deterministic process tests before claiming the model maps to implementation.

| Claim                                                                                                       | Bound                                                                                                                                                       |
| ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LA-01: every activating TUI connects first; present compatible, help, and version paths never launch.       | Models command intent and abstract observations, not parser, transport, or process refinement.                                                              |
| LA-02: concurrent absence produces at most one exact installed-cohort launch under one lock.                | Exactly two Client slots and one activation round; no runtime-base, lock-file, or process implementation claim.                                             |
| LA-03: incompatibility, partial cohort, launcher failure, and timeout are explicit and do not stop Session. | Failure presentation, cleanup, recovery, retry, and stale-artifact policy remain excluded.                                                                  |
| LA-04: connection releases Client activation ownership; Client exit does not supervise or stop Session.     | Models ready-service ownership only; it does not select how processes detach or how service lifetime is implemented.                                        |
| LA-05: every started TUI settles without assuming that an external process responds.                        | Assumes the future adapter actually guarantees observation of one finite monotonic total deadline; no numerical value, OS scheduling, or external liveness. |

## Measurement

For an actual TUI invocation, `ORCHBEE_TUI_MEASUREMENTS` may select one new absolute diagnostic
file in an existing owned directory. Help, version, queries and other non-TUI commands do not read
it or create diagnostic files. The opt-in collector records bounded timing/count samples in memory
and writes them after terminal restoration; it records no input bytes or pane text. The
[timing contract](docs/native-terminal-environment.md#tui-timing-observations) defines the schema,
failure and truncation rules, packet correlation, and the limits of internal completion times.
Compare enabled and disabled runs before attributing delay to the measured runtime.
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
node scripts/tla/check-inventory.mjs
node scripts/tla/run-models.mjs --workspace apps/orchbee
node scripts/tla/run-models.mjs --models local-session-startup
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```

Windows CLI peer fixtures share immutable cohort preparation and compiled schema validators.
Each fixture owns its runtime endpoints and settings; tests that change cohort files use
`Fixture::private_cohort()`. Shared artifacts live only while a test thread or fixture uses them,
and the last owner removes their temporary directory. Executable files retain normal single-link
cohort admission. Schema membership and grammar matrices belong to core tests; process tests retain
offline publication, real IPC, failure, deadline and cleanup witnesses.

Each captured command retains a Windows Job through root and descendant settlement. The default
exchange has a 45-second backstop after process creation, an 8 MiB combined stdout/stderr cap, and
nonblocking input/output; early stdin closure preserves the child's actual exit and output. Deadline,
cancellation, capture and output-limit failures report their original cause and bounded partial
output only after confirmed Job settlement. An unconfirmed settlement instead reports
`PhysicalUnknown`, never successful cleanup. Diagnostics
retain at most the first 8 KiB of each stream; cleanup failure reports the owned path without a second
panic during unwinding.

A peer guard borrows its fixture and owns cancellation plus joining, including parent assertion
failure. Its admission deadline starts only after successful process creation; accepted streams have
10-second read/write bounds, and peer scripts must also bound their own waits. Dropping the guard
waits for the current bounded peer operation before fixture/cohort release. A failed peer cancels the
command; a normally completed script leaves the CLI free to publish its result. The
[fixture lifecycle witnesses](tests/cli_windows_fixture_lifecycle.rs) exercise full-duplex pipes,
limits, early stdin closure, admission cancellation, peer joining and retained descendant exit.

Set `ORCHBEE_TEST_NATIVE_COHORT` to an absolute staged directory to run the same real-process
acceptance suite against the actual release artifacts. Fixtures validate that cohort and copy it
into isolated directories; tests do not change the source cohort or default Session.

```sh
ORCHBEE_TEST_NATIVE_COHORT=/tmp/orchbee-native-dev cargo test -p orchbee --test local_startup_process --locked
```

### Agent integration conformance fixtures

The [Agent integration writer](../../crates/agent-integration-writer/README.md#conformance-fixtures)
owns the provider conformance and effect corpora; the application's CLI tests replay both packs
with its integration exit mapping.

## Workspace terminal startup

The connected TUI reads a correlated OBWP snapshot before choosing a location. Selection uses an exact Workspace/Context pair, remembered selection when available, or the first canonical Workspace in display order, including an empty one. Windows eligible directory startup may explicitly launch the first Workspace; other empty Workbenches remain empty. There is no `TerminalIntent::Standalone`, `terminal.open` request or Workspace-ID-less selection.

An identity-less `NoWorkspace` is only the Client's empty landing state: it owns no Context, creates no terminal, appears in no navigation row and is never saved as a Workspace position. Workspace Project membership remains nullable. Empty Workspaces preserve their identity and support explicit configuration/creation.

The Client attaches only the snapshot's exact Session/Worker/Terminal binding through `attach-binding`. Known pending, failed, unavailable or outcome-unknown creation provenance cannot be replaced by attachment permission. View-only, controller, Resize and first-frame readiness keep their existing authority and deadlines. Client exit preserves shared membership and live terminals; last-Tab Close leaves an empty Workspace and later startup does not recreate it. [Context selection](docs/native-context-selection.md) and [terminal creation](docs/native-terminal-creation.md) own the detailed rules.

## Windows explicit Session stop

`server stop --confirm` ends the existing managed Windows Session and its terminal processes. Use `--detach` for admission only and retain the returned ID for `server stop-status <id>`. Explicit retry requires `--retry --request-id <id> --confirm`. Cancellation or timeout does not cancel an accepted stop. A matching stopped record and free lifetime lock prove completion; the next `server start` consumes that record and creates a fresh Session. See the [CLI stop contract](docs/native-cli-query.md#windows-session-stop). No Linux/macOS stop runtime is implemented. The release build replaces `--confirm` with the [release confirmation](#windows-release-destructive-confirmation).

## Windows release destructive confirmation

The Windows release build guards `server stop`, `session stop <name>` and `session delete <name>`. Each runs only when standard input and standard output are both consoles and the person types the target Session's name at the prompt on standard error exactly as the prompt shows it, letter case included, although letter case does not otherwise tell Sessions apart; surrounding whitespace is ignored. `server stop` names the Session the invocation selects: `--session`, an inherited Session route, `ORCHBEE_SESSION` or `default`. Release refuses `--confirm`, a stream that is not a console, a target it cannot name and a mismatched name with `denied` (exit 3), details `{stage:"confirmation",reason}` where `reason` is `confirm-option`, `not-interactive`, `session-unknown` or `name-mismatch`, and a localized message naming the command to run by hand. Ctrl+C at the prompt cancels with exit 130. Development and test builds require `--confirm` as before and never prompt; TUI and Desktop stop actions keep their own confirmation dialogs. Desktop's Quit All, in the Windows notification-area menu, confirms once in a native dialog and records the stop through its admitted Client connection, not through this command, so the release prompt and its log do not apply to it; `server stop-status <id>` reads that stop as any other. The guard slows unintended Agent actions and is not a security boundary. macOS and Linux keep `--confirm` until ORC-328.

Before the command runs, every Windows invocation of these three commands that passes grammar, accepted or refused, in every environment, appends one JSON line to `logs\destructive-commands.jsonl` in the environment's data root. A grammar failure, such as a development `server stop` without `--confirm`, is not a request and is not logged. Each record has:

| Field         | Value                                                                                                                              |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `version`     | `1`                                                                                                                                |
| `time`        | UTC RFC 3339 with milliseconds, or `null` when the clock cannot be read                                                            |
| `environment` | The compiled environment stem                                                                                                      |
| `command`     | `server stop`, `session stop` or `session delete`                                                                                  |
| `session`     | The target Session name, or `null` when it cannot be named                                                                         |
| `interactive` | `{stdin, stdout}`: whether each stream is a console                                                                                |
| `parent`      | `{pid, image}` from a process snapshot; `image` is `null` when the parent has exited or its PID now names a newer process          |
| `outcome`     | `accepted`, `refused-confirm-option`, `refused-not-interactive`, `refused-session-unknown`, `refused-name-mismatch` or `cancelled` |

`outcome` is the admission decision; the command's own result stays in its output. A failed append never blocks or changes the command. The [local runtime guide](../../crates/local-runtime/README.md#windows-destructive-request-log) owns the file's location, privacy and rotation.

## Release-withheld surfaces

A release build removes the entry points of features that are not yet polished and keeps their
code; development and test builds offer every entry point. `orchbee_local_runtime::is_release()`,
derived from the compiled environment stem, is the only owner of the decision. POSIX builds keep
the release stem until ORC-328, so every POSIX build counts as release.

| Surface                       | What a release withholds                                                                                                                                                                                                                                                                                     |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `plugin` and `popup` families | Help, completion and the `--skill` guide leave them out. Any invocation that reaches one, including `help plugin`, a family without a verb or a usage error, is refused before grammar or runtime work with `stage: "release"`, `reason: "withheld-in-release"`, `feature: "plugin"` or `"popup"`, exit `4`. |
| `update` and `channel`        | Withheld the same way, the whole `channel` family included, with `feature: "update"` or `"channel"`. Development and test builds offer both, and there `update` and `channel set` still refuse with `non-release-build` before saving a channel or touching the installation.                                |

Sidebar Search is withheld in every build, not only in a release, until it is finished (ORC-424):
the TUI leaves out the Search row and its `/` key, and its code stays in place. The Machines action
(`Ctrl+B Shift+M`) still opens the whole machine picker, because remote machines ship. See
[shell layout](docs/tui-shell-layout.md#withheld-search).

The `--skill` guide encloses withheld passages between `<!-- unreleased -->` and
`<!-- /unreleased -->` lines; a release leaves the passages out and every build drops the marker
lines. Unit tests describe the development grammar on every host. The Desktop and Web Client
withhold their Search too and keep the Actions-only palette, as the TUI keeps its action
menu; see the [Web Workbench](../web/docs/workbench.md#release-withheld-search).

## Windows Worktree operations

`worktree create/open/remove/status/resume` now uses retained Server operations and Windows Worker Git execution. The [Worktree contract](docs/windows-worktrees.md) owns source selection, defaults, confirmation, partial outcomes and recovery. These commands do not activate a Session. Create/Open accept optional `--focus` and exact `--client`; default `--no-focus` keeps the view. List/Create/Open follow the validated caller or selected TUI context when source is omitted.

## Windows Workspace group Close

`project close-all <target> --confirm` closes the captured Workspaces in that Project while retaining the Project record and Git checkouts. Ordinary `workspace close` always closes only its target. Use `project close-status --request <operation-id>` after members disappear, and `project close-resume --request <operation-id> --generation <attempt> --confirm` only for a declared resumable failure. Project closure supports detached/bounded waiting and reports each child Close identity and observed state. See the [Project close-all contract](docs/windows-worktrees.md#project-close-all).

Windows CLI Panel/Tab/Workspace Close captures and settles every bound primary and child Presentation, including grouped/pinned Tabs. Canonical removal waits for all target disposal/cleanup and exact membership revalidation; unknown effects keep their retained receipts and recovery constraints. Unbound Presentations, child navigation and other structural operations retain their separate contracts; see the [Close model](../orchbee-server/docs/terminal-tab-close.md#complete-presentation-membership).

## Windows Client navigation

[Windows Client view control](docs/windows-client-control.md) owns `client list/get/control-status`, `workspace focus`, `tab focus`, `panel focus` and `panel zoom`. Navigation targets one current TUI via `--client` or unambiguous validated caller context. It submits once and waits for a drawn acknowledgement; timeout or Ctrl+C stops waiting only. Single-panel maximize has an explicit no-op receipt. Catalog v68 includes 76 schemas, adding `orchbee.client.view`, `orchbee.client.control` and `orchbee.cli.failure.client-control`, all v1. No operating-system window activation or Mac/Linux control runtime is added.

Windows process tests execute the emitted completion in Git for Windows Bash, found relative to `git --exec-path`; that installed Bash is required for this witness. Conflicting flags are checked before and after their command verb without contacting a Session. PowerShell has a separate real-shell grammar witness.

## Windows schema export

`api schema [<schema-id>] --output <path>` exports the embedded JSON document offline and also emits it on stdout. The destination parent must exist; a complete sibling file replaces an existing regular target. The [export contract](docs/native-cli-query.md#windows-offline-schema-export) owns failure stages, temporary cleanup and post-publication stdout failures. Catalog v68 contains 76 documents, including `orchbee.cli.failure.schema-export` v1.

## Windows diagnostic bundle

`diagnostics create --output <path> [--format text|json]` writes one stored ZIP archive to a literal
path the person chooses and creates nothing in the data root; nothing is uploaded. It runs offline,
never starts a Session, is not release-withheld, and answers the typed unsupported-platform failure
outside Windows until ORC-328. Publication shares the
[schema export](docs/native-cli-query.md#windows-offline-schema-export) steps with a
`.orchbee-diagnostics-<nonce>.tmp` sibling: the parent must exist and only a regular file is
replaced. A write failure exits `1` with `orchbee.cli.failure.diagnostics` (`write-failed`, its
stage, the path and any temporary file left); success answers `orchbee.diagnostics.create` with the
path, size, entries and section states.

| Entry                     | Contents                                                                                                                         |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `manifest.json`           | `orchbee.diagnostics.bundle` v1, creation time, each section's `included`, `absent` or `unavailable` state, omitted log lines    |
| `versions.json`           | Product version, environment stem, release flag, cohort identity and artifact digests, and whether each Session runs this Server |
| `system.json`             | Operating system, architecture, Windows product and build, locale and the variable it came from, and a closed terminal token     |
| `config-check.json`       | The `config check` document                                                                                                      |
| `sessions.json`           | The `session list` document with states                                                                                          |
| `integrations.json`       | `never-enabled` without a Capsule folder, else `agent integration list` from a running default Session or `session-not-running`  |
| `config.toml`             | The settings file, redacted                                                                                                      |
| `preferences.json`        | `internal/state/preferences.json`, redacted                                                                                      |
| `logs/<YYYY-MM-DD>.jsonl` | Well-formed records of today's daily log and the six before it                                                                   |

Every carried document passes the
[redaction rules](../../crates/local-runtime/README.md#diagnostic-bundle-redaction): commands,
arguments, environment values, credentials and unrecognized settings values become placeholders,
and home paths, data-root variables, URL credentials and URL queries are scrubbed from every string.
The bundle never reads checkpoints, layouts, terminal output, plugin data, saved machines or the
destructive request log. The Client writes `tui-started` and `tui-exited` with a closed outcome,
`server-start` and `server-start-finished` with the exit status, and `relay-started` and
`relay-exited` to the
[daily log](../../crates/local-runtime/README.md#windows-diagnostic-log); other commands write
none. The TUI also writes `client-retired` when its connection retires on a failure: the failure's
token, the kind of command the connection worker held then, such as `poll` or `panel`, and the
source directory, file stem and line of the mailbox check or Client step that closed it, never the
path. A process exit's close writes nothing. The TUI has no entry of its own: the command is one
line in any terminal, and Desktop offers it as an Action.

## Windows Panel layout queries

`panel layout <panel>` without `--client` reads canonical structure without a TUI. With `--client`, it reads that Client’s cell geometry; `panel neighbor --direction left|right|up|down` and `panel edges` also require Client geometry. Panel commands accept a positional Panel or `--current`, and reject the former `--panel` alias. Reads preserve focus and terminal sizes, including inactive or unvisited targets. See the [geometry contract](docs/windows-client-layout.md) for structural and rendered receipts.

`panel focus [source] --direction left|right|up|down` finds one neighbor in the selected TUI's unmaximized cell layout and preserves the maximized state. No neighbor returns `direction-unchanged` without activating the source Tab. A found destination is pinned and returns `direction-focused` only after a successful draw. Receipt reads never repeat the movement.

`panel resize [panel] --direction left|right|up|down [--amount <number>]` adjusts one eligible split border through a Windows TUI, preserving focus and the maximized state. `layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` sets an exact binary split path. An explicit target without `--client` commits the shared ratio directly; Client-routed adjustments retain a `layout-adjusted` receipt and wait for a draw when selected. Finite ratios clamp to 0.1–0.9, and finite resize amounts normalize to an absolute magnitude at most 0.5 (default 0.05). Server owns shared ratios; Clients project them at their own viewport sizes and retain local focus, maximized state and control. See the [layout contract](docs/windows-client-layout.md).

## Shared Panel titles

`panel rename <panel-id> --label <text>` sets the shared primary Presentation override; `--clear` (or blank text) restores its creation label. It accepts an exact ID or `@panel:<suffix>`, with no focus or terminal effect. The [Panel label contract](docs/native-panel-labels.md) owns normalization, revision/replay rules and coordinated cohort compatibility. `orchbee.panel.rename` v1 distinguishes `renamed` from `unchanged`; its nullable `customLabel` echoes normalized request metadata. `panel get` and `panel current` v3 expose immutable `label`, nullable `customLabel` and effective `displayLabel`. `api snapshot` v4 preserves the canonical optional override. Catalog v68 contains 76 schemas and the native command inventory contains 88 paths.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf exchange, preserving Split/slot and terminal identities. Explicit Panels without `--client` use the canonical workflow without focus changes. A Client-routed swap separately acknowledges source focus; known canonical commits survive later failure or unknown focus. [Native Panel swap](docs/native-panel-swap.md) owns the workflow, receipt and evidence.

Windows `tab create` and `panel split` resolve `--cwd follow|home|current|workspace|<absolute-path>` before creation; follow is the default and samples the resolved source root process. Their receipt records the chosen directory source/path. They accept optional targets and labels, with no focus by default; `--focus` requests post-creation focus, and split accepts a shared `--ratio`. Explicit targets can create without a TUI. `orchbee.creation.result` separates committed creation from later ratio/focus results; a later failure is not permission to repeat creation. [Native terminal creation](docs/native-terminal-creation.md) owns target selection, total budget, concurrency guards and recovery for these Windows commands.

Windows `panel input (<panel>|--current) --right-click orchbee|panel` sets the shared right-button policy without moving focus. `--current` requires validated caller metadata; explicit targets bypass it. `panel split --right-click` commits the new Panel policy with creation. The default `orchbee` reserves right-click to the Client; `panel` permits unmodified right clicks in mouse-reporting terminal content. An admitted gesture retains its matching release after policy changes. See the [Panel input contract](docs/native-panel-input.md) for routing, pending-input rechecks and failure recovery. The Client's [action menus](docs/native-action-menus.md) use this policy for pane content. `orchbee.panel.input` v1 reports `configured` or `unchanged`; `panel list` v2 and `panel get/current` v3 expose `rightClick`. `tab get` v2 and `api snapshot` v4 admit the optional canonical field. Catalog v68 has 76 schemas and the native inventory has 88 command paths.

Windows `tab move <target> --insert-index N` moves a complete Tab to a zero-based boundary in the
original order: 0 is first, the Tab count is last, source/source+1 are unchanged, and out-of-range
values are refused. It excludes `--order` and preserves focus and terminals. Success uses
`orchbee.tab.move` v1; runtime failures use `orchbee.cli.failure.tab-move` v1.

## Automatic Tab naming

The [Tab label contract](docs/native-tab-labels.md) owns optional `automaticLabel` metadata. New Tabs without a supplied label use their current context position; explicit numeric names and legacy Tabs remain fixed. CLI queries, label selectors and TUI display agree after move or close, and rename chooses a fixed name. Native `api snapshot` v4 exposes canonical metadata; catalog v68 contains 76 schemas. Shared protocol consumers require a matching cohort.

## Source-selected terminal output

Linux and Windows `terminal read` and `agent read` accept
`--source visible|recent|recent-unwrapped|detection`, defaulting to recent output.
Recent combines retained history with the live screen; detection reads the recent screen-height
range. `--lines` permits zero, clamps to 1000 and defaults to 80 for recent. The retired
`--history`, `--rows` and `--unwrap` flags are rejected. `--plain` exports UTF-8 directly and
excludes JSON. The [output contract](docs/native-terminal-output.md) owns selection, exact sample
validation and `orchbee.terminal.output` v3, including its nullable output cursor.

`terminal wait-output` uses the same sources and line limits, with a default total timeout of
30000 ms. `--forever` explicitly removes that ceiling, conflicts with `--timeout`, and retains
five-second I/O bounds and Ctrl+C cancellation. Recent waits join soft wraps; visible/detection
match physical lines. Ordinary waits emit v3 and may match existing output. `--after <cursor|now>`
emits v4 and restricts matching to rows after a validated boundary. A match does not prove command
completion; managed-shell completion is observed by `terminal run --wait`.

`terminal read` and `agent read --ansi` export styled snapshots. JSON uses
`orchbee.terminal.output` v4 with `result.encoding: "ansi"`; plain text reads use v3. ANSI excludes
`--plain`, preserves styled blanks and Unicode, generates only SGR, and joins verified soft wraps
under `recent-unwrapped`. Detection remains plain text. Original PTY bytes, hyperlinks and graphics
are not reproduced. Render requests and prepared output each retain the 8 MiB bound. See the
[output contract](docs/native-terminal-output.md).

Windows `status server` lists the Session's processes and their versions with one handshake `query`, without a Client Connection; `status client` checks this executable offline. Both accept `--format text|json` and never activate a Session. Bare `status` remains a Workbench query. The [runtime status contract](docs/native-runtime-status.md) owns the five-second budget, refusals, schemas and failures. Release preference and installation are owned by the explicit Windows update commands.

## Agent navigation

Windows `agent focus <target> [--client <id>]` resolves a name, session or Panel to its unique live primary Panel before one Client control submission, which a definite `stale-view` refusal repeats on a fresh read up to three times while the target resolves the same way. Explicit `--agent-session`, `--name` and `--panel` selectors use the existing resolver; `--request-id`, `--detach` and the 30000 ms total default timeout retain the common navigation recovery contract. Starting or Unavailable Clients fail with `client-not-ready` before Agent reads; Busy permits the Client-owned cancellable-mode preflight. The [Agent navigation contract](docs/native-agent-navigation.md) defines primary binding, ambiguity, cancellation and retained operation status. Successful explicit focus acknowledges current known Agent versions in the selected Tab even when the outer terminal is unfocused. Ordinary paint still requires host focus. `agent attention <target> [--client <id>]` reads that Client independently without marking it seen.

Windows host focus reporting is owned by [native Client attention](docs/native-client-attention.md); focus reports are Client metadata and never terminal text. The attention/CLI acknowledgement integration remains under development.

Windows `agent changes [--cursor <c>] [--limit <n>]` returns a bounded canonical change page or current-head checkpoint. The default limit is 50 (1–100); reuse the returned cursor. `reset-required` and `cursor-invalid` require explicit baseline recovery. See the [query contract](docs/native-cli-query.md#windows-agent-change-pages). `orchbee.agent.changes` v1 is available in catalog v68 (76 schemas, 88 command paths).

Cursor, Qoder and Antigravity session identity hooks are managed on Windows with `agent integration enable --provider cursor`, `agent integration enable --provider qodercli` or `agent integration enable --provider antigravity`. Existing hooks and other configuration values are preserved.

Windows `agent integration enable --provider mastracode` installs session, run, tool, permission and subagent observations; partial owned hook sets can be repaired.

Windows `agent integration enable --provider droid` manages session identity hooks in the effective user configuration without shadowing existing fallback hooks.

Windows `agent integration enable --provider kimi` manages current Kimi TOML session hooks; working/blocked/idle parity remains open.

Windows `agent integration enable --provider letta` installs a silent SessionStart hook and keeps default conversations distinct per Agent.

Windows `agent integration enable --provider devin` manages six session identity hook groups in APPDATA/devin/config.json.

Windows `agent integration enable --provider hermes` publishes two owned Python plugin entries and enables them through the provider CLI; disable retains the plugin and its cache.

Windows `workspace create` accepts omitted Project, label and cwd, resolves source terminal/Workspace/home defaults, creates an ungrouped Workspace by default, and optionally focuses an existing TUI with `--focus`. Every Windows form launches its first terminal; explicit operands and no-focus use the same workflow and receipt. Workspace and focus retain separate outcomes; optional Project placement commits atomically with Workspace creation, without a separate Project preparation step. See the [Workspace creation contract](docs/windows-workspace-creation.md); `orchbee api schema` supplies the current command and schema catalog.

## Windows CLI configuration

Shared application and terminal themes apply to the native TUI from the same preference authority as Desktop. See [native themes](docs/native-themes.md).

`config check` also validates `config.toml` in the data root, `%USERPROFILE%/.orchbee` for release, which `ORCHBEE_HOME` moves, with an absolute `ORCHBEE_CONFIG_FILE` override. `[terminal] new_cwd` sets creation cwd defaults; `[worktrees] directory` selects the checkout root. Explicit command arguments override valid settings. See [configuration and precedence](docs/windows-cli-configuration.md). Windows CLI creation reads these defaults from the running Session's published configuration, so file errors are reported by the Server and `config check`, not by creation; common TUI/CLI activation reads startup policy before commitment. Direct TUI creation and Server reload are separate capabilities.

## Windows terminal observation stream

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` emits NDJSON from one existing binding without control or Host resizing. The [stream contract](docs/windows-terminal-streams.md) owns full safe ANSI frames, resets, flush-before-ACK, deadlines and cleanup. Default viewport: 120×40; default duration: unlimited. Both output formats emit `orchbee.terminal.stream` v2 records. Catalog v68 has 76 schemas and 90 native command paths. Machine control now accepts NDJSON input through `terminal session control`; interactive `terminal attach <terminal>` or `agent attach --name <name>` requires console stdin/stdout and uses the actual console size. Both accept `--takeover`, `--mouse on|off` and an optional total `--timeout`. `Ctrl+B`, then `q`, detaches; a double prefix sends one literal prefix. Paste cannot trigger detach. Output stays interactive with either global format. No Session start, Workbench focus change, reconnect or input replay occurs.

## Workspace metadata integration

The [Workspace metadata contract](docs/windows-workspace-metadata.md) defines report normalization, source sequence fences, TTL and the consistent read join. Its Server and shared wire owners are separate from canonical topology and terminal OSC metadata. `workspace report-metadata` and Workspace list/get consume that owner with bounded, correlated reads and unknown-outcome receipts. Committed metadata changes publish through the event journal; subscription invalidations and exact TTL deadlines keep TUI reads current. Configurable TUI token templates require their own integration.

Windows `panel report-metadata` supports source-owned presentation fields, guards, token patches, sequence and TTL with a binding-fenced, once-only receipt. See the [Panel metadata contract](docs/windows-panel-metadata.md).

Windows `panel report-agent`, `clear-agent-authority` and `release-agent` submit explicit source-owned reports and return correlated dispositions. See the [Agent report contract](docs/windows-panel-agent-reports.md).

## Windows CLI notifications

`orchbee notification show "Build complete" --body "All checks passed" --position bottom-right --sound done` queues a custom notification for accepting main-TUI Clients on the running Session. Configure `[ui.toast] delivery="orchbee"` for cards, `terminal` or `system` for external delivery; the default is `off`. Use `--format json` for the closed delivery receipt. See [delivery, limits and lifecycle](docs/windows-notifications.md); acceptance is queue admission, not a display or sound acknowledgement.

## Windows outer terminal title

`orchbee terminal title set "Build workspace"` publishes an override for the foreground Client, a main TUI or a Desktop window, which shows it as its window title; `orchbee terminal title clear` returns to the live `ui.window_title` template (fallback `Orchbee`). Both accept `--format json` and retain publication when no Client is connected. See [title ownership and restoration](docs/windows-terminal-title.md) for configured templates and physical-host limits.

Windows `plugin link/unlink/enable/disable/list/config-dir` and `plugin action list` manage the shared user catalog offline. `plugin action invoke` submits owned commands to an existing Session; `plugin log list` reads retained results. See [Windows plugins](docs/windows-plugins.md) for context, process ownership, bounded output, startup and event hooks, managed install/update/uninstall and Pane integration. A start receipt confirms admission; logs report execution results.

## Windows Panel graphics commands

`panel graphics set|clear|info|stream` admits closed image inputs, file/pipe acquisition and bounded raw frame streams, with one submission through existing Workbench ownership. See [Windows Panel graphics CLI](docs/windows-panel-graphics.md) for parameters, unknown receipts and producer cleanup. The [native TUI renderer](docs/native-panel-graphics.md) stages complete images and publishes successful draw facts; final composed Windows acceptance is still required.

## Native Settings

Press Ctrl+B F4 in the Windows or Linux TUI to open General and Launch profiles; Windows also exposes Settings through navigation. All common preferences use the same device record as Desktop, including locale, application and terminal themes, selection, Agent tail display, notifications, sound, scrollback and launch profiles. Edits remain drafts until Save, reset requires confirmation, and stale sources or unknown results remain explicit without replay. See the [native Settings contract](docs/native-settings.md) and [shared inventory](../../crates/workbench-protocol/docs/user-preferences.md).

Windows first-run guidance keeps terminal input active while exposing Workspace creation, Agent selection, actions and Help. Dismissal is remembered per native configuration profile. The guidance is withheld for now and its code is kept. See [native first-run guidance](docs/native-onboarding.md).
