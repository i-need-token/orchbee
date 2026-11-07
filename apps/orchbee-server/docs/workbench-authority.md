# Native Workbench authority

The [restart checkpoint contract](workbench-checkpoint.md) owns retained configuration and fresh
authority reconstruction; it does not by itself enable disk persistence or resume terminal processes.

## Close authority and local integration

[Canonical terminal Tab Close](terminal-tab-close.md) owns known Host disposal and exact target cleanup before removal, reserved revision/ledger/Resume capacity, and publication rebased over current membership. Other canonical writes may proceed during cleanup. Removing the last Tab leaves its Workspace empty; closing the last Workspace leaves an empty Workbench. New execution requires explicit Workspace/Tab creation.

The Server library implements the retained authority and continuation core below. Session, exact
controller/Worker cleanup and the process-lifetime maintenance owner compose it with the local
Close/Read/Resume endpoints. Deterministic and Linux protocol-client process tests cover credits,
retained controls, current-snapshot removal, idle reservation release and eight live Client
connections. Native confirmation and membership/Empty runtime have separate Linux TUI process
evidence in the Close record; these Server checks alone do not establish them. Installed-cohort
convergence remains unfinished.

## Close authority runtime refinement

The accepted implementation cut retains each primary Close in the existing command ledger as one
`CloseRecord`. It owns the immutable command, fixed acknowledgment, complete Workspace member and
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
entry capacity, Session/base/context and the applicable strict or tree membership. Session validates
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
captures the ordered `exact_removed_members` and exposes the before/after membership to Session's
disjoint slot-only retarget preparation.
Both plans commit under the same Session gate with no further fallible work. Dropping preparation
leaves membership and credit unchanged; failure retains disposed classification and permits only
the remaining cleanup/publication. The continuation preserves outstanding receipts across expiry,
and explicit Resume preserves signal, grace and query facts.

Authority tests own concrete ledger/encoded bounds, alias/replay precedence, maximum revision and
generation, receipt replacement, preparation rollback and current-snapshot removal. Continuation
tests own clock/step/query boundaries and exact receipt settlement. Session and process tests must
establish the joint association, cleanup and actual carrier/Host cuts before Close delivery is
claimed. `cargo test -p orchbee-server --lib --locked` runs these authority/continuation tests;
`cargo clippy -p orchbee-server --lib --locked -- -D warnings` checks the library implementation.
These focused checks do not replace the root gate or the owning cross-process evidence.
No separate Close workflow TLA model is retained.

## Accepted Panel Split and aggregate Tab Close refinement

`panel.split` is the first accepted tree mutation. Workbench validates the exact context, Tab and
leaf Panel, bounds the resulting tree, and commits the source leaf, new leaf, split orientation,
two slot identities and presentation identity before dispatching the second terminal launch. The
Server and Session retain all leaf bindings as one exact tree membership; a missing or unknown leaf
outcome quarantines the pending operation and cannot publish a partial tree.

Aggregate Close captures a Tab's complete ordered bound membership, disposes and cleans each target, and publishes only after all proofs are known. Leaf-scoped `panel.close` removes that leaf and collapses its parent Split; a sole leaf closes the Tab. Aggregate scope fences overlapping split/move with transient `terminal-busy`, after normal grant/replay/revision/context checks. Unrelated Tabs proceed. Publication re-proves the captured scope against current membership. Native membership recovery attaches a surviving leaf or retains a retryable unavailable state; protocol/deadline failure follows the existing retirement rules. Provisional TUI split, close and move shortcuts use this authority.

## Accepted Panel relocation model

`panel.relocate` moves a complete Panel into another Tab, a new Tab in an existing Workspace, or a new Workspace with nullable Project membership. Workbench owns topology; Client view authority owns zoom and focus, and canonical split ratios retain their existing owner. Relocation never launches, stops or rebinds a terminal and never deletes a directory or Git tree.

