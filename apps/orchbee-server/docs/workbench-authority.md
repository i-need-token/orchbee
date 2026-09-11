# Native Workbench authority

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
These focused checks do not replace the root gate or the owning cross-process evidence. The existing
finite Close models remain bounded abstractions of those runtime claims.

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
The authority unit test, the standalone and Workspace Linux process witnesses
`real_workbench_panel_split_and_aggregate_close_releases_both_terminals` and
`real_workspace_panel_split_and_aggregate_close_releases_both_terminals`, and the bounded
`WorkbenchPanelSplit` TLA+ model cover this refinement. The native TUI consumes Client-local panel
layout and focus through strict projection and local focus; it admits a peer leaf close as an exact
sibling-substituted contraction of its baseline tree and retires only when its own attached leaf was
removed. The TUI still does not submit
`panel.split` or `panel.close` from a shortcut.

## Accepted canonical Tab move model

### Workspace context extension

The local Move owner also accepts a Terminal Context referenced by a canonical Workspace.
It applies the same four directions, exact request fingerprint, shared revision, no-op and retained
result rules below. Context admission accepts the standalone context or an existing Workspace's
context; an unknown context is `TerminalContextUnavailable`. A Tab belonging to another context
is `TabUnavailable`. Move does not require a primary Directory, filesystem probe or live Worker.

Both context kinds require the existing simple layout: no groups, pinned/grouped Tabs, splits or
child presentations, and every primary presentation has a binding. Layout divergence precedes
target lookup and no-op and retires the authority without permutation. This preserves the current
local cohort's shape; it does not define pin/group movement or recovery of imported layouts.

Only standalone Move admission supplies `WorkbenchTerminalMembership` to Site's existing
reference-derived composition check. Workspace admission neither reads nor modifies the standalone
slot or its retained authorization. Under the same Site gate, the borrowed admission locates the
target in the validated context and prepares a complete-Tab permutation. The only canonical changes
are that context's Tab order and one shared revision increment; all other contexts, navigation,
Directories, bindings, structural identity allocation, terminal operations and notifications remain
unchanged. Dropping preparation has no effect. Replay retains the original result even after a
different context advances the shared revision; changing context under the same request ID conflicts.

`WorkspaceTabMove` models two contexts with three immutable complete Tabs each, a bounded shared
request ledger and revision, and independent Empty/Creating/Settled standalone slots. Its safety
claims are whole-Tab preservation, isolation of the other context, one revision per actual move,
fixed historical replay and cross-context request conflict. Its context-local Tab index abstracts
a globally unique identity as `(owner, index)`; wrong-context target rejection remains a Rust test.
Required coverage witnesses a Workspace move in each of the three standalone slot states.
Existing `WorkbenchNavigationCommit`
retains capacity/retirement evidence; concrete layout, authorization, allocation and byte bounds
remain Rust tests. The Site test `workspace_pending_create_move_completion_preserves_origin_and_standalone_states`
establishes pending Workspace Create followed by Move and exact completion; the process test
`real_server_worker_workspace_directory_association_and_tab_lifecycle` establishes shared snapshot
order and replay after Worker loss and directory deletion. These do not claim
native Workspace UI, Workspace Close, cross-context transfer, persistence or remote delivery.

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
archive/restart, pin/group, splits, persistence, remote operations, Web migration and push remain
excluded from Move. Close has the separate local Server integration described above; its native
integration has separate Linux process evidence; release convergence remains pending.
No explicit-empty Open result is added.

The `WorkbenchTerminalMembership` returned by `standalone_membership()` is a bounded read-only vector of every strict standalone member
in canonical order, exposing exact Tab/Panel/Presentation identities and binding. Workbench validates
the entire context and constructs it; Site never caches it. Empty membership is valid exactly when
the Site slot is Empty. Nonempty membership requires Creating or Settled with reference R,
`TerminalCoordinator.retained_authorization(R)`, successful exact conversion to
`PresentationBinding`, and exactly one matching binding anywhere in the vector. Position never
identifies bootstrap or completion. Zero/multiple matches, absent/unconvertible authorization,
duplicate context or pin/group/split/child/missing-binding layout are internal divergence, never
repaired or returned as `TabUnavailable`.

