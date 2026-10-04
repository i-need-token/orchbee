# Native controller takeover

## Ownership and interaction

On Windows, the built-in `Ctrl+B t` action explicitly requests control of the focused,
attached native pane. Ordinary typing, clicking, selection and Viewer polling never request
takeover. The Server remains the authority for controller grants through the existing
OBCS v2 Takeover action; the Client does not select or revoke another Client's identity.
The terminal, program and Viewer attachments survive the handoff.

The successful requester supplies its pane content geometry. The previous controller
keeps receiving output; its next input or resize rejection changes its displayed mode to
read-only. A stationary Client's control label can lag until that interaction. The status
line preserves the known read-only explanation and takeover key when input is blocked.
Blocked input is discarded and never replayed. A later explicit gesture can transfer control back
using the same Viewer. Repeating the gesture while still the actual controller preserves
the verified current grant.

## Operation model

The native session owns one pending operation: exact pane incarnation, refresh generation,
request identity, captured pane geometry and the original forward/total deadlines. Admission
uses the existing serialized refresh barrier, excluding concurrent structural operations.
While pending, input and selection of another native context are fenced. The marker has
one typed purpose; takeover does not pretend to be a Workbench snapshot mutation.

| State     | Evidence and permitted transition                                                   |
| --------- | ----------------------------------------------------------------------------------- |
| Idle      | Only an explicit action on a current attached Ready pane can admit an operation.    |
| Queued    | No submission yet; one original deadline covers queueing and execution.             |
| Submitted | Missing reply is unknown; no automatic second takeover or reacquisition.            |
| Granted   | Retain the exact returned controller before allocation, resize or UI settlement.    |
| Ready     | A matching resize reply establishes the new controller's geometry.                  |
| Refused   | Preserve attachment and focus; report that control was not obtained.                |
| Unknown   | Preserve operation identity and any known grant; retire the carrier when necessary. |

A known grant cannot regress to unknown or be replaced by another grant in the same
operation. Loss during delivery retains evidence in both the executor and mailbox until
the UI consumes it. Receipt settlement must match the operation and pane incarnation.
Accepted frame and geometry gates still determine when input may reach the program;
takeover waits for its first compatible draw/feedback before input. Once accepted, ordinary
frame updates follow the existing Ready input rules and do not reopen that handoff gate. Pending indication uses
the injected clock after 200 ms and localized status, without a modal prompt.

## Evidence and limits

The deterministic core and carrier checks cover admission, stale identity, monotone
receipt retention, original deadlines and input fencing. The Server regression checks
exercise repeated transfer and stale cleanup with the same Viewers. The bounded model
checks explicit-only handoffs, exclusive actual authority, old-input rejection, geometry
ownership and retention. Windows process evidence uses two native ConPTY Clients with
different pane sizes and verifies uninterrupted program identity and bidirectional input
transfer. These checks do not establish a physical Windows Terminal or real Agent-provider
acceptance, nor a proactive control-label notification contract.

`formal/tla/NativeControlTakeover.tla` bounds two Clients and three gestures, separating
reservation, acknowledged detach, attach, grant receipt, resize, UI delivery and draw feedback.
It checks exclusive Host authority, explicit-only sends, old-input and frame fences, controller
geometry and receipt preservation through exit, without a fairness/recovery claim. The Server's
verified self-grant shortcut and real encodings are bridged by implementation tests rather than
modeled identity transformations. Same-author review is not independent confirmation.
