# Windows Agent startup

`agent start` composes the Server's bounded name reservation with existing terminal input,
process inspection and canonical Agent observations. It operates on a running Windows Session.
It does not activate a Session, install providers or infer readiness from process creation.

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

Before admission, the CLI inspects this machine's managed integration for the kind, bounded by
the start deadline and at most 10000 ms. A missing executable refuses with `provider-not-found`;
a `not-installed`, `disabled` or `update-required` integration, or an unpublished adapter capsule,
refuses with `integration-not-enabled`; `conflict` refuses with
`integration-blocked`. These are `check-failed` (exit 1) with no reservation or input, and the
diagnostic names `agent integration enable` or `agent integration explain`. The preflight never
enables an integration, because that edits the provider's own configuration. Kinds without a
managed integration, manual-only management, an `unknown` state and an inspection cut short by its
budget proceed to canonical readiness. On success the text result names the
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
admission. Manual choices use the same reserve, preflight, single-submit and readiness owners. No caller
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
Session teardown wakes and joins the bounded owner. Every completion carries its generation and
Panel target, so an old result cannot settle a replacement draft or target. Unknown reservation or
input effects remain visible as unknown and are never replayed. A fully accepted command whose readiness
wait expires is labeled unconfirmed, with instructions to check terminal prompts, and remains non-retryable.
Pinned canonical attention requests update a non-capturing notice and the reopened board while the
background workflow keeps observing within its original deadline. Clearing attention can complete the
same reservation; it never submits again. The CLI retains its existing blocked-start refusal. The board does not install Agents or
change integration/default configuration. When discovery finds nothing, manual selection remains
available without claiming that any provider was detected. Cancellation wakes process sampling and
the single send exchange, but an in-progress Windows process-creation/Job-assignment call and a
best-effort reservation release of at most one second can still delay teardown; cancellation is not
instantaneous.

The terminal must contain an observed idle cmd, PowerShell or pwsh process. Running commands,
missing process details and additional processes other than known console infrastructure
refuse startup. The runtime's exact PowerShell initialization is admitted only with `-NoExit`
and the ordinary interactive switches. Arbitrary command bodies, appended arguments and
noninteractive launches remain refused. Provider hooks must report readiness. [Windows integrations](windows-agent-integrations.md)
defines managed setup and each provider's identity/state limits. Recognizing an executable does not
establish hook coverage or verify a provider's own interactive UI.

