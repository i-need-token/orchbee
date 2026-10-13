# Native Workspace Close

## Scope and user outcome

The Windows TUI closes only the current Workspace with `Ctrl+B D` (`Shift+D`), followed by the
exact lowercase `y`; `Esc` cancels. The complete confirmation names the Workspace and explains
that its terminals end for every Client while Git branches and checkouts remain. Project grouping,
shared directories and worktree relationships never expand this scope. Ungrouped Workspaces are
valid targets; Standalone is not.

The Workspace context menu closes its addressed Workspace without selecting it first. Confirmation,
cancellation and completion preserve an unrelated current Context, selected pane and terminal
attachment. The selected shell continues running. Closing the current Workspace retains the existing
canonical removal and Standalone fallback; opening Close from Standalone may target a named Workspace.

The visible cancel and close buttons support a left-button click. A matching press and release
must still identify the same completely painted confirmation; releasing outside, modifiers or a stale
paint cannot authorize closure. Keyboard and mouse use the same submission guard.

The Project menu exposes a separate close-all operation for its exact Project ID. Its confirmation
names the Project, shows its Workspace count, and explains that all member Workspaces and their
terminals close for every Client. The Server captures Project membership at admission. The Project
record remains, including when already empty. Dissolving a Project is a separate organization
operation that preserves Workspaces and terminals.

Ordinary Close uses `WorkbenchAction::WorkspaceClose`; only explicit Project close-all uses group
Start. Neither path deletes Git data or automatically resumes a failed attempt. Read retained status
with `workspace close-status --workspace <workspace-id>` or
`project close-status --request <operation-id>`. Eligible recovery uses explicit CLI Resume;
Pending and Unknown permit observation only.

## Inputs and ownership

| Fact                                                      | Owner and validation                                                                                                                                                                     |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session, Workspace identity and optional Project metadata | Validated Workbench snapshot projected as `NativeContextTabs`; grouping changes do not change Context identity                                                                           |
| Confirmation                                              | `WorkspaceCloseState`: typed Workspace/Context or Project target, frozen full snapshot with the origin Context and desired pane, label, Project member count and monotonic display token |
| Complete draw and geometry                                | Pure renderer returns a typed receipt; UI acknowledges only the offered token after a complete draw                                                                                      |
| Request identity, queue, lease and deadlines              | Command mailbox; one bounded Refresh flight serializes closure with structural work                                                                                                      |
| Captured membership and terminal lifecycle                | Server retains scope, member and child identities; Worker/Host owns disposal                                                                                                             |
| Submission and observation evidence                       | Carrier retains typed admission and observations before another read or UI wait; mailbox and joined-worker handoff preserve them                                                         |
| Selection and Viewer cleanup                              | Existing context-removal and membership owners consume canonical snapshots and exact attachment identities                                                                               |

The existing confirmation packet owner decodes keys. Modified keys, uppercase `Y`, Enter, paste
and trailing bytes never confirm or reach the terminal. Missing or incomplete draw evidence cannot
submit. A source revision, membership or selection change invalidates the prompt. The addressed
Workspace must still name the exact captured terminal Context; disappearance or identity replacement
invalidates confirmation even when the current Context and revision are unchanged. Before handoff,
a fresh snapshot must equal the frozen projection, including background membership. Changed or
unavailable input refuses submission.
This read and Server admission remain separate boundaries: Project membership is captured atomically
at Start, without claiming a transaction spanning the preceding Client read.

## Transition model

`WorkspaceCloseOperation` retains its typed target, origin projection, request identity and original
`OperationDeadlines`. Its Workspace target carries its own Context identity; it never borrows the
current terminal's identity for submission or receipt validation.
`WorkspaceCloseEvidence` distinguishes unsent/refused, unknown, Workspace admission, observed
Workspace status and observed Project results. Ordinary admission retains the Server's canonical
`close_request_id`, context and accepted revision even before the first status read succeeds.
Project evidence preserves frozen member order and every known child Close identity.

