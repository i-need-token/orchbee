# Native Workbench authority

The [restart checkpoint contract](workbench-checkpoint.md) owns retained configuration and fresh
authority reconstruction; it does not by itself enable disk persistence or resume terminal processes.

## Close authority and local integration

[Canonical terminal Tab Close](terminal-tab-close.md) owns the accepted local lifecycle contract:
known Host disposal and exact target-owner cleanup before canonical removal, reserved revision and
ledger/Resume capacity, publication rebased over current membership, and reference-current Create
settlement without changing historical tokens. Other canonical writes may continue during cleanup.
The last removal leaves the Site Empty; native Empty remains usable and a later bare Open may
bootstrap a fresh identity. This adds no explicit-empty Open response.

The Server library implements the retained authority and continuation core below. Site, exact
controller/Worker cleanup and the process-lifetime maintenance owner compose it with the local
Close/Read/Resume endpoints. Deterministic and Linux protocol-client process tests cover credits,
retained controls, current-snapshot removal, idle reservation release and eight live Client
connections. Native confirmation and membership/Empty runtime have separate Linux TUI process
evidence in the Close record; these Server checks alone do not establish them. Installed-cohort
convergence remains unfinished.

## Close authority runtime refinement

The accepted implementation cut retains each primary Close in the existing command ledger as one
`CloseRecord`. It owns the immutable command, fixed acknowledgment, complete standalone member and
validated Worker generation/Host identity/fence, attempt generation, one last accepted Resume
receipt and reserved serialized charge. Aliases retain only their own command and the primary's
fixed acknowledgment. The nested `CloseContinuation` exclusively owns phase, attempt deadline,
submitted steps, query spacing, acknowledged signal/grace facts, bounded inventory walk and the
prepared/admitted outstanding step seal. Service code carries only transient tokens and scheduling
mechanisms; it does not retain another mutable Close status or generation.
Effect-free binding lookup includes closed tombstones and fences new target effects. That Boolean
is not exact cleanup authority: cleanup additionally compares the retained Worker generation,
complete Host identity and binding fence. Original launch/Start replay keeps its existing precedence.

Close admission checks lifecycle authority, exact retained replay/conflict, retirement, ledger
entry capacity, Site/base/context and the applicable strict or tree membership. Site validates
composition before target resolution. A visible existing primary is joined before any fresh runtime lookup, including an
Unknown or disposed primary whose Worker is now unavailable. A new primary requires exact runtime
association and all reservations before joint acceptance. Close replay remains readable after
retirement; ordinary commands keep their existing retirement-first preflight. Status reads and
Resume use the retained primary, including closed tombstones, without ordinary ledger insertion.
The inline Resume cell replaces only its last accepted fingerprint/response after generation,
resumability, outstanding-step, exact dependency and checked deadline preparation succeed.

Credit is derived by scanning non-Closed primary records in the ledger, bounded by 4096 entries;
there is no second mutable credit counter. Every actual ordinary revision advance, including
bootstrap, Create and Move, checks `r + 1 + c <= u64::MAX`; a primary checks the same inequality
without advancing revision. No-ops and terminal-result settlement spend no credit. Primary storage
reserves the maximum encoded command/acknowledgment, status/tombstone and Resume request/response
charge, covering maximum decimal generation/revision and every bounded classification. Tombstones
retain that charge. These byte limits do not establish allocation, panic or crash atomicity.

After known Dispose and exact local cleanup, a borrowed publication plan validates the complete
retained target against current applicable membership and prepares the current projection, encoded
delta/full-snapshot fallback and final Closed revision/status. For an aggregate tree Close it
captures the ordered `exact_removed_members` and exposes the before/after membership to Site's
disjoint slot-only retarget preparation.
Both plans commit under the same Site gate with no further fallible work. Dropping preparation
leaves membership and credit unchanged; failure retains disposed classification and permits only
the remaining cleanup/publication. The continuation preserves outstanding receipts across expiry,
and explicit Resume preserves signal, grace and query facts.

Authority tests own concrete ledger/encoded bounds, alias/replay precedence, maximum revision and
generation, receipt replacement, preparation rollback and current-snapshot removal. Continuation
tests own clock/step/query boundaries and exact receipt settlement. Site and process tests must
establish the joint association, cleanup and actual carrier/Host cuts before Close delivery is
claimed. `cargo test -p orchbee-server --lib --locked` runs these authority/continuation tests;
`cargo clippy -p orchbee-server --lib --locked -- -D warnings` checks the library implementation.
These focused checks do not replace the root gate or the owning cross-process evidence.
No separate Close workflow TLA model is retained.

## Accepted Panel Split and aggregate Tab Close refinement

`panel.split` is the first accepted tree mutation. Workbench validates the exact context, Tab and
leaf Panel, bounds the resulting tree, and commits the source leaf, new leaf, split orientation,
two slot identities and presentation identity before dispatching the second terminal launch. The
Server and Site retain all leaf bindings as one exact tree membership; a missing or unknown leaf
outcome quarantines the pending operation and cannot publish a partial tree.

Aggregate Close captures the complete ordered leaf membership of the Tab. It disposes and cleans up
each leaf under its retained lifecycle authority, then publishes one Tab removal only after every
leaf has a known proof. `panel.close` is the leaf-scoped counterpart: it captures exactly one leaf
member, reuses the same continuation, and publishes the surviving Tab with the removed leaf's parent
Split replaced by the sibling subtree; closing a Tab's only leaf admits as an ordinary aggregate
Close. Standalone retargeting compares the exact removed set against the remaining
tree and preserves the current reference when one remains, or publishes Empty when none remains.
While an aggregate Close is open on a Tab, `panel.split` and `panel.move` admissions naming the same
context/Tab are refused the existing transient `terminal-busy`: publication re-proves the complete
captured ordered leaf membership, which any same-Tab membership change would invalidate. The fence
follows grant, replay/conflict, retirement, ledger-capacity, Site, base-revision and context checks;
it retains no result and launches no terminal, and the same request can be retried after the Close
publishes. A leaf-scoped Close re-proves only its own leaf tuple and fences neither operation; every
other Tab's writes proceed.
The authority unit test, the standalone and Workspace Linux process witnesses
`real_workbench_panel_split_and_aggregate_close_releases_both_terminals` and
`real_workspace_panel_split_and_aggregate_close_releases_both_terminals` cover this refinement. The native TUI consumes Client-local panel
layout and focus through strict projection and local focus; it admits a peer leaf close as an exact
sibling-substituted contraction of its baseline tree; when that contraction removes the leaf this
Client is attached to while the Tab survives, the session enters the common membership
cleanup/install path to re-attach onto a surviving leaf instead of retiring outright: a successful
attach resumes normally, an unavailable attach may park at `RetryableAttachUnavailable` with the
known commit retained, and protocol or deadline failures still retire under the shared fail-closed
rules. The TUI submits
`panel.split`, `panel.close`, and `panel.move` from provisional `Ctrl+B` shortcuts pending the
input-design review.

## Accepted Panel relocation model