Apply that relation under the Site gate after preceding admission decisions for new first/later
create and Move, where initial Open already requires composition, and before joint exact
committed-create completion. This is not an unconditional entry guard: authorization, retirement,
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
8. complete strict target-context layout;
9. reference-derived unique-binding Site composition for standalone only;
10. exact target Tab, otherwise `TabUnavailable`;
11. semantic no-op, yielding existing `Unchanged { revision }`;
12. revision exhaustion for an actual permutation;
13. prebuild/validate complete candidate projection, publication and fixed result;
14. reserve exact retained request/result bytes;
15. infallibly commit projection, revision and settled ledger entry.

Unsupported layout therefore precedes missing target/no-op; missing context keeps its typed
rejection. No-op precedes revision exhaustion. Typed rejection/no-op is retained only if the byte
policy admits it. Entry/byte exhaustion is transient unavailable with no ledger insert,
order/revision/cursor/Site/terminal change or effect. Exact replay preserves the original result
revision after later commits. The fingerprint includes Site, base revision, context, target and
direction. Move neither reads nor advances the structural identity cursor.

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

| Claim / transition                                                                                      | Owning evidence and refinement                                                             | Limits                                                                                     |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Four directions, target absence, no-op, revisions, replay/loss/conflict, both capacities and retirement | `WorkbenchNavigationCommit`; strict corpus and deterministic/stateful prepared-Move tests  | Three-member and singleton finite models; exact strings/bytes and publication remain tests |
| Pending first/later create, permutation, retirement, exact completion and both orders                   | `WorkbenchTabCreate`; Site token/composition/stateful traces                               | Internal retirement is a Site proof cut, not a supported wire trigger                      |
| Slot reference/outcome, unique derived binding, two Open joins/wait/timeout                             | `LocalStandaloneTerminal`; deterministic Open/Site tests                                   | Concrete conversion and zero/duplicate matches require tests                               |
| Parked launch permits Move/read/replay                                                                  | Real Server-process order/revision/dispatch witness                                        | Supported interfaces only; no manufactured retirement hook                                 |
| Existing Client identity and fresh canonical-first startup                                              | [Native Session contract](../../orchbee/docs/native-tab-session.md), native/process traces | No push, fallback, reattachment or universal permutation theorem                           |

Selected evidence is bounded TLA safety with applicable existing progress, stateful generation with
shrinking/replay, deterministic core/adapters and controlled process/PTY traces. It makes no
unbounded transformation/refinement claim; disposable narrowing TLC is not owning evidence.
Records/models precede runtime; tests establish concrete refinement before integration is claimed.

The Move composition variant of `WorkbenchTabCreate` executes first and later creation from empty,
up to two swaps, retirement and exact completion after retirement. It uses three request identities,
two fingerprints, two members/ledger entries, six abstract ledger bytes, one available Worker and
Started completion. The existing capacity/concurrent variants retain other outcomes, Worker
replacement and preflight branches; the rename variant remains separate. `firstSlot` abstracts the
retained bootstrap reference, and `BindingFor` the exact retained authorization conversion; no
positional anchor is stored. Concrete absent/duplicate/unconvertible authorization remains Site
test evidence. Move preserves terminal phases, dispatch counts, both token seals and fixed create
result revisions; chronological completion-order witnesses use the bootstrap reference, not order.

`LocalStandaloneTerminal` retains two Clients, two references/bindings, one bootstrap dispatch and
at most two other projection commits. Those commits may actually append the later member and swap
the two. `authorization` belongs to the Terminal owner, ordered `membership` to Workbench, and the
Site slot retains only its reference/outcome. `member` is derived from that authorization. Reads use
revision-indexed canonical-head history; Open still observes its original reference. Existing
two-Open wait/timeout fairness remains required. These bounded models are prerequisites for the
membership/Site refinement; they do not establish the later Move wire, navigation-ledger or native
transport/projector refinements.

`WorkbenchNavigationCommit` keeps its two existing navigation/rename variants and adds independent
Move order, singleton, byte-bound and revision-bound variants. Move uses three existing member IDs
or one singleton, two target IDs plus a missing target, all four directions, base revisions 0..2,
and separate wrong-Site/context fingerprints. Full command-record equality covers every fingerprint
field. The finite ledger holds two records with two abstract bytes each; byte limits are two/four
and changed-revision limits one/two. New semantic submissions serialize at the authority, and a
typed response flight may be delivered or lost. After consumption its transient payload is removed
from model state; the immutable ledger and exact lost committed request ID remain, so replay after
a later commit retains the original result revision. The original variants retain explicit two-Client
competing submissions; Move's input bases abstract independently captured stale revisions.