| State/event                            | Guard and output                                                                                                                                    |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open                                   | Quiet structural owners; freeze the current or addressed Workspace, or explicitly named Project, without moving focus                               |
| Draw                                   | Complete content at admitted geometry; acknowledge only its token                                                                                   |
| Esc, stale source or refused geometry  | Cancel without allocating or submitting a Close request                                                                                             |
| Exact `y` or valid close-button click  | Same fully drawn source; atomically allocate and queue one typed operation                                                                          |
| Execute                                | Live lease and original forward cutoff; fresh source proof precedes one ordinary command or Project Start                                           |
| Definitely unsent or admission refusal | Retain evidence without an admission claim or automatic retry                                                                                       |
| Workspace accepted or observed Pending | Retain admission before the first read; poll the same canonical operation with read-only requests                                                   |
| Project accepted or observed Pending   | Retain the group receipt and known child identities before read-only polling                                                                        |
| Closed                                 | Retain Closed before reading a canonical snapshot; preserve a surviving origin, or use existing cleanup and Standalone fallback when it was removed |
| Failed, unavailable read or deadline   | Preserve admission and all known observations; show the scope's executable status command                                                           |
| Worker exit or transport loss          | Preserve receipt or unknown submission identity through mailbox/join handoff; never replay effects or infer Closed                                  |
| Client exit after submission           | End local waiting; the Server-owned operation may continue                                                                                          |

Every observation must match its operation, Workspace/context or Project scope. Attempt generations,
known members, child identities, disposal and Closed results cannot regress. Observed OutcomeUnknown
cannot change status or attempt. A less informative
worker result cannot erase admission. A late completion cannot replace a newer operation or select
an earlier Context. Closing a background Workspace or another Project preserves the current surviving
Context. Begin and finish validate the target against its frozen snapshot; a valid Closed receipt is
still retained after canonical removal, rather than requiring the removed target to remain present.

Confirmation and active execution block membership changes. Retained unresolved evidence continues
to block new structural effects, but finished flights permit read-only canonical membership repair.
A snapshot can prove Context removal and repair selection even when the latest Close read is
unavailable; it cannot promote that retained status to Closed. A new Close cannot replace the
unresolved recovery identity.

Polling uses 50 ms intervals and the existing 50-second total budget with a five-second cleanup
reserve. Reads never extend the original cutoff; timeout limits observation rather than cancelling
Server work. After 200 ms, the injected clock enables a localized pending status without opening a
progress modal. Final receipts can appear immediately.

## Correctness and evidence

State, carrier and UI tests cover single Workspace scope including ungrouped membership, explicit
Project member capture and empty Projects, exact drawn confirmation, token-bound mouse controls, packet-tail capture, source
drift, admission/read/publication loss, monotone evidence, canonical repair and preserved focus.
Background-close witnesses cover cancellation and publication from Workspace and Standalone origins,
retained pane/terminal binding and attachment identity, and no origin cleanup or reattachment. Missing
or replaced target Contexts are rejected before submission even at the same revision; origin or stale
target receipts cannot overwrite valid admission evidence.
The framed peer asserts that ordinary Close never sends group Start and that neither path replays
mutations. Snapshot fixtures retain Project metadata and shared checkout directories.

Maintain witnesses in `src/terminal_session/state/tests/workspace_close.rs`,
`src/terminal_session/connection/native/tests/workspace_close.rs`,
`src/terminal_session/ui/tests/workspace_close.rs` and the local-client Close adapter suites.
Render evidence covers both locales, terminal-native and RGB themes, Standard, Compact, Minimal and
refused geometries. The applicable repository gate remains authoritative; focused development uses
`cargo test --locked -p orchbee --bin orchbee workspace_close` and the Project close-all carrier test.
These controlled implementation traces do not establish exhaustive interleavings, physical terminal
appearance or untested platforms. Shared identity and commit protocols retain their registered models.