`panel.relocate` moves an existing terminal Panel into another Tab, a new Tab in an existing
context, or a new Workspace with optional Project membership. The reference is Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34`, `pane.move` (H066/R049). Workbench owns topology;
Site owns the standalone Open reference; Client view authority owns ratios, zoom and focus.
Relocation never launches, stops or rebinds a terminal, and never deletes a directory or Git tree.

The source tuple and expected revision are exact. Existing destination forms retain their
source-context default. An explicit destination context enables cross-context moves, and an
optional target Panel selects the graft leaf; absence retains the existing root-graft contract.
A new Workspace specifies nullable Project membership, an optional Directory from the Site pool and labels. A new `primaryDirectoryPath` alternatively creates a Directory in that same commit. It is mutually exclusive with an existing directory ID. The local service probes the path through the current Worker, rechecks its generation and requires directory-association authority; the proven Worker must own the retained source terminal. Missing proof, capacity or path failure cannot publish topology. A created Directory adds one allocated identity and `transfer.createdDirectoryId`. Only new structural and directory identities are allocated. A moved leaf retains its Panel, Presentation, binding,
custom label and input policy, including every ordered child Presentation. Unbound-member lifecycle
remains separately tracked.

| Guard or transition                                                               | Observable result                                                                                              |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Missing source/destination, foreign directory, stale revision or exhausted bounds | Refuse before any topology or identity allocation is published.                                                |
| Destination is the source Tab                                                     | Existing typed same-Tab refusal; the CLI workflow owns the reference no-op projection.                         |
| Source has another leaf                                                           | Detach the leaf and collapse its parent Split.                                                                 |
| Source has only that leaf                                                         | Remove the source Tab, without disposing its terminal.                                                         |
| Cross-context move empties a source Workspace                                     | Remove that Workspace/context and its display-order entry; keep Project/directories and all terminal bindings. |
| Same-context move or empty standalone source                                      | Retain the context; a new Tab may replace the removed last Tab at the collection bound.                        |
| Existing target Tab                                                               | Graft at the requested leaf or root with a fresh Split and two slots; enforce depth and resulting node bounds. |
| New Tab or Workspace                                                              | Append the new Tab; append a new Workspace to explicit display order, preserving ownership.                    |
| Lifecycle operation overlaps source/destination or a moved pending terminal       | Transient terminal-busy; no retained success or partial move.                                                  |

One cloned candidate is validated and encoded before ledger admission. One commit publishes the
snapshot, structural identity high-water and exact result. Retained replay returns that result;
request conflict and publication failure cannot allocate a second move. Transfer facts identify
the destination context and any removed source Tab/Workspace or created Workspace. Standalone
before/after membership is prepared with the Site reference under the same gate: departure retargets
or empties Open, arrival binds Open to an already retained terminal. Composition failure retires
before publication. The binding multiset across the Site is conserved.

Public Windows integration must additionally resolve the selected Client's target leaf, enforce
zoom/no-op behavior and apply requested ratio/focus through that Client. These are separately
acknowledged phases; a later failure preserves the known topology commit. Structural tests cover
the decision table, conservation, replay and bounded exchange; shared wire fixtures cover both
language bindings. Real multi-Client Windows movement, shell survival, refresh and restart are
required for complete user acceptance. Generic semantic events remain a separate event contract.

## Accepted Workspace ordering model

Users reorder a Site-wide Workspace list without moving Projects, directories, terminal contexts,
or running terminals. Navigation retains identity-sorted membership and an optional complete
`workspaceOrder` permutation; absent/empty means the global Workspace identity order. Historical
checkpoint migration materializes the former Project traversal as an explicit permutation. Display
order remains independent of optional Project membership and supports a block spanning groups.

| Decision                                                                       | Result                                                                                      |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| Indexed source missing, or original insertion boundary exceeds Workspace count | Reject before mutation.                                                                     |
| Source S, original insertion boundary N                                        | Destination is N - 1 when S < N, otherwise N; S and S + 1 are no-ops.                       |
| Empty block, duplicate/missing source or missing target                        | Reject the whole block.                                                                     |
| Before-target belongs to the block                                             | Reject; no partial reorder.                                                                 |
| Valid block and optional before-target                                         | Remove all source IDs, insert them in caller order before the target or at the end.         |
| Effective order unchanged                                                      | Retain Unchanged without revision, identity allocation or a redundant ordering field.       |
| Effective order changed                                                        | Publish the complete permutation and one revision through the existing pure command commit. |

The existing mutation grant, Site/revision checks, request replay/conflict and bounded ledger own
admission and publication; a lost result does not authorize replay under a fresh identity. Every
fallible permutation/projection preparation precedes the authoritative commit. Reordering leaves
Workspace/Project membership, Directory references, terminal bindings, worktree associations and
Client-local selected IDs unchanged. New Workspace creation appends to an explicit order; Close
removes the committed closed ID. Checkpoint capture/restore retains and validates ordering.

Reference behavior is Herdr `e0507237` R020/R021: original insertion boundaries, caller-ordered
noncontiguous blocks, atomic validation, and preserved logical selection. Native public operations
are `workspace move <target> --insert-index N` and `workspace move-block <targets...> [--before target]`.
Their Windows CLI and TUI consumers use this shared order, including filtered-list subsequences.
Generic moved/reordered event delivery belongs to the separate event-stream owner and remains an
explicit integration obligation; an ordering receipt is not event-delivery evidence.

Evidence obligations are shared Rust/TypeScript permutation and delta conformance; cross-Project
indexed/block/no-op/invalid/replay/stale cases; create/close/checkpoint continuation; bounded full
Workspace capacity; per-Client selection preservation; and real Windows list/TUI/terminal survival
and restart. Focused evidence does not establish full parity, Desktop UI behavior or other platform
delivery. There are no process, Git or filesystem effects in the ordering commit itself.

## Accepted canonical Tab move model

### Insertion boundary extension

`tab.move-index` carries exact `terminalContextId`, `tabId` and integer `insertIndex` in 0..256.
The existing scoped Move owner handles both strict action variants; `tab.move` retains its direction
wire unchanged. For original zero-based source position S and original Tab count L, accept N in
0..L and choose destination N - 1 when S < N, otherwise N. Boundaries S and S + 1 are no-ops;
0 moves first and L appends. No clamping or repeated relative commands are permitted. Invalid
current bounds yield retained `tab-move-index-out-of-bounds` after target/composition validation,
before no-op/revision allocation, with unchanged order and identity cursor. Exact request replay,
conflict, capacity, pending creation settlement and context-close fencing use the existing owner.
Complete records, including nested Panel trees and input policy, move intact. No Client focus,
terminal attachment, controller, Host input or Worker action belongs to this transition.

### Workspace context extension

The local Move owner also accepts a Terminal Context referenced by a canonical Workspace.
It applies the same four directions or insertion boundary, exact request fingerprint, shared revision, no-op and retained
result rules below. Context admission accepts the standalone context or an existing Workspace's
context; an unknown context is `TerminalContextUnavailable`. A Tab belonging to another context
is `TabUnavailable`. Move does not require a primary Directory, filesystem probe or live Worker.

Both context kinds accept canonical split trees, grouped/pinned Tabs and bound or unbound primary/child
Presentations. Unbound Presentations contribute no physical members; an unbound primary never
substitutes a child binding. Canonical
metadata is not a composition failure. Tab Move changes order only: it neither changes group/pin
metadata nor expands one Tab into a whole group. Exact requested placement that would break the
pinned prefix or split a contiguous group returns retained `tab-move-order-conflict`, with unchanged
state and usable authority, before revision allocation. Valid placements and exact no-ops retain
the ordinary result and replay rules; no clamping or implicit regrouping occurs.

Only standalone Move admission supplies `WorkbenchTerminalMembership` to Site's existing
reference-derived composition check. Workspace admission neither reads nor modifies the standalone
slot or its retained authorization. Under the same Site gate, the borrowed admission locates the
target in the validated context and prepares a complete-Tab permutation. The only canonical changes
are that context's Tab order and one shared revision increment; all other contexts, navigation,
Directories, bindings, structural identity allocation, terminal operations and notifications remain
unchanged. Dropping preparation has no effect. Replay retains the original result even after a
different context advances the shared revision; changing context under the same request ID conflicts.

The local, unpublished `tab.move` wire and Server operation are implemented with strict
membership/Site composition. The native standalone identity projector and Move flight are integrated, with
deterministic and Linux cohort process evidence in the
[native Session contract](../../orchbee/docs/native-tab-session.md). Protocol, Server and Client activate and roll back
together: the old positional Site check cannot coexist with reordered state. Rollback restores the
last verified in-memory cohort without deleting or terminating retained processes.

The problem is shared canonical Tab order: an admitted Client moves a complete standalone Tab while
existing Clients retain their active Tab identity and terminal. The Workspace extension above uses
the same permutation; the reference-derived Site composition below is standalone-only. Workbench owns order, revision,
strict membership and request/result retention. Site retains only
`Empty | Creating(reference) | Settled { reference, outcome }`; TerminalCoordinator owns retained
authorization and execution. Move adds no stored anchor identity/binding, per-Tab Site map, Open
field, structural identity, terminal plan, Worker IPC, viewer or controller effect. Close/Stop,
archive/restart, pin/group mutation, split creation, persistence, remote operations, Web migration and push remain
excluded from Move. Close has the separate local Server integration described above; its native
integration has separate Linux process evidence; release convergence remains pending.
No explicit-empty Open result is added.

The `WorkbenchTerminalMembership` returned by `tree_standalone_membership()` is a bounded read-only vector of every bound standalone member
in canonical order, exposing exact Tab/Panel/Presentation identities and binding. Workbench validates
the entire context and constructs it; Site never caches it. Empty membership is valid exactly when
the Site slot is Empty. Nonempty membership requires Creating or Settled with reference R,
`TerminalCoordinator.retained_authorization(R)`, successful exact conversion to
`PresentationBinding`, and exactly one matching binding anywhere in the vector. Position never
identifies bootstrap or completion. Zero/multiple matches, absent/unconvertible authorization,
duplicate context or missing retained authorizations for bound members are internal divergence, never
repaired or returned as `TabUnavailable`.

`standalone_membership()` remains the strict-simple projection for callers requiring one member
per Tab. Create, completion, Move and Open use `tree_standalone_membership()`, the bound tree read:
every bound primary and child of the standalone Tab tree in canonical order under the same context validation. Site
applies the same reference-derived anchor relation over its members, so a legal multi-leaf shape is
never a composition fault; "every member has a retained binding" alone does not prove the slot anchor,
and missing retained authorization, zero or duplicate anchor matches and every structural violation
remain fail-closed divergence.

Apply that relation under the Site gate after preceding admission decisions for new first/later
create and Move, and before joint exact
committed-create completion; Open applies it over the strict tree membership. This is not an unconditional entry guard: authorization, retirement,
active exact pending/final replay or conflict and earlier preflight retain precedence.
Precommit divergence retires Workbench and returns top-level `unavailable/service-unavailable`;
Open retains `server-busy`. Postcommit token/composition failure closes the carrier under the
existing fail-closed rule, never a precommit refusal. Settled Open remains readable after unrelated
Workbench retirement.

For a syntactically valid Move, the exact precedence is:

1. navigation-mutation grant/action authorization;
2. retired Workbench;
3. retained exact replay or full-fingerprint request-ID conflict;
4. request-ledger entry capacity;
5. exact Site, otherwise `SiteMismatch`;
6. expected revision, otherwise `RevisionMismatch`;
7. exact standalone or Workspace context, otherwise `TerminalContextUnavailable`;
8. open context-close fence, otherwise transient `unavailable/terminal-busy`;
9. complete strict target-context layout;
10. reference-derived unique-binding Site composition for standalone only;
11. exact target Tab, otherwise `TabUnavailable`;
12. insertion boundary not exceeding the current Tab count, otherwise `TabMoveIndexOutOfBounds`;
13. semantic no-op, yielding existing `Unchanged { revision }`;
14. preserve pinned-prefix and group contiguity, otherwise `TabMoveOrderConflict`;
15. revision exhaustion for an actual permutation;
16. prebuild/validate complete candidate projection, publication and fixed result;
17. reserve exact retained request/result bytes;
18. infallibly commit projection, revision and settled ledger entry.

Unsupported layout therefore precedes missing target/no-op; missing context keeps its typed
rejection. No-op precedes revision exhaustion. Typed rejection/no-op is retained only if the byte
policy admits it. Entry/byte exhaustion is transient unavailable with no ledger insert,
order/revision/cursor/Site/terminal change or effect. Exact replay preserves the original result
revision after later commits. The fingerprint includes Site, base revision, context, target and
direction or insertion boundary, including action kind. Move neither reads nor advances the structural identity cursor.

Common preflight and full membership yield an opaque borrowed admission; Site validates composition
before the prepared Move owns its complete publication and fixed ledger insertion. Dropping plans
has no effect. Recoverable validation, serialization and publication failures precede mutation.
OOM, panic and crash atomicity remain excluded. Move acquires only the Site gate, never Worker
registry/handle. Create/Open retain registry-to-Site ordering for placement commit, releasing both
before IPC; Move/read can settle while launch is parked. Response writes follow Site-gate release.

The required composition trace starts with first create A Pending, actually commits B, and moves
to `[B,A]`. Unrelated internal Workbench retirement preserves membership/tokens. Exact A completion
still validates both origin seals, wrapper association, retained authorization, unique binding and
slot reference before settling Terminal phase, Workbench result and Site outcome once and notifying
Open waiters. Later external reads, new Move, pending/final replay and same-ID conflict are fenced
by authorization-then-retired precedence. The admitted original response flight may deliver its
final result. Move/completion preserve fixed create-result revision, tuple, creation label and
dispatch count. Both completion orders are required.

| Claim / transition                                                                                      | Owning evidence and refinement                                                                | Limits                                                                |
| ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Four directions, target absence, no-op, revisions, replay/loss/conflict, both capacities and retirement | controlled implementation tests; strict corpus and deterministic/stateful prepared-Move tests | Named and generated traces; no exhaustive interleaving proof          |
| Pending first/later create, permutation, retirement, exact completion and both orders                   | controlled implementation tests; Site token/composition/stateful traces                       | Internal retirement is a Site proof cut, not a supported wire trigger |
| Slot reference/outcome, unique derived binding, two Open joins/wait/timeout                             | controlled implementation tests; deterministic Open/Site tests                                | Concrete conversion and zero/duplicate matches require tests          |
| Parked launch permits Move/read/replay                                                                  | Real Server-process order/revision/dispatch witness                                           | Supported interfaces only; no manufactured retirement hook            |
| Existing Client identity and fresh canonical-first startup                                              | [Native Session contract](../../orchbee/docs/native-tab-session.md), native/process traces    | No push, fallback, reattachment or universal permutation theorem      |

Selected evidence is stateful generation with shrinking/replay, deterministic core/adapter tests and controlled native-process traces. Fixed Workbench workflows have no separate TLA model. No new universal transformation/refinement theorem is claimed.

Move checks permutation/complete member-fact preservation, unchanged product cursor and unrelated
facts, exact commit revisions, immutable ledger, both capacities, no-op before revision exhaustion,
committed read history and retired/replay precedence. Concrete JSON/string/byte accounting,
unsupported-layout precedence, dropped preparation, publication refusal and gate/transport
composition remain deterministic/stateful and process obligations. These finite traces do not
prove all parallel transport schedules or unbounded permutations.

## Accepted multi-leaf membership model

A Tab legally owns an ordered sequence of leaf members: `panel.split` produces a second leaf in the
same Tab, and `panel.close`/aggregate Close remove them. The membership view consumed by
`tab.create` admission, `tab.move` admission and standalone Create completion must therefore be the
tree-aware ordered member list, not a one-leaf-per-Tab shape assertion. Reading a valid split tree
through the legacy shape retired the shared authority; the accepted correction changes only the
membership view at those gates and keeps every structural validation.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                      |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | TabCreate, TabMove and Create completion read terminal membership through a one-leaf-per-Tab predicate, so any legal split-bearing tree failed admission or completion as if it were corrupt.                                                                                                                                                 |
| Input        | The admitted command fingerprint, exact Site/context, expected revision, and the current canonical tree whose Tabs each own an ordered leaf sequence.                                                                                                                                                                                         |
| Data         | `tree_standalone_membership()`/`tree_terminal_membership()` ordered members, the Site-owned `Empty`/`Creating`/`Settled` slot, retained terminal authorizations and per-owner presentation bindings, and the immutable retained Create entry.                                                                                                 |
| Output       | The unchanged mutation publications and the retained Create settlement; no new result kind, ledger entry, revision rule or effect is introduced.                                                                                                                                                                                              |
| Invariants   | Tree membership replaces the one-leaf assertion only; the standalone slot still composes against members with the reference-derived unique-binding check; workspace admission never touches the standalone slot; completion re-reads the CURRENT slot and members; a removed member is never resurrected and its ledger settles exactly once. |
| Model        | Read ordered tree membership, validate standalone anchor composition where required, commit under the Site gate, and settle against the current slot and retained Create entry.                                                                                                                                                               |
| Owner        | SiteCoordinator composes under the existing Site gate; WorkbenchAuthority owns the membership views and projection; TerminalCoordinator owns retained authorization, phase and the completion seal. No owner or lock ownership changes.                                                                                                       |
| Dependencies | Existing tree membership readers, the shared command ledger, revision and publication machinery; no protocol, dependency, crate or scheduler addition.                                                                                                                                                                                        |
| Verification | Deterministic Site/SiteCoordinator witnesses and the Linux process cohort cover split/create/move and pending-Close fencing on real PTYs. The interleaved-completion witness parks Launch at a controlled Worker seam and is explicitly synthetic. No separate workflow TLA proof is retained.                                                |
| Limits       | Named deterministic fixtures and controlled process schedules cover finite layouts, corruption and runtime loss. They do not exhaust arbitrary fault combinations or transport schedules; concrete byte/layout bounds and open-reentry Empty-only intent policy remain with their existing owners.                                            |

| Claim                                                                                                    | Owning evidence and refinement                                                                                                                                                                                           | Limits                                                                                                                     |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| A valid multi-leaf tree admits create/move and completes pending Creates without shape-driven retirement | [Multi-leaf Site traces][multi-leaf-tests] and [pending Move traces][move-tests] cover standalone and workspace contexts                                                                                                 | Named layouts and fault cases, not an exhaustive fault model                                                               |
| The standalone anchor stays exact: Empty iff no members, otherwise the slot reference matches once       | [Site Create tests][create-tests] exercise both Empty mismatches, exact bootstrap binding, and missing authorization                                                                                                     | Composition is never weakened to the member-binding-only form                                                              |
| Workspace admission and mutation never read or write the standalone slot                                 | [Workspace Create cases][create-tests] preserve Empty/Creating/Settled standalone state; [Move cases][move-tests] preserve it during pending workspace moves                                                             | A shared request ledger and revision still serialize both contexts                                                         |
| Completion preserves projection, settles exactly once, and updates the slot only while it still anchors  | [Reference completion tests][completion-tests] and [multi-leaf traces][multi-leaf-tests] cover current and non-current completion after retarget                                                                         | Post-commit gate failure yields honest unknown; it never converts into a rejection or retire                               |
| Close retarget follows the surviving ordered members and the aggregate fence stays exact                 | [Multi-leaf traces][multi-leaf-tests] cover legal retarget, last Close to Empty and same-Tab split fencing while a new Create remains admissible                                                                         | Leaf-scoped Close keeps its own owner; same-Tab panel mutations stay fenced                                                |
| Close admission distinguishes pending/unknown execution from a known failed launch                       | [Pending-member Close tests][multi-leaf-tests] refuse Busy; [failed-launch Close tests][failed-close-tests] allow removal without a Worker for known failed launches and reject unknown execution or stopped maintenance | Live members still require their admitted runtime; a surviving pending member outside the target may anchor after retarget |
| Each admitted pending Create must reach retained settlement or honest unknown under completing dispatch  | [Create outcome tests][create-tests] and [service deadline tests][create-service-tests] exercise completion, retirement and silent-Worker timeout                                                                        | Finite completion/timeout witnesses; no general fair-scheduling liveness proof is retained                                 |

The predicates stay distinct by design. `composition_matches` resolves the current slot reference's
retained authorization and requires its binding to appear exactly once among the ordered members;
`tree_composition_matches` requires only Empty-equivalence plus every member binding retained. The
strong form applies wherever the standalone anchor association is written or validated: Create
admission (which may open the slot), Move admission (which must preserve the existing slot while
reordering whole Tabs), and Create completion (which re-reads the CURRENT slot and members). Split
admission/completion and Close admission/publication keep the weaker form because they never resolve
the anchor binding through composition: Split only appends a leaf under an existing Tab, and Close
publication derives the new anchor directly from the surviving ordered members rather than through
composition resolution.

Close therefore changes the anchor without re-anchoring through composition, and its admissible
trajectories are exact: a pending member can never be a Close target (Busy refusal), so no pending
anchor is ever removed or retargeted away by Close, and no pending completion can arrive after the
last Close emptied the slot. A Close whose target members are all settled may still retarget onto a
surviving pending member, which anchors as Creating and settles the CURRENT slot when its completion
arrives; a never-current pending Create settles its ledger only while a different surviving pending
member anchors; and the last Close of settled members leaves Empty while its already-final ledger
stays replayable. The [parked-completion process witness][multi-leaf-process] establishes ordering
through a controlled Worker seam. Its finite schedule does not establish independent Worker timing
or impose a production concurrency restriction.

## Scope and ownership

The native Server owns one canonical Workbench state per local Site. Projects hold organization
metadata; Workspaces and Directories belong to the Site. The structural `workspace.create` command
creates one Workspace with nullable Project membership and one empty Terminal Context. The default
user creation path instead uses `workspace.launch` to commit a Workspace, Directory and first terminal
structure atomically before shell dispatch, as described in the [directory contract](workspace-terminal-directory.md).
The first standalone Open also commits shared Tab/Panel/Presentation membership and logical terminal
binding before launch, as defined below. The [Workspace directory contract](workspace-terminal-directory.md)
adds explicit, independently authorized directory association after Worker validation. Local
`tab.create` appends a simple standalone or configured Workspace Tab
with the same joint membership/terminal-intent boundary and exact pending/final replay. Pure
`tab.rename` changes the canonical Tab label and clears automatic naming; `tab.move` changes its canonical position.
Both may run while that completion is pending. Other Tab/Panel transitions and Client rendering
have their owning contracts. Persistence, restart and historical navigation migration belong to
the [checkpoint contract](workbench-checkpoint.md); push subscriptions remain separate work.

The projection store is a private implementation component of this authority. It has no public
whole-snapshot mutation entry; only this authority may advance it. Every committed publication is
retained in a bounded 64-entry journal served through `workbench.publication.read` under the same
read grant, so baseline-holding Clients fast-forward with ordered publications. Push subscriptions
and complete Client Workbench integration remain unfinished.

The language-neutral request/result authority is
[the Workbench contract](../../../schemas/workbench/README.md). Workbench uses its own OBWP carrier
on the same admitted local connection; OBCS remains the terminal-only contract. Same-user OBLC
admission precedes either protocol. The Server then selects an explicit Site-scoped Workbench grant.
Development composition grants snapshot and publication reading plus the pure Workbench
operations;
`tab.create` also requires the explicit terminal-create grant. Directory association requires navigation
and its independent directory-association grant. For externally submitted OBWP reads and commands, the
authority checks that grant before any state read or ledger lookup. Read-only and denied policies
remain representable. An absent or wrong-Site grant returns typed denied without state, revision,
identity or ledger disclosure; a grant never comes from wire input. Clients submit semantic operations and expected revisions;
they cannot supply new product identities, replace the shared document, or publish Client deltas.

## Navigation model

`panel.rename` adds exact context/Tab/leaf-primary Presentation overrides to the pure metadata owner.
Its [Panel label contract](../../orchbee/docs/native-panel-labels.md) keeps the creation fallback
immutable and defines bounded normalized set/clear, no-op and projection semantics.
[Panel label authority tests][panel-label-tests] exercise normalization, set/clear, replay,
missing targets, authorization and capacity while preserving the binding and fallback.

| Field              | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem            | Independent admitted Clients must modify and observe one shared navigation structure without lost updates or duplicate logical effects.                                                                                                                                                                                                                                                                                                  |
| Inputs             | A strict snapshot or publication read intent, or a strict command with request identity, exact Site, expected revision, and an admitted pure action. Creation and Tab labels preserve their exact nonempty text, bounded to 512 Unicode code points; Panel overrides normalize bounded nullable updates as defined above. A Tab rename fingerprint includes its context, Tab, label, and base revision.                                  |
| Data               | Active or retired authority, one validated Snapshot, a Server-owned non-reusing identity cursor, and a bounded request/result ledger. The initial revision is zero and the standalone Terminal Context is empty. The adapter obtains a nonzero Site identity from operating-system randomness. The cursor never reuses navigation aggregate identities; unrelated Terminal/operation identity domains retain their existing owners.      |
| Outputs            | Read returns correlated available { snapshot }, denied {}, or unavailable { reason }. Publication read returns current { revision }, published { publications }, denied {}, or unavailable { reason }. Command returns its typed navigation commit, unchanged, rejected, denied, or unavailable. Created identities appear only in their corresponding success variants.                                                                 |
| Invariants         | Only Server commits shared structure. Every Workspace references exactly one context. Rejection preserves structure, revision and allocation state. A changed candidate advances exactly one revision. An equal fixed-name rename preserves revision. Tab rename changes only its target `Tab.label` and `Tab.automaticLabel`; non-target labels and every other projection fact remain exact. One request identity cannot commit twice. |
| Decisions          | Authorization precedes retired-state and ledger inspection. Active exact replay precedes expected-revision checks. Changed content under a retained request identity is a conflict. A rejected command has no structural effect. An unexpected internal invariant failure retires the authority atomically; every later authorized read or command, including replay, is unavailable.                                                    |
| Model              | A deterministic serialized transition over the complete candidate; network, clock, filesystem, randomness and process effects stay in adapters.                                                                                                                                                                                                                                                                                          |
| Dependencies       | Existing Workbench snapshot validation and bounds, strict OBWP framing, and the admitted local Session. No Worker or Host is required for these navigation operations.                                                                                                                                                                                                                                                                   |
| Ownership          | The authority owns snapshot/cursor/ledger together. The service owns initialization and serialization of calls. The application-session adapter owns protocol selection and bounded response delivery after the lock is released.                                                                                                                                                                                                        |
| Verification       | Schema-authored Rust/TypeScript conformance, deterministic stateful command traces with reduction and a real two-Client Server-process witness cover navigation and replay behavior.                                                                                                                                                                                                                                                     |
| Correctness limits | Deterministic tests cover concrete labels, identity encoding and JSON; generated traces and process schedules are finite. The Linux process cohort requires its native prerequisites. This transition group alone does not establish persistence, transport retry, restart identity or complete Client integration.                                                                                                                      |

| Action                   | Guard                                                                                         | Committed result                                                                 |
| ------------------------ | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| project.create           | Label and projection capacity accepted                                                        | One new Project with no Workspace                                                |
| project.rename           | Exact Project exists                                                                          | Its label changes; equal text is unchanged                                       |
| workspace.create         | Optional Project exists and context capacity accepted                                         | One new Workspace and its one empty Terminal Context                             |
| workspace.rename         | Exact Workspace exists                                                                        | Its label changes; equal text is unchanged                                       |
| workspace.set-project    | Exact Workspace and optional destination Project exist; changed membership has no Close fence | Change only nullable Project membership; equal membership is unchanged           |
| workspace.create-project | Exact Workspace exists; no Close fence; label and capacity accepted                           | Atomically create a Project and move that Workspace into it                      |
| project.dissolve         | Exact Project exists; its members have no Close fence                                         | Remove the Project and ungroup its Workspaces, preserving execution resources    |
| tab.rename               | Exact context contains the exact Tab                                                          | Its Tab label changes and automatic naming clears; equal fixed text is unchanged |
| panel.rename             | Exact context/Tab contains the Panel                                                          | Primary override changes or clears; equal normalized value is unchanged          |

Only Panel override updates normalize whitespace. No optimistic Client commit, automatic conflict retry, focus change or terminal
effect accompanies these operations. Reading does not occupy a command-ledger entry.

## Commit and replay

Authorization linearizes at the Server policy check, before any state read or ledger lookup.
Read linearizes at snapshot cloning under the same commit gate used by writes. A changed command
linearizes at the atomic snapshot/cursor/ledger commit; a settled rejection or unchanged command
linearizes at ledger insertion. Same-identity equal and conflicting races follow that same gate.

For an authorized valid command, the owner first resolves exact replay or request-identity conflict. A new
command is evaluated against the current Site and revision. It stages the complete candidate,
identity cursor, typed result and ledger entry before committing any of them. Candidate validation,
encoding, and projection publication preflight precede ledger-byte reservation. `CommitPlan` retains
an optional `PreparedProjectionAdvance`; after reservation, commit performs only `commit_prepared`
and cursor/ledger moves, with no recoverable check after the first mutation. An invalid or
unpublishable candidate cannot consume an identity or ledger capacity. The transport writes the correlated response after releasing state ownership.
Losing that response never rolls back the committed state.

Decision precedence is authorization, retired state, retained replay/conflict, ledger count, Site,
expected revision, exact target context and then target member, equal-rename no-op, collection capacity, identity allocation,
changed revision, and encoded candidate capacity. An equal rename is unchanged at `u64::MAX`.
Workspace creation preflights both identities together. Finally, canonical ledger byte accounting
must admit the staged request/result before any new settlement; byte exhaustion overrides that
prospective outcome with unavailable and leaves all state unchanged. This last check also covers
prospective rejections and no-ops. Existing replay is never subject to either capacity check.

The service serializes retirement under the same gate as reads and writes. A failed initialization
random source leaves only Workbench unavailable; unrelated terminal admission and execution remain
available. No retired authority automatically resets or accepts new work in this process.

Replay equality compares validated semantic command values, independent of JSON whitespace,
key order and equivalent escapes. Duplicate JSON keys are protocol failures before typed admission.
The ledger retains settled well-formed requests, including rejections and unchanged results.
Reusing an identity with different content does not replace its existing record. The limits are
4,096 retained request identities and 8 MiB of combined canonical request/result JSON. These are
serialized accounting bounds, not an exact heap-size claim. New work exceeding either limit is
unavailable without effects; exact retained replay remains available. There is no automatic
eviction that could turn an old request into a new effect.

This ledger and Site identity are process-local until the persistence/recovery integration is
implemented. A disconnected Client does not destroy the Site, and a replacement Client may
reconcile an exact request in the same Site process. This does not authorize blind input replay or
cross-restart request replay. A new Site identity rejects requests addressed to the former Site.

| Condition                                                                      | Result                                                                                                     |
| ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Missing or wrong-Site grant, or a mutation under a read-only grant             | denied; no current revision or state is disclosed                                                          |
| Command addresses another Site                                                 | rejected: site-mismatch                                                                                    |
| Expected revision differs                                                      | rejected: revision-mismatch                                                                                |
| Project missing                                                                | rejected: project-unavailable                                                                              |
| Workspace missing or unavailable for the addressed operation                   | rejected: workspace-unavailable                                                                            |
| Terminal Context missing                                                       | rejected: terminal-context-unavailable                                                                     |
| Tab missing from the named Terminal Context                                    | rejected: tab-unavailable                                                                                  |
| Retained identity with different semantic command content                      | rejected: request-identity-conflict                                                                        |
| Project, Workspace, context count or encoded snapshot bound exceeded           | rejected: resource-limit                                                                                   |
| Navigation identity cursor or changed revision cannot advance                  | rejected: identity-exhausted or revision-exhausted                                                         |
| New request exceeds either ledger bound                                        | unavailable: request-ledger-full                                                                           |
| Service retired or unexpected internal candidate/serialization invariant fails | unavailable: service-unavailable; the service retires rather than representing a bug as a client rejection |

## Launch Profile catalog

The authority also owns one ephemeral, per-Site Launch Profile catalog: private Server state that
names reusable custom-executable launch selections for `tab.create` and `panel.split`. The catalog
is memory-only and dies with the Site process; it never enters the public snapshot, so a
navigation commit cannot observe or mutate it and `deny_unknown_fields` fences old readers from
new shapes entirely.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Admitted Clients must list, resolve, create and delete named custom-executable selections and consume them at terminal creation without a second catalog writer or a persisted registry.                                                                                                                                                                                                                              |
| Inputs       | A strict `workbench.launch-profile-catalog.read` intent with request identity and an optional selector, or a strict `launch-profile.create`/`launch-profile.delete` command. A selector is opaque 1–512-byte text; a name shares the label envelope; a selection is the closed launch contract.                                                                                                                       |
| Data         | A bounded entry vector (64 entries, 256 KiB serialized aggregate), a private monotonically increasing catalog revision, and the same request/result ledger the navigation group uses. Profiles mint opaque ids `<siteHex>:<counterHex>` from the Site identity and the existing Server identity cursor.                                                                                                               |
| Outputs      | Read returns catalog { entries }, profile { entry }, not-found, ambiguous { candidateIds }, denied, or unavailable. Commands return launch-profile-created/deleted { revision, catalogRevision, profileId }, the journaled selector resolutions launch-profile-unavailable and launch-profile-ambiguous { candidateIds }, plus the ordinary rejected/denied/unavailable set.                                          |
| Invariants   | Only Server commits catalog state. A committed create/delete advances the Workbench revision and the private catalog revision together, atomically with projection, ledger and retained-request updates. An exact profile-id match resolves before any name lookup; otherwise zero name matches is not-found, one is found, and more than one is ambiguous with the bounded id list captured at that same resolution. |
| Decisions    | Reads carry the same site-identity grant semantics as snapshot reads and never enter the command ledger. Selector failures journal like every other settled outcome, so an identical request replays the same answer. Byte-capacity and revision-exhaustion rejections precede any catalog mutation. A dead Site's ids embed that Site identity and can never alias a different Site's entries.                       |
| Model        | The same deterministic serialized transition: candidate state is staged under the commit gate, and no recoverable check follows the first mutation.                                                                                                                                                                                                                                                                   |
| Ownership    | The authority owns catalog entries, the private catalog revision and minting. The service owns policy mapping (reads under the read grant, mutations under navigation) and call serialization. Terminal creation consumes a resolved `LaunchProfileUse` — never the selector text — so the Worker launch payload still carries only executable and arguments.                                                         |
| Verification | Authority stateful tests covering id-first/cardinality resolution, replay and conflict, count and serialized bounds, revision exhaustion with zero mutation, and cross-Site id non-aliasing; coordinator tests covering prepare-time freeze and provenance publication.                                                                                                                                               |

`tab.create` and `panel.split` accept either the existing explicit `launch` selection or a `profile`
selector, never both — the wire shape rejects the combination at parse and at serialize. A `profile`
selector resolves inside command admission under the same coordinator lock that owns the catalog:
the resolved selection and a `{profileId, catalogRevision}` provenance pair are frozen before the
asynchronous Worker gap, so a profile deleted after dispatch still launches its captured payload and
the terminal's launch read still publishes its provenance. Resolution failures settle as journaled
typed outcomes, and the ambiguity payload carries only candidate ids, captured at that resolution —
never executable text from another resolution.

## Evidence and refinement

| Claim                                                                                             | Required witness                                                                               | Bounds                                                                        |
| ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| One commit for competing expected revisions                                                       | [Authority traces][authority-tests] and [two-Client process race][authority-process]           | Controlled competing requests; serialized in-process authority                |
| Reads observe one committed state; denied calls reveal none                                       | [Publication history and grant tests][authority-tests]                                         | Read linearizes at cloning under the commit gate                              |
| Retirement fences subsequent reads, writes and replay                                             | [Authorization/retirement tests][authority-tests]                                              | Process-local retirement, no recovery claim                                   |
| A slow snapshot writer does not block another Client                                              | [Stalled-writer application-session test][session-tests]                                       | Controlled stalled writer; response I/O follows cloning and lock release      |
| Exact replay survives response loss without a new effect                                          | [Replay/conflict traces][authority-stateful] and [disconnect process trace][authority-process] | Retained Site-process ledger only                                             |
| A Tab rename changes only its target label and replays its original revision after a later commit | [Concrete rename tests][authority-tests] and [stateful traces][authority-stateful]             | Exact target, Unicode and byte cases; finite generated histories              |
| Navigation remains valid and rejection is effect-free                                             | [Stateful command traces][authority-stateful] and snapshot validator                           | Existing snapshot collection, identity and byte limits                        |
| Contract meanings agree across languages                                                          | Shared navigation command corpus consumed by Rust and TypeScript                               | Strict decoded JSON plus encoded carrier limits                               |
| Large snapshots preserve framing and direction                                                    | OBWP codec and [process traces][authority-process]                                             | 16 KiB requests/command results; snapshot results at most 1 MiB + 1,024 bytes |

Run from the repository root:

```sh
cargo test -p orchbee-server -p orchbee-workbench-protocol --locked
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol typecheck
cargo test --locked -p orchbee-server --all-targets --all-features
pnpm verify
```

The [stateful authority suite][authority-stateful] runs 96 fixed seeds of 80 steps with replay and
deletion shrinking. It covers navigation, exact replay, conflict, stale revision, denied admission
and reads; [deterministic tests][authority-tests] cover concrete target resolution, Unicode labels,
identity allocation, publication history and separate count/byte preflight boundaries.
[Application-session tests][session-tests] and [Server-process traces][authority-process] own the
transport and competing-Client witnesses. These finite traces are not an exhaustive interleaving
proof, and passing core or binding tests does not complete the native process witness.

## Standalone terminal membership model

The first local `terminal.open` that passes all preflights in an empty standalone context commits
one Tab, one Panel and its primary Presentation before submitting the Worker launch. This internal
semantic operation does not enter the OBWP navigation command ledger. The existing standalone Open
state supplies coalescing and its no-replay rule. Later successful reads may observe different
navigation revisions; they preserve the same committed standalone membership and binding.

| Field        | Contract                                                                                                                                                                                                                                                                               |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A retained native terminal must belong to the shared Workbench without an untracked launch or a second membership writer.                                                                                                                                                              |
| Input        | The admitted local Open, its real geometry, a fresh Server-generated Create reference, exact current Worker placement and lease.                                                                                                                                                       |
| Data         | One Site gate serializes the Workbench authority, terminal-intent coordinator and SiteCoordinator-owned Empty/Creating/Settled standalone state. Client identity, Create reference, aggregate identities and logical Terminal/Worker binding are distinct domains.                     |
| Output       | The existing correlated Open result plus one canonical snapshot revision containing the first standalone Tab, Panel and Presentation.                                                                                                                                                  |
| Invariants   | No launch without committed membership and terminal intent; no membership without its exact planned terminal intent; preflight failure consumes neither allocator; later navigation preserves the binding; no automatic rollback, reset or relaunch after commit.                      |
| Model        | Pure terminal preflight, validated Workbench candidate, one gated cross-component commit, unlocked dispatch, correlated terminal settlement, then response publication. Settled replay reads the strict tree membership and requires the retained anchor binding exactly once.         |
| Owner        | Binary-private SiteCoordinator composes both deterministic owners under the existing service gate. WorkbenchAuthority alone writes projection; TerminalCoordinator alone owns launch/start authorization and execution phases.                                                         |
| Dependencies | Existing OBCS Open, OBCN authorization, OBWP read, Workbench projection and local Worker registry. No new protocol, dependency, package or network authority.                                                                                                                          |
| Verification | [Standalone Site tests][standalone-tests] and [Create composition tests][create-tests] cover exact references, navigation and Open/Create ordering; [Server/Worker-carrier process traces][standalone-process] own the process boundary. Retained-shell PTY evidence remains required. |
| Limits       | This contract owns the process-local first-Open slot, not a limit of one terminal in the Workbench. Further Tab/Panel mutations, Client integration, subscriptions, persistence and recovery require their respective owners and evidence.                                             |

The terminal coordinator first builds a complete, non-cloneable Create plan without mutation.
Workbench then preflights an empty standalone context, three fresh aggregate identities, the next
revision, the complete encoded snapshot and publication. The context has no groups; its one Tab is
ungrouped and unpinned, with one Panel and no child Presentations. The first Tab and primary
Presentation use the locale-neutral ordinal label `1`. Their binding is the Server logical `TerminalId` and selected
`WorkerId` from the planned OBCN authorization. It never contains a Host terminal identity, Worker
generation, operation/lease identity, Client request identity or internal Create reference.

A non-null binding identifies retained product intent; it does not promise that Worker or Host is
currently available or that launch succeeded. Execution availability is resolved by its terminal
owner. A missing binding still conveys no lifecycle reason. Launch completion changes execution
phase, which this projection does not encode, and therefore does not add a second Workbench revision.

After Workbench validation and commitment, the same gate commits the prepared terminal binding,
its allocator and the standalone Creating reference through infallible ownership transfers.
Readers and other commands cannot observe between those steps. Atomicity covers successful
in-process execution under this gate, not crash/persistence recovery. Clock sampling and deadlines
remain in the adapter; no randomness, filesystem, Worker IPC, condition wait or response write occurs
under this gate. WorkbenchService applies its existing explicit read/navigation policy and delegates to this same Site authority; it owns no
second snapshot or mutex. Site randomness is read once during service construction outside locks.

Missing Worker, bad reference, exhausted identity/revision/capacity, or unavailable Workbench before
this commit returns the existing typed unavailable result without launch or membership. An internal
Workbench preflight/publication invariant failure retires Workbench before dispatch. A later
navigation retirement does not cancel an already committed launch. Direct experimental Create and
Start remain independent of Workbench initialization; they do not create Tab membership.

Every post-commit failure retains the shared Tab and logical binding and settles the existing typed
failed, unavailable or unknown Open result. Client loss and unread responses cannot undo it. Exact
Worker-generation checks still fence success; replacement does not adopt or relaunch the terminal.
The existing condition wait releases the gate and preserves its one absolute deadline.

Creating and Settled replay read `tree_standalone_membership()` — every leaf member of the
standalone Tab tree in canonical order — so a legal split or reordered tree is never an invariant
failure and never retires the authority. The anchor relation is unchanged: the slot is Empty exactly
when the tree has no members; otherwise the slot reference's retained authorization must convert to
a `PresentationBinding` occurring exactly once across all members. A committed split leaf is a
member before its terminal outcome is known, so a pending sibling neither retargets nor blocks
replay of a settled reference; a legal leaf reorder preserves it. Only the existing Close settlement
changes the reference — to the retained survivor — and the final removal returns Empty; Open never
selects a substitute terminal. Missing retained authorization, zero or duplicate anchor occurrences
and every tree-shape or binding violation remain fail-closed divergence that retires the authority
and returns `server-busy`.

[Standalone Site tests][standalone-tests] cover first-Open commitment, navigation after commitment,
replay and dispatched failure outcomes. [Create composition tests][create-tests] cover both
Open/Create orders, multiple retained references, both completion orders, anchor mismatches and
retirement. [Multi-leaf tests][multi-leaf-tests] cover split membership and legal Close retarget.
These are concrete finite traces; no separate standalone membership TLA model is retained. Exact
encoding, allocation and the real-process boundary still require their owning tests.

## Terminal selection from Workbench bindings

A local Client may request read-only viewer attachment using the exact `siteId`, `workerId`
and logical `terminalId` obtained from a canonical Workbench snapshot. The OBCS `attach-binding`
action is terminal selection; it does not submit a Workbench command or change membership.
The native Client uses it after standalone Open and a correlated OBWP snapshot read on the same
admitted connection. Existing explicit Create-reference attachment remains available for the
experimental Create/Attach path.

| Field        | Contract                                                                                                                                                                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A Client reading shared product state must select its retained terminal without depending on an internal Create reference or silently following a different execution identity.                                                                               |
| Input        | An admitted Session and the exact Site/Worker/Server-terminal tuple. All three are independent opaque identity domains; none is a Host identity, generation, operation, lease or Create reference.                                                            |
| Data         | Canonical exposed bindings, retained terminal authorization/phase, current Worker placement and Session-local attachment state.                                                                                                                               |
| Output       | Existing Attached scope, or a typed refusal/unavailability. Selection has no terminal creation, start, input, resize, controller grant or Workbench effect.                                                                                                   |
| Invariants   | Resolve only a binding exposed by the active current Site; pin the retained exact Worker generation; never substitute a terminal or Worker; refuse before allocating attachment identities when target validation fails.                                      |
| Ownership    | WorkbenchAuthority checks canonical membership; TerminalCoordinator resolves retained execution; SiteCoordinator composes those checks under the existing registry-then-Site lock order. ViewerSession owns attachment IPC and cleanup after releasing locks. |
| Model        | Resolve a typed target, prepare an attachment, dispatch through the pinned handle, and commit the Session attachment at the existing current-Worker check.                                                                                                    |
| Dependencies | Unpublished OBCS v1 values, unchanged OBCN/OBSR relay, existing OBWP snapshot read, and the native Client's existing absolute submission deadline.                                                                                                            |
| Limits       | Same-user local target resolution and no takeover. The Selection model does not prove multiple viewer routes, remote authorization, persistence, restart adoption, selection UI or continuous Worker availability.                                            |

The [Session routing owner](../README.md#accepted-session-viewer-routing-modeling-record) first
reaps exact known-Disposed entries and enforces its capacity bound. With capacity available, a
duplicate resolved live target in the same Session returns `already-attached` before Worker IPC;
different targets can coexist. Within target resolution, poisoned registry or Site ownership returns
`server-busy`. Missing or retired Workbench returns `workbench-unavailable`; a wrong Site or a
tuple absent from its currently exposed Presentation bindings returns `terminal-unavailable`.
A missing current Worker or mismatched Worker identity is `terminal-runtime-unavailable`.
Next, a missing exact retained record is `terminal-unavailable`; a mismatched retained generation
is `terminal-runtime-unavailable`; finally, a phase that cannot attach is `terminal-unavailable`. These target refusals precede attachment identity allocation and Worker
IPC. Identity capacity and known unchanged queue/attachment capacity retain the existing
`server-busy` meaning. The existing known-unchanged `server-busy` viewer retry may submit the identical binding tuple with
a fresh OBCS request identity under the same logical deadline. There is no snapshot-read retry,
fallback, terminal creation, retargeting or unknown-effect replay.

Membership validation and target resolution linearize together under the Site gate while the
Worker registry is held. Selection does not require the snapshot's revision to remain current:
unrelated navigation may commit while the exact binding remains exposed. The prepared target pins
the retained Worker generation and Host identity; the Client supplies neither. Later Workbench
retirement does not cancel an already resolved attachment. Successful attachment linearizes at
the existing post-IPC current-Worker check. Worker loss after that check can race response delivery;
the next viewer/control operation still applies existing generation fencing. Attached is not a
promise that the Worker stays available.

OBCS retains the existing same-user terminal admission. OBWP reading/navigation policy still owns
only its respective requests; knowing a tuple creates no new authorization source. Production Site composition commits membership from the exact planned terminal authorization.
An exposed tuple with no matching retained record, including a terminal stored under another
Worker, is therefore unreachable through that construction. The query still returns
`terminal-unavailable` for such an inconsistent fixture; it introduces no retirement or repair.
The Workbench lookup is an internal read, not a second snapshot writer or a command-ledger entry.

| Claim                                                                          | Required witness                                                                                                                                                                                         | Bounds                                                                                                      |
| ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Exact exposed tuple resolves without changing membership or terminal execution | [Site selection tests][selection-tests]: `exact_exposed_binding_prepares_the_retained_host_without_changing_workbench` and `workbench_refusals_precede_runtime_interpretation_and_consume_no_identity`   | Existing bounded canonical snapshot and retained terminal map                                               |
| A replacement Worker cannot retarget or newly authorize a stale selection      | [Viewer-session tests][viewer-tests] cover Worker loss before and after the post-IPC check; [standalone process tests][standalone-process] retain membership across Worker replacement                   | Controlled generation/replacement cases; an accepted attachment does not promise future Worker availability |
| A Client selects from the shared snapshot on its admitted connection           | `standalone_selects_the_snapshot_binding_and_never_replays_unknown_open`, `canonical_snapshot_binding_attaches_exact_host_without_launch_and_is_session_scoped` and fresh release PTY startup            | One standalone Tab with one Panel and a bound primary Presentation                                          |
| Mixed OBWP/OBCS responses remain correlated and deadline bounded               | `snapshot_read_supports_partial_io_and_preserves_the_same_mixed_carrier`, `snapshot_read_retires_on_wrong_kind_or_correlation` and `standalone_snapshot_does_not_complete_or_reset_the_startup_deadline` | Existing startup ceiling; no snapshot-read or ambiguous retry                                               |
| Wire meanings agree with their authority                                       | `schema_and_corpus_are_well_formed_and_every_canonical_message_round_trips` and `consumes_every_invalid_and_forbidden_corpus_case_as_a_rejection`                                                        | Additive unpublished v1, existing framing and byte bounds                                                   |

The Client read-error and structural selector rules belong to its
[standalone intent](../../orchbee/README.md#standalone-terminal-intent).
Identity domains impose no pairwise inequality on encoded bytes. Each field must independently
satisfy its nonzero identity contract; equal text in different domains is valid.
Deterministic selection, carrier and runtime-deadline tests own admission, progress and failure settlement; this workflow has no separate TLA proof.

## Standalone Tab creation model

The local `tab.create` refinement composes one shared membership commit with one retained-terminal
intent. Its [wire contract](../../../schemas/workbench/README.md#standalone-tab-creation-contract)
is an unpublished OBWP v1 extension. The Server command, mixed-carrier dispatch and native Client
development interaction are implemented. The [native Tab guide](../../orchbee/README.md#native-tab-interactions)
owns Client evidence; installed delivery remains unfinished.

| Field           | Contract                                                                                                                                                                                                                                                                                                                              |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem         | Independent Clients must create another standalone terminal Tab without a second membership writer, an untracked launch, or a duplicate effect after response loss.                                                                                                                                                                   |
| Input and trust | A strict command with request identity, exact Site/context, expected revision, exact label and validated geometry; a Server-selected combined Workbench-command and terminal-create grant. Navigation permission alone does not authorize launch. Entropy, lease time and current Worker placement are adapter inputs.                |
| Data            | One canonical projection/revision/cursor, both creation and current Tab labels, the immutable Presentation creation label, a bounded Pending/Settled command ledger with committed result revisions, one retained terminal record per committed intent, and a Site-owned first-terminal slot. Each identity domain retains its owner. |
| Output          | Precommit denied/rejected/unavailable; exact-replay pending; or one final tab-created result with started, unavailable, failed or unknown terminal outcome. Every committed result retains the same Tab/Panel/Presentation tuple and binding.                                                                                         |
| Owners          | WorkbenchAuthority alone writes projection, revision, cursor, ledger and retirement. TerminalCoordinator alone writes retained terminal authorization and phase. SiteCoordinator composes both owners and the first slot. Adapters own entropy, clock, locks, IPC and result delivery.                                                |
| Model           | Prepare both owners without mutation, jointly commit under the Site gate, allow an already-authorized pure rename to commit, dispatch once outside owner gates, prepare both completions without mutation, then jointly settle. Completion preserves projection revision and both labels.                                             |
| Dependencies    | Existing bounded protocol validators, projection publication, same-user admission, current Worker registry, retained-terminal coordinator and one sequential mixed Client carrier. No library dependency on OBCS/OBCN, Worker handles or Host execution is introduced.                                                                |
| Non-goals       | Other Terminal Contexts, split/group/pin/reorder/close, persistence, restart adoption, remote authorization, cancellation, automatic retry or replacement launch, subscriptions, release packaging and legacy cutover.                                                                                                                |

### Admission and joint commit

The library classifies a wire request without reading authority state. Its opaque
`WorkbenchReadOrNavigationRequest` accepts only snapshot reads, publication reads and the pure Workbench actions;
`TryFrom<WorkbenchRequest>` returns the complete original owned request when it cannot classify it.
The service explicitly routes rejected TabCreate and TabMove commands to their Site composition
paths; any other rejected request receives a definite unavailable response. Rejection never
implicitly selects terminal creation and preserves all request metadata.
The pure `execute` entry accepts only that classified type. Terminal creation uses the dedicated
admission/preparation path below; a generic executor never fabricates server-busy merely because
it has no terminal plan. Classification does not authorize a request, and pure reads/navigation
retain their independence from Worker registry availability.

Decision precedence is the combined grant, absent/retired Workbench, exact retained replay or
semantic conflict, ledger count, Site, expected revision, exact standalone context, Tab/node
capacity and exact placement source/group, three structural identities (four for a new group), changed revision, current Worker, retained-terminal
resources, candidate publication, and ledger bytes. Byte admission also covers retained
rejections. Known precommit missing Worker or busy resource results are transient and do not
create a ledger entry. Full semantic equality includes context, label, naming mode, placement, launch options, decoded geometry and
expected revision; a conflict never replaces its existing record.

The adapter samples inert reference/lease candidates before locks. Their absence cannot override
authorization or a retained replay. It acquires the Worker registry, captures and retains the
current handle and placement, then acquires the Site gate. Both gates remain held through joint
commit, preventing a same-Worker generation change between placement capture and publication.
No clock, entropy or Worker capability enters the deterministic owners.

Workbench admission and prepared publication hold an exclusive borrow of their exact authority.
Admission stages the structural identities and revision before runtime interpretation. Finishing
with the terminal plan's exact binding validates and prebuilds the entire candidate projection,
its publication, pending result and every possible final result. Terminal preparation likewise
holds an exclusive borrow of its exact coordinator. Dropping either plan has no effect. The
Site borrows its owner fields disjointly; after both plans succeed, commitment consists only of
infallible moves into the retained Creating record, complete Workbench publication/ledger, and
first slot when applicable. No serialization, candidate validation or fallible publication step
may follow the first mutation. Infallibility means no recoverable Result failure; ordinary Rust
container allocation may abort. OOM, panic and crash atomicity are excluded, and whole-map copying
is not required. The supplied label becomes both Tab.label and primary
Presentation.label exactly, except duplicate placement uses the source raw label and naming mode;
duplicate labels remain valid. Shared `create_terminal_tab` constructs one complete candidate for
append, group insertion, exact target-relative insertion, new unnamed grey group or single metadata duplicate. `after-tab`
resolves the current target group and index + 1 under CAS; an unpinned insertion inside the pinned
prefix refuses with `tab-move-order-conflict` before runtime or identity allocation, while the last
pinned boundary succeeds. Empty groups are pruned
and the final group count bounded for new-group placement. No intermediate ungrouped Tab is published.
IDs, projection, revision and pending receipt commit with the same retained-terminal intent. Source
Panel trees, children, bindings and runtime state are never cloned. Placement adds no second launch
or persistence owner. [Native groups](../../orchbee/docs/native-tab-groups.md#terminal-creation-and-duplication)
owns GUI semantics, Client selection and exact-receipt refinement. These concrete placement cases are
covered by the shared GUI/Rust corpus and [authority placement tests][placement-tests], including
preparation drop, replay and capacity. Placement shares the same membership commit.

Pending entries count toward the existing 4,096-entry and 8 MiB canonical request/result limits.
Before commitment, the entry reserves request bytes plus the maximum result size across pending,
started, both unavailable reasons, failed and unknown. The prevalidated final-result table stays
private to WorkbenchAuthority. Settlement only selects a table member and releases unused charge;
it does not advance the revision or change membership. Exact replay clones the pending or settled
result independently for that invocation. The original creating invocation waits for final
settlement; only replay may return pending. Later settlement or conflict cannot rewrite a
previously cloned response awaiting delivery.

A committed Create retains its membership revision in pending and final replay. A later Tab rename
may change the Tab label while the Presentation label remains the creation anchor. Completion
settles only the original Create ledger entry and cannot advance revision or rewrite either label.
The [parked-Create rename process trace][rename-process] checks read/create/rename/read/settle and
both replays under a controlled Worker schedule; it is not an exhaustive rename-interleaving proof.

### Completion and first-terminal correspondence

Each Workbench Pending entry and retained Terminal Creating record owns a private per-entry
`Arc` seal; its corresponding sole opaque, non-Clone completion token retains the same allocation.
Pointer equality identifies the exact entry without a wire identity, random nonce or additional
counter. An opaque Site pending wrapper is the only composition constructor and keeps both tokens,
their reference/binding/revision association and pinned placement together across IPC.

Both completion owners validate their exact token, phase and immutable association before either
mutates. Borrowed completion plans compute the final terminal phase and select the prevalidated
Workbench result. The Site checks their association and first-slot correspondence, then consumes
both plans and the applicable slot transition with infallible moves. Foreign, stale, consumed or
mixed tokens leave every semantic owner unchanged. An impossible postcommit coordination or lock
failure closes the mixed carrier without a semantic command result; it cannot claim precommit
service-unavailable after a Tab was committed. The Client then observes command outcome unknown.

The accepted Move composition makes empty standalone Tabs equivalent to an Empty first slot.
Nonempty Tabs require Creating or Settled and exactly one full-membership match for the binding
derived from its retained reference, independent of position. Settled retains that reference even
for unavailable, failed and unknown results. Both directions and exact retained reference/binding
are checked after preceding admission decisions before an initial joint mutation. An inconsistent
precommit composition retires Workbench without modifying terminal/slot/effects: OBWP returns
service-unavailable; Open returns its existing server-busy outcome. A postcommit mismatch instead
uses the carrier-close rule above.

Open and tab.create share the simple membership builder and first slot. If Open wins, an
old-revision command rejects; if tab.create wins, Open joins its existing Creating slot without
another reference or launch. Every first-terminal final outcome settles that slot before waking
all waiters. A settled Open remains readable after unrelated Workbench retirement. Retirement
fences later Workbench reads, commands and replay but does not cancel a previously valid completion
or clear itself. Worker replacement never substitutes a reference, binding or launch. These
owners remain process-local.

| Worker observation                                                                  | Final nested terminal outcome              |
| ----------------------------------------------------------------------------------- | ------------------------------------------ |
| Known queue backpressure before handoff                                             | unavailable / server-busy                  |
| Definite delivery unavailability, including pre-delivery wrong generation           | unavailable / terminal-runtime-unavailable |
| Correlated Host rejection or failure                                                | failed / terminal-create-failed            |
| Exact accepted immediate launch, requested geometry and still-current pinned Worker | started                                    |
| Received Launch/Start followed by current-Worker mismatch                           | outcome-unknown                            |
| Ambiguous delivery or mismatched post-handoff observation                           | outcome-unknown                            |

A failed Tab, Workspace-launch or Panel creation also publishes `launchFailure` on the created
Presentation in one further revision under the settlement's Site gate, so observers that never
received the result learn the failure from the snapshot. It names only the submitted executable,
or `null` when the Worker resolved its default shell. Publication is skipped when the Presentation
was rebound, removed or already failed, or when the projection budget or revision space is
exhausted; the result then remains the only failure fact. Nothing retries the launch, and
restoration rebinding clears the fact.

Worker IPC and Client result writes occur after releasing registry and Site gates. Another
admitted Client can complete a snapshot read or pure navigation command while launch or response
delivery stalls. The existing Worker carrier still serializes its one in-flight and one queued
operation; no viewer/terminal completion during a parked same-Worker launch is promised.

### Creation evidence and limits

| Claim                                                       | Required evidence                                                                                                                                                                                                      |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| One shared commit precedes at most one pinned launch        | [Site Create tests][create-tests] cover Open-wins, command-wins, two-command and all-outcome traces; [parked-Launch process witness][create-process]                                                                   |
| Preflight refusal leaves owners and allocation unchanged    | Core cases for every precedence boundary, retained record/reference/identity/lease capacity, candidate publication and ledger charge; plan-drop and generation-fence cases                                             |
| Exact replay preserves effect count and immutable responses | [Site pending/final replay tests][create-tests], [authority reservation tests][create-authority-tests] and [process replay witnesses][create-process] cover conflict, retirement and controlled response ordering/loss |
| Rename during pending creation survives final settlement    | [Read/create/rename/read/settle/replay process trace][rename-process]                                                                                                                                                  |
| Completion cannot mutate a foreign or partial composition   | Equal-state distinct-owner token tests, stale/mixed association cases, compiler-enforced borrowed plans and private Site wrapper; both preparation-failure boundaries                                                  |
| Pending reservation always covers settlement                | Exact canonical byte/max-final and ledger-count boundaries; generated begin/commit/replay/dispatch/settle/retire/capacity traces with deterministic shrinking and replay                                               |
| First-slot correspondence and waiter settlement             | Empty/nonempty mismatch tests, both Open/create orders, all final outcomes, valid settlement after retirement, and bounded silent-Worker timeout through joint settlement                                              |
| Other Clients progress without a second launch              | Separate OBWP snapshot/replay/navigation streams during parked launch; a preattached OBCS viewer Poll as a positive subsequent Worker FIFO witness                                                                     |
| Concrete integration agrees with the abstraction            | Strict Rust/TypeScript corpus, owner/core tests, controlled Server process traces, native two-Tab PTY and second-Client refresh, independent implementation review and selected repository gates                       |

The maintained process witnesses are [standalone terminal operations][standalone-process],
[parked Tab creation][create-process], [rename during creation][rename-process] and
[multi-leaf interleavings][multi-leaf-process]. They require the supported Linux process cohort
and built Worker/Terminal Host prerequisites; a Windows host gate or cross-target compilation
does not execute them. The [service silent-Worker test][create-service-tests] uses the production
20-second response timeout and checks settlement before notifying the first-Open waiter. The
parked-Launch process trace observes the second canonical Tab, pending replay and another Client
commit before releasing the Worker, then checks final replay and a positive viewer-Poll boundary
against duplicate dispatch. Controlled process evidence does not replace native Client Tab
interaction, crash/restart evidence or the required root verification gate. Historical test counts
are not a current verification result.

Schema, both bindings and consuming binaries move together in one unpublished v1 activation cohort.
Old closed decoders reject the extension. Rollback restores the whole cohort; no mixed-version
rollout, durable migration or cross-restart replay is claimed.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf exchange, preserving Split/slot and terminal identities. The Windows CLI separately acknowledges source focus through the original Client; known canonical commits survive later failure or unknown focus. [Native Panel swap](../../orchbee/docs/native-panel-swap.md) owns the workflow, receipt and evidence.

## Automatic Tab naming

The [Tab label contract](../../orchbee/docs/native-tab-labels.md) owns optional canonical mode, compatibility and consumer refinement. Server fingerprints `tab.create` `automaticLabel` and commits it atomically to the Tab; omission is fixed, while the first implicit standalone Tab is automatic. Move and close preserve surviving naming metadata. Rename sets text and fixed mode together; pending creation settlement and replay never restore the previous mode. Abstract label equality includes text and mode, so clearing automatic naming is changed even when text is equal.

## Panel presentation metadata

`WorkbenchAuthority::panel_metadata` owns transient reports for each logical terminal binding
(`workerId`, `terminalId`). Legacy reports resolve a Panel's current primary binding and require an exact
`expectedBinding`; a changed, missing or unbound target fails before applying the report. Primary
and child memberships retain the resource across moves and swaps. An operation prunes bindings
absent from all committed memberships. Site disposal or restart drops this state.

Inputs are strict `PanelMetadataRequest` values with normalized source, display text, token keys,
optional sequence and a 1–86,400,000 ms TTL. Read takes 0–4 unique ordered Panel IDs and returns
binding, tokens and source presentation records. Source and text reuse the Workspace metadata
canonical forms. Display states are `idle`, `working`, `blocked`, `done`, `error`, `unknown`. Guards on
Agent label and authority source compare exact observed facts; missing context never matches.
A generic observation category such as `hook` is not an authority-source identity.

| Transition                                        | Guard and result                                                                                                                         |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Fresh report with presentation sets and no clears | Replace that source's entire presentation, including guards and omitted fields; assign write order to supplied fields                    |
| Report containing a clear flag                    | Clear only named fields, apply supplied values/guards, preserve untouched field priority; setting and clearing the same field is invalid |
| Presentation TTL                                  | Sets or explicit TTL refresh the source presentation expiry; clear-only without TTL preserves it                                         |
| Token patch                                       | Update or remove named global terminal tokens; each value has its own expiry; unrelated keys retain their expiry                         |
| Old or equal source sequence                      | Return `stale` without mutation or expiry refresh; omitted sequence never resets the high-water mark                                     |
| Read or report time boundary                      | Expire values at the supplied monotonic time; retain sequence fences until binding disposal                                              |
| Capacity or checked revision/deadline failure     | Reject the whole report without consuming its sequence or partially applying fields                                                      |

Effective presentation selects the newest eligible value independently for title, display Agent
and each state label. A clear of an unrelated field cannot promote an older title. Metadata has
its own checked revision and never increments canonical Workbench revision. Expiry/pruning is an
independent maintenance transition before an otherwise valid operation. Limits are 32 sources,
32 token keys per binding, 4,096 retained bindings and 16 MiB of accounted retained payload; the
accounting is not a process RSS bound. Sequence-only records count toward the source limit.

The Site gate is the single writer and samples time after admission. This reducer has no I/O,
background expiry timer, durable storage, canonical topology mutation or notification event.
Read/report DTOs and the deterministic owner tests cover replacement, selective clearing,
per-field precedence, guards, sequence replay, TTL boundaries, atomic capacity failures,
revision exhaustion and primary/child binding lifetime. Compatibility targets Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34` presentation/token behavior; provider state authority,
resumable sessions and TUI rendering remain separate consumers. A metadata report never creates
an Agent authority or changes OSC terminal title. Public CLI integration requires the carrier and
query projection; the owner alone does not establish CLI parity.