Move checks permutation/complete member-fact preservation, unchanged product cursor and unrelated
facts, exact commit revisions, immutable ledger, both capacities, no-op before revision exhaustion,
committed read history and retired/replay precedence. Concrete JSON/string/byte accounting,
unsupported-layout precedence, dropped preparation, publication refusal and gate/transport
composition remain deterministic/stateful and process obligations; the model has no parallel
transport-flight or unbounded permutation theorem claim.

## Scope and ownership

The native Server owns one canonical Workbench state per local Site. The first navigation
transition group creates and renames Projects and Workspaces. Creating a Workspace creates exactly
one empty Terminal Context; it does not launch a terminal, create a directory, or select a Worker.
The first standalone Open also commits shared Tab/Panel/Presentation membership and logical terminal
binding before launch, as defined below. The [Workspace directory contract](workspace-terminal-directory.md)
adds explicit, independently authorized directory association after Worker validation. Local
`tab.create` appends a simple standalone or configured Workspace Tab
with the same joint membership/terminal-intent boundary and exact pending/final replay. Pure
`tab.rename` changes only the canonical Tab label; `tab.move` changes its canonical position.
Both may run while that completion is pending. Other
Tab/Panel transitions, complete Client Workbench rendering,
subscriptions, persistence, restart recovery and legacy migration remain unfinished.

The projection store is a private implementation component of this authority. It has no public
whole-snapshot mutation entry; only this authority may advance it. Every committed publication is
retained in a bounded 64-entry journal served through `workbench.publication.read` under the same
read grant, so baseline-holding Clients fast-forward with ordered publications. Push subscriptions
and complete Client Workbench integration remain unfinished.

The language-neutral request/result authority is
[the Workbench contract](../../../schemas/workbench/README.md). Workbench uses its own OBWP carrier
on the same admitted local connection; OBCS remains the terminal-only contract. Same-user OBLC
admission precedes either protocol. The Server then selects an explicit Site-scoped Workbench grant.
Development composition grants snapshot and publication reading plus the five pure Workbench
operations;
`tab.create` also requires the explicit terminal-create grant. Directory association requires navigation
and its independent directory-association grant. For externally submitted OBWP reads and commands, the
authority checks that grant before any state read or ledger lookup. Read-only and denied policies
remain representable. An absent or wrong-Site grant returns typed denied without state, revision,
identity or ledger disclosure; a grant never comes from wire input. Clients submit semantic operations and expected revisions;
they cannot supply new product identities, replace the shared document, or publish Client deltas.

## Navigation model