The fixed Windows launch names for canonical `cursor` and `qodercli` are `agent` and `qoder`, matching
their current native installations. Historical names remain recognition aliases; startup does not
fall back to them. An absent current executable follows the existing startup failure workflow.
Name-based process recognition is heuristic and cannot substitute for the required canonical hook
observation. Owned Windows subprocess tests exercise both canonical selections with literal arguments;
they do not establish authenticated provider readiness. References: [Cursor installation](https://cursor.com/docs/cli/installation)
and [Qoder installation](https://docs.qoder.com/cli/installation).

## Workflow and invariants

1. Validate and quote all arguments before admission. Resolve and pin the primary terminal,
   observe its shell identity, and reserve the name against its exact native binding.
2. Recheck the physical terminal, Host epoch and shell PID/creation identity. Submit one
   literal paste plus Enter; PowerShell adds Right before Enter to flush paste handling.
3. Inspect the same reservation and terminal. The Server excludes unchanged preexisting
   observations and ambiguous candidates. The CLI also requires the expected running provider.
4. Complete only for the pinned live session with fresh idle/completed state and no pending
   attention, at least 3000 ms after submission and the Server's reservation settling period.
   The Server rechecks and atomically binds the name while removing the reservation.

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

A process sample is a preflight, not an atomic input lease. User input and independent sends
may race. Canonical hooks, matching process identity and observation baselines provide bounded
local evidence, not causal authentication of a provider or a newly launched session.

## Windows argument boundary

The argument-free native form uses a short provider command: cmd invokes the fixed command name; PowerShell
resolves Application/ExternalScript explicitly before invocation, excluding functions and aliases.
No temporary launcher or execution-policy change is needed. The CLI keeps its literal argument
wrapper; its outer shell receives
an ASCII PowerShell UTF-16LE encoded command. Script arguments use
single-quoted values and array splatting. Native executables use Windows argv quoting.
Batch files use an explicit system cmd with command extensions enabled, delayed expansion
disabled and AutoRun disabled. Embedded quotes are doubled; percent expansion is interrupted
using the empty CD-substring technique also used by the pinned Rust standard library.
The process inherits the terminal console, environment and working directory. No execution
policy bypass or detached native window is introduced. A provider script remains responsible
for any further interpretation it performs internally.

## Results and recovery

Success is `orchbee.agent.start` v2 with name, provider, session, reservation identity, elapsed
time and the original terminal-send receipt. Failure is `orchbee.cli.failure.agent-start` v1
with independently retained reservation disposition, last observed reservation, bounded upstream cause and input
receipt or submission failure. Remaining time in the last observation is historical.
Known partial input retains exit 9; unknown input or a lost mutating reply retains exit 8.
Accepted input with unconfirmed cleanup is not startup success.

The board keeps a non-retryable recovery result for any pending or unknown reservation, accepted or
partially accepted input, unknown delivery frontier, or accepted worker that disconnects without its
typed completion. Closing and reopening does not clear that result. It offers a retry only after the
workflow proves zero input and a settled reservation. A queued completion is channel-authoritative
even if its worker has already exited, so worker-exit observation cannot discard a Started result.

Timeout, cancellation and release do not kill the Agent. If release is not confirmed, inspect
`agent names` and the terminal before retrying. Never replay an uncertain reserve, complete or
send automatically. `agent names` v2 lists committed bindings and separate pending reservations;
their combined Server capacity is 512. Resolve/rename/clear refuse reserved names or terminals
with `launch-pending` (names failure v3). Cold restart drops ordinary names and pending reservations. Completed managed names with an
admitted provider reference restore onto fresh terminals only when Agent resume is enabled.
Closing a terminal retires its pending reservation. A committed name follows the canonical
session lifecycle: detaching a still-live session keeps its name until end/removal or explicit
`agent rename <name> --clear`; detachment alone does not prove the provider session ended.

## Evidence and ownership

`src/agent_start` owns the typed workflow and `src/cli/agent_start.rs` adapts its report to the CLI;
the Server's
[canonical model](../../orchbee-server/docs/agent-canonical-model.md) owns reservation state.
The shared OBAP protocol owns wire validation. Tests exercise real Windows exe/ps1/cmd argument
boundaries, parser and completion scope, competing reservations, exact expiry, stale bindings,
canonical readiness, interrupted delivery and lost replies. The Windows CLI named-pipe tests
retain receipts and prove no automatic replay. Real Session/Worker/Host validation checks the
combined runtime separately. TLA model execution is not required for this Windows slice.

## Inspecting completed startup

Windows Agent Get v9 (v10 with a selected Client) includes `management` with its own read source, sampled canonical revision and `state`. The Server retains active startup identity only after accepted Complete; manual naming remains unmanaged. Active means that launch was confirmed interactive, independently of later work, attention, freshness or OS focus. Name clear, end/removal/disposal, provider mismatch and exact physical-binding retirement/replacement clear the managed record. Renaming preserves it. Completed managed names and provider intent survive clean restart through
[checkpoint version 4](../../orchbee-server/docs/agent-canonical-model.md#managed-name-restart).
Restored physical Get v14 reports active provider ownership with its own read source, without an old
Session, reservation or new interactive-readiness claim. Clearing a restored name also clears only its
matching owned persisted resume reference; later independent reports remain.

Pending status carries its original reservation and a current blocked flag. Reads never extend its deadline or complete it. Get marks promptReady false while pending; prompt reads management after its canonical/process preflight and refuses pending startup before sending. The same original admission/total and signal owner apply. Failed Status reads preserve the Agent failure family and operation management in v3. Successful Start v2 adds required interactiveReady true to its existing completion and send evidence; the v1 schema remains available for older callers. The CLI blocked-start cancellation policy is unchanged. The interactive board can wait for canonical attention to clear within its original budget; neither path infers trust prompts from screen text.

The native form leaves provider and automatic name unset while detection has no result. Found providers precede manual choices; an explicit choice stays pinned by provider identity across catalog updates. Without an explicit choice, the first found provider becomes the default; a completed empty catalog selects the first manual provider. Startup feedback appears once in the board status row and directs unconfirmed outcomes to the terminal. Only a pinned canonical attention observation declares that input is needed; process presence alone never proves readiness.