The source tuple and expected revision are exact. Existing destination forms retain their
source-context default. An explicit destination context enables cross-context moves, and an
optional target Panel selects the graft leaf; absence retains the existing root-graft contract.
A new Workspace specifies nullable Project membership, an optional Directory from the Session pool and labels. A new `primaryDirectoryPath` alternatively creates a Directory in that same commit. It is mutually exclusive with an existing directory ID. The local service probes the path through the current Worker, rechecks its generation and requires directory-association authority; the proven Worker must own the retained source terminal. Missing proof, capacity or path failure cannot publish topology. A created Directory adds one allocated identity and `transfer.createdDirectoryId`. Only new structural and directory identities are allocated. A moved leaf retains its Panel, Presentation, binding,
custom label and input policy, including every ordered child Presentation. Unbound-member lifecycle
remains separately tracked.

| Guard or transition                                                               | Observable result                                                                                              |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Missing source/destination, foreign directory, stale revision or exhausted bounds | Refuse before any topology or identity allocation is published.                                                |
| Destination is the source Tab                                                     | Existing typed same-Tab refusal; the CLI workflow owns the no-op projection.                                   |
| Source has another leaf                                                           | Detach the leaf and collapse its parent Split.                                                                 |
| Source has only that leaf                                                         | Remove the source Tab, without disposing its terminal.                                                         |
| Cross-context move empties a source Workspace                                     | Remove that Workspace/context and its display-order entry; keep Project/directories and all terminal bindings. |
| Same-context move                                                                 | Retain the context; a new Tab may replace the removed last Tab at the collection bound.                        |
| Existing target Tab                                                               | Graft at the requested leaf or root with a fresh Split and two slots; enforce depth and resulting node bounds. |
| New Tab or Workspace                                                              | Append the new Tab; append a new Workspace to explicit display order, preserving ownership.                    |
| Lifecycle operation overlaps source/destination or a moved pending terminal       | Transient terminal-busy; no retained success or partial move.                                                  |

One cloned candidate is validated and encoded before ledger admission. One commit publishes the snapshot, identity high-water and exact result. Replay returns that result; conflict and publication failure cannot allocate another move. Transfer facts identify the destination Context and removed/created Tab/Workspace identities. The Session-wide binding multiset is conserved under the shared commit gate.

Public Windows integration must additionally resolve the selected Client's target leaf, enforce
zoom/no-op behavior and apply requested ratio/focus through that Client. These are separately
acknowledged phases; a later failure preserves the known topology commit. Structural tests cover
the decision table, conservation, replay and bounded exchange; shared wire fixtures cover both
language bindings. Real multi-Client Windows movement, shell survival, refresh and restart are
required for complete user acceptance. Generic semantic events remain a separate event contract.

## Accepted Workspace ordering model

Users reorder a Session-wide Workspace list without moving Projects, directories, terminal contexts,
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

The existing mutation grant, Session/revision checks, request replay/conflict and bounded ledger own
admission and publication; a lost result does not authorize replay under a fresh identity. Every
fallible permutation/projection preparation precedes the authoritative commit. Reordering leaves
Workspace/Project membership, Directory references, terminal bindings, worktree associations and
Client-local selected IDs unchanged. New Workspace creation appends to an explicit order; Close
removes the committed closed ID. Checkpoint capture/restore retains and validates ordering.

Moves use original insertion boundaries, caller-ordered noncontiguous blocks, atomic validation and preserved logical selection. Native public operations
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

Move resolves an exact Workspace-owned Terminal Context. An ungrouped Workspace has a null Project ID and the same mutation rights as a grouped Workspace. Move requires neither an accepted Directory nor a live Worker. There is no reserved context, first-terminal slot or Open reference to reconcile.

Navigation grant, retirement, retained replay/conflict, ledger capacity, Session and expected revision precede context and target lookup. Pending aggregate Close fences changed membership; an unchanged boundary move preserves revision. Candidate validation, publication and result-byte reservation complete before the one commit. Exact replay returns the original order and revision even after later writes; changed content under the same request ID conflicts. Moving Tabs preserves every Panel, Presentation, binding, naming mode and group/pinned invariant. No Worker effect is dispatched.

## Accepted multi-leaf membership model

Every Terminal Context belongs to exactly one Workspace. Membership enumerates primary and child Presentations in canonical Tab/tree order, retaining the complete Context/Tab/Panel/Presentation/Worker/Terminal tuple. An unbound primary cannot borrow a child's binding. Read-only inventory may retain unbound layout while complete-bound operations apply their own strict admission.