| Field              | Contract                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem            | Independent admitted Clients must modify and observe one shared navigation structure without lost updates or duplicate logical effects.                                                                                                                                                                                                                                                                                             |
| Inputs             | A strict snapshot or publication read intent, or a strict command with request identity, exact Site, expected revision, and one of the five actions below. Labels preserve their exact nonempty text, bounded to 512 Unicode code points. A Tab rename fingerprint includes its context, Tab, label, and base revision.                                                                                                             |
| Data               | Active or retired authority, one validated Snapshot, a Server-owned non-reusing identity cursor, and a bounded request/result ledger. The initial revision is zero and the standalone Terminal Context is empty. The adapter obtains a nonzero Site identity from operating-system randomness. The cursor never reuses navigation aggregate identities; unrelated Terminal/operation identity domains retain their existing owners. |
| Outputs            | Read returns correlated available { snapshot }, denied {}, or unavailable { reason }. Publication read returns current { revision }, published { publications }, denied {}, or unavailable { reason }. Command returns project-created, workspace-created, renamed, unchanged, rejected, denied, or unavailable. Created identities appear only in their corresponding success variants.                                            |
| Invariants         | Only Server commits shared structure. Every Workspace references exactly one context. Rejection preserves structure, revision and allocation state. A changed candidate advances exactly one revision. An equal rename preserves revision. Tab rename changes only its target `Tab.label`; non-target labels and every other projection fact remain exact. One request identity cannot commit twice.                                |
| Decisions          | Authorization precedes retired-state and ledger inspection. Active exact replay precedes expected-revision checks. Changed content under a retained request identity is a conflict. A rejected command has no structural effect. An unexpected internal invariant failure retires the authority atomically; every later authorized read or command, including replay, is unavailable.                                               |
| Model              | A deterministic serialized transition over the complete candidate; network, clock, filesystem, randomness and process effects stay in adapters.                                                                                                                                                                                                                                                                                     |
| Dependencies       | Existing Workbench snapshot validation and bounds, strict OBWP framing, and the admitted local Session. No Worker or Host is required for these navigation operations.                                                                                                                                                                                                                                                              |
| Ownership          | The authority owns snapshot/cursor/ledger together. The service owns initialization and serialization of calls. The application-session adapter owns protocol selection and bounded response delivery after the lock is released.                                                                                                                                                                                                   |
| Verification       | Schema-authored Rust/TypeScript conformance, deterministic stateful command traces with reduction, bounded TLA concurrency/replay checking, and a real two-Client Server-process witness.                                                                                                                                                                                                                                           |
| Correctness limits | The model abstracts labels, identity encoding, OS scheduling and JSON. Process evidence is Linux; no persistence, transport retry, restored Site identity after restart, or public Client integration is claimed by this transition group.                                                                                                                                                                                          |

| Action           | Guard                                              | Committed result                                     |
| ---------------- | -------------------------------------------------- | ---------------------------------------------------- |
| project.create   | Label and projection capacity accepted             | One new Project with no Workspace                    |
| project.rename   | Exact Project exists                               | Its label changes; equal text is unchanged           |
| workspace.create | Exact Project exists and context capacity accepted | One new Workspace and its one empty Terminal Context |
| workspace.rename | Exact Workspace belongs to the named Project       | Its label changes; equal text is unchanged           |
| tab.rename       | Exact context contains the exact Tab               | Its Tab label changes; equal text is unchanged       |

No normalization, optimistic Client commit, automatic conflict retry, focus change or terminal
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
| Workspace missing or not in the named Project                                  | rejected: workspace-unavailable                                                                            |
| Terminal Context missing                                                       | rejected: terminal-context-unavailable                                                                     |
| Tab missing from the named Terminal Context                                    | rejected: tab-unavailable                                                                                  |
| Retained identity with different semantic command content                      | rejected: request-identity-conflict                                                                        |
| Project, Workspace, context count or encoded snapshot bound exceeded           | rejected: resource-limit                                                                                   |
| Navigation identity cursor or changed revision cannot advance                  | rejected: identity-exhausted or revision-exhausted                                                         |
| New request exceeds either ledger bound                                        | unavailable: request-ledger-full                                                                           |
| Service retired or unexpected internal candidate/serialization invariant fails | unavailable: service-unavailable; the service retires rather than representing a bug as a client rejection |

## Evidence and refinement

| Claim                                                                                             | Required witness                                                               | Bounds                                                                        |
| ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| One commit for competing expected revisions                                                       | WorkbenchNavigationCommit model and two-Client process trace                   | Two abstract Clients; serialized in-process authority                         |
| Reads observe one committed state; denied calls reveal none                                       | Atomic read model with revision/value history; admitted read-only policy tests | Read linearizes at cloning under the commit gate                              |
| Retirement fences subsequent reads, writes and replay                                             | Retire and Unavailable model actions; deterministic service traces             | Process-local retirement, no recovery claim                                   |
| A slow snapshot writer does not block another Client                                              | Stalled response writer and concurrent read/write witness                      | Response I/O follows cloning and lock release                                 |
| Exact replay survives response loss without a new effect                                          | Model replay/conflict transitions and reconnect process trace                  | Retained Site-process ledger only                                             |
| A Tab rename changes only its target label and replays its original revision after a later commit | Two-target/missing-target model variant and stateful traces                    | Two labels; exact strings and canonical bytes remain deterministic evidence   |
| Navigation remains valid and rejection is effect-free                                             | Stateful command traces and snapshot validator                                 | Existing snapshot collection, identity and byte limits                        |
| Contract meanings agree across languages                                                          | Shared navigation command corpus consumed by Rust and TypeScript               | Strict decoded JSON plus encoded carrier limits                               |
| Large snapshots preserve framing and direction                                                    | OBWP codec and process traces                                                  | 16 KiB requests/command results; snapshot results at most 1 MiB + 1,024 bytes |

