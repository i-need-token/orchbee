# Windows Agent startup

`agent start` starts one provider command on a running Windows Session, in a new Tab, a new split
or an existing Panel's idle shell. A new Panel's shell can run the exact catalog command as its
first command; otherwise the command is typed once at the confirmed prompt. Without a name that is
the whole start: after a short
[early check](#workflow-and-invariants) the Agent is observed exactly like one the person typed by
hand, and its row shows it [opening](../../orchbee-server/docs/agent-screen-model.md#opening) until
its screen is recognised. With a name, the start also composes the Server's bounded name
reservation with process inspection and canonical Agent observations, or the Panel's observed
Agent while no canonical Session has reported. It does not activate a Session, install providers,
inspect or require an Agent integration, or infer readiness from process creation.

## Invocation and prerequisites

```text
orchbee agent start <provider> [--new-tab | --here | --split right|down] [--name <name>]
    [--panel <id> | --current] [--timeout <ms>] [--format text|json] [-- <args>...]
```

The positional `<provider>` is a canonical provider ID from the runtime recognition registry, the
same ID `agent integration` and `agent doctor` take with `--provider`; executable aliases do not
become additional kinds. `--provider` is not an `agent start` option, and any other word before
`--` is refused, so it never reaches the Agent. Help and the invalid-value diagnostic list the
complete provider set. The optional name uses `[a-z][a-z0-9_-]{0,31}`.

The place is relative to a reference Panel, named by `--panel`, by `--current`, or, with neither,
by valid caller context; `--panel` and `--current` conflict, and there is no Client-selection
fallback. `--new-tab`, the default, opens a new Tab in the reference Panel's Workspace;
`--split right|down` splits the reference Panel; `--here` types into the reference Panel's own idle
shell. The place flags are mutually exclusive. `--here` aimed at the caller's own Panel, whose
shell is running the command, is refused with `own-panel` (exit 2) before any effect: from the
caller context alone when `--panel` is absent, or once the Snapshot resolves an explicit `--panel`
to the caller's Panel. The target is the chosen Panel's primary terminal.

Agent arguments follow `--`. Repeated `--arg=<value>` is an alternative to trailing arguments;
mixing both forms is refused. Empty arguments are retained, control characters are refused, and the
encoded command must fit 8000 ASCII characters.

A new Tab or split is created with the TUI's Server operations, `tab.create` or `panel.split`,
under one stale-view-retried request identity. A start with no arguments and no name is exactly
the catalog command: the creation carries `options.agentStart` and the new shell runs the Agent
as its [first command](#start-as-the-shells-first-command); the CLI returns once the Panel exists
and that shell is confirmed managed PowerShell, and types the start like the TUI on any other
shell. A start with arguments or a name creates a plain shell and types the workflow once at its
first confirmed prompt; a named start keeps the reservation and readiness wait. While the new
terminal is unbound or its shell is not idle, the CLI only reads again, every 250 ms until two
seconds before the deadline. A submission the [prompt gate](#prompt-gated-input) refused typed
nothing and is this workflow's stale-base refusal: it is attempted again under a fresh request
identity at most three times. A refused creation reports `place-failed` with the Server's reason;
a Panel created before a later step fails stays open, and the outcome names it.

Agent integrations are opt-in, so starting never inspects one and never refuses because one is
missing: neither the CLI nor the TUI runs a provider command or reads provider configuration
before it types. A start without a name reserves nothing. A named start without an integration is
confirmed by [observation](#workflow-and-invariants) and binds no name. On success the text result
is one outcome line that names the place and Panel, such as
`Started Codex in a new Tab (Panel <id>).`, then the next step:
`agent wait --panel <panel> --until idle` after a start without a name,
`agent prompt --name <name>` after a canonical confirmation, or `agent prompt --panel <panel>`
after an observed one, whose name was not bound.

Each time the [Start Agent picker](agent-board.md) opens, it runs fixed System32 `where.exe`
queries through `agent_provider::detection_executable()`. Up to four scoped workers publish individual
candidates into a mailbox bounded by the provider count plus one final verdict. Queries have a 900 ms
run slot and 600 ms teardown allowance inside one six-second catalog deadline. A partial or failed
round retains its found candidates, and an incomplete first round claims nothing installed. Each helper owns no output pipe and runs in
a kill-on-close Windows Job after fixed local process creation and suspended assignment. All workers
join before the catalog owner exits; UI retirement only reaps exited owners.

The `.exe`, `.com`, `.cmd`, `.bat` and `.ps1` matches are Client-PATH hints, never target availability
proof. A command that the process-identity registry classes as generic is excluded from automatic
detection: [Cursor uses `agent`](https://prod.cursor.com/docs/cli/installation), but other providers
can install the same basename, and Snow CLI's `snow` is a common word that unrelated programs
also use. The picker lists these providers as "can't check" and still starts them using their
existing launch mapping. Shared names such as `amp` and `mcode` are still advertised, as before. A
remote origin performs no Client-PATH lookup and lists every provider as not checked. The picker
lists each provider by its product name, has no name field, and after current-draw admission
passes the focused Panel ID, or the first Panel of a Tab it created, to the same single-submit
owner. No caller context environment variable or subprocess adapter is introduced.

Fixed local System32 process creation and suspended Job assignment are synchronous Windows ownership
primitives which application cancellation cannot interrupt. After the owned child is returned, every
wait polls cancellation and the absolute deadline; Job termination plus kill-on-close covers the
process tree, and the owner joins only after confirmed exit or the fixed teardown slot expires.
Closing or superseding the catalog cancels its owner into a retirement queue. The input thread drops
and joins only owners whose worker has exited. Session teardown owns the final join, which can still
wait for an initial process-creation or Job-assignment call already in progress; after ownership, the
deadline and teardown slot bound it.

The picker's background task calls the app-private typed workflow directly. A start without a name
runs under one 20000 ms deadline covering admission, the single send and the early check; a named
start runs under one 300000 ms deadline, including time for interactive trust and sign-in prompts.
Choosing a provider closes the picker and restores terminal input on the same frame; nothing in
the TUI cancels an accepted start, and Session teardown wakes and joins every bounded owner. Every
completion carries its generation and Panel target, so an old result cannot settle another target.
Each start guards only its own Panel, so a start still running or unknown in another Panel never
blocks a new one; a guarded focused Panel makes the picker open the Agent in a new Tab. A refusal
that typed nothing in an existing Panel starts in a new Tab instead. A start guards its Panel while its
workflow runs and, after an unknown result, until the workflow deadline; observing the shell prompt
again (`command-not-found`, `agent-exited` or `launch-command-failed`) ends the guard at once.
Unknown reservation or input effects remain visible as unknown and are never replayed. A named
start whose fully accepted command's readiness wait expires is labeled unconfirmed, with
instructions to check the terminal; its deadline has passed, so the Panel may start again, and
[prompt-gated input](#prompt-gated-input) types nothing into an Agent that is still running.
For a named start, pinned canonical attention requests update a non-capturing notice while the
background workflow keeps observing within its original deadline. Clearing attention can complete the
same reservation; it never submits again. The CLI retains its existing blocked-start refusal. The picker does not install Agents or
change integration/default configuration. When discovery finds nothing, every provider remains
startable from the not-found group without claiming that any was detected. Cancellation wakes process sampling and
the single send exchange, but an in-progress Windows process-creation/Job-assignment call and a
best-effort reservation release of at most one second can still delay teardown; cancellation is not
instantaneous.

The terminal's root must be a shell the Host itself launched with prompt marks: managed
PowerShell or pwsh, or interactive cmd, as the `launchKind` of the Host's process sample records.
Startup never reads the root's command line. Any other root, running commands, missing process
details and additional processes other than known console infrastructure refuse startup with
`terminal-not-idle`. Unmanaged PowerShell and cmd started with `/Q` or under `echo off` show no
marked prompt, so they cannot start an Agent. A sample without a launch kind comes from a Host
that predates the prompt gate and refuses as `terminal-too-old`. The rest of this strict
idle-shell rule is `idle_shell_only` in `orchbee-agent-observation-protocol`; the Server's
[Agent session resume](../../orchbee-server/docs/agent-session-resume.md#delivery) applies it, the
run prefix and records before it types a resume command into a restored shell. Canonical
readiness needs a provider integration, which is opt-in; [Windows integrations](windows-agent-integrations.md)
defines managed setup and each provider's identity/state limits. Recognizing an executable does not
establish hook coverage or verify a provider's own interactive UI.

### Prompt-gated input

The start line is an idle-prompt send: the terminal's Host types it only while the root shell
waits at a fresh, empty, marked prompt, and otherwise refuses with `prompt-unconfirmed`, having
typed nothing. A named start then releases its name, so starting again is safe. A prompt younger
than the Host's 100 ms quiet period refuses as `settling`; the workflow waits that period on its
own thread, which holds no shared channel and wakes on cancellation, and asks once more. A second
`settling` is final. The Client sends the line only when the Session's handshake listed
`prompt-gated-input`, checked before any sample or reservation, and never falls back to an ungated
send; `orchbee terminal send` and `orchbee terminal run` stay ungated.

| Refusal                                                           | TUI short form                                    | CLI document and exit code                                                 |
| ----------------------------------------------------------------- | ------------------------------------------------- | -------------------------------------------------------------------------- |
| Preflight idle-shell refusal, or the Host's `busy` or `not-shell` | Nothing was typed: shell not at an empty prompt   | `rejected`, `reason: "terminal-not-idle"`, `3`                             |
| The Host's `unmarked`                                             | Nothing was typed: no marked prompt yet           | `rejected`, `reason: "prompt-unconfirmed"`, `promptState: "unmarked"`, `3` |
| Any other Host prompt state, including a second `settling`        | Nothing was typed: shell not at an empty prompt   | `rejected`, `reason: "prompt-unconfirmed"`, `promptState`, `3`             |
| The Session's handshake lacks `prompt-gated-input`                | Nothing was typed: Session too old                | `incompatible`, cause `stage: "capability"`, `session-too-old`, `6`        |
| The Host lacks the gate or reports no launch kind                 | Nothing was typed: terminal from an older Orchbee | `incompatible`, cause `stage: "capability"`, `terminal-too-old`, `6`       |

The TUI and stderr text say what to do next; the JSON carries only the reason. The TUI's
not-at-prompt text and the CLI's `prompt-unconfirmed` text other than `unmarked` say to finish the
running program, or clear the line and press Enter for a new prompt. Input typed after a prompt
blocks typing until the shell's next prompt, so the person clears the line and presses Enter. The
TUI status line shows its full text when it fits and otherwise the short form above, in
the same locale; every other start notice also has a short form under 60 columns. A capability refusal has `details.reason` `startup-failed` with the shared
[capability failure](../../../docs/cli/output.md) as its `cause`.

The fixed Windows launch names for canonical `cursor`, `qodercli` and `minimax-code` are `agent`,
`qoder` and `mcode`, matching their current native installations. Command Code starts as
`command-code`, never as its package's `cmd` command, which names the system command processor.
Historical and sibling names remain recognition aliases; startup does not fall back to them.

OpenClaw opens its interactive session through a subcommand rather than its bare command:
`openclaw chat`. The closed provider table in `agent_provider::launch_arguments()` holds that fixed
word, never caller text; the short argument-free form appends it after the command, and caller
arguments follow it in the literal wrapper. Process recognition requires the same subcommand: its bare command stays unclassified. An absent current executable follows the existing startup failure workflow.
Name-based process recognition alone is heuristic: an observed start also needs the Server's
published run and published Agent state. Owned Windows subprocess tests exercise both canonical selections with literal arguments;
they do not establish authenticated provider readiness. References: [Cursor installation](https://cursor.com/docs/cli/installation)
and [Qoder installation](https://docs.qoder.com/cli/installation).

### Launch-time conversation ids

Grok, Gemini, Qwen Code, Copilot and Pi open a new conversation under an id the caller chooses,
`--session-id <uuid>`. A start of one of them, from the TUI or `orchbee agent start`, passes a
fresh id that way so the Server can [resume the conversation](../../orchbee-server/docs/agent-session-resume.md#command-line)
after a restart without a hook:

- The id is a version-4 UUID in lowercase hyphenated form, from 16 bytes of operating-system
  entropy, made once per start, so both launch forms carry the same one. Without entropy the start
  goes ahead without an id.
- `agent_provider::launch_session_option()` names the option beside the fixed launch words. It
  follows them and precedes the caller's arguments; the short form single-quotes the option and the
  id for PowerShell and leaves them bare for cmd, where both are inert.
- Caller arguments that already choose a conversation add no id: the session option itself, with or
  without `=`, a resume, a continue of the latest conversation, a fork, or none at all (Grok `-s`,
  `--session-id`, `-r`, `--resume`, `-c`, `--continue`, `--load`, `--fork-session`; Gemini
  `--session-id`, `-r`, `--resume`; Qwen Code adds `-c`, `--continue`; Copilot `--session-id`,
  `-r`, `--resume`, `--continue`; Pi `--session-id`, `--session`, `-r`, `--resume`, `-c`,
  `--continue`, `--no-session`). Words after `--` are not options.
- Orchbee keeps no copy of the id. The Server reads it back from the Agent process's own command
  line, the same way it reads one the person typed.
- Claude Code gets no id: its own run table follows the conversation it holds, which a launch id
  would not. The other providers document no launch-time id.
- The command budget check reserves room for an id before admission, so a start that fits never
  fails later for the id it adds.
- A start in a new Tab, split or Worktree gets its id from the Server, which composes the first
  command below; the same rules apply, since such a start has no caller arguments.

### Start as the shell's first command

A picker start in a new Tab, split or Worktree types nothing, nor does a CLI start in a new Tab or
split without arguments or a name. The Client sends the ordinary
`tab.create`, or the split keys' `panel.split` of the focused Panel, with `options.agentStart`, or
the New Worktree form's create with `agentStart`, the provider's canonical id from the closed
startable Agent catalog; no request carries command text. A Worktree create hands the id to the
launch of its new Workspace's first shell; a reused Workspace starts nothing. The Server resolves the catalog's command name and fixed arguments,
adds a fresh launch-time conversation id where the provider takes one, and attaches the command to
the authorized launch. The Worker passes it to managed PowerShell through `ORCHBEE_AGENT_START`, and
the managed startup's last step clears that variable, writes the run's start marker under a fresh run
token, and runs exactly the line a typed start would:

```text
& (Get-Command '<command>' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1) '<argument>' ...
```

The shell is the Panel's process and the Agent its first command, so the process tree, command line,
recognition and resume evidence match a typed start's, and the person is left at the same shell when
the Agent exits. A command that is not found ends only the startup script: the shell's own error
stays visible above its first prompt and the shell keeps working.

Every other shell starts without the command; the Worker also removes the variable so a value
inherited from its environment never runs. When the Tab's creation or the split commits, or the
Worktree create's Opened result reports that its first shell started, the Client watches the new
Panel. For managed PowerShell, per the Host's `launchKind`, it only runs the early check, counted
from the start's acceptance with a 10000 ms settle that also covers the shell's own startup: an idle
shell after the Agent was seen is `agent-exited`; an idle shell with no Agent seen by the end is
`command-not-found`. Any other shell, such as cmd or a custom launch profile, gets the typed start at
its first idle prompt, repeated while the new shell settles within a 30000 ms deadline. Such a start
reserves nothing and offers no name. The CLI does the same, except that once the new shell is
confirmed managed PowerShell it returns `launched` without the early check. POSIX shells have no
managed startup that takes a first command, and the TUI starts no Agent there; the CLI parses the
same grammar on POSIX and reports its existing unsupported outcome.

## Workflow and invariants

1. Validate and quote all arguments before admission. Resolve and pin the primary terminal and
   observe its shell identity. A named start reserves the name against its exact native binding,
   then rechecks the physical terminal, Host epoch and shell PID/creation identity.
2. Submit one literal paste plus Enter as one [prompt-gated](#prompt-gated-input) line;
   PowerShell adds Right before Enter to flush paste handling.
3. A start without a name ends with its early check, sampling the terminal's processes for up to
   3000 ms after submission. A shell back at its idle prompt after a process of the requested
   provider appeared reports `agent-exited`; a shell back at its prompt at 3000 ms without one
   reports `command-not-found`. Otherwise the shell still runs something, or the sample can no
   longer be read, and the start is sent: from then on the Server observes the Agent like one
   typed by hand. An Agent that exits before the first sample, about 100 ms after submission,
   reports `command-not-found`; the Panel's output tells the two apart. Steps 4 to 6 apply only
   to a named start.
4. Inspect the same reservation and terminal. The Server excludes unchanged preexisting
   observations and ambiguous candidates. The CLI also requires the expected running provider,
   which the [launch receipt](#launch-receipt) and the Server identity result can refute early.
5. Complete only for the pinned live session with fresh idle/completed state and no pending
   attention, at least 3000 ms after submission and the Server's reservation settling period.
   The Server rechecks and atomically binds the name while removing the reservation.
6. Observation also confirms, whichever comes first: while no canonical candidate exists, the
   Server's published run names the requested provider and the Panel's published Agent state for
   the reserved Terminal generation is idle or blocked and names the same provider. It waits
   3000 ms after submission when no integration was ever enabled, meaning this environment's
   Capsule folder and release's legacy folder are absent; otherwise, and for a remote origin
   whose integrations this Client cannot see, it waits 10000 ms, so an enabled hook can bind the
   name first. Observation binds no name: the workflow releases the reservation and reports the
   Agent started. The CLI keeps its blocked-start rule and refuses a blocked state as
   `agent-blocked`; the TUI accepts it. Session resume needs a hook session or a session the
   published run's own process names: Claude Code's run table entry, else the conversation id
   bound on its command line, such as a [launch-time id](#launch-time-conversation-ids)
   ([Agent session resume](../../orchbee-server/docs/agent-session-resume.md#process-source)).

The default total deadline is 30000 ms, with 3001–300000 admitted. Each I/O step is bounded
by five seconds and the remaining original budget. Cancellation stops observation; name
release may use at most one second of that original budget. No operation renews the deadline.
Server TTL starts at reserve acceptance, so transport delay can make it outlast the CLI's
deadline. Exact expiry, replacement, retirement and disposal remove the reservation.

A changed or temporarily unavailable Host process sample is polled again under the same
original deadline. Only these two typed sampling outcomes are retried; permanent Host
refusals and transport failures stop the workflow. No input or reservation mutation is replayed.
If a read exhausts the total startup deadline, the result is timeout even when that read's
transport reports a closed connection. Its cause and the accepted input receipt remain visible;
an unconfirmed mutation still reports outcome unknown.

A process sample is a preflight, not an atomic input lease. The Host's prompt gate decides when
it types, against its own screen state and input queue, so user input and independent sends that
leave the shell busy or its line nonempty refuse the start instead of racing it. Canonical hooks,
matching process identity and observation baselines provide bounded local evidence, not causal
authentication of a provider or a newly launched session.

## Launch receipt

A named start's Reserve also returns a Server-minted run token: a nonzero, never-reused 32-digit
lowercase hexadecimal value. When the Host's launch kind is managed PowerShell, Agent
start prefixes its one submitted line with `<#orchbee-run:<token>#> `. A start without a name has
no reservation: its line carries a prefix with a token the Client mints from OS entropy, only so
that the line stays out of the person's history, and its early check reads the shell's prompt
instead of a receipt. The verified reader
wrapper strips that prefix, exports the token as `ORCHBEE_RUN_TOKEN` to that command only, and
echoes it in the start and finish markers; the prompt removes the variable when the command
returns. The prefixed line enters neither PSReadLine history (memory, saved file or Up-arrow
recall) nor `Get-History`. An interactive PowerShell whose own prompt shows the marks without the
managed wrapper reads the prefix as a comment, runs the command unchanged and keeps the line in its
own history. cmd receives no prefix; its console
line history keeps the short provider command, because removing one entry would reset the user's
whole doskey history.

The Server latches each reservation's receipt from its sampler's latest frame of the same Host
Terminal: `awaiting`, `started`, then `finished` with PowerShell's `$?`. Inspect returns it as
`receipt`. A finished receipt ends startup at once: `$?` false reports `launch-command-failed`,
for example when the provider is not found, and true reports `agent-exited`. Without a receipt,
the idle-shell check after 3000 ms still reports `agent-exited`.

After each process sample, startup reads the Server's identity result and run anchor for the
reserved Terminal, and its foreground follows the
[process-info rule](native-cli-query.md#windows-interpretation-and-agent-input-preflight); startup
selects nothing from its own sample. A published run naming another provider reports
`agent-provider-mismatch`, such as Grok answering Cursor's generic `agent` name; ambiguity reports
`agent-runtime-ambiguous` and a missing root `agent-exited`. Readiness also requires the published
run to name the requested provider. A settling Server, or a failed or mismatched read, selects no
foreground for that step and startup keeps polling within its budget.

- One reservation has one run token; only its own Host Terminal echoing that token advances its
  receipt, and a receipt phase never moves back.
- The prefix never adds input: one line is typed at most once and never replayed.
- The token reaches only the command the managed wrapper ran.
- The receipt can only end startup early and never completes it. The Server's published run is
  required for completion, so a settling Server delays completion until the run settles but never
  fails startup on its own.
- The identity result is `confirmed` under the
  [typed-run boundary rule](native-terminal-run.md#typed-run-boundaries), which also covers this
  prefixed command; cmd launches stay `recognized`.

## Windows argument boundary

An argument-free start, from the TUI or the CLI, uses a short provider command so the shell
echoes one line: cmd invokes the fixed command name; PowerShell resolves Application/ExternalScript
explicitly before invocation, excluding functions and aliases. A
[launch-time id](#launch-time-conversation-ids) rides both forms: after the fixed words in the
short form, and as ordinary arguments ahead of the caller's in the wrapper. No temporary launcher or
execution-policy change is needed. A start with arguments uses the literal argument wrapper; its
outer shell receives an ASCII PowerShell UTF-16LE encoded command that sets only UTF-8 encoding
before dispatch. Script arguments use
single-quoted values and array splatting. Native executables use Windows argv quoting.
Batch files use an explicit system cmd with command extensions enabled, delayed expansion
disabled and AutoRun disabled. Embedded quotes are doubled; percent expansion is interrupted
using the empty CD-substring technique also used by the pinned Rust standard library.
The process inherits the terminal console, environment and working directory. No execution
policy bypass or detached native window is introduced. A provider script remains responsible
for any further interpretation it performs internally.

## Results and recovery

Success is `orchbee.agent.start` v1 with `confirmation`, the requested name, provider,
reservation identity, elapsed time, the original terminal-send receipt, `place` (`new-tab`,
`here`, `split-right` or `split-down`) and `panelId`, the Panel the Agent started in.
`confirmation` is `launched` when a new Panel's shell runs the Agent as its first command:
`submission` is null, because nothing was typed. It is `sent` after a typed start without a name.
After either, `name` and `reservationId` are null and `interactiveReady` is false, because nothing
waited for readiness. It is `session` when the canonical Session confirmed readiness; `sessionId`
then names it and holds the name. It is `observed` when observation confirmed readiness;
`sessionId` is absent and the name is not bound, so later commands address the Agent by `--panel`
or `--terminal`. Failure is `orchbee.cli.failure.agent-start` v1 with independently retained
reservation disposition, last observed reservation, bounded upstream cause, input receipt or
submission failure, `place`, and `createdPanelId`, the Panel this invocation created, which stays
open, or null. `own-panel` is a usage refusal. `place-failed` follows the creation's own failure
kind; its cause names the `tab.create` or `panel.split` operation and the Server's reason, or
`terminal-launch-failed` when the created Panel's shell did not start. Remaining time in the last
observation is historical. Known partial input retains exit 9; unknown input or a lost mutating
reply retains exit 8. Accepted input with unconfirmed cleanup is not startup success.

The picker's startup owner keeps an unknown result for any pending or unknown reservation, accepted
or partially accepted input, unknown delivery frontier, or accepted worker that disconnects without
its typed completion. Closing and reopening does not clear that result; it guards only its own
Panel and only until the workflow deadline. The Panel may start again earlier only after the workflow
proves zero input and a settled reservation, or observes the shell prompt again. A queued completion
is channel-authoritative even if its worker has already exited, so worker-exit observation cannot
discard a Started result.

Timeout, cancellation and release do not kill the Agent. If release is not confirmed, inspect
`agent names` and the terminal before retrying. Never replay an uncertain reserve, complete or
send automatically. `agent names` v1 lists committed bindings and separate pending reservations;
their combined Server capacity is 512. Resolve/rename/clear refuse reserved names or terminals
with `launch-pending` (names failure v1). Cold restart drops all names and pending reservations.
Closing a terminal retires its pending reservation. A committed name follows the canonical
session lifecycle: detaching a still-live session keeps its name until end/removal or explicit
`agent rename <name> --clear`; detachment alone does not prove the provider session ended.

## Evidence and ownership

`crates/workbench-client/src/agent_start.rs` and `crates/workbench-client/src/agent_start/` own the typed workflow and report, retaining
`ReservationState` and `DeliveryDisposition` independently of the final verdict.
`crates/workbench-client/src/agent_runtime/startup/` owns the shared Snapshot/OBAP reads, exact binding resolution,
process attach/read/detach, the Server identity and published-state reads, and single prompt
submission. These operations preserve the admitted
Client, original absolute deadline and wake; process-read failures still reach one best-effort
detach, and the Paste/optional Codex Right/Enter line is typed at most once; it is submitted a
second time only after a `settling` refusal that typed nothing.

`src/cli/agent_start.rs` retains CLI selection, the reference and own-Panel check, the new-Panel
attempt loop and signal finalization; `src/cli/agent_start/place.rs` creates the new Tab or split;
`src/cli/agent_start/report.rs` projects domain facts into the existing CLI failure, status and
JSON fields. Workbench consumes typed reservation, delivery and failure facts directly.
The Server's [canonical model](../../orchbee-server/docs/agent-canonical-model.md) owns reservation
state, and the shared OBAP protocol owns wire validation.

Host envelope and process payload validation is covered in
`crates/workbench-client/src/agent_runtime/startup/processes/tests.rs`. `src/cli/agent_start/tests.rs` drives the existing
receipt matrix through the production domain-to-CLI conversion; Workbench recovery precedence
and the Panel-scoped guard remain covered in `crates/workbench-client/src/agent_board/start/jobs/tests.rs`. The existing tests also
exercise real Windows exe/ps1/cmd argument boundaries, parser and completion scope, competing
reservations, exact expiry, stale bindings, canonical readiness, interrupted delivery and lost
replies. `crates/workbench-client/src/agent_start/command/tests.rs` covers launch-time ids for
the five providers in both launch forms: exactly one valid fresh id, none where the caller chose a
conversation or the provider takes none, and a budget that leaves room for it. Windows CLI
named-pipe tests retain receipts and prove no automatic replay.
Real Session/Worker/Host validation checks the combined runtime separately. TLA model execution
is not required for this Windows slice.

## Inspecting completed startup

Windows Agent Get v1 (v1 with a selected Client) includes `management` with its own read source, sampled canonical revision and `state`. The Server retains active startup identity only after accepted Complete; manual naming remains unmanaged. Active means that launch was confirmed interactive, independently of later work, attention, freshness or OS focus. Name clear, end/removal/disposal, provider mismatch and exact physical-binding retirement/replacement clear the managed record. Renaming preserves it. Managed names are transient and are not restored from checkpoints.

Pending status carries its original reservation and a current blocked flag. Reads never extend its deadline or complete it. Get marks promptReady false while pending; prompt reads management after its canonical/process preflight and refuses pending startup before sending. The same original admission/total and signal owner apply. Failed Status reads preserve the Agent failure family and operation management in v1. Successful Start v1 adds required interactiveReady to its existing completion and send evidence: true after a named start's confirmation and false after a start without a name. The CLI blocked-start cancellation policy is unchanged. The interactive board can wait for canonical attention to clear within its original budget; neither path reports input as needed from screen text, though the board's observed confirmation accepts a published blocked state as started and the
CLI refuses it as `agent-blocked`.

The native form leaves the provider unset while detection has no result; its name field starts empty and stays optional. Found providers precede manual choices; an explicit choice stays pinned by provider identity across catalog updates. Without an explicit choice, the first found provider becomes the default; a completed empty catalog selects the first manual provider. Startup feedback appears once in the board status row and directs unconfirmed outcomes to the terminal. Only a pinned canonical attention observation declares that input is needed; process presence alone never proves readiness.