Split, relocation and aggregate Close preserve unrelated leaves and exact runtime identities. Close captures its scope before effects and removes it only after known disposal and exact cleanup; publication rebases over unrelated committed writes. A last-Tab removal leaves an empty Workspace. Removing the last Workspace leaves a valid empty Workbench. Neither transition creates a terminal.

Pending creation settlement stays tied to its original operation token, including after another Tab is moved or closed. Exact replay and stale/foreign settlement checks must preserve the original terminal result. The retained authority, service and process tests exercise these interleavings using real Workspace owners.

## Scope and ownership

The native Server owns one canonical Workbench per Session. Projects hold organization metadata; Workspaces and Directories belong to the Session. Navigation stores global identity-sorted registries plus exact optional display-order permutations. Each Workspace owns exactly one Context and may have a null Project or primary Directory. Zero Workspaces/Contexts is valid. Every Context must have an owner; old ownerless navigation is rejected. Counts, paths, identity uniqueness and references are validated before publication.

The projection store is a private implementation component of this authority. It has no public
whole-snapshot mutation entry; only this authority may advance it. Every committed publication is
retained in a bounded 64-entry journal served through `workbench.publication.read` under the same
read grant, so baseline-holding Clients fast-forward with ordered publications. Push subscriptions
and complete Client Workbench integration remain unfinished.

The language-neutral request/result authority is
[the Workbench contract](../../../schemas/workbench/README.md). Workbench uses its own OBWP carrier
on the same admitted local connection; OBCS remains the terminal-only contract. Same-user OBLC
admission precedes either protocol. The Server then selects an explicit Session-scoped Workbench grant.
Development composition grants snapshot and publication reading plus the pure Workbench
operations;
`tab.create` also requires the explicit terminal-create grant. Directory association requires navigation
and its independent directory-association grant. For externally submitted OBWP reads and commands, the
authority checks that grant before any state read or ledger lookup. Read-only and denied policies
remain representable. An absent or wrong-Session grant returns typed denied without state, revision,
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
| Inputs             | A strict snapshot or publication read intent, or a strict command with request identity, exact Session, expected revision, and an admitted pure action. Creation and Tab labels preserve their exact nonempty text, bounded to 512 Unicode code points; Panel overrides normalize bounded nullable updates as defined above. A Tab rename fingerprint includes its context, Tab, label, and base revision.                               |
| Data               | Active or retired authority, one validated Snapshot, a Server-owned non-reusing identity cursor, and a bounded request/result ledger. The initial revision is zero and the Workspace Terminal Context is empty. The adapter obtains a nonzero Session identity from operating-system randomness. The cursor never reuses navigation aggregate identities; unrelated Terminal/operation identity domains retain their existing owners.    |
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
| workspace.rename         | Exact Workspace exists                                                                        | Its label changes and automatic naming clears; equal fixed text is unchanged     |
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
command is evaluated against the current Session and revision. It stages the complete candidate,
identity cursor, typed result and ledger entry before committing any of them. Candidate validation,
encoding, and projection publication preflight precede ledger-byte reservation. `CommitPlan` retains
an optional `PreparedProjectionAdvance`; after reservation, commit performs only `commit_prepared`
and cursor/ledger moves, with no recoverable check after the first mutation. An invalid or
unpublishable candidate cannot consume an identity or ledger capacity. The transport writes the correlated response after releasing state ownership.
Losing that response never rolls back the committed state.

Decision precedence is authorization, retired state, retained replay/conflict, ledger count, Session,
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

This ledger and Session identity are process-local until the persistence/recovery integration is
implemented. A disconnected Client does not destroy the Session, and a replacement Client may
reconcile an exact request in the same Session process. This does not authorize blind input replay or
cross-restart request replay. A new Session identity rejects requests addressed to the former Session.

| Condition                                                                      | Result                                                                                                     |
| ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Missing or wrong-Session grant, or a mutation under a read-only grant          | denied; no current revision or state is disclosed                                                          |
| Command addresses another Session                                              | rejected: session-mismatch                                                                                 |
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

