# Native Workspace setup

## Scope and ownership

The native Client creates a Workspace in an existing or newly created Project and explicitly
configures its first directory. Server remains the sole navigation and Directory authority; Worker
validates the literal path. This reference owns the Client interaction and command-consumption
extension to [context selection](native-context-selection.md) and the Server's
[directory contract](../../orchbee-server/docs/workspace-terminal-directory.md).

`Ctrl+B a` opens Workspace creation: select an existing Project in Server order or the New Project
item, optionally enter and submit a Project name, then enter and submit a Workspace name. The
current Workspace's Project is initially selected when available; otherwise New Project is selected.
The picker uses `[` / `]` and Enter. Each step requires its own complete visible draw before Enter
can submit. An accepted new Project remains if the user cancels the later Workspace step.

`Ctrl+B e` configures the active Workspace only when its context is Empty and it has no primary
Directory. The input is a literal absolute UTF-8 path, at most 4,096 bytes, without NUL; there is no
home fallback, shell expansion, filesystem creation or directory browser. Labels retain the existing
512-Unicode-scalar contract. All prompts, refusals and committed-result notices ship in both locales.
Bare startup and the existing `q` / `d` exit bindings retain their meanings.

A successfully created Workspace is selected through the existing context cleanup and installation
owner after validating its exact committed identity. Directory acceptance does not start a shell;
explicit `c` remains the only launch intent. This unpublished cohort activates and rolls back
together. Rollback or cancellation does not undo committed Project, Workspace or Directory facts.
Remote access, rebinding, deletion, general Directory CRUD, persistence and release packaging are
excluded.
[Location label editing](native-location-labels.md) extends this admission and result-consumption
owner with the existing Project and Workspace rename commands.

## State and command contract

The Client retains complete Project identity/label summaries, including empty Projects, and active
Workspace directory metadata from the same snapshot revision as its current context. Picker labels
and positions never replace captured Project/Workspace IDs. A draft captures its exact Site,
revision and target; competing local structural work, Close and context changes cannot silently
retarget it. Server still rejects stale revisions and unavailable targets.

| State or event                 | Result                                                                                                                                            |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open draft                     | Stable Session admission and a fresh step token; no wire command.                                                                                 |
| Edit draft                     | Bounded text under the same step token; ordinary edits do not require a new first-draw acknowledgment.                                            |
| Change picker selection        | Issue a fresh token and revoke display authorization until the selected Project target is fully drawn.                                            |
| Cancel draft                   | Release modal input ownership; preserve canonical state and any earlier known commit.                                                             |
| Complete draw                  | Acknowledge only the exact current draft token and complete rendered content.                                                                     |
| Submit                         | Freeze one typed command and one request identity; drain earlier accepted work within the existing single carrier and original operation budget.  |
| Definite refusal               | Preserve membership and the current terminal; show the failure, without command replay or automatic rebase.                                       |
| Known commit                   | Retain the immutable result before any fallible snapshot read or UI handoff.                                                                      |
| Committed snapshot available   | Validate exact Site, revision and returned identities; advance the wizard or install the created Workspace through existing exact Viewer cleanup. |
| Committed snapshot unavailable | Keep the known result visible and require explicit snapshot recovery; never resubmit the mutation.                                                |
| Ambiguous handoff              | Retire the carrier with typed outcome unknown; do not retry, fabricate absence or run a later wizard command.                                     |

Raw UTF-8, bracketed paste, incomplete sequences and post-submit input suppression reuse one
bounded modal decoder. Rename keeps its own typed draft and operation contract. Directory byte
limits must not be approximated by a Unicode-scalar limit. A packet containing Enter and more input
cannot submit a later step before that step has been fully drawn. Resize, stale tokens, teardown
and late results cannot authorize a mutation for a different draft.

## Correctness evidence

The bounded setup model must cover Project and Workspace steps, independent directory acceptance,
draw authority, cancellation after a known earlier commit, definite refusal, result loss,
post-commit snapshot failure, explicit recovery and at-most-once handoff. Existing Server models
retain canonical commit and directory-validation obligations; the native membership and input
models retain exact cleanup, first-frame, geometry and decoder obligations.

[`NativeWorkspaceSetup`](../formal/tla/NativeWorkspaceSetup.tla) explores one Client and Site,
one existing and one new Project/Workspace, three mutation identities, one peer revision write,
two modal openings and six tokens shared by openings, stage changes and picker changes.
These are finite exploration bounds, not product interaction limits. Its registry entry requires
all 54 named actions to be exercised. Labels and paths project to validity and completeness;
the model does not establish byte-level decoding, full Worker behavior or detailed Viewer cleanup.

Deterministic tests cover empty Projects, duplicate labels, ID-based picker selection, Unicode
label versus byte-limited paths, same-packet submission, stale draw, busy admission, exact command
outcome compatibility and retained known commits. Carrier traces distinguish command, snapshot,
cleanup and attach ordering. Linux native process evidence starts with a fresh Site, creates and
configures a Workspace through visible native controls, explicitly launches in the chosen path,
returns to the original standalone shell, and observes invalid-directory refusal without fallback.
These are local development witnesses; they do not establish installed or cross-platform readiness.
