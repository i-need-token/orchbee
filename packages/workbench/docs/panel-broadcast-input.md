# Panel Broadcast Input

## Contract

A person operating several terminal Panels in one Tab can open one dedicated broadcast input bar
and send live committed text or physical key taps to every displayed terminal. The mode is explicit:
ordinary terminal input is never mirrored, every target remains visible, and the bar identifies the
target count and end-session controls before accepting input.

The public `workbench.panel.active.broadcast-input.open` Action is discoverable through Actions
search and Panel menus. It has no default keyboard shortcut and creates no durable setting or
storage migration. The Action is enabled only when the active Tab is not maximized, has at least two
canonical Panel leaves, owns no conflicting Panel interaction, and every leaf's displayed
Presentation is `running` with one mounted write-ready terminal sink.

The target snapshot contains exactly one displayed Presentation per canonical depth-first Panel
leaf, including the previously focused Panel. Hidden primary or child Presentations, other Tabs,
other native Windows, remote clients, and unmounted or read-only surfaces are never targets.

## Domain model

| Type                           | Meaning                                                                                          |
| ------------------------------ | ------------------------------------------------------------------------------------------------ |
| `PanelBroadcastTarget`         | One exact Panel ID and its displayed Presentation ID                                             |
| `PanelBroadcastTargetSnapshot` | The active Tab ID, source focus Panel ID, and unique ordered targets captured at start           |
| `PanelBroadcastInput`          | Either non-empty committed text or one target-neutral physical key tap with exact modifier facts |
| `PanelBroadcastInputEnvelope`  | One session-local monotonic input ID and its input                                               |
| `PanelBroadcastSessionState`   | `idle`, `active` with its immutable snapshot and FIFO, or `stopped` with a non-sensitive notice  |
| `PanelBroadcastCommand`        | Dispatch the exact queue head or restore the exact source terminal focus                         |
| `PanelBroadcastTransition`     | Complete next state, observable result, and zero or more commands                                |

DOM keyboard, composition, input, paste, focus, and blur events are untrusted presentation input.
The broadcast bar normalizes them before the domain boundary. Composition updates never become
input; only the final composition commit does. Clipboard paste, file drop, mouse input, empty text,
unknown keys, and operating-system Meta chords are consumed without a broadcast command.

The transient FIFO may contain terminal content and is never persisted, logged, included in Action
observations, copied into a stopped notice, or projected into shared-document or Client View state.
Closing or invalidating a session clears every queued payload.

## Start decision

| Condition                                                                                  | Result                                                          |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| No active Tab, fewer than two Panel leaves, or maximization                                | Unavailable                                                     |
| Visual selection, resize, close confirmation, drag, or another broadcast session is active | Unavailable                                                     |
| Any displayed Presentation is not `running`, lacks a mounted sink, or is not write-ready   | Unavailable; no partial target set is created                   |
| Duplicate Panel or Presentation identity in canonical order                                | Unavailable as an invalid snapshot                              |
| Every exact target is eligible                                                             | Start one `active` session with an empty FIFO and visible input |

Start is the only session-state commit performed by the Action. It does not send terminal input.
The Action revalidates all conditions immediately before commit and marks the Action committed only
after the active session exists. A contextual Panel target must still exist, but the snapshot's
source focus is always the active Tab's actual focused Panel; a secondary-pointer menu invocation
does not silently become the later focus-restoration owner.

Accepted events are the sole writer of session state. React renders project that state and may lag
behind an admitted event; they cannot restore an earlier state or admit a second active session.
Readiness checks validate the current admitted snapshot, so an older render cannot invalidate a
replacement session because its own snapshot has expired.

Each mounted sink owns an exact registration. Failed subscription acquisition removes only that
registration, preserving any replacement installed during the callback. Disposal removes the sink
from admission before invoking external cleanup, releases its subscription at most once even when
cleanup throws, and cannot retire a newer registration. Notifications from retired registrations
are inert.

## Transitions

| State and event                | Guard and result                                                                                                                                                                                               |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `active` + enqueue input       | Assign the next input ID and append it. If no input is in flight, emit one dispatch command for the queue head.                                                                                                |
| `active` + toggle visibility   | Change only local input visibility. Delivery targets, FIFO, and payloads are unchanged.                                                                                                                        |
| `active` + complete delivery   | Accept only the exact epoch and queue-head ID. Full target success removes the head and dispatches the next item. Any missing target enters `stopped`, clears the FIFO, and reports possible partial delivery. |
| `active` + explicit end        | Enter `idle`, clear the FIFO, invalidate the workflow generation, and restore the source terminal when it is still the current focus owner.                                                                    |
| `active` + target invalidation | Enter `stopped`, clear the FIFO, invalidate the generation, and report that the target set changed before accepting later input.                                                                               |
| `stopped` + dismiss            | Enter `idle` without sending input.                                                                                                                                                                            |
| Any state + stale completion   | Ignore a completion whose epoch or input ID does not identify the current queue head. It cannot close, advance, or fail a newer session.                                                                       |