The authority also owns one ephemeral, per-Session Launch Profile catalog: private Server state that
names reusable custom-executable launch selections for `tab.create` and `panel.split`. The catalog
is memory-only and dies with the Session process; it never enters the public snapshot, so a
navigation commit cannot observe or mutate it and `deny_unknown_fields` fences old readers from
new shapes entirely.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Admitted Clients must list, resolve, create and delete named custom-executable selections and consume them at terminal creation without a second catalog writer or a persisted registry.                                                                                                                                                                                                                              |
| Inputs       | A strict `workbench.launch-profile-catalog.read` intent with request identity and an optional selector, or a strict `launch-profile.create`/`launch-profile.delete` command. A selector is opaque 1–512-byte text; a name shares the label envelope; a selection is the closed launch contract.                                                                                                                       |
| Data         | A bounded entry vector (64 entries, 256 KiB serialized aggregate), a private monotonically increasing catalog revision, and the same request/result ledger the navigation group uses. Profiles mint opaque ids `<sessionHex>:<counterHex>` from the Session identity and the existing Server identity cursor.                                                                                                         |
| Outputs      | Read returns catalog { entries }, profile { entry }, not-found, ambiguous { candidateIds }, denied, or unavailable. Commands return launch-profile-created/deleted { revision, catalogRevision, profileId }, the journaled selector resolutions launch-profile-unavailable and launch-profile-ambiguous { candidateIds }, plus the ordinary rejected/denied/unavailable set.                                          |
| Invariants   | Only Server commits catalog state. A committed create/delete advances the Workbench revision and the private catalog revision together, atomically with projection, ledger and retained-request updates. An exact profile-id match resolves before any name lookup; otherwise zero name matches is not-found, one is found, and more than one is ambiguous with the bounded id list captured at that same resolution. |
| Decisions    | Reads carry the same site-identity grant semantics as snapshot reads and never enter the command ledger. Selector failures journal like every other settled outcome, so an identical request replays the same answer. Byte-capacity and revision-exhaustion rejections precede any catalog mutation. A dead Session's ids embed that Session identity and can never alias a different Session's entries.              |
| Model        | The same deterministic serialized transition: candidate state is staged under the commit gate, and no recoverable check follows the first mutation.                                                                                                                                                                                                                                                                   |
| Ownership    | The authority owns catalog entries, the private catalog revision and minting. The service owns policy mapping (reads under the read grant, mutations under navigation) and call serialization. Terminal creation consumes a resolved `LaunchProfileUse` — never the selector text — so the Worker launch payload still carries only executable and arguments.                                                         |
| Verification | Authority stateful tests covering id-first/cardinality resolution, replay and conflict, count and serialized bounds, revision exhaustion with zero mutation, and cross-Session id non-aliasing; coordinator tests covering prepare-time freeze and provenance publication.                                                                                                                                            |

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
| Exact replay survives response loss without a new effect                                          | [Replay/conflict traces][authority-stateful] and [disconnect process trace][authority-process] | Retained Session-process ledger only                                          |
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

## Workspace terminal membership model

A new authority contains zero Workspaces and zero Terminal Contexts. `workspace.create` creates one real Workspace and its empty Context; `workspace.launch` additionally commits its first terminal and its optional `automaticLabel`, fingerprinted with the request. Server alone advances a Workspace's `terminalDirectory` and an automatic Workspace's label from its first terminal's sampled directory, as one commit without a request; the [location label contract](../../orchbee/docs/native-location-labels.md#automatic-workspace-names) owns that rule. Workspace Project membership is required-nullable. The navigation/context relation is a bijection, bounded by 1,024 Workspaces and contexts, with no identity-less compatibility owner.

Terminal creation uses the Workspace's accepted Directory and exact Worker. Missing directory configuration or unavailable runtime produces a typed refusal, never a home-directory fallback or hidden Workspace. The canonical snapshot exposes bindings for `attach-binding`; attachment grants no creation or navigation authority.

Prepared membership and terminal reservations are effect-free until their joint commit under the Session gate. Failure before commit leaves identities, revision, ledger and terminal state unchanged. No retained singleton Open state participates in creation, close, move or checkpoint restoration.

## Terminal selection from Workbench bindings

An admitted local Client selects an exact Session/Worker/logical Terminal binding from a canonical snapshot through `attach-binding`. Workbench proves current exposure before runtime lookup; Session then checks the retained Worker generation and Host identity. Selection grants no create, navigation, input or resize authority. Missing membership, unavailable runtime and changed attachment identity remain distinct typed outcomes.