Run from the repository root:

```sh
cargo test -p orchbee-server -p orchbee-workbench-protocol --locked
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol typecheck
node scripts/tla/run-models.mjs --models workbench-navigation-commit
pnpm verify
```

The concurrency model uses two Clients, three request identities and at most three submitted
operations. Its core variant retains the original authorization, retirement, capacity, concurrency,
read-history, response-loss, and replay coverage. The target-domain variant uses two existing
context/Tab pairs plus one cross-context missing pair and two abstract labels, making non-target
label stability non-vacuous. Ledger entries and response flights retain the original result revision,
including an exact replay after a later mutation. Context-before-Tab resolution, exact Unicode label
text, identity encoding, canonical bytes, and the separate count/byte preflight boundaries remain
deterministic implementation evidence; the model's capacity predicate abstracts both ledger bounds.
Passing an abstract model or binding alone does not complete the Server-process witness or any of
the remaining Workbench delivery work.

## Standalone terminal membership model

The first local `terminal.open` that passes all preflights in an empty standalone context commits
one Tab, one Panel and its primary Presentation before submitting the Worker launch. This internal
semantic operation does not enter the OBWP navigation command ledger. The existing standalone Open
state supplies coalescing and its no-replay rule. Later successful reads may observe different
navigation revisions; they preserve the same committed standalone membership and binding.

| Field        | Contract                                                                                                                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | A retained native terminal must belong to the shared Workbench without an untracked launch or a second membership writer.                                                                                                                                          |
| Input        | The admitted local Open, its real geometry, a fresh Server-generated Create reference, exact current Worker placement and lease.                                                                                                                                   |
| Data         | One Site gate serializes the Workbench authority, terminal-intent coordinator and SiteCoordinator-owned Empty/Creating/Settled standalone state. Client identity, Create reference, aggregate identities and logical Terminal/Worker binding are distinct domains. |
| Output       | The existing correlated Open result plus one canonical snapshot revision containing the first standalone Tab, Panel and Presentation.                                                                                                                              |
| Invariants   | No launch without committed membership and terminal intent; no membership without its exact planned terminal intent; preflight failure consumes neither allocator; later navigation preserves the binding; no automatic rollback, reset or relaunch after commit.  |
| Model        | Pure terminal preflight, validated Workbench candidate, one gated cross-component commit, unlocked dispatch, correlated terminal settlement, then response publication.                                                                                            |
| Owner        | Binary-private SiteCoordinator composes both deterministic owners under the existing service gate. WorkbenchAuthority alone writes projection; TerminalCoordinator alone owns launch/start authorization and execution phases.                                     |
| Dependencies | Existing OBCS Open, OBCN authorization, OBWP read, Workbench projection and local Worker registry. No new protocol, dependency, package or network authority.                                                                                                      |
| Verification | Core boundary and sequence tests, LocalStandaloneTerminal with separate identity domains and navigation/read interleavings, real Server/Worker-carrier traces and retained-shell PTY evidence.                                                                     |
| Limits       | One process-local standalone terminal. Further Tab/Panel mutations, complete Client Workbench integration, subscriptions, persistence and recovery remain unfinished.                                                                                              |

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

The formal model uses two Clients, two independent Create references, two opaque logical bindings
and at most two interleaved navigation commits. Reserve models the complete gated commit; Dispatch
is separate and must carry its binding. ReadWorkbench observes one committed snapshot. Abstract
capacity and Workbench retirement do not replace exact encoding, allocation and real-process tests.

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