Deferred source-focus restoration belongs to the ended session's epoch. Starting another session
invalidates that callback before it can move focus, including while React still projects the old
session. An ordinary end still restores the source terminal when its Tab and Panel remain current.

The input coordinator serializes envelopes. Before each fan-out it re-resolves the active Tab,
canonical target snapshot, Presentation lifecycle, sink membership, and write readiness. It starts
no target send when preflight fails. Once preflight succeeds, sends begin concurrently for all exact
targets with each captured sink revision as a compare-and-send guard, and the next envelope cannot
start until every result for the current envelope settles.

## Invariants and ownership

- An active snapshot contains at least two targets; Panel IDs and Presentation IDs are each unique,
  and their order equals canonical depth-first Panel order.
- At most one envelope is in flight, and every target observes admitted envelope IDs in FIFO order.
- Full completion requires every target to report `sent`; no empty or reduced target set can be
  reported as success.
- Ending, invalidation, partial failure, unmount, or disposal prevents every later queued envelope
  from starting. Already-started target sends remain irreversible.
- Input visibility never changes delivery. `Ctrl+Alt+Escape` toggles visibility,
  `Ctrl+Escape` ends the session, and unmodified Escape is a broadcast key.
- While active, the bar owns keyboard focus and Panel target shields own pointer hit testing.
  Workbench shortcuts and ordinary terminal DOM input cannot observe a broadcast keystroke.
- `@orchbee/workbench` is the only writer of session state, target membership, FIFO order,
  notices, and focus-restoration commands.
- Each injected terminal surface owns current write readiness, target-specific modifier and
  Option-as-Alt interpretation, semantic key publication, capability revision, and transport
  result. Workbench never constructs Host protocol commands.
- Terminal Host remains authoritative for VT mode encoding and PTY writes. No Host protocol,
  process, network, remote-control, or durable-document writer is added.

## Failure, delivery, and recovery

Multi-PTY delivery is not atomic. A synchronous rejection or asynchronous transport failure may
occur after another target accepted the same envelope. The coordinator waits for the bounded
fan-out to settle, enters `stopped`, reports how many targets are known to have accepted the
envelope out of the target count, and states that additional terminals may have received it. It
never retries, compensates, rolls back, or replays an envelope.

Target invalidation and explicit end prevent later input but cannot recall an envelope whose fan-out
already started. Reopening creates a new epoch with a fresh target snapshot; late results from the
old epoch are inert.

The feature ships as an active-Tab-only transient capability with no migration or compatibility
alias. Rollback removes the Action, Workbench session/bar, and injected surface sink together.
Previously delivered PTY input is outside rollback.

## Verification

Orchbee uses a dedicated live input surface to fan out text and physical key taps to fixed targets. `Ctrl+Escape` ends the session and `Ctrl+Alt+Escape` toggles visibility; the input surface is not a terminal target. Startup captures an immutable active-Tab snapshot, preflights exact mounted sinks and exposes partial delivery failures. Remote control, matcher expressions, cross-Tab or Window targets, unfocused cursor rendering, initial command arguments, paste, files, history and replay are excluded.

| Claim                                                                                      | Executable evidence                                                               | Limits                                                                     |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Snapshot membership and start eligibility are exact                                        | Resolver examples and fixed-seed generated nested layouts                         | Generated layouts and mounted-sink fakes                                   |
| FIFO, invalidation, and stale completion preserve session ownership                        | Pure transition examples, stateful property tests, and invariant assertions       | Fixed property seeds and generated identities, epochs, targets, and inputs |
| Full success requires every target; partial delivery stops without retry                   | Coordinator tests with controlled completion orders and partial-delivery cases    | Transport acceptance cannot prove child-process consumption                |
| DOM input refines to committed text or one physical key tap                                | Normalizer tests plus deterministic Chromium system traces                        | Other browser engines and operating-system-reserved shortcuts are excluded |
| Focus, target shields, visibility controls, Action/menu paths, and localized notices agree | React integration, both-locale Stories, and controlled three-Panel browser traces | Pixel identity and compositor timing are excluded                          |
| Input and target behavior follows the owned contract                                       | Deterministic Orchbee contract scenarios                                          | No source, remote-control, matcher, or pixel parity claim                  |