| Field        | Contract                                                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A Client reading shared product state must select its retained terminal without depending on an internal Create reference or silently following a different execution identity.                                                                                     |
| Input        | An admitted Session and the exact Session/Worker/Server-terminal tuple. All three are independent opaque identity domains; none is a Host identity, generation, operation, lease or Create reference.                                                               |
| Data         | Canonical exposed bindings, retained terminal authorization/phase, current Worker placement and Session-local attachment state.                                                                                                                                     |
| Output       | Existing Attached scope, or a typed refusal/unavailability. Selection has no terminal creation, start, input, resize, controller grant or Workbench effect.                                                                                                         |
| Invariants   | Resolve only a binding exposed by the active current Session; pin the retained exact Worker generation; never substitute a terminal or Worker; refuse before allocating attachment identities when target validation fails.                                         |
| Ownership    | WorkbenchAuthority checks canonical membership; TerminalCoordinator resolves retained execution; SessionCoordinator composes those checks under the existing registry-then-Session lock order. ViewerSession owns attachment IPC and cleanup after releasing locks. |
| Model        | Resolve a typed target, prepare an attachment, dispatch through the pinned handle, and commit the Session attachment at the existing current-Worker check.                                                                                                          |
| Dependencies | Unpublished OBCS v1 values, unchanged OBCN/OBSR relay, existing OBWP snapshot read, and the native Client's existing absolute submission deadline.                                                                                                                  |
| Limits       | Same-user local target resolution and no takeover. The Selection model does not prove multiple viewer routes, remote authorization, persistence, restart adoption, selection UI or continuous Worker availability.                                                  |