| Claim                                                                          | Required witness                                                                                                                                                                                                                                                                 | Bounds                                                                                                          |
| ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Exact exposed tuple resolves without changing membership or terminal execution | ExactSelection, SelectionWasAuthorized, DispatchWasResolved and SelectionPreservesTerminal; `exact_exposed_binding_prepares_the_retained_host_without_changing_workbench` and `workbench_refusals_precede_runtime_interpretation_and_consume_no_identity`                        | Existing bounded canonical snapshot and retained terminal map                                                   |
| A replacement Worker cannot retarget or newly authorize a stale selection      | InstallationWasCurrent and NoRetarget; `worker_loss_before_post_ipc_current_check_prevents_attachment_publication`, `worker_loss_after_post_ipc_current_check_does_not_revoke_attachment` and `standalone_unknown_after_worker_loss_retains_membership_across_worker_generation` | Model: two Clients, one logical Worker and one generation replacement; core tests cover other Worker identities |
| A Client selects from the shared snapshot on its admitted connection           | `standalone_selects_the_snapshot_binding_and_never_replays_unknown_open`, `canonical_snapshot_binding_attaches_exact_host_without_launch_and_is_session_scoped` and fresh release PTY startup                                                                                    | One standalone Tab with one Panel and a bound primary Presentation                                              |
| Mixed OBWP/OBCS responses remain correlated and deadline bounded               | `snapshot_read_supports_partial_io_and_preserves_the_same_mixed_carrier`, `snapshot_read_retires_on_wrong_kind_or_correlation` and `standalone_snapshot_does_not_complete_or_reset_the_startup_deadline`                                                                         | Existing startup ceiling; no snapshot-read or ambiguous retry                                                   |
| Wire meanings agree with their authority                                       | `schema_and_corpus_are_well_formed_and_every_canonical_message_round_trips` and `consumes_every_invalid_and_forbidden_corpus_case_as_a_rejection`                                                                                                                                | Additive unpublished v1, existing framing and byte bounds                                                       |

`WorkbenchTerminalSelection` separates exact lookup from asynchronous Worker acceptance and
Session installation, and permits retirement, same-identity Worker loss/replacement and launch completion between those
steps. Its Worker replacement keeps the same logical Worker identity and changes generation once.
It abstracts other Worker identities, detailed rejection precedence, JSON, identity encoding,
clocks, allocation, OS scheduling, Host cleanup and Client rendering. Core tests cover the
identity and reason distinctions. Controlled service traces cover loss on both sides of the
post-IPC check; process tests do not claim to schedule that exact instruction boundary. Existing navigation, standalone and relay models continue to own those state
families; deterministic and real-process tests establish the selection refinement.

