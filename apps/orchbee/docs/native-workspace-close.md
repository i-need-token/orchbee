# Native Workspace Close

## Scope and user outcome

The native TUI closes only the current Workspace with `Ctrl+B D` (`Shift+D`). Proven idle Workspaces
close directly; active or unproven program state opens confirmation, where unmodified `Enter`
confirms and `Esc` cancels. The complete confirmation names the Workspace and explains
that its terminals end for every Client while Git branches and checkouts remain. Project grouping,
shared directories and worktree relationships never expand this scope. Ungrouped Workspaces are
valid targets. The legacy compatibility Workspace has no canonical Workspace ID; close its Tabs
individually. Its whole-Workspace Close entry is disabled with a localized explanation.

The Workspace row menu's Close Workspace… addresses its Workspace without selecting it first. Confirmation,
cancellation and completion preserve an unrelated current Context, selected pane and terminal
attachment. The selected shell continues running. Closing the current Workspace retains the existing
canonical removal: select the first surviving canonical Workspace, including an empty one, then
nonempty legacy content, otherwise show the explicit no-Workspace creation entry. Deliberate Close
never recreates a Workspace or terminal. Opening Close from legacy content may target a named Workspace.

The visible cancel and close buttons support a left-button click. A matching press and release
must still identify the same completely painted confirmation; releasing outside, modifiers or a stale
paint cannot authorize closure. Keyboard and mouse use the same submission guard. Esc and Enter hints sit inside their
buttons in subordinate text. A matching plain-left click on the outside backdrop cancels through
the same exact painted X target, without selecting or sending input to the terminal underneath.

Explicit Project close-all compatibility addresses an exact Project ID. Its confirmation
names the Project, shows its Workspace count, and explains that all member Workspaces and their
terminals close for every Client. The Server captures Project membership at admission. The Project
record remains, including when already empty. Dissolving a Project is a separate organization
operation that preserves Workspaces and terminals.

Explicitly confirmed Close uses `WorkbenchAction::WorkspaceClose`; only explicit Project close-all uses group
Start. Neither path deletes Git data or automatically resumes a failed attempt. Read retained status
with `workspace close-status --workspace <workspace-id>` or
`project close-status --request <operation-id>`. Eligible recovery uses explicit CLI Resume;
Pending and Unknown permit observation only.

## Conditional sidebar admission

The row menu's Close Workspace…, addressed Close and the keyboard shortcut share `WorkspaceCloseIfIdle`; none
uses a Client idle flag followed by ordinary Close. The Server captures all Tabs, split leaves and
child Presentations and, outside its gates, reads fresh native inventory and then each live
terminal's own shell report and process sample, by the same rule as a Tab or Panel close. It then
revalidates the complete scope and exact runtime identities at final admission. Empty/never-created
scope, ended runtimes and terminals whose evidence shows only an idle shell close directly. A
running command, a program holding the console, or missing, unavailable or unknown evidence requires
the existing fully drawn confirmation with a fresh unconditional request ID. Concurrent create/split
and revision drift retain ordinary safety fences.

New interactive PowerShell launches receive process-local reader markers that preserve the user's
prompt, history and key hooks; a shell without reliable prompt markers is judged by its process
sample alone. An accepted conditional Close then runs as the ordinary Close, because the Server's
evidence already decided, so a program started after the read is terminated as an ordinary Close
would terminate it. A Host refusal preserves the Close operation
and any completed members. Repeating Close on its known resumable refusal opens explicit
confirmation, and the ordinary confirmed command resumes that same canonical operation.
Unknown/in-flight effects remain fenced.
Keyboard Close and the Workspace menu's Close Workspace… share this exact-target recovery admission;
other blocked reasons and nonresumable refusals stay disabled. Menu retry opens a fresh review
and requires a complete draw and explicit confirmation before another effect.

Project Delete uses `ProjectDissolveIfIdle` over the entire captured member scope and preserves all
Workspaces and resources. It never invokes close-all. A definite same-base `confirmation-required`
is retained as no effect and opens a new reviewed decision with a fresh display token. Its warning
states that members and terminals are kept. Only a complete successful draw and explicit ordinary
Enter may submit a new unconditional request ID. Other refusal, stale, missing, transport or unknown
evidence never opens this escalation or retries an effect. Ordinary explicitly confirmed Close and
Project dissolve keep their existing compatibility semantics and distinct fingerprints.