The [Agent report model](agent-report-model.md) owns explicit report transitions, the bounded
physical-generation registry and the Windows report/read carrier. Its trusted process operation
prepares report and metadata cleanup together under this commit gate, preserving tokens and newer
custom presentation. Later metadata from a retained exited Agent is stale. Canonical observations
retain their existing owner. Automatic Host process production, names and joined query/TUI context
remain integration obligations; the report wire cannot assert process existence.

## Structural operations preserve child Presentations

A Panel remains a layout leaf when it owns child Presentations. Bound tree membership enumerates
the primary followed by every ordered child, retaining each exact binding. `member_for_panel`
selects the primary; a member count is a Presentation count, not a Panel count. Legacy single-leaf
membership still rejects additional Presentations and grouped/pinned Tabs. Complete tree membership
includes group/pin metadata under canonical validation. Unbound members remain in the canonical
Panel tree but do not enter the runtime-binding vector; explicit primary-role metadata prevents a
bound child from satisfying `member_for_panel` for an unbound primary. Agent registration can
observe surviving bound members while other members remain unbound.

Canonical occupancy is separate from bound inventory. Bootstrap requires a context with no Tabs;
Open on a wholly unbound layout returns `terminal-runtime-unavailable` without launch or retirement.
Explicit Tab Create or Split can establish the first bound standalone reference. Close scope uses
canonical Tab/Panel addresses and topology; unbound siblings survive, and an entirely unbound
scope uses retained publication-only Close. Mixed-member and empty-scope traces cover registration,
Open, split settlement, replay, exact publication, and standalone retargeting.