The Client read-error and structural selector rules belong to its
[standalone intent](../../orchbee/README.md#standalone-terminal-intent).
Identity domains impose no pairwise inequality on encoded bytes. Each field must independently
satisfy its nonzero identity contract; equal text in different domains is valid.
The selection model checks safety only, with no fairness or settlement property. Existing
LocalWorkerObcnCarrier and LocalClientSubmission/Backpressure bounds and runtime deadline traces
own progress and failure settlement.

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
`WorkbenchReadOrNavigationRequest` accepts only snapshot reads, publication reads and the five pure Workbench actions;
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
capacity, all three structural identities, changed revision, current Worker, retained-terminal
resources, candidate publication, and ledger bytes. Byte admission also covers retained
rejections. Known precommit missing Worker or busy resource results are transient and do not
create a ledger entry. Full semantic equality includes context, label, decoded geometry and
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
Presentation.label exactly; duplicate labels remain valid.

Pending entries count toward the existing 4,096-entry and 8 MiB canonical request/result limits.
Before commitment, the entry reserves request bytes plus the maximum result size across pending,
started, both unavailable reasons, failed and unknown. The prevalidated final-result table stays
private to WorkbenchAuthority. Settlement only selects a table member and releases unused charge;
it does not advance the revision or change membership. Exact replay clones the pending or settled
result independently for that invocation. The original creating invocation waits for final
settlement; only replay may return pending. Later settlement or conflict cannot rewrite a
previously cloned response awaiting delivery.

The bounded composition variant models at most one external rename after its own authorization,
replay, count, byte, target, and publication checks have succeeded. It does not duplicate that pure
operation's ledger. A committed create retains its membership revision in pending and final replay;
the Tab label may then change while the Presentation label remains the creation anchor. Completion
settles only the original create ledger entry and cannot advance revision or rewrite either label.
The original capacity and concurrency variants set `MaxRenames = 0`; the composition variant sets
`MaxRenames = 1` and requires pending-rename-final ordering plus pending and final replay witnesses.

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

Worker IPC and Client result writes occur after releasing registry and Site gates. Another
admitted Client can complete a snapshot read or pure navigation command while launch or response
delivery stalls. The existing Worker carrier still serializes its one in-flight and one queued
operation; no viewer/terminal completion during a parked same-Worker launch is promised.

### Creation evidence and limits

| Claim                                                       | Required evidence                                                                                                                                                                                |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| One shared commit precedes at most one pinned launch        | WorkbenchTabCreate model; Open-wins, command-wins, two-command and all-outcome core traces; parked-Launch process witness                                                                        |
| Preflight refusal leaves owners and allocation unchanged    | Core cases for every precedence boundary, retained record/reference/identity/lease capacity, candidate publication and ledger charge; plan-drop and generation-fence cases                       |
| Exact replay preserves effect count and immutable responses | Pending/final replay, semantic conflicts, pending response delivery, lost response and re-delivery, retirement; bounded independent response flights in the model                                |
| Rename during pending creation survives final settlement    | Read/create/rename/read/settle/replay process trace plus the bounded rename-composition variant                                                                                                  |
| Completion cannot mutate a foreign or partial composition   | Equal-state distinct-owner token tests, stale/mixed association cases, compiler-enforced borrowed plans and private Site wrapper; both preparation-failure boundaries                            |
| Pending reservation always covers settlement                | Exact canonical byte/max-final and ledger-count boundaries; generated begin/commit/replay/dispatch/settle/retire/capacity traces with deterministic shrinking and replay                         |
| First-slot correspondence and waiter settlement             | Empty/nonempty mismatch tests, both Open/create orders, all final outcomes, valid settlement after retirement, and bounded silent-Worker timeout through joint settlement                        |
| Other Clients progress without a second launch              | Separate OBWP snapshot/replay/navigation streams during parked launch; a preattached OBCS viewer Poll as a positive subsequent Worker FIFO witness                                               |
| Concrete integration agrees with the abstraction            | Strict Rust/TypeScript corpus, owner/core tests, controlled Server process traces, native two-Tab PTY and second-Client refresh, independent implementation review and selected repository gates |

WorkbenchTabCreate is safety-only. All three registered variants use three request identities, two
semantic fingerprints, at most two Tabs and two ledger entries, a pending charge of three and final
charge of two abstract bytes, and one independent replay flight. The capacity variant admits four
abstract ledger bytes; the concurrent variant admits six so two commands can remain Pending and
settle in either order. Both set `MaxRenames = 0`. The rename-composition variant admits four bytes
and one abstract external rename, retains each create's committed result revision, and covers
pending and final replay around completion. It fixes Worker availability, base revision zero, and a
Started completion so the original variants continue to own other preflight and outcome branches.
Required coverage combines only these variants of this same model. It abstracts
JSON bytes, concrete pointer allocation, borrow checking, lock/CV scheduling and two-owner Rust
publication. Core and process tests own those refinements. WorkbenchNavigationCommit continues to
own pure navigation; LocalStandaloneTerminal owns detailed Open join/wait/timeout. Existing
LocalWorkerObcnCarrier and LocalTerminalCoordinator deadline/settlement properties own progress
under their stated weak fairness. A waiter timing out alone does not prove a pending command
settles; the integrated silent-Worker trace must feed its bounded result through both completion
owners and notify the first slot.

Linux/WSL integration evidence covers 40 library tests, 102 binary tests, 26 Client-process tests,
four Worker-process tests and three authority integration tests, with the required Worker and
Terminal Host prerequisites built; all-target, all-feature Clippy also passes. The
integrated silent-Worker case uses the production 20-second response timeout and settles both owners
before notifying the first-Open waiter. The parked-Launch trace observes the second canonical Tab,
pending replay and a navigation commit from another Client before releasing the Worker, then checks
final replay and a positive viewer-Poll boundary against duplicate dispatch. This is controlled
process evidence, not native Client Tab interaction or crash/restart evidence. Focused checks do
not constitute a passing root verification gate.

Schema, both bindings and consuming binaries move together in one unpublished v1 activation cohort.
Old closed decoders reject the extension. Rollback restores the whole cohort; no mixed-version
rollout, durable migration or cross-restart replay is claimed.
