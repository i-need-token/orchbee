# Windows Agent startup

`agent start` composes the Server's bounded name reservation with existing terminal input,
process inspection and canonical Agent observations. It operates on a running Windows Session.
It does not activate a Session, install providers or infer readiness from process creation.

## Invocation and prerequisites

```text
orchbee agent start alpha --kind codex --panel <id> --timeout 30000 --format json -- <args>...
```

The name uses `[a-z][a-z0-9_-]{0,31}`. `--kind` selects a canonical provider ID from the
runtime recognition registry; executable aliases do not become additional kinds. The target
is the Panel's primary terminal. Omitting `--panel`, or using `--current`, requires valid
caller context. `--panel` and `--current` conflict. Repeated `--arg=<value>` is an alternative
to trailing arguments; mixing both forms is refused. Empty arguments are retained, control
characters are refused, and the encoded command must fit 8000 ASCII characters.

The terminal must contain an observed idle cmd, PowerShell or pwsh process. Running commands,
missing process details and additional processes other than known console infrastructure
refuse startup. Provider hooks must report readiness. [Windows integrations](windows-agent-integrations.md)
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

The outer shell receives an ASCII PowerShell UTF-16LE encoded command. Script arguments use
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

`src/cli/agent_start` owns composition; the Server's
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

Pending status carries its original reservation and a current blocked flag. Reads never extend its deadline or complete it. Get marks promptReady false while pending; prompt reads management after its canonical/process preflight and refuses pending startup before sending. The same original admission/total and signal owner apply. Failed Status reads preserve the Agent failure family and operation management in v3. Successful Start v2 adds required interactiveReady true to its existing completion and send evidence; the v1 schema remains available for older callers. This does not change the existing blocked-start cancellation policy or add a screen-recognition rule engine.