Tab creation and ordering admit contexts containing child Presentations. Panel split wraps the
complete original Panel and one fresh sibling. Same-Tab move/swap and cross-Tab/context relocation
transfer the complete Panel node, including ordered child identities, labels, bindings and input
policy. No existing Presentation is recreated, rebound or dropped. New Workspace directory
selection retains its existing primary-terminal ownership; this operation starts no extra process.
Relocation checks pending Close ownership of its canonical Panel, including unbound members. Standalone reference
reconciliation validates the entire before/after binding set under the existing Site commit gate.

Evidence must compare complete Panel payloads after split, move/swap and relocation, cover new Tab
and Tab ordering with children, preserve primary-versus-child identity, and verify exact standalone
reference composition and existing Close fences. Native child selection/navigation, child creation,
unbound-member lifecycle remain separately tracked obligations. Native group/pin and creation
coverage is recorded in [native groups](../../orchbee/docs/native-tab-groups.md). There is no
wire or checkpoint migration; the matched Server cohort activates or rolls back this behavior.

## Grouped and pinned structural operations

Workbench accepts canonical group metadata and pinned prefixes for bound structural operations.
Tab creation without placement appends one ungrouped/unpinned Tab; typed placement also admits
target-relative creation, group-tail creation, an atomic new group with Tab, and exact single-Tab duplication. Panel split/move/swap and relocation preserve
complete existing Panel payloads and surviving Tab/group metadata; removing the final Panel removes
its Tab, and moving into a new Tab creates an ungrouped/unpinned destination. Empty groups remain
independent metadata except new-group creation cleanup. Structural operations without explicit Tab
placement do not implicitly pin, unpin, group or ungroup a Tab.

