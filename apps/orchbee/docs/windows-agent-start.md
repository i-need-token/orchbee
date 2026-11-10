# Windows Agent startup

`agent start` composes the Server's bounded name reservation with existing terminal input,
process inspection and canonical Agent observations; the board also accepts the Panel's observed
Agent. It operates on a running Windows Session. It does not activate a Session, install
providers, inspect or require an Agent integration, or infer readiness from process creation.

## Invocation and prerequisites

```text
orchbee agent start alpha --provider codex --panel <id> --timeout 30000 --format json -- <args>...
```

The name uses `[a-z][a-z0-9_-]{0,31}`. `--provider` selects a canonical provider ID from the
runtime recognition registry; executable aliases do not become additional kinds. The target
is the Panel's primary terminal. Omitting `--panel`, or using `--current`, requires valid
caller context. `--panel` and `--current` conflict. Repeated `--arg=<value>` is an alternative
to trailing arguments; mixing both forms is refused. Empty arguments are retained, control
characters are refused, and the encoded command must fit 8000 ASCII characters. Help and the
invalid-value diagnostic list the complete `--provider` set.

Agent integrations are opt-in, so starting never inspects one and never refuses because one is
missing: neither the CLI nor the board runs a provider command or reads provider configuration
before it reserves. The failure schema still lists the retired `provider-not-found`,
`integration-not-enabled` and `integration-blocked` reasons; no start produces them. A CLI result
names its canonical Session, so only [canonical readiness](#workflow-and-invariants) completes a
CLI start; without an integration the Agent still starts and the command ends with
`start-deadline`, keeping its accepted input receipt. On success the text result names the
`agent prompt --name <name>` next step.

After a complete empty read, the [Agent board](agent-board.md) runs fixed System32 `where.exe`
queries through `agent_provider::detection_executable()`. Up to four scoped workers publish individual
candidates into a mailbox bounded by the provider count plus one final verdict. Queries have a 900 ms
run slot and 600 ms teardown allowance inside one six-second catalog deadline. A partial or failed
round retains its found candidates and the manual form. Each helper owns no output pipe and runs in
a kill-on-close Windows Job after fixed local process creation and suspended assignment. All workers
join before the catalog owner exits; UI retirement only reaps exited owners.

The `.exe`, `.com`, `.cmd`, `.bat` and `.ps1` matches are Client-PATH hints, never target availability
proof. Generic command `agent` is excluded from automatic detection: [Cursor uses that command](https://prod.cursor.com/docs/cli/installation),
but other providers can install the same basename. Cursor stays manually selectable using its existing
launch mapping. A remote origin performs no Client-PATH lookup and also offers manual selection.
The form supplies an editable automatic name and injects the exact focused Panel ID after current-draw
admission. Manual choices use the same reserve, single-submit and readiness owners. No caller
context environment variable or subprocess adapter is introduced.

Fixed local System32 process creation and suspended Job assignment are synchronous Windows ownership
primitives which application cancellation cannot interrupt. After the owned child is returned, every
wait polls cancellation and the absolute deadline; Job termination plus kill-on-close covers the
process tree, and the owner joins only after confirmed exit or the fixed teardown slot expires.
Closing or superseding the catalog cancels its owner into a retirement queue. The input thread drops
and joins only owners whose worker has exited. Session teardown owns the final join, which can still
wait for an initial process-creation or Job-assignment call already in progress; after ownership, the
deadline and teardown slot bound it.

The board's background task calls the app-private typed workflow directly under one 300000 ms
deadline, including time for interactive trust and sign-in prompts. Accepting Start closes the board
and restores terminal input immediately. Closing the board does not cancel it. `c` and the visible cancel action are explicit;
Session teardown wakes and joins every bounded owner. Every completion carries its generation and
Panel target, so an old result cannot settle a replacement draft or target. Each start guards only
its own Panel: the board opens on the focused Panel and presents that Panel's start, so a start
still running or unknown in another Panel never locks the form. A start guards its Panel while its
workflow runs and, after an unknown result, until the workflow deadline; observing the shell prompt
again (`agent-exited` or `launch-command-failed`) ends the guard at once. Unknown reservation or
input effects remain visible as unknown and are never replayed. A fully accepted command whose readiness
wait expires is labeled unconfirmed, with instructions to check the terminal; its deadline has
passed, so the Panel may start again, and [prompt-gated input](#prompt-gated-input) types nothing
into an Agent that is still running.
Pinned canonical attention requests update a non-capturing notice and the reopened board while the
background workflow keeps observing within its original deadline. Clearing attention can complete the
same reservation; it never submits again. The CLI retains its existing blocked-start refusal. The board does not install Agents or
change integration/default configuration. When discovery finds nothing, manual selection remains
available without claiming that any provider was detected. Cancellation wakes process sampling and
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
typed nothing. The workflow then releases the name, so starting again is safe. A prompt younger
than the Host's 100 ms quiet period refuses as `settling`; the workflow waits that period on its
own thread, which holds no shared channel and wakes on cancellation, and asks once more. A second
`settling` is final. The Client sends the line only when the Session's handshake listed
`prompt-gated-input`, checked before any sample or reservation, and never falls back to an ungated
send; `orchbee terminal send` and `orchbee terminal run` stay ungated.

| Refusal                                                           | Board short form (retry allowed)                  | CLI document and exit code                                                 |
| ----------------------------------------------------------------- | ------------------------------------------------- | -------------------------------------------------------------------------- |
| Preflight idle-shell refusal, or the Host's `busy` or `not-shell` | Nothing was typed: shell not at an empty prompt   | `rejected`, `reason: "terminal-not-idle"`, `3`                             |
| The Host's `unmarked`                                             | Nothing was typed: no marked prompt yet           | `rejected`, `reason: "prompt-unconfirmed"`, `promptState: "unmarked"`, `3` |
| Any other Host prompt state, including a second `settling`        | Nothing was typed: shell not at an empty prompt   | `rejected`, `reason: "prompt-unconfirmed"`, `promptState`, `3`             |
| The Session's handshake lacks `prompt-gated-input`                | Nothing was typed: Session too old                | `incompatible`, cause `stage: "capability"`, `session-too-old`, `6`        |
| The Host lacks the gate or reports no launch kind                 | Nothing was typed: terminal from an older Orchbee | `incompatible`, cause `stage: "capability"`, `terminal-too-old`, `6`       |

The board and stderr text say what to do next; the JSON carries only the reason. The board's
not-at-prompt text and the CLI's `prompt-unconfirmed` text other than `unmarked` say to finish the
running program, or clear the line and press Enter for a new prompt. Input typed after a prompt
blocks typing until the shell's next prompt, so the person clears the line and presses Enter. The
TUI status line shows the board's full text when it fits and otherwise the short form above, in
the same locale; every other start notice except the already short cancelling one also has a
short form. A capability refusal has `details.reason` `startup-failed` with the shared
[capability failure](../../../docs/cli/output.md) as its `cause`.

The fixed Windows launch names for canonical `cursor` and `qodercli` are `agent` and `qoder`, matching
their current native installations. Historical names remain recognition aliases; startup does not
fall back to them. An absent current executable follows the existing startup failure workflow.
Name-based process recognition alone is heuristic: an observed board start also needs the
Server's published run and published Agent state, and a CLI start needs the canonical observation. Owned Windows subprocess tests exercise both canonical selections with literal arguments;
they do not establish authenticated provider readiness. References: [Cursor installation](https://cursor.com/docs/cli/installation)
and [Qoder installation](https://docs.qoder.com/cli/installation).

## Workflow and invariants

1. Validate and quote all arguments before admission. Resolve and pin the primary terminal,
   observe its shell identity, and reserve the name against its exact native binding.
2. Recheck the physical terminal, Host epoch and shell PID/creation identity. Submit one
   literal paste plus Enter as one [prompt-gated](#prompt-gated-input) line; PowerShell adds
   Right before Enter to flush paste handling.
3. Inspect the same reservation and terminal. The Server excludes unchanged preexisting
   observations and ambiguous candidates. The CLI also requires the expected running provider,
   which the [launch receipt](#launch-receipt) and the Server identity result can refute early.
4. Complete only for the pinned live session with fresh idle/completed state and no pending
   attention, at least 3000 ms after submission and the Server's reservation settling period.
   The Server rechecks and atomically binds the name while removing the reservation.
5. The board alone also confirms by observation, whichever comes first: while no canonical
   candidate exists, at least 3000 ms after submission, the Server's published run names the
   requested provider and the Panel's published Agent state for the reserved Terminal generation
   is idle or blocked and names the same provider. Observation binds no name: the workflow
   releases the reservation and reports the Agent started. Session resume stays hook-only.

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

Reserve also returns a Server-minted run token: a nonzero, never-reused 32-digit lowercase
hexadecimal value. When the Host's launch kind is managed PowerShell, Agent
start prefixes its one submitted line with `<#orchbee-run:<token>#> `. The verified reader
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

An argument-free start, from the board or the CLI, uses a short provider command so the shell
echoes one line: cmd invokes the fixed command name; PowerShell resolves Application/ExternalScript
explicitly before invocation, excluding functions and aliases. No temporary launcher or
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

Success is `orchbee.agent.start` v1 with name, provider, session, reservation identity, elapsed
time and the original terminal-send receipt. Failure is `orchbee.cli.failure.agent-start` v1
with independently retained reservation disposition, last observed reservation, bounded upstream cause and input
receipt or submission failure. Remaining time in the last observation is historical.
Known partial input retains exit 9; unknown input or a lost mutating reply retains exit 8.
Accepted input with unconfirmed cleanup is not startup success.

The board keeps an unknown recovery result for any pending or unknown reservation, accepted or
partially accepted input, unknown delivery frontier, or accepted worker that disconnects without its
typed completion. Closing and reopening does not clear that result; it guards only its own Panel
and only until the workflow deadline. The Panel may start again earlier only after the workflow
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

`src/cli/agent_start.rs` retains CLI selection and signal finalization;
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
replies. Windows CLI named-pipe tests retain receipts and prove no automatic replay.
Real Session/Worker/Host validation checks the combined runtime separately. TLA model execution
is not required for this Windows slice.

## Inspecting completed startup

Windows Agent Get v1 (v1 with a selected Client) includes `management` with its own read source, sampled canonical revision and `state`. The Server retains active startup identity only after accepted Complete; manual naming remains unmanaged. Active means that launch was confirmed interactive, independently of later work, attention, freshness or OS focus. Name clear, end/removal/disposal, provider mismatch and exact physical-binding retirement/replacement clear the managed record. Renaming preserves it. Managed names are transient and are not restored from checkpoints.

Pending status carries its original reservation and a current blocked flag. Reads never extend its deadline or complete it. Get marks promptReady false while pending; prompt reads management after its canonical/process preflight and refuses pending startup before sending. The same original admission/total and signal owner apply. Failed Status reads preserve the Agent failure family and operation management in v1. Successful Start v1 adds required interactiveReady true to its existing completion and send evidence; the v1 schema remains available for older callers. The CLI blocked-start cancellation policy is unchanged. The interactive board can wait for canonical attention to clear within its original budget; neither path reports input as needed from screen text, though the board's observed confirmation accepts a published blocked state as started.

The native form leaves provider and automatic name unset while detection has no result. Found providers precede manual choices; an explicit choice stays pinned by provider identity across catalog updates. Without an explicit choice, the first found provider becomes the default; a completed empty catalog selects the first manual provider. Startup feedback appears once in the board status row and directs unconfirmed outcomes to the terminal. Only a pinned canonical attention observation declares that input is needed; process presence alone never proves readiness.
