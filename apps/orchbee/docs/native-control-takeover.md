# Native controller takeover

## Ownership and interaction

On Windows, the built-in `Ctrl+B t` action explicitly requests control of the focused,
attached native pane. Ordinary typing, clicking, selection and Viewer polling never request
takeover. The Server remains the authority for controller grants through the existing
OBCS v1 Takeover action; the Client does not select or revoke another Client's identity.
The terminal, program and Viewer attachments survive the handoff.

The successful requester supplies its pane content geometry. The previous controller
keeps receiving output; its next input or resize rejection changes its displayed mode to
read-only. A stationary Client's control label can lag until that interaction. The status
line preserves the known read-only explanation and takeover key when input is blocked.
Blocked input is discarded and never replayed. A later explicit gesture can transfer control back
using the same Viewer. Repeating the gesture while still the actual controller preserves
the verified current grant. An ended terminal accepts no controller: the gesture submits nothing,
and the status line keeps the retained exit fact and offers a new Tab instead of the takeover key.

After a matching resize reply, the requester pauses and reactivates projection demand on
that same Viewer, under the original operation deadline. Reactivation requests a full frame
even for an idle terminal, a same-size handoff or a self-grant. The UI reports a display wait
until that frame has been drawn and its feedback accepted; a resize receipt alone never
reports input readiness. Failure in either demand leg retains the known controller grant
and reports incomplete display synchronization without replaying the operation or input.

Once Ready, a valid full snapshot determines display geometry even when it differs from
this Client's last resize receipt. A peer may have taken control without this Client yet
receiving a control rejection. The snapshot never grants input or resize authority; initial
setup and the handoff's first frame keep their acknowledged-geometry gates. Ordinary deltas
still require the exact accepted base, generation and geometry. Thus the former controller
continues viewing new output without a local resize or reselection.

## Operation model

The native session owns one pending operation: exact pane incarnation, refresh generation,
request identity, captured pane geometry and the original forward/total deadlines. Admission
uses the existing serialized refresh barrier, excluding concurrent structural operations.
While pending, input and selection of another native context are fenced. The marker has
one typed purpose; takeover does not pretend to be a Workbench snapshot mutation.

| State     | Evidence and permitted transition                                                    |
| --------- | ------------------------------------------------------------------------------------ |
| Idle      | Only an explicit action on a current attached Ready pane can admit an operation.     |
| Queued    | No submission yet; one original deadline covers queueing and execution.              |
| Submitted | Missing reply is unknown; no automatic second takeover or reacquisition.             |
| Granted   | Retain the exact returned controller before allocation, resize or UI settlement.     |
| Ready     | Resize and projection reactivation are acknowledged; UI awaits drawn frame feedback. |
| Refused   | Preserve attachment and focus; report that control was not obtained.                 |
| Unknown   | Preserve operation identity and any known grant; retire the carrier when necessary.  |

A known grant cannot regress to unknown or be replaced by another grant in the same
operation. Loss during delivery retains evidence in both the executor and mailbox until
the UI consumes it. Receipt settlement must match the operation and pane incarnation.
Accepted frame and geometry gates still determine when input may reach the program;
takeover waits for its first compatible draw/feedback before input. Once accepted, ordinary
frame updates follow the existing Ready input rules and do not reopen that handoff gate. Pending indication uses
the injected clock after 200 ms and localized status, without a modal prompt.

## Evidence and limits

The serialized native handoff uses controlled implementation evidence; it has no separate TLA
model. Its executable witnesses exercise the actual state, framed carrier, reservation and Host
operation boundaries:

| Claim                                                                                                                   | Maintained evidence                                                                                                  |
| ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Explicit admission, captured geometry, frame-before-input and monotone receipts after exit                              | [`state/tests/control_takeover.rs`](../src/workbench_client/state/tests/control_takeover.rs)                         |
| Refusal, unknown outcomes without resubmission, retained grants after resize/demand loss and UI delivery loss           | [`connection/native/tests/control_takeover.rs`](../src/workbench_client/connection/native/tests/control_takeover.rs) |
| Draw/feedback restores input without replay; peer geometry never grants authority                                       | [`ui/tests/control_takeover.rs`](../src/workbench_client/ui/tests/control_takeover.rs)                               |
| Pending reservations exclude competitors; late cleanup preserves replacement grants                                     | [Server reservation tests](../../orchbee-server/src/local_terminal_service/controller_connection/tests.rs)           |
| Acknowledged detach precedes attach; failures cannot restore old authority; old input, resize and cleanup remain fenced | [Worker local-pipe tests](../../orchbee-worker/src/host_connection/tests/controller_takeover.rs)                     |

The retained [`ConnectionViewerOwnership`](../../orchbee-server/formal/tla/ConnectionViewerOwnership.tla)
model separately checks shared Viewer/grant ownership and cleanup under its own bounds; it does
not model the native handoff sequence. The implementation traces above do not establish exhaustive
interleavings, fairness or crash recovery. Windows process evidence uses two native ConPTY Clients
with different pane sizes and verifies uninterrupted program identity and bidirectional input
transfer. These checks do not establish physical Windows Terminal or real Agent-provider
acceptance, nor proactive control-label notification. Same-author review is not independent
confirmation.