Workspace escalation reports that confirmation is required, rather than displaying an ordinary
refusal/retry instruction under the fresh decision. This definite no-effect status has no admitted
Close operation ID or close-status recovery command. Ordinary refusal and unknown outcomes retain
their separate feedback and recovery rules.

A peer publication can change the snapshot between the Server's reply and the read that follows it.
The reply speaks only for the addressed Workspace record and its complete terminal Context in the
same Session. When those are unchanged, the confirmation opens over the current snapshot. When they
changed, the Client repeats the conditional request once; it has no effect before confirmation. A
second mismatch, or a Client that cannot review or repeat it, shows the localized Workspace changed
notice instead of the confirmation-required status, and the user retries. A conditional request that
meets a background publication read waits for that read and then starts over the projection it
installed; one that a newer projection or `RevisionMismatch` kept from admission while its target
facts stayed unchanged is repeated over the newer projection, up to four times, because commits
elsewhere in the Workbench keep advancing the revision while other Workspaces work. A request
pressed while the user's own switch is still landing, including a sidebar click that still waits
behind an earlier switch or read, waits for the switch. `Ctrl+b D` names the Workspace the newest
switch request was headed for when the key was pressed and closes it only once the switch landed
there; a switch that lands elsewhere or not at all shows the localized Workspace changed notice and
closes nothing, so the key never falls back to the Workspace being left. While the switch lands, the
shell keeps the previous frame on screen without pointer targets; a click there, such as on another
Workspace's row, is dropped with the localized switching notice and makes the destination uncertain.
`Ctrl+b D` pressed before or after that click is then refused the same way while the switch lands,
and within five seconds of the click once more after it landed; the next press closes the Workspace
shown. A conditional request the
Client cannot admit shows the localized busy or unavailable notice. No path discards a request
silently.

## Inputs and ownership

| Fact                                                      | Owner and validation                                                                                                                                                                                     |
| --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Session, Workspace identity and optional Project metadata | Validated Workbench snapshot projected as `ClientContextTabs`; grouping changes do not change Context identity                                                                                           |
| Confirmation                                              | `WorkspaceCloseState`: typed Workspace/Context or Project target, frozen full snapshot with the origin Context and desired pane, label, Project member count and monotonic display token                 |
| Complete draw and geometry                                | Pure renderer returns `ModalPresentation`; `tui_presentation` and `WorkspaceCloseState` distinguish a blocked first draw from suspension after admission, while exact shown-token proof remains separate |
| Request identity, queue, lease and deadlines              | Command mailbox; one bounded Refresh flight serializes closure with structural work                                                                                                                      |
| Captured membership and terminal lifecycle                | Server retains scope, member and child identities; Worker/Host owns disposal                                                                                                                             |
| Submission and observation evidence                       | Carrier retains typed admission and observations before another read or UI wait; mailbox and joined-worker handoff preserve them                                                                         |
| Selection and Viewer cleanup                              | Existing context-removal and membership owners consume canonical snapshots and exact attachment identities                                                                                               |

The existing confirmation packet owner decodes keys. Modified keys, `y`/`Y`, paste
and trailing bytes never confirm or reach the terminal. Missing or incomplete draw evidence cannot
submit. Before the first complete presentation, an evaluated geometry block refuses the new prompt
and the status line says it cannot open there. After presentation, the same block suspends the
existing prompt while retaining its token, frozen source and focus: `Enter` and stale pointer releases
are inert, local `Esc` still cancels, and a later complete draw of the same token restores it. A
missing receipt only revokes shown authority; it does not classify geometry or discard the prompt.
A newer projection that leaves the confirmed scope's facts unchanged — the addressed Workspace with
its whole terminal Context, or the named Project with its ordered members — rebases the prompt onto
that projection. A change to those facts or to the Session cancels it with the localized
"Workspace changed; try again" notice, and a selection change cancels it. A background publication
read neither blocks opening nor cancels the prompt; `Enter` during one waits for it to settle and
submits before the next background read. The addressed Workspace must still name the exact
captured terminal Context; disappearance or identity replacement invalidates confirmation even when
the current Context and revision are unchanged. Before handoff, a fresh snapshot must equal the
frozen projection, including background membership. A newer snapshot, or a `RevisionMismatch`
answer to the Workspace command, commits nothing: the Client installs that projection and submits
the same confirmed action again, with a new request identity, when the scope's facts are
unchanged, up to four times; otherwise it reports the changed Workspace. Unavailable input
refuses submission.
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

