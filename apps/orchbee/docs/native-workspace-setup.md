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
Bare Windows [directory startup](native-directory-startup.md) is a separate workflow;
the existing `q` / `d` exit bindings retain their meanings.

A successfully created Workspace is selected through the existing context cleanup and installation
owner after validating its exact committed identity. In this wizard, directory acceptance does not
start a shell; explicit `c` remains the launch intent. This unpublished cohort activates and rolls back
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

## Quick Workspace launch on Windows

`Ctrl+B Shift+N` opens one name/directory form and, after confirmation, creates a Workspace with
its first shell and enters that terminal. `Ctrl+B a` retains the advanced empty-Workspace wizard.
The quick form does not create Git checkouts or branches. Workspace close remains a separate action.

### Inputs, decisions and ownership

The setup owner captures the initiating Client's exact context, focused Panel, primary Terminal
binding, revision and preparation token. A bounded read-only preparation uses a fresh snapshot and
the primary Terminal's root-process directory, even when an Agent presentation is foregrounded.
An explicitly absent root cwd falls back to the Workspace primary directory, then the Windows
user home; an unavailable sample, missing root, invalid path or changed source is a refusal.
The executor borrows the exact live primary pane Viewer, validating its recorded Host epoch and
physical Terminal identity; it never attaches the same Terminal twice in one Session or detaches
that borrowed Viewer. When the primary is not attached, a temporary read-only Viewer is released
under the preparation's original cleanup deadline before accepting or refusing the sample.
Labels, process samples and editable paths remain untrusted. Paths use the existing Windows user
path normalizer, including `~`; relative inputs resolve against the prepared source directory.
The form limits the name to 512 Unicode scalars and the path to 4096 UTF-8 bytes without truncation.

| Project situation                     | Selection                                               |
| ------------------------------------- | ------------------------------------------------------- |
| Source is a Workspace                 | Its exact Project ID                                    |
| No source Workspace; one Project      | That exact Project ID                                   |
| No source Workspace; zero Projects    | Create one localized Default Project after confirmation |
| No source Workspace; several Projects | Require an explicit existing Project selection          |

The directory basename supplies the initial name, with a localized root fallback. Tab changes
between name and directory; the Project picker uses exact identities, including duplicate labels.
The setup draft owns raw fields, completeness, draw receipt and token. The mailbox owns FIFO,
deadline, preparation/command lease and retained Worker evidence. Server navigation owns revisions
and committed structural identities; Worker/Host owns actual shell start. The existing membership
and attachment owners dispose old Viewers, attach the new target and establish the first frame.
Other Clients retain their selections.

### Transitions and outcomes

| State/event                                      | Guard and result                                                                                  |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| Open quick form                                  | Existing structural authority is idle; capture source and enqueue one read-only preparation       |
| Preparation completes                            | Same source and revision; show prefilled editable fields under a fresh token                      |
| Cancel preparation/form                          | Discard draft; no Project or Workspace mutation, and ignore a late preparation                    |
| Select Project/switch field                      | Fresh token and a new complete draw before Enter can confirm                                      |
| Confirm                                          | Complete decoding and full draw; validate both fields, capture one immutable launch plan          |
| Zero Projects                                    | Submit one revision-fenced ProjectCreate; retain its ID before the required fresh read            |
| Project known/fresh                              | Continue the previously confirmed plan once; do not request a second form confirmation            |
| Launch                                           | Submit one WorkspaceLaunch with explicit normalized cwd and default shell                         |
| Launch committed                                 | Retain Project/Workspace/directory/context/tab/Panel/presentation IDs before reading again        |
| Terminal Started and fresh membership proven     | Use existing cleanup, attach, controller/geometry and first-frame flow; then enter the terminal   |
| Definite refusal                                 | Preserve existing membership and any known earlier Project; no replay or automatic rebase         |
| Known pending/failed launch                      | Preserve structural result; show an inspectable result, never claim Ready or create a replacement |
| Lost outcome or post-commit read/handoff failure | Retain known evidence, retire when required, show explicit recovery; never replay or compensate   |