For Tab Move, the existing direction or original insertion boundary first determines one exact
permutation. No-op returns unchanged before revision allocation. The candidate must retain a pinned
prefix and contiguous members for every group; otherwise `tab-move-order-conflict` is retained
without mutation or retirement. This is an Orchbee metadata constraint over Herdr's exact
remove/insert semantics (pinned `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `workspace.rs::move_tab`);
Herdr does not define these group/pin semantics. Request replay, context-Close fences, all-member
Site composition, capacity and rollback retain their existing owners. No process operation occurs.

Evidence compares complete records against independent remove/insert and contiguous-span oracles,
including legal/illegal placements, all directions, all insertion boundaries, no-op at exhausted
revision, exact refusal replay and subsequent valid work. Restored grouped/pinned Site composition
and child-preserving create/split/placement/relocation remain required integration witnesses.
Rust/TypeScript/wire schemas and native failure classification include the new reason together.
Native indexed failures use v2 while schema v1 remains closed; direction uses the dedicated v2
envelope only for this new refusal. Matched protocol/Server/Client rollout or source rollback does
not compensate already committed order. Native group/pin creation UI is covered by [native groups](../../orchbee/docs/native-tab-groups.md);
unbound lifecycle and actual Windows multi-Client TUI acceptance remain separate obligations.

## Exact Presentation metadata reads

Panel metadata `read-presentations` accepts at most four unique Presentation IDs in request order, including primary and child members and empty revision fences. Each `presentations-read` row contains `presentationId` and a nested `panel` row with owning Panel ID and the selected member’s binding, tokens and source records. Duplicate Panel IDs across different members are allowed. Unknown members reject the batch; unbound members expose empty metadata. Source uniqueness and nonzero field orders bounded by metadata revision apply identically to both read paths. Membership and binding-owned resources are resolved under the existing Site gate; reads do not mutate topology, selection or input ownership. Existing primary Panel reads and metadata writes retain their scope.

## Exact Presentation metadata writes

`report-presentation` extends the existing metadata carrier with a required Presentation ID. The
Site gate proves exact Panel ownership and a nonnull matching expected binding before applying
source guards, sequence ordering, token patches or TTL. It reuses the binding-owned resource;
primary and child records therefore remain isolated, while moves retain the same terminal data.
A wrong owner/member, unbound target or stale binding produces a typed failure with no redirected
write. `presentation-reported` echoes the exact identity and both revisions, including stale
no-op reports. Legacy `report` remains primary-only. Both write variants require navigation
permission. Shared contract and owner traces cover correlation, isolation, replay and rejection;
existing TTL and resource accounting remain authoritative. The native
[metadata command](../../orchbee/docs/windows-panel-metadata.md#exact-member-targets) owns CLI
selection and unknown outcomes. Matched cohort deployment introduces no durable migration.

## Terminal creation inside an existing Panel

`panel.create-terminal` appends a child or binds one exact unbound primary/child. The shared
`terminal_panel_create` authority owns split and member placement; the placement sum stores split
identities only for a real split. Allocation is five/one/zero identities for split/child/bind.
The canonical Panel, existing member order, labels and overrides survive. The 16-child bound,
exact member eligibility, current revision, both navigation/create grants, Workspace Worker/directory,
Close scope and profile resolution are preconditions. An open leaf Close blocks creation inside
that Panel; aggregate Close blocks its Tab. Pending creation fences relocation of the actual
terminal-owning Panel, including a retained Panel receiving a child.

Site freezes cwd/environment/program/provenance, prepares canonical and terminal plans, and checks
all associations before either commit. It then moves both owners and dispatches; the first bound
standalone member installs the Creating reference. Existing prebuilt result reservation and opaque
settlement seals apply unchanged. New pending/final outcomes carry Context/Tab/Panel/Presentation,
without invented split fields. Unknown launch outcomes remain unknown, exact replay never launches
again, and known failure retains the committed member. Targeted evidence covers identity budgets,
limits/refusals, preparation drop, foreign settlement, child/primary binding, literal dispatch,
first-reference settlement, scope fences and service grants. This private creation path is the
foundation for layout application and member controls; those consumers remain separate work.

## Unbound layout creation

`tab.create-layout` creates a complete Tab structure in one canonical transaction. Its identity-free
layout contains primary and ordered child labels, optional manual labels, Panel right-click policy
and binary Split orientations. The Server allocates every Tab, Panel, Presentation, Split and slot
identity; all Presentations begin unbound. No Worker or terminal is created, reused or required.
A navigation grant admits structure creation; launching each intended member later uses the separate
`panel.create-terminal` lifecycle and its navigation plus terminal-creation grants.

The template admits at most 24 Panels, 16 levels including the root and 16 children per Panel,
within a 1 MiB command envelope. Existing context, Tab, projection, identity and retained-request
limits still apply. An open Context Close fences creation. Revision checking, publication, identity
allocation and the final `tab-layout-created` receipt commit together; exact request replay returns
the same IDs without another effect. Labels and input policy survive materialization. Client focus,
ratios, group assignment, pinning and physical launches are separate owners and are not implied by
this receipt. An all-unbound Tab participates in checkpoint, selection and zero-terminal Close.

[multi-leaf-tests]: ../src/site_coordinator/tests/multileaf_membership.rs
[move-tests]: ../src/site_coordinator/tests/tab_create/tab_move.rs
[create-tests]: ../src/site_coordinator/tests/tab_create.rs
[completion-tests]: ../src/site_coordinator/tests/tab_create/reference_completion.rs
[failed-close-tests]: ../src/site_coordinator/tests/failed_launch_close.rs
[create-service-tests]: ../src/local_terminal_service/tab_create/tests.rs
[multi-leaf-process]: ../tests/client_process/tab_create/multileaf.rs
[panel-label-tests]: ../src/workbench_authority/tests/panel_labels.rs
[authority-tests]: ../src/workbench_authority/tests.rs
[authority-stateful]: ../src/workbench_authority/tests/stateful.rs
[authority-process]: ../tests/client_process/workbench.rs
[session-tests]: ../src/local_client_service/application_session/tests/workbench.rs
[standalone-tests]: ../src/site_coordinator/tests/standalone.rs
[standalone-process]: ../tests/client_process/terminal_operations/standalone.rs
[selection-tests]: ../src/site_coordinator/tests/terminal_selection.rs
[viewer-tests]: ../src/local_terminal_service/viewer_session/tests.rs
[placement-tests]: ../src/workbench_authority/terminal_tab_create/tests/placement.rs
[rename-process]: ../tests/client_process/tab_create/tab_rename.rs
[create-process]: ../tests/client_process/tab_create.rs
[create-authority-tests]: ../src/workbench_authority/terminal_tab_create/tests.rs