The [Session routing owner](../README.md#accepted-connection-viewer-routing-modeling-record) first
reaps exact known-Disposed entries and enforces its capacity bound. With capacity available, a
duplicate resolved live target in the same Session returns `already-attached` before Worker IPC;
different targets can coexist. Within target resolution, poisoned registry or Session ownership returns
`server-busy`. Missing or retired Workbench returns `workbench-unavailable`; a wrong Session or a
tuple absent from its currently exposed Presentation bindings returns `terminal-unavailable`.
A missing current Worker or mismatched Worker identity is `terminal-runtime-unavailable`.
Next, a missing exact retained record is `terminal-unavailable`; a mismatched retained generation
is `terminal-runtime-unavailable`; finally, a phase that cannot attach is `terminal-unavailable`. These target refusals precede attachment identity allocation and Worker
IPC. Identity capacity and known unchanged queue/attachment capacity retain the existing
`server-busy` meaning. The existing known-unchanged `server-busy` viewer retry may submit the identical binding tuple with
a fresh OBCS request identity under the same logical deadline. There is no snapshot-read retry,
fallback, terminal creation, retargeting or unknown-effect replay.

Membership validation and target resolution linearize together under the Session gate while the
Worker registry is held. Selection does not require the snapshot's revision to remain current:
unrelated navigation may commit while the exact binding remains exposed. The prepared target pins
the retained Worker generation and Host identity; the Client supplies neither. Later Workbench
retirement does not cancel an already resolved attachment. Successful attachment linearizes at
the existing post-IPC current-Worker check. Worker loss after that check can race response delivery;
the next viewer/control operation still applies existing generation fencing. Attached is not a
promise that the Worker stays available.

OBCS retains the existing same-user terminal admission. OBWP reading/navigation policy still owns
only its respective requests; knowing a tuple creates no new authorization source. Production Session composition commits membership from the exact planned terminal authorization.
An exposed tuple with no matching retained record, including a terminal stored under another
Worker, is therefore unreachable through that construction. The query still returns
`terminal-unavailable` for such an inconsistent fixture; it introduces no retirement or repair.
The Workbench lookup is an internal read, not a second snapshot writer or a command-ledger entry.

Required attachment witnesses cover foreign Session and missing membership precedence, retained generation/Host identity, exact primary-versus-child selection, terminal failure/unknown outcomes, retirement and attachment races. They must prove no launch or mutation occurs during selection and that unrelated canonical members remain unchanged.

The Client's read-error and structural-selection rules belong to [Workspace startup](../../orchbee/README.md#workspace-terminal-startup). Typed identity domains and current snapshot membership remain the authority; labels or row positions never substitute for them.

## Workspace Tab creation model

`tab.create` requires navigation and terminal-create grants, the exact Workspace Context and an accepted Directory owned by the selected Worker. Canonical command preflight preserves grant, retirement, exact replay/conflict, capacity, Session and revision precedence. Placement validates the addressed Tab/group and all resulting collection/tree bounds. Geometry, launch profile/options and directory facts are frozen before mutation.

### Admission and joint commit

Workbench prepares the candidate Tab/Panel/Presentation, publication, identities and pending/final ledger reservation. The terminal coordinator separately prepares its exact runtime reservation. Session validates both plans before committing either under one gate; Worker I/O runs after releasing it. Dropping preparation has no effect. Transient precommit Worker/resource refusal does not retain success. A known launch failure retains the committed member and reports failure; an uncertain outcome remains unknown and never causes an automatic second launch.

An encoded launch exceeding the existing Capability Node frame limit is a definite precommit
`rejected {reason:launch-request-too-large}`, not retryable `unavailable {reason:server-busy}`.
The direct terminal-create protocol uses the same rejected reason. Plugin environment and captured
preferences count toward that unchanged frame budget. No terminal reservation, Workbench revision,
identity allocation or Worker dispatch occurs. Shorten the custom program's arguments or environment
before submitting again. Worktree restore reports the same named failure without making it resumable.
Worktree creation/opening preserves this failure before association. A failed removal keeps its
original Git failure and records the oversized launch in its separate shell-restoration receipt;
neither path offers Resume for that deterministic launch refusal. Clients localize the restoration
cause as well as the main failure, and the CLI's embedded command, operation and event schemas mirror
the extended reason vocabulary.
Other encoding failures retain their existing invariant-failure classification and planning is unchanged.

This extends the closed OBCS/OBWP v1 reason sets. Deploy the matching Server, native and GUI Clients
together; older strict readers can reject the new value. Rust/TypeScript mirrors, public JSON schemas
and conformance fixtures travel in this delivery. There is no frame-version, size-limit or persistent
state migration; reverting the matched cohort restores the previous reason vocabulary.

### Completion and replay

Origin-bound consuming tokens name the original pending command, retained runtime request and exact binding. Completion validates those facts before publishing a final result. A stale or foreign token cannot settle another request. Close or relocation can change current membership without rewriting a historical Create result; replay returns that original result and dispatches no Worker effect. Postcommit internal inconsistency closes the mixed carrier without fabricating a result.

### Creation evidence and limits

Maintained authority/service tests cover effect-free preparation, directory/Worker admission, placement and resource bounds, exact replay, failure/unknown settlement, and concurrent Close or Move. Process evidence uses [Workspace terminal operations][workspace-process], [parked Tab creation][create-process], [rename during creation][rename-process] and [multi-leaf interleavings][multi-leaf-process]. These require the supported Linux process cohort and Worker/Terminal Host prerequisites; Windows checks or cross-target compilation do not execute them. Test source and earlier candidate results do not establish that a changed candidate has passed its final gate.

## Automatic Tab naming

The [Tab label contract](../../orchbee/docs/native-tab-labels.md) owns optional canonical mode, compatibility and consumer refinement. Server fingerprints `tab.create` `automaticLabel` and commits it atomically to the Tab; omission is fixed, while Workspace launch requests automatic naming. Move and close preserve surviving naming metadata. Rename sets text and fixed mode together; pending creation settlement and replay never restore the previous mode. Abstract label equality includes text and mode, so clearing automatic naming is changed even when text is equal.

## Panel presentation metadata

`WorkbenchAuthority::panel_metadata` owns transient reports for each logical terminal binding
(`workerId`, `terminalId`). Legacy reports resolve a Panel's current primary binding and require an exact
`expectedBinding`; a changed, missing or unbound target fails before applying the report. Primary
and child memberships retain the resource across moves and swaps. An operation prunes bindings
absent from all committed memberships. Session disposal or restart drops this state.

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

The Session gate is the single writer and samples time after admission. This reducer has no I/O,
background expiry timer, durable storage, canonical topology mutation or notification event.
Read/report DTOs and the deterministic owner tests cover replacement, selective clearing,
per-field precedence, guards, sequence replay, TTL boundaries, atomic capacity failures,
revision exhaustion and primary/child binding lifetime. Provider state authority and TUI rendering remain separate consumers. A metadata report never creates
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

Canonical occupancy is separate from bound inventory. An unbound layout remains occupied and attachment creates no terminal. Explicit Tab Create, Split or member creation can establish bindings through Workspace directory admission. Close captures canonical addresses/topology; unbound siblings survive and entirely unbound scopes use publication-only Close. Mixed and empty-scope evidence covers registration, settlement, replay and exact publication.

Tab creation and ordering admit child Presentations. Split wraps the complete source Panel and a fresh sibling; same-Tab move/swap and cross-Tab/Context relocation transfer the complete Panel payload, including ordered children, labels, bindings and input policy. No existing Presentation is recreated or rebound. Relocation retains Workspace directory ownership and checks canonical Close fences, including unbound members, without starting another process.

Evidence compares full Panel payloads after split/move/swap/relocation, preserves primary-versus-child identity, and verifies Close fences and binding conservation. [Native groups](../../orchbee/docs/native-tab-groups.md) owns group/pin consumer coverage. The current strict wire/checkpoint shape rejects older incompatible documents; no compatibility owner or migration path is retained.

## Grouped and pinned structural operations

Workbench accepts canonical group metadata and pinned prefixes for bound structural operations.
Tab creation without placement appends one ungrouped/unpinned Tab; typed placement also admits
target-relative creation, group-tail creation, an atomic new group with Tab, and exact single-Tab duplication; several Tabs duplicate through the [retained operation](#tab-duplication-operation). Panel split/move/swap and relocation preserve
complete existing Panel payloads and surviving Tab/group metadata; removing the final Panel removes
its Tab, and moving into a new Tab creates an ungrouped/unpinned destination. Empty groups remain
independent metadata except new-group creation cleanup. Structural operations without explicit Tab
placement do not implicitly pin, unpin, group or ungroup a Tab.

For Tab Move, the existing direction or original insertion boundary first determines one exact
permutation. No-op returns unchanged before revision allocation. The candidate must retain a pinned
prefix and contiguous members for every group; otherwise `tab-move-order-conflict` is retained
without mutation or retirement. Request replay, context-Close fences, all-member
Session composition, capacity and rollback retain their existing owners. No process operation occurs.

Evidence compares complete records against independent remove/insert and contiguous-span oracles,
including legal/illegal placements, all directions, all insertion boundaries, no-op at exhausted
revision, exact refusal replay and subsequent valid work. Restored grouped/pinned Session composition
and child-preserving create/split/placement/relocation remain required integration witnesses.
Rust/TypeScript/wire schemas and native failure classification include the new reason together.
Native indexed failures use v2 while schema v1 remains closed; direction uses the dedicated v2
envelope only for this new refusal. Matched protocol/Server/Client rollout or source rollback does
not compensate already committed order. Native group/pin creation UI is covered by [native groups](../../orchbee/docs/native-tab-groups.md);
unbound lifecycle and actual Windows multi-Client TUI acceptance remain separate obligations.

## Tab duplication operation

`workbench.tab-duplicate` (OBWP 69/70) copies several Tabs of one terminal context as one retained
Server operation, so a Client sends one request and the same request, repeated after a lost reply,
observes the same operation instead of copying again. `start` names the context, 1–256 distinct
Tabs in its current order, the expected revision and the terminal geometry every copy starts at;
`read` observes the operation by its ID. The [wire schema](../../../schemas/workbench/v1/workbench-tab-duplicate.schema.json)
owns the bytes.

Admission runs under the Session gate with the navigation and terminal-creation grant; a Read needs
only the read policy. It requires the current revision, the context and every named Tab in
canonical order (`tab-order-mismatch` otherwise), and room in the context for one copy of every
member (`resource-limit`). A Close that fences the context, or a pending group Close of its
Workspace, answers `busy`, since every copy would be refused. At most 64 operations are retained:
the oldest settled one gives way, a pending one never does (`capacity`). A repeated Start with
equal context, members, expected revision and geometry observes the existing operation; any change
under the same operation ID is a `conflict`. Refusals retain nothing and the operation ID stays
free.

One owner thread, `orchbee-tab-duplicate`, runs pending operations in admission order, each for at
most 120 seconds, one member at a time. For each member it takes the registry → Session gates,
reads the source's current label (a renamed source is copied under its new name; a source or
context that is gone stops the operation with `source-unavailable`) and admits an ordinary
duplicate `tab.create` at the current revision, so no revision race can refuse it. That creation
copies the source's raw label, naming mode, pin and group, inserts the copy right after the source
and starts the default shell; the copy has no split tree, child Presentations, launch profile or
current directory of its source. The committed copy is published under the gate that committed
it, with a null terminal outcome, before its launch is delivered outside the gates; the settled
outcome is published next. A refused creation stops the operation with `copy-refused`, a copy whose
terminal is unavailable or failed with `terminal-not-started`, and an unknown launch or settlement
with `outcome-unknown`. Earlier copies stay, later members are never copied, and nothing is
replayed; there is no Cancel or Resume. A Client that disconnects ends only its own observation:
the operation continues and any Client with the read policy can read it by ID. Another
duplication waits for the owner; a concurrent Close or rename meets each copy at its own turn.
Windows managed stop stops this owner while the Worker still runs; every operation it reaches
fails `session-stopping`, and one whose time runs out fails `deadline`. Publication only moves
forward: a member keeps its copy's Tab once committed, the copy's revision never falls, a settled
terminal outcome is final, and `duplicated` requires every copy started.

## Exact Presentation metadata reads

Panel metadata `read-presentations` accepts at most four unique Presentation IDs in request order, including primary and child members and empty revision fences. Each `presentations-read` row contains `presentationId` and a nested `panel` row with owning Panel ID and the selected member’s binding, tokens and source records. Duplicate Panel IDs across different members are allowed. Unknown members reject the batch; unbound members expose empty metadata. Source uniqueness and nonzero field orders bounded by metadata revision apply identically to both read paths. Membership and binding-owned resources are resolved under the existing Session gate; reads do not mutate topology, selection or input ownership. Existing primary Panel reads and metadata writes retain their scope.

## Exact Presentation metadata writes

`report-presentation` extends the existing metadata carrier with a required Presentation ID. The
Session gate proves exact Panel ownership and a nonnull matching expected binding before applying
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

Session freezes cwd/environment/program/provenance and prepares canonical/runtime plans before joint commit and dispatch. Prebuilt bounded results and opaque settlement seals apply to primary and child creation. Receipts carry actual Context/Tab/Panel/Presentation identities. Unknown results stay unknown, replay never launches again, and known failure retains the committed member. Evidence covers allocation budgets, bounds/refusals, dropped preparation, foreign settlement, exact literal dispatch and Close fences; layout/member consumers share this owner.

Binding a member a restore held for an unavailable directory drops that hold with the new binding.
`panel.restore-terminal` is a separate Windows path that needs the same navigation and
terminal-creation grants; the [checkpoint owner](workbench-checkpoint.md#held-restore) defines it.

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

[multi-leaf-tests]: ../src/session_coordinator/tests/multileaf_membership.rs
[move-tests]: ../src/session_coordinator/tests/tab_create/tab_move.rs
[create-tests]: ../src/session_coordinator/tests/tab_create.rs
[failed-close-tests]: ../src/session_coordinator/tests/failed_launch_close.rs
[create-service-tests]: ../src/local_terminal_service/tab_create/tests.rs
[multi-leaf-process]: ../tests/client_process/tab_create/multileaf.rs
[panel-label-tests]: ../src/workbench_authority/tests/panel_labels.rs
[authority-tests]: ../src/workbench_authority/tests.rs
[authority-stateful]: ../src/workbench_authority/tests/stateful.rs
[authority-process]: ../tests/client_process/workbench.rs
[session-tests]: ../src/local_client_service/application_connection/tests/workbench.rs
[workspace-process]: ../tests/client_process/terminal_operations/workspace.rs
[selection-tests]: ../src/session_coordinator/tests/terminal_selection.rs
[viewer-tests]: ../src/local_terminal_service/viewer_connection/tests.rs
[placement-tests]: ../src/workbench_authority/terminal_tab_create/tests/placement.rs
[rename-process]: ../tests/client_process/tab_create/tab_rename.rs
[create-process]: ../tests/client_process/tab_create.rs
[create-authority-tests]: ../src/workbench_authority/terminal_tab_create/tests.rs