Escape after accepted submission cannot undo Server work. Late input, stale tokens, narrow or
failed draws, peer revision changes and cancelled preparations cannot authorize a different launch.
Default Project creation and Workspace launch are separate commits; cancellation or failure never
deletes the earlier committed Project. Existing checkpoint and position-preference formats do not
change, and position publication still requires the target's successfully drawn first frame.

### Correctness scope

The executable [NativeQuickWorkspace model](../formal/tla/NativeQuickWorkspace.tla) and its
[configuration](../formal/tla/NativeQuickWorkspace.cfg), registered as `native-quick-workspace`,
cover one Client/Site, four Project situations, at most two preparation/form openings, two field
changes, eight issued token values (zero is unset), one confirmed plan, one optional Project handoff, one Workspace
handoff and one peer revision write. Fields and complete decoding are abstract booleans; the
model does not enumerate paths or terminal bytes. Six invariants and two temporal action properties
check confirmation, cancellation, at-most-once submission, Project prerequisites, retained receipts,
unknown-outcome non-replay and the Started/fresh-target/first-frame Ready gate. Deadlock checking
is disabled for intentional stopped states; there are no state constraints, symmetry reductions,
fairness or liveness claims. TLC 2.19 exhausts 4,187 distinct states at depth 18 with zero remaining
states and all 22 required coverage transitions nonzero; this proves only those finite safety bounds.

The model refines into the setup draft's preparation/token/draw admission and immutable plan,
the mailbox's one command flight and retained evidence, and existing membership/attachment
completion. Maintained core tests cover Project ID policy and Windows field limits; input and
actual NativeUi/mailbox tests cover field token replacement, cancellation, one confirmation and
Default continuation. The real local carrier test samples the primary root rather than a child,
requires the exact Attach/ReadProcessInfo/Detach sequence and verifies release before success or
refusal when a temporary Viewer is necessary, and verifies a live primary Viewer receives only
ReadProcessInfo and retains its ownership on success or invalid identity. The actual shell renderer
checks both locales and rejects an incomplete form. These layers do not prove Windows process
sampling, filesystem accessibility, full Viewer behavior or
a human Windows Terminal/provider session; the normal Windows ConPTY flow is a separate witness.

A normal development cohort under controlled Windows ConPTY passes the English and Chinese
flows: cancelling before submission, following a shell's changed Unicode/space cwd, editing both
fields, relative paths, `~`, repeated creation, and preserving another live Client's selection.
Zero-Project creation continues through Default and shell start after one confirmation; a
duplicate-label Project picker launches into the selected exact ID. Each entered Workspace has
one first Tab/Panel, a Ready Client and a live shell whose root cwd matches the confirmed path.
These isolated local witnesses do not establish a human Windows Terminal or real provider session.

## Global Workspace ordering

Windows `workspace move <target> --insert-index N` and `workspace move-block <targets...> [--before target]` resolve every exact ID, label or Workspace suffix against one snapshot, then submit once through the navigation authority. The global display order preserves Project/directory ownership, terminal identities and each Client selection. Indices are insertion boundaries in the original order; a block uses caller order and may cross Projects. The source block contains 1–1023 distinct resolved IDs. No-op preserves the revision.

The navigation submission adapter accepts the dedicated reordered result only for these actions and validates its revision; incompatible or uncorrelated post-submission answers retire the session and report outcome unknown. CLI output uses `orchbee.workspace.move` v1 with a complete `workspaceOrder` on commit and an explicit unchanged outcome otherwise; refusals use its dedicated failure schema. Nothing replays or compensates automatically. Snapshot output advances to v5 for optional `navigation.workspaceOrder`; lists and native navigation consume the same order. Focused Windows acceptance covers cross-Project single/block moves, no-op and refusal invariants, two TUI Clients retaining selections and live shell PIDs, canonical keyboard navigation, creation/close order maintenance and clean-stop restart with working fresh shells. Full replacement acceptance remains separate.
