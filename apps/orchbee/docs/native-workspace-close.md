# Native Workspace Close

## Scope and user outcome

The Windows TUI closes the current Workspace with `Ctrl+B D` (`Shift+D`), followed by the exact
lowercase `y`; `Esc` cancels. The complete confirmation names the Workspace, states that all its
terminals and those of its associated worktree Workspaces end for every Client, and states that
Git directories remain. Standalone cannot be closed through this action. This is the native
projection of [`workspace close --group --confirm`](windows-worktrees.md#workspace-group-closure):
ordinary and linked Workspaces are singleton scopes; a known main-checkout Workspace closes its
captured associated group, source last. Project records, Git checkouts and branches remain.

The action does not create a new Close protocol, delete directories, resume failed Close attempts,
take over live orphan processes or promise rollback. The Server's existing retained group and
child Close owners remain authoritative. `workspace close-status --group --request <operation-id>`
observes a retained operation; eligible recovery remains the existing explicit CLI Resume.

## Inputs and ownership

| Fact                                                     | Owner and validation                                                                                                                         |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Current Site, Project, Workspace, context and navigation | Validated Workbench snapshot, projected as `NativeContextTabs`; IDs select scope, labels only display it                                     |
| Confirmation                                             | `WorkspaceCloseState`: frozen source cache, exact desired pane and monotonically allocated display token                                     |
| Complete draw and geometry                               | Pure renderer returns a typed receipt; UI acknowledges only the offered token after draw                                                     |
| Request identity, queue, lease and deadlines             | Command mailbox; one bounded Refresh flight serializes the close with other structural work                                                  |
| Group membership and terminal lifecycle                  | Server retains operation/member/child identities; Worker/Host owns actual disposal                                                           |
| Submission and observation evidence                      | Native carrier deposits typed evidence in the mailbox before a subsequent read or UI wait can fail; UI and joined-worker handoff preserve it |
| Selection and Viewer cleanup                             | Existing context-removal and membership owners consume a fresh snapshot and exact attachment identities                                      |

Untrusted keys are decoded by the existing confirmation packet owner. Modified keys, uppercase
`Y`, Enter, paste and trailing bytes never confirm or reach the terminal. A missing draw receipt
is not permission. A source revision, membership or selection change invalidates the prompt.
Before transport handoff, a fresh snapshot must project to the complete frozen source cache;
unavailable or changed input produces a definite refusal. This check and Server admission are
separate process boundaries: group membership is atomically captured by the existing Server at
admission, as in CLI group Close; the TUI does not claim a new read/Start transaction.

## Transition model

`WorkspaceCloseOperation` owns one group Start and the original `OperationDeadlines`. Its
`WorkspaceCloseEvidence` distinguishes definitely unsent/refused, outcome unknown and observed
group results. An accepted group result includes its immutable member order and each known child
Close identity. A receipt describes lifecycle truth, independently of UI delivery.

| State/event                                     | Guard and output                                                                                              |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Open                                            | Current Workspace and quiet structural owners; freeze source and offer one confirmation                       |
| Draw                                            | Complete content at admitted geometry; acknowledge only its token                                             |
| Esc, stale source or refused geometry           | Cancel prompt without allocating/submitting a Close request                                                   |
| Exact `y`                                       | Same fully drawn source; atomically allocate and queue one operation, dismiss prompt                          |
| Execute                                         | Live lease and original forward cutoff; fresh source proof precedes one Start                                 |
| Definitely unsent or definite admission refusal | Retain that evidence; no accepted-close claim and no automatic mutation retry                                 |
| Accepted or observed Pending                    | Retain the correlated receipt before each fallible boundary; bounded read-only polling on the same operation  |
| Closed                                          | Retain known Closed, then read a fresh snapshot; existing membership cleanup can land on Standalone           |
| Failed, observation unavailable or deadline     | Preserve all known member/child facts; report status and original executable status command                   |
| Transport loss or worker exit                   | Preserve receipt or unknown request identity through mailbox/join handoff; never replay Start or infer Closed |
| Client exit after submission                    | Ends local waiting only; Server-owned closure may continue                                                    |

The native wait polls at 50 ms, uses the existing 50-second total budget with its five-second
cleanup reserve, and stops at the original forward cutoff. It does not extend that cutoff for
reads. A timeout is an observation limit, not cancellation of the retained Server operation.
After 200 ms of pending work, the injected monotonic clock enables a localized status in the
affected region; it never opens a progress modal. Final receipts can be shown immediately.
Only an actual Closed receipt reports successful closure. Canonical disappearance may repair
selection without upgrading an unavailable observation to Closed. Other surviving Workspaces'
Clients retain their own selection; Clients on removed contexts follow their existing peer-removal
contract. A new Close cannot replace an unresolved operation's recovery identity.

## Correctness and evidence

The owning implementation tests cover exact drawn confirmation before one Start, source drift with no effect, known receipt retention, Pending/Unknown classification, canonical cleanup, group closure and checkout-path preservation.

Maintain the concrete witnesses in `src/terminal_session/state/workspace_close.rs`, `src/terminal_session/mailbox/tests/close_reconciliation.rs`, `src/terminal_session/connection/native/tests/workspace_close.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
