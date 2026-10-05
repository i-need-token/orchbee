# Native Worktree launch

## Interaction and scope

Windows `Ctrl+B W` creates a checkout from a branch and a base (empty means `HEAD`).
`Ctrl+B O` opens an exact checkout selected from the repository listing. F1 help and
`[keys].new_worktree` / `open_worktree` use the existing binding registry.
The directory follows the [CLI Worktree policy](windows-worktrees.md); an existing branch
uses that branch rather than the entered base. Closing a Workspace group remains the
[existing interaction](native-workspace-close.md). Checkout deletion is outside this surface.

The form owns keyboard input, paste, mouse input and the complete decision packet.
Tab changes fields; Enter confirms; Esc cancels before submission. The branch and base
each admit at most 1024 UTF-8 bytes, reject controls and leading `-`, and leave full
reference validation to Git. The list selects stable absolute paths, shows main and linked
checkouts, and scrolls around the selected row. Duplicate branch or Workspace labels never
resolve identity. A surface that cannot fit closes with a localized notice.

After submission, Esc hides observation without cancelling Server work. W or O reopens
the retained operation; `r` only reads that operation. Its ID and known Project, checkout
and Workspace results remain available after failure. No unknown Start or Project creation
is replayed; no Resume, deletion, trust override or rollback is implicit. A terminal result
can be dismissed to begin a new explicit operation. Server restart has the
[existing retention limit](windows-worktrees.md#retained-continuation).

## Model and ownership

The Client wants to create/open a Worktree and enter its first shell without running a CLI
command or losing its current task while Git is busy.

| Record field    | Contract                                                                                                                                                                                                                                                                                                                      |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs          | Exact origin context/member, canonical snapshot, selected Project ID, primary source directory or exact primary root cwd, bounded Git listing, editable branch/base, geometry, configuration, complete decoded packet and successful draw token                                                                               |
| Data            | One opening token, immutable confirmed plan, one retained operation ID, optional known Default Project receipt, canonical checkout/Workspace outcome; these are separate from local focus                                                                                                                                     |
| Outputs         | Read-only preparation, optional Default Project creation, one Start, subsequent Read, localized rejection/pending/failed/unknown/opened status, or guarded navigation                                                                                                                                                         |
| Decisions       | Workspace Project wins; standalone uses its sole Project, explicitly chooses among several, or creates Default only after confirmation. Source uses Workspace primary directory first; otherwise only the origin Panel primary root cwd. Absence/unavailable samples never fall back to home or another pane                  |
| Invariants      | An undrawn, stale, incomplete, resized or source-changed draft cannot submit. Confirmation freezes identity and intent. Start is issued once; reads never mutate Git. Known receipts survive lost later observations. Dismissal/navigation suppress late focus; cancellation before confirmation has no Project or Git effect |
| Model           | Deterministic interaction transitions and typed effects, refined by the bounded carrier and existing membership selection                                                                                                                                                                                                     |
| Dependencies    | Monotonic time; existing settings owner; bounded canonical snapshot/Worktree APIs; exact borrowed or temporary Viewer process sampling; existing native membership, control, geometry and first-frame owners                                                                                                                  |
| Ownership       | Client interaction owns draft/observation and its joined lane. Server owns retained operation/canonical Project/Workspace. Worker alone executes Git. Existing viewer and membership owners alone attach/detach/control terminals                                                                                             |
| Reference scope | Reuse the current Windows CLI contracts, not full Herdr compatibility; no new wire, persistence, dependency, trust or automatic retry policy                                                                                                                                                                                  |

The Git observation lane has one command and one result slot. It never holds a lock during
IPC; shutdown stops admission, wakes blocked I/O and joins its thread. Slow Git work remains
Server-owned and does not occupy the ordinary terminal Viewer FIFO. Only exact root-cwd
preparation uses that FIFO and its existing bounded deadline/cleanup rules. A borrowed
primary Viewer is never detached; a temporary Viewer is released before preparation settles.

Configured destinations have one application-owned pure policy used by CLI and TUI.
Unconfigured destinations remain Worker-owned. The TUI reads the current admitted settings
at preparation; it does not invent a configuration publication field or accept a changed
source as continuation of the same confirmation.

Create destination validation and canonical path observation remain Server/Worker-owned;
the Client does not compare a literal configured operand with the canonical receipt.
Open still matches the exact canonical path selected from the listing. A Project-only
result shows its request/known Project instead of an unsubmitted Worktree status command;
an unknown Project result requires inspecting Projects before another explicit creation.

Local entry waits for an in-flight publication refresh and rechecks the exact origin after
it settles. Missing membership permits at most one additional read-only refresh within
the fixed entry deadline. Hiding an unpresentable surface cancels entry and retains the
canonical receipt. Directory spelling uses the same pure comparison as Server association;
it does not invent filesystem identity or drop the Worker/path guard.

Entry freezes the target's first Tab, Panel, primary presentation, physical terminal
binding and primary directory ID from the canonical receipt snapshot. Every accepted
source refresh must still prove that exact target. The context-selection snapshot is
checked again before old Viewer cleanup or installing a selection, and selects that
exact member through the existing membership owner even after legal Tab reordering.
The guard belongs only to the successfully admitted context refresh generation;
other navigation requests keep their own semantics. A peer deletion or replacement
refuses entry while preserving the current context and the known operation result.
The exact primary presentation also takes precedence over remembered child terminals
through cleanup and attachment. Within the current context, the existing presentation
switch owner selects that primary before the first-frame success check.

## Transitions and failure

| State / event                    | Guard and result                                                                                         |
| -------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Hidden → prepare                 | Current semantic origin, no other modal/mutation; read canonical source and listing                      |
| Prepared → form/list             | Exact source, Site and non-stale revision; Project policy and stable checkout paths                      |
| Edit / choose                    | Replace draw token; require complete decoding and fresh successful draw before confirm                   |
| Cancel before confirm            | Drop draft only; no Project/Git effects                                                                  |
| Confirm → optional Project       | Freeze plan, origin and operation ID; create Default once only when required                             |
| Known Project → Start            | Retain receipt first; fresh snapshot and unchanged semantic origin authorize continuation                |
| Start → observe                  | Exactly one handoff, independently retained operation identity; uncertainty only allows Read             |
| Pending → Read                   | One bounded request per observation step; ordinary terminal output continues                             |
| Opened → navigate                | Started first terminal or existing Workspace reuse, exact fresh membership, visible and unchanged origin |
| Navigate → Ready                 | Existing cleanup/attach/control/geometry/first-frame sequence, only originating Client                   |
| Hide / source change / lost read | Preserve receipts; suppress focus and expose read-only recovery                                          |
| Failed / unknown terminal        | Preserve checkout/Workspace; never recreate or compensate                                                |

Canonical commit and window entry are separate results. A post-commit snapshot, attach or
draw failure does not undo Git, remove a Workspace or resend Start. The operation ID is also
usable with `orchbee worktree status <id> --format json` while the Server retains it.

## Correctness and verification

[NativeWorktreeLaunch](../formal/tla/NativeWorktreeLaunch.tla) and its
[configuration](../formal/tla/NativeWorktreeLaunch.cfg) bound two openings, two editable
changes, one Default receipt, one Start per operation, and one stale callback. Paths,
Git syntax, decoded packets and snapshots are abstract admission facts. The safety
properties cover confirmation/draw/packet admission, at-most-once mutation, Project receipt
retention, cancellation, unknown non-replay and guarded local focus. Intentional stopped
states disable deadlock checking; there is no fairness or liveness claim.

Selected verification with pinned TLC 1.7.4 exhausted 22,404 generated / 8,591 distinct
states at depth 23 and covered all 25 required actions. The affected existing
[NativeQuickWorkspace](../formal/tla/NativeQuickWorkspace.tla) model also exhausted
11,431 generated / 4,187 distinct states at depth 18 with its required action coverage.
Both queues were empty. These are finite safety checks of the stated abstraction.

Core tests refine source/Project/ref and interaction decisions. Actual carrier traces cover
correlation, ambiguous handoff, retained receipts, Start/Read separation and Viewer release.
Renderer/input tests cover both locales, geometry refusal, Unicode, modal precedence and
packet tails. Real UI mailbox traces cover refresh handoff and target deletion or
replacement before context installation. Normal Windows Git/ConPTY witnesses cover create, exact-path open/reuse,
cancellation, continued output and another Client preserving its selection.
These bounded witnesses do not establish physical Windows Terminal, real provider, reboot,
checkout deletion or the entire historical model inventory.