| State/event                                    | Guard and output                                                                                                                                                     |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open                                           | Quiet structural owners; freeze the current or addressed Workspace, or explicitly named Project, without moving focus                                                |
| Draw                                           | Complete content at admitted geometry; acknowledge only its token                                                                                                    |
| Geometry block before first draw               | Refuse and remove the new prompt with the cannot-open status notice; allocate no Close request and report no user cancellation                                       |
| Geometry block after a successful draw         | Suspend the same token, frozen source and focus; revoke submit and pointer authority until the same token is completely presented again                              |
| Missing or covered draw receipt                | Revoke exact shown authority only; preserve the presentation phase and frozen prompt                                                                                 |
| Esc, changed scope or selection change         | Cancel without allocating or submitting a Close request; Esc also clears a retained confirmation-required result                                                     |
| Newer projection with unchanged scope          | Rebase the prompt; a submitted operation is retried once against it with a new request identity                                                                      |
| Unmodified `Enter` or valid close-button click | Same fully drawn source; atomically allocate and queue one typed operation                                                                                           |
| Execute                                        | Live lease and original forward cutoff; fresh source proof precedes one ordinary command or Project Start                                                            |
| Definitely unsent or admission refusal         | Retain evidence without an admission claim or automatic retry                                                                                                        |
| Workspace accepted or observed Pending         | Retain admission before the first read; poll the same canonical operation with read-only requests                                                                    |
| Project accepted or observed Pending           | Retain the group receipt and known child identities before read-only polling                                                                                         |
| Closed                                         | Retain Closed before reading a canonical snapshot; preserve a surviving origin, or use exact cleanup and the surviving-Workspace selection order when it was removed |
| Failed, unavailable read or deadline           | Preserve admission and all known observations; show the scope's executable status command                                                                            |
| Worker exit or transport loss                  | Preserve receipt or unknown submission identity through mailbox/join handoff; never replay effects or infer Closed                                                   |
| Client exit after submission                   | End local waiting; the Server-owned operation may continue                                                                                                           |

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

The status line shows the complete status, including the scope's status command before Closed, only
when it fits. Otherwise it shows the documented shorter forms, first the phase with the scope label
and then the phase alone, and never draws a partial status. After the Client Connection is lost,
the pane keeps the complete status.

## Correctness and evidence

A validated removal install retains the known Closed notice at its automatic canonical Workspace
landing. This private display witness binds the request, refresh generation and exact landing to the
existing removal permit; it grants no navigation or terminal input. Later manual Context navigation
retires it, including on return. Closed is a settled one-shot result: its notice expires on the
four-second notice lifetime in [operation status](native-operation-status.md), counted from its
first display and carried across the automatic landing. Expiry and navigation retire only the
notice; the retained Closed evidence and its fences remain. Pending and unknown outcomes retain
their existing guards. While viewer cleanup still defers that install, a Closed receipt reads as
Closing, so Closed never appears beside the removed Workspace still drawn as current.

State, carrier and UI tests cover single Workspace scope including ungrouped membership, explicit
Project member capture and empty Projects, exact drawn confirmation, token-bound mouse controls, packet-tail capture, source
drift, admission/read/publication loss, monotone evidence, canonical repair and preserved focus.
Background-close witnesses cover cancellation and publication from current and sibling Workspace origins,
retained pane/terminal binding and attachment identity, and no origin cleanup or reattachment. Missing
or replaced target Contexts are rejected before submission even at the same revision; origin or stale
target receipts cannot overwrite valid admission evidence.
The framed peer asserts that ordinary Close never sends group Start and that neither path replays
mutations. Snapshot fixtures retain Project metadata and shared checkout directories.

Maintain witnesses in `crates/workbench-client/src/workbench_state/tests/workspace_close.rs`,
`crates/workbench-client/src/workbench_client/connection/native/tests/workspace_close.rs`,
`crates/workbench-client/src/workbench_client/ui/tests/workspace_close.rs` and the local-client Close adapter suites.
Render evidence covers both locales, terminal-native and RGB themes, Standard, Compact, Minimal and
refused geometries. The applicable repository gate remains authoritative; focused development uses
`cargo test --locked -p orchbee --bin orchbee workspace_close` and the Project close-all carrier test.
These controlled implementation traces do not establish exhaustive interleavings, physical terminal
appearance or untested platforms. Shared identity and commit protocols retain their registered models.
