# Workbench Cross-Language Contract

## Authority

[`v1/workbench.schema.json`](v1/workbench.schema.json) is the language-neutral authority for the
version-1 read-only Workbench projection kernel. The TypeScript binding in
`@orchbee/workbench-client-protocol` and the Rust binding in `orchbee-workbench-protocol` are
independent implementations. Neither binding may add accepted values, omit invariants, or redefine
field meaning.

Version 1 uses decoded JSON values governed by JSON Schema 2020-12. Every object is closed, every
collection and string is bounded, identities are typed non-zero 128-bit lowercase hexadecimal
strings, and revisions are canonical unsigned-64 decimal strings. A transport must reject an
encoded intent larger than 16 KiB or an encoded snapshot or delta larger than 1 MiB before JSON
decoding. The Workbench command extension below owns OBWP framing and duplicate-key rejection.
Authentication, authorization policy and transport lifecycle remain with the application owners.

Labels contain Unicode scalar values; unpaired UTF-16 surrogates are rejected. Protocol version
is the decoded numeric value 1, including JSON spellings 1.0 and 1e0; canonical encoding emits 1.

[`v1/workbench-conformance-v1.json`](v1/workbench-conformance-v1.json) is the contract-authored
cross-language oracle. Valid and invalid documents exercise strict parsing and semantic invariants;
transition cases exercise exact delta results and typed rejections. An implementation must consume
every case without rewriting the expected result from its own parser or reducer.

## Close extension

The schemas, canonical conformance corpus and independent Rust/TypeScript bindings implement
strict Close command, status and Resume grammar. The
[Close wire contract](#accepted-tab-close-wire-contract) defines this staged foundation's boundaries.
The [Server Close authority](../../apps/orchbee-server/docs/terminal-tab-close.md) owns lifecycle
behavior and the maintained Server/Worker finite models. The native model and runtime integration
remain pending; parser and model evidence do not establish working Close effects.

## Projection

A `workbench.snapshot` is a complete authorized product projection for one Site revision. It
contains:

- ordered Projects, each with ordered Directories and ordered Workspaces;
- exactly one standalone Terminal Context and one Terminal Context for every Workspace;
- ordered Tab groups and Tabs;
- complete binary Panel/Split topology for every Tab;
- one primary and zero or more child Terminal Presentations per Panel; and
- an optional opaque Worker/Terminal binding for a Presentation.

Each Directory carries its typed identity, exact Worker identity, and an absolute local-platform UTF-8 path.
Each Workspace carries a required nullable primary Directory reference into its own Project.
The projection contains labels but no directory discovery or mutation policy, terminal content,
terminal lifecycle policy, Environment, Checkout, Worktree, Agent, Launch Profile, Archive, SSH, lease, fence, Host
epoch, attachment, or persistence facts. It also contains no Client-local active location, active
Tab, focus, disclosure, selection, maximization, split weight, viewport, geometry, search, window,
DOM, React, or pixel state. A missing Presentation binding means only that this projection exposes
no attachable Terminal identity; a Client must not infer a lifecycle reason.

Project, Directory, Workspace, and Terminal Context arrays use ascending identity order. Directory
identities are unique across the Site projection, and a non-null Workspace reference resolves in
the containing Project. Tab groups and Tabs
retain their explicit product order. Pinned Tabs form one leading ungrouped prefix, and every Tab
group's members are contiguous. Every referenced context exists exactly once and every context is
referenced exactly once. Identity uniqueness, tree depth, node quantity, membership, and reference
invariants apply to the complete snapshot in addition to JSON Schema validation.

### Workspace display order

Workspace move commands use a nonempty unique list of at most 1023 Workspace IDs for block input and reordered output. Dedicated request/result limits are 48 KiB; other commands retain their existing limits. The complete reordered output carries the committed revision. An unchanged move uses the existing `unchanged` outcome. The optional block before-target accepts omission or null; a target within the source block is rejected before mutation.

Navigation optionally carries `workspaceOrder`, bounded to 1023 Workspace identities. Absent or
empty means the existing Project-major identity traversal. A nonempty order is an exact permutation
of all Workspace identities in that navigation: duplicates, missing members and foreign identities
are invalid. This invariant applies to a complete navigation replacement as well as a snapshot.
The Projects, Directories and Workspace membership arrays retain their canonical identity order;
display order never changes ownership, Directory references or terminal bindings. Filtered views
preserve the corresponding subsequence. Client-local selected identities remain Client-owned.

The native ordering authority appends newly created Workspaces to an explicit order and removes
closed identities. A private checkpoint retains this field; older checkpoints without it keep
their original traversal. All participants use one compatible cohort; an older strict decoder may
reject the additive field and must never discard it as a repair.

## Intent and delta

`workbench.snapshot.read` requests the complete authorized projection. The original projection
document union still admits only this intent, snapshots and deltas. Workbench command request/result
envelopes are a separate additive contract; neither contract defines command-line grammar.

A `workbench.delta` names one exact Site, base revision, greater result revision, and an unordered
atomic set of aggregate changes. A change may replace complete navigation, upsert one complete
Terminal Context, or remove one complete Terminal Context. At most one navigation replacement and
one change for each Terminal Context identity are allowed. An empty change set may advance the
projection revision when authorized Site facts changed outside this projection.

`applyWorkbenchDelta` performs these steps without side effects:

1. reject a Site mismatch, base-revision mismatch, or non-increasing result revision;
2. reject duplicate navigation or Terminal Context targets;
3. stage every aggregate replacement and sort Terminal Contexts by identity;
4. validate the complete candidate snapshot; and
5. return the complete next snapshot only after every check succeeds.

Any rejection leaves the caller's snapshot unchanged. Recovery obtains a complete snapshot; this
slice defines no replay buffer, retry, persistence, or transport request.

## Commands and OBWP

[`v1/workbench-command.schema.json`](v1/workbench-command.schema.json) owns strict Workbench
requests and results. [`v1/workbench-command-conformance-v1.json`](v1/workbench-command-conformance-v1.json)
is their shared Rust/TypeScript oracle; raw duplicate-key cases additionally exercise the Rust wire
parser. TypeScript validates decoded values; its transport must reject duplicate keys before decoding.

`workbench.command` carries protocolVersion, requestId, siteId, expectedRevision and one action:
project.create, project.rename, workspace.create, workspace.rename,
workspace.primary-directory.accept, tab.rename, tab.create, panel.split, tab.move, tab.close,
panel.close or panel.move. Labels retain exact text,
including whitespace, within the existing label bound. New product identities are Server-owned.
`workbench.command.result` correlates project-created, workspace-created,
workspace-primary-directory-accepted, renamed, unchanged,
tab-create-pending, tab-created, panel-split-pending, panel-split-created, tab-moved,
tab-close-accepted, panel-close-accepted, panel-moved, rejected, denied or unavailable outcomes. Revision and created identities occur only in the variants
that declare them. Denied results disclose neither. `workbench.snapshot.result` contains available
with a complete nested `workbench.snapshot`, denied, or unavailable. The schema owns closed reasons;
the [Server authority](../../apps/orchbee-server/docs/workbench-authority.md) owns decision precedence,
atomic commitment, authorization and its bounded process-local ledger. No automatic replay is implied.

`workbench.publication.read` carries protocolVersion, requestId and the caller's committed
`knownRevision`. `workbench.publication.result` correlates `current` with the still-current
revision, `published` with one through 64 ordered committed publications (each a complete
`workbench.snapshot` or `workbench.delta`), `denied`, or `unavailable`. Applying the ordered
publications to a snapshot at `knownRevision` reproduces the committed projection. When a Server
journal cannot connect `knownRevision` to its oldest retained publication, it may answer
`published` with one complete snapshot as the boundary publication. Denied and unavailable
disclose no revision or content.

OBWP uses a 12-byte header: magic `OBWP`, big-endian u16 version 1, big-endian u16 kind,
and big-endian u32 payload length. Kinds 1/2/9 are snapshot.read/command/publication.read
requests; kinds 3/4/10 are snapshot.result/command.result/publication.result responses. JSON kind
must match the header. Readers reject the wrong
version, direction, kind or payload bound before body allocation. Requests and command results are
at most 16 KiB. Raw snapshot and publication results are at most 1 MiB + 1,024 bytes, and each
published document retains its standalone canonical bound. The complete nested snapshot
is additionally limited to 1 MiB in canonical JSON; whitespace and alternate escapes inside the
envelope count toward the raw outer bound. Standalone snapshots retain their raw 1 MiB bound.
A valid record leaves following records unread; EOF truncation fails.
The Rust binding supplies Read/Write codecs without owning sockets, timeouts, admission or retry.
OBCS remains the separate terminal protocol, and OBLC admission precedes either application carrier.

## Workspace primary Directory contract

`workspace.primary-directory.accept` carries exactly `projectId`, `workspaceId`, and `path`.
`path` is absolute local-platform UTF-8 text from 1 through 4,096 encoded bytes with no NUL; its spelling is
preserved. The Client supplies no Directory or Worker identity. A successful
`workspace-primary-directory-accepted` outcome carries the committed `revision`, Server-allocated
`directoryId`, and selected `workerId`.

The projection represents the atomic result by appending one ordered
`Directory { directoryId, workerId, path }` to the Project and selecting it through the Workspace's
required nullable `primaryDirectoryId`. A Project admits at most 256 Directories. Directory
identities are unique across the Site projection; a Workspace reference must resolve inside its
own Project. Missing required nullable fields, dangling or cross-Project references, duplicate or
unordered Directory identities, invalid paths, unknown fields, and collection overflow are invalid.

The rejection reason domain adds `workspace-directory-unavailable` and
`workspace-directory-conflict`; the command-unavailable domain adds `directory-unavailable`.
The [Server Directory authority](../../apps/orchbee-server/docs/workspace-terminal-directory.md)
owns authorization, Worker validation, set-once/empty-context admission, atomic commit, replay and
runtime behavior. These schema and binding contracts neither validate the filesystem nor select a
Worker.

## Canonical Tab rename contract

`tab.rename` carries exactly `terminalContextId`, `tabId`, and `label`. The label is nonempty,
bounded to 512 Unicode code points, and retained exactly, including whitespace. The Server first
resolves the named Terminal Context and then the Tab inside that context. An unavailable context is
`terminal-context-unavailable`; a missing Tab, including a Tab identity from another context, is
`tab-unavailable`.

A changed label returns `renamed` at the next revision. Equal text returns `unchanged` at the current
revision. The operation changes only `Tab.label`: `Presentation.label`, every identity, ordering,
topology, binding, terminal state, and the Server identity cursor remain exact. It allocates no
identity, launches no Worker effect, and requires only the ordinary Workbench mutation grant.

The request fingerprint contains expected revision, context, Tab, and exact label. An exact retained
replay returns its original result and result revision even after later mutations; changed content
under the same request identity is `request-identity-conflict`. A rename may commit while the target's
`tab.create` result is pending. Later create completion settles only that retained create result and
does not change the projection or either label.

### Tab rename refinement record

| Field                   | Contract                                                                                                                                                                                                                                                               |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and scope       | Independently implemented boundaries need one canonical semantic rename that cannot change terminal identity or presentation provenance. This record admits the unpublished wire meaning; the Server authority owns authorization, replay, capacity, and commit.       |
| Inputs and trust        | Untrusted bounded JSON carrying exact Site, request identity, expected revision, Terminal Context, Tab, and label. Decoded values have no authority before validation and Server admission.                                                                            |
| Data and ownership      | The schema owns closed fields and `tab-unavailable`; the Server owns the canonical projection, revision, and ledger. The target `Tab.label` is the only mutable product fact.                                                                                          |
| Outputs and decisions   | `renamed`, `unchanged`, or an existing typed denied/rejected/unavailable result. Context resolution precedes Tab resolution; no result supplies a new identity or terminal outcome.                                                                                    |
| Model and transitions   | One synchronous pure projection transition or no-op. Exact replay uses the retained fingerprint/result revision. Pending create settlement is independent and cannot rewrite the projection.                                                                           |
| Invariants              | Only the target `Tab.label` may differ after commit; `Presentation.label`, order, topology, binding, terminal state, all identities, and cursor remain exact. A changed rename advances exactly one revision; equal text advances none.                                |
| Dependencies            | Existing OBWP framing, Workbench label/identity validation, and the pure Server Workbench grant. No Worker, Host, OBCS, terminal-create grant, clock, entropy, persistence, retry, renderer, or selection authority enters this operation.                             |
| Verification and limits | Shared Rust/TypeScript corpus and codec tests establish wire agreement. Server stateful tests, bounded concurrency/replay models, projection tests, and a create/rename/completion process witness establish runtime refinement within one process and bounded ledger. |

## Standalone Tab creation contract

`tab.create` carries exactly `terminalContextId`, `label`, and `geometry`. Its context must be the
canonical standalone Terminal Context. The semantic operation appends one ungrouped, unpinned Tab,
one root Panel, and one bound primary Presentation with no children. The submitted label populates
both `Tab.label` and the primary `Presentation.label` exactly. Duplicate labels are valid; labels
never select an identity. The Server owns all new identities and the Worker/Terminal binding.
The request carries no placement, generation, create reference, cwd, executable, or structural identity.

The independent `WorkbenchTerminalGeometry` value contains `columns`, `rows`, `cellPixelWidth`,
and `cellPixelHeight`. Each is a decoded finite integer in 1..4096, including equivalent integral
decimal/exponent JSON spellings — except the cell-pixel pair, which ranges 0..4096 and must be
either both positive (pixels known) or both zero (pixels unknown); a mixed pair is invalid.
Canonical encoding emits integer tokens. Semantic validation also
requires `columns*rows <= 262144`, `columns*cellPixelWidth <= 65535`, and
`rows*cellPixelHeight <= 65535`. JSON Schema owns scalar bounds and closed shape; the bindings own
these three products, documented by the schema comment and semantic corpus cases. OBWP owns this
value independently of OBCS; a runtime adapter must check conversion parity without a protocol dependency.

Both committed outcomes carry exactly `revision`, `terminalContextId`, `tabId`, `panelId`, and
`presentationId`. `tab-create-pending` means membership and the exact logical binding are committed,
but terminal completion remains pending. `tab-created` additionally carries a closed
`terminalOutcome`:

| Kind              | Fields and meaning                                         |
| ----------------- | ---------------------------------------------------------- |
| `started`         | `geometry`, exactly the accepted requested geometry.       |
| `unavailable`     | `reason`: `terminal-runtime-unavailable` or `server-busy`. |
| `failed`          | `reason`: `terminal-create-failed`.                        |
| `outcome-unknown` | No additional fields; completion cannot be established.    |

Every committed variant retains the same structural identities and binding. An unavailable,
failed, or unknown terminal outcome does not roll membership back or authorize another launch.
An equal replay during pending completion returns the fixed pending tuple; an equal settled replay
returns the retained final result. Neither dispatches a second effect. Process-local admission,
retirement, ledger reservation and response-delivery ordering belong to the Server authority.

Top-level `denied`, `rejected`, and `unavailable` commit no new Tab for a new request identity.
`WorkbenchRejectionReason` additionally admits `terminal-context-unavailable` and `tab-unavailable`.
The reason domains are separate: `WorkbenchCommandUnavailableReason` admits `request-ledger-full`,
`no-compatible-worker`, `server-busy`, `service-unavailable`, `terminal-busy`, and `terminal-runtime-unavailable`;
`WorkbenchSnapshotUnavailableReason` admits only `service-unavailable`;
`WorkbenchTerminalUnavailableReason` and `WorkbenchTerminalFailureReason` own the nested reasons above.
The action union is named `WorkbenchAction` in both language bindings. The existing four actions
retain their exact spelling and behavior.

### Contract refinement record

| Field                   | Contract                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and scope       | Independently implemented Rust and TypeScript boundaries must agree on one shared terminal-Tab intent and distinguish a committed membership from terminal completion. This record admits contract types, schema, corpus and codecs; it does not establish a working Server command or Client interaction.                                                                                                              |
| Inputs and trust        | Untrusted bounded JSON or framed bytes; decoded identities, labels, revision and geometry have no authority until validation. Required fields have no default or inferred absence value.                                                                                                                                                                                                                                |
| Data and ownership      | The schema owns closed wire variants and field meaning. Each binding independently validates them. Runtime owners alone authorize commands, allocate identities, commit membership, dispatch Worker effects and select a Client-local Tab.                                                                                                                                                                              |
| Outputs and decisions   | A complete typed request/result or rejection. Decisions cover shape, direction, kind, identity, numeric value, geometry products, labels, encoded limits and the action-specific pending/final variants. Parsing and serialization have no business effects.                                                                                                                                                            |
| Model and transitions   | A pure parser/validator/encoder; no local business state transition. Pending and final are semantic result values. The Server state machine owns the transition between them, exact replay and bounded retention.                                                                                                                                                                                                       |
| Invariants              | Closed fields and reasons; exact text and typed identity domains; equivalent decoded numbers; unchanged framing/byte limits; committed failure retains membership; no implicit new request or effect.                                                                                                                                                                                                                   |
| Dependencies            | Existing JSON/serde and TypeScript Zod boundaries plus bounded Read/Write codecs. No OBCS dependency, process, socket, clock, permission, retry or renderer capability enters the contract.                                                                                                                                                                                                                             |
| Verification and limits | The contract-authored corpus drives independent Rust/TypeScript acceptance and rejection, including raw numeric spellings, duplicate keys at the wire boundary, schema-versus-semantic geometry failures, every result variant and exact canonical round trips. Geometry conversion parity and action/result correlation additionally require runtime adapter tests; corpus agreement alone proves no runtime behavior. |

These Tab extensions remain local and unpublished. Schema, corpus, Rust/TypeScript bindings and consuming
runtime binaries move together in one activation cohort; OBWP stays version 1. Old closed decoders
reject the additive action/results. Rollback restores the whole source/runtime cohort together;
there is no mixed-version rollout, public compatibility promise, durable migration or replay recovery.

## Accepted Tab move contract

The local unpublished Move extension is implemented in the schema, corpus, both bindings and Server.
Native standalone Client interaction is integrated with bounded Linux cohort evidence in the
[native Session contract](../../apps/orchbee/docs/native-tab-session.md).
It uses the existing OBWP v1 command envelope and navigation-mutation grant.
The action is `tab.move { terminalContextId, tabId, direction }`, where direction is exactly
`previous | next | first | last`. Previous/next swap with one adjacent Tab without wrapping;
first/last move the complete Tab to that boundary. A singleton or target already at the requested
boundary yields existing `Unchanged { revision }`. An actual permutation yields
`TabMoved { revision, terminalContextId, tabId }` (wire outcome `tab-moved`) with exactly one
revision advance. No new product/terminal identity or terminal/viewer effect is permitted.

Existing `TabUnavailable` / `tab-unavailable` identifies an absent target only after the exact
standalone or Workspace context and complete supported layout pass, with Site composition required
only for standalone. Workspace Move requires neither a Directory nor a live Worker. Unsupported schema-valid
native layout is internal divergence, not a product rejection. Full validated command equality,
including Site, expected revision, context, Tab and direction, is the replay fingerprint.
Retained exact replay returns the fixed original result/revision; same-ID changed content conflicts.
[Server precedence and commit](../../apps/orchbee-server/docs/workbench-authority.md#accepted-canonical-tab-move-model)
own authorization, retirement, capacity and publication. The strict corpus covers each direction
and malformed/extra fields in both language bindings. Consuming adapters separately validate
request/result correlation and action compatibility.
OBCS Open is unchanged. This extension activates and rolls back with both binaries and both
bindings in one unpublished cohort, with no old-action alias or mixed-version compatibility.

## Accepted Panel Move wire contract

`panel.move { terminalContextId, tabId, panelId, targetPanelId, direction }` relocates one leaf
Panel inside its Tab, where direction is exactly `above | below | left | right` — the same
`WorkbenchPanelSplitDirection` grammar `panel.split` uses. Above/left insert the source leaf before
the target leaf under a fresh vertical/horizontal Split; below/right insert it after. A move that
names the same leaf twice, or whose placement is already satisfied because the pair are the direct
children of a matching-orientation Split in the implied order, yields existing
`Unchanged { revision }`. A committed relocation yields
`PanelMoved { revision, terminalContextId, tabId, panelId, targetPanelId }` (wire outcome
`panel-moved`) with exactly one revision advance. The source leaf keeps its complete Panel node —
identity, Presentations and binding — and the fresh Split is the only new structure: one fresh
Split identity plus two fresh Slot identities, while the source's vacated parent Split collapses
into the sibling subtree. No terminal or viewer effect is permitted. A source or target leaf
absent from the named Tab is `PanelUnavailable`; a missing Tab is `TabUnavailable`; an unknown
context is `TerminalContextUnavailable`. Server precedence, replay retention, identity allocation
and publication follow the shared command rules; the strict corpus covers each direction, the
no-op and malformed/extra fields in both language bindings.

## Accepted Tab Close wire contract

The [Server Close contract](../../apps/orchbee-server/docs/terminal-tab-close.md) owns lifecycle
admission, immutable acceptance, status, explicit Resume and publication. Its strict wire projection
is staged independently of Server/Worker/native runtime activation. `tab.close` adds only
`terminalContextId` and `tabId` to the action; `panel.close` additionally carries `panelId`.
`tab-close-accepted` carries `closeRequestId`,
`terminalContextId`, `tabId`, and `acceptedRevision`; `panel-close-accepted` echoes the same fields
plus `panelId`. Aliases retain the primary operation's
original admitted revision, which is not a membership-removal revision. The leaf-scoped operation
shares the same retained Close record, `workbench.tab-close.read` status lookup and
`workbench.tab-close.resume` control; those wire shapes carry no `panelId`.

The effect-free `workbench.tab-close.read` and explicit `workbench.tab-close.resume` requests,
their results, every status and reason, and the required target echoes follow that owning record.
All objects remain closed. `attemptGeneration` and `expectedAttemptGeneration` share one
`WorkbenchTabCloseAttemptGeneration` domain, distinct from revision, with canonical decimal text
in `1..=18446744073709551615`. Increment is checked; zero, overflow and noncanonical text fail.
OutcomeUnknown phase is exactly `terminate | force | dispose`; Closing phase is separately
`queued | waiting-grace | waiting-outcome | disposing`.

OBWP preserves kinds 1/2/3/4 and adds 5 for Close read, 6 for Close read result, 7 for Resume,
and 8 for Resume result. Kinds 5..8 retain the 16 KiB encoded limit and exact direction/kind checks;
only snapshot result retains its larger bound. Version 1 and the unpublished cohort rollout remain.

Command Unavailable additionally admits `terminal-busy` and `terminal-runtime-unavailable`.
The canonical case `command-uses-terminal-unavailable-reason` is valid under this intentional
unpublished grammar expansion. Other prior oracle verdicts remain unchanged. Parsing validates
syntax; consuming Move/Create adapters must still reject action-inappropriate reasons and results.
Canonical fixtures and independent bindings establish grammar agreement, not lifecycle effects,
replay immutability, authorization, state transitions or eventual cleanup.

## Limits

| Value                     | Version-1 limit            |
| ------------------------- | -------------------------- |
| Encoded read intent       | 16 KiB                     |
| Encoded snapshot or delta | 1 MiB                      |
| Product label             | 512 Unicode code points    |
| Projects                  | 256 per snapshot           |
| Directories               | 256 per Project            |
| Directory path            | 4,096 encoded UTF-8 bytes  |
| Workspaces                | 256 per Project            |
| Terminal Contexts         | 1,024 per snapshot         |
| Tab groups                | 64 per Terminal Context    |
| Tabs                      | 256 per Terminal Context   |
| Child Presentations       | 16 per Panel               |
| Panel/Split depth         | 16 nodes from a Tab root   |
| Panel/Split nodes         | 4,096 per Terminal Context |
| Delta changes             | 1,024 per delta            |

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol test
cargo test -p orchbee-workbench-protocol --locked
pnpm verify
```

These checks prove schema/binding conformance and the pure delta refinement only. They do not prove
a Server, transport, authorization policy, persistence model, renderer, CLI command, or mobile
implementation.

## Native Windows path refinement

Absolute wire paths accept a POSIX leading slash or an ASCII drive letter followed by `:` and `/` or `\`, optionally prefixed by `\\?\` for a Windows verbatim drive path. This lexical rule is independent of the decoder platform; bytes are preserved without expansion or normalization. Backslash-rooted, drive-relative, UNC, and device paths are not admitted by this Windows increment. Existing byte bounds and NUL/Unicode validation remain unchanged. The execution owner still validates the path against its local filesystem before effects.

## Retained Worktree operations

[`workbench-worktree.schema.json`](v1/workbench-worktree.schema.json) owns the separate `workbench.worktree.operation` request and `workbench.worktree.operation.result` reply. OBWP kinds 19/20 carry these directions with independent 96 KiB pre-allocation bounds. Operation IDs remain distinct from exchange request IDs; Start retains Project and intent, Read observes, and Resume fences the prior attempt (1–64). Outcomes preserve Git facts and Workspace disposition independently, including closed-before-remove failures. The [shared corpus](v1/workbench-worktree-conformance-v1.json) is consumed by both Rust and TypeScript, with structural JSON Schema checks and explicit semantic byte/cross-field checks. These wire contracts do not themselves grant mutation authority or imply Desktop support. Runtime and CLI lifecycle belong to the [Windows Worktree owner](../../apps/orchbee/docs/windows-worktrees.md).

## Source-based Worktree queries

[`workbench-worktree-query.schema.json`](v1/workbench-worktree-query.schema.json) owns `workbench.worktree.query` and its result, OBWP kinds 21/22. Requests retain the 16 KiB intent cap; responses have a separate 32 KiB pre-allocation cap. Workspace or literal directory sources need no mutation operation. Results include Git checkout facts and zero or more canonical Workspace associations. Required nullable source/exit-status fields, strict closed failure variants, UTF-8 path/reference bounds, unique checkout/Workspace identities and source/root/linked relationships are enforced by Rust and TypeScript. Both consume the [shared query corpus](v1/workbench-worktree-query-conformance-v1.json); JSON Schema covers structural constraints while bindings also enforce byte bounds and cross-entry relationships. Runtime behavior belongs to the [Windows Worktree owner](../../apps/orchbee/docs/windows-worktrees.md).

## Workspace group Close wire contract

[`workbench-workspace-group-close.schema.json`](v1/workbench-workspace-group-close.schema.json) owns `workbench.workspace-group-close` and its result, OBWP kinds 23/24. Requests retain the 16 KiB intent cap; results have an independent 512 KiB pre-allocation cap. Start captures a scope, Read uses the operation ID even after source removal, and Resume fences attempts 1–64. Available replies carry 1–128 unique ordered members with the selected source last. A nullable child record means unsubmitted; a child record preserves its Close request ID even when its required-nullable observation is unavailable. Existing Close owns status and generation definitions. Bindings enforce a closed predecessor prefix, unique child IDs, complete Closed claims and no recovery from unknown observations. The [shared corpus](v1/workbench-workspace-group-close-conformance-v1.json) covers Rust, TypeScript and structural schema validation. [Runtime ownership](../../apps/orchbee/docs/windows-worktrees.md#workspace-group-closure) supplies authorization, physical cleanup and monotonic retained observations.

## Ephemeral Client view control

`presentation` passively reads one Panel's exact Client-local member. Its `presentation-read`
receipt includes canonical revision, Panel ID and a required nullable selection with Presentation
ID, binding, local attachment generation/incarnation and installed viewport. Payloads are bounded
to 4 KiB. Bindings require the exact requested Panel and a Ready/Busy view at or after the minimum
revision; Applied cannot acknowledge the read. No Client selection fields or canonical data change.

`viewports` reads 1–16 ordered unique Panel IDs; `viewport-read` returns the canonical
revision and nullable primary bindings with unavailable, pending or live/history facts.
Position counters are decimal uint64, and offsets derive from total/start rows and the
actual Client viewport height. Sources preserve Host epoch/attachment/generation/revision
and geometry. The independent bindings enforce metric equations, exact requested membership,
nonzero source identity and Ready/Busy success; pending states cannot fabricate a position.
Payloads are bounded to 12 KiB within the existing frame limits, including owner completion.
The Windows owner reserves memory, permits read-only history observation while Busy and
preserves mutation gates. Queries never acquire viewers or request history/acknowledgement.

[`workbench-client-view.schema.json`](v1/workbench-client-view.schema.json) owns Client discovery, owner-only synchronization and acknowledged navigation independently of canonical Workbench snapshots. OBWP kinds 25/26 bound requests to 16 KiB and results to 64 KiB. Required nullable fields, nonzero u64 sequences, at most 32 unique Clients, synchronized command fences and observed Applied or single-Panel zoom-unchanged results are checked by independent Rust/TypeScript bindings and the [shared corpus](v1/workbench-client-view-conformance-v1.json). The [Windows runtime contract](../../apps/orchbee/docs/windows-client-control.md) owns freshness, receipt retention, routing, post-draw acknowledgement and shutdown. `ClientViewControl` is the registered bounded lifecycle model; model execution is separate evidence.

## Client layout observations

The Client view contract additionally owns `layout` intents and `layout-read` completions/status. OBWP kinds 27/28 use a dedicated 1 MiB pre-allocation ceiling; kinds 25/26 keep 16 KiB/64 KiB limits. Payloads preserve required nullable focus and geometry, canonical Panel/split identities, finite ratios, source availability and four recomputed neighbors. Rust and TypeScript independently enforce bounds, references, actual cell containment and exact request/view correspondence against the extended shared corpus. The [Windows layout owner](../../apps/orchbee/docs/windows-client-layout.md) defines geometry, memory reservations and read-only behavior.

Directional control adds `focus-direction` intent and `direction-focused` / `direction-unchanged` receipts on ordinary frames 25/26. Focus success validates an exact destination distinct from the source and its observed view. No-neighbor validates the source layout revision while allowing an unrelated current context. The shared corpus covers closed shapes, revisions, target/intent mismatches and invalid directions.

Split adjustment adds strict `resize` and `set-split-ratio` intents and `layout-adjusted` receipts on frames 25/26. Amounts are finite 0–0.5, ratios finite 0.1–0.9, and binary split paths contain at most 16 indices. The bindings independently validate target/revision/view relationships, distinct ordered slots, the exact setter value or clamped signed resize delta, and the resize-only no-split result. Ordinary Applied cannot acknowledge either intent. The shared corpus contains 64 valid and 115 invalid documents; structural JSON Schema checks supplement independent semantic validators. Runtime lifetime, draw ordering and inactive storage belong to the Windows layout owner.

## Panel custom labels

The [Panel label contract](../../apps/orchbee/docs/native-panel-labels.md) adds optional canonical `Presentation.customLabel` and `panel.rename` with exact context/Tab/Panel IDs. Stored overrides are nonempty, trimmed, at most 512 Unicode code points, and never null; omission preserves legacy snapshots. Update `label` is optional/nullable, bounded before Unicode White_Space trimming, and normalizes blank input to null. Rust and TypeScript independently implement that normalization (NEL is whitespace; BOM is not). The creation `label` and all identities/bindings stay unchanged. This is one coordinated experimental OBWP v1 cohort; strict older readers may reject the new field/action. Shared command conformance includes canonical set/clear and normalized forms.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf exchange, preserving Split/slot and terminal identities. The Windows CLI separately acknowledges source focus through the original Client; known canonical commits survive later failure or unknown focus. [Native Panel swap](../../apps/orchbee/docs/native-panel-swap.md) owns the workflow, receipt and evidence.

`set-split-ratio` accepts optional `expectedSplit:{splitId,firstSlotId,secondSlotId}`. When present all three are required, ordered slots must differ, and explicit null is invalid. The Client compares exact identity before writing; `layout-adjusted` receipts must match all three. Omission retains ordinary path-setter behavior. Shared corpus cases cover replaced identities, missing fields and duplicate slots. This additive field requires a matched cohort and adds no Desktop CLI command.

The [Panel input contract](../../apps/orchbee/docs/native-panel-input.md) adds Panel-owned optional `rightClick` (`orchbee` by default, or `panel`), required policy on `panel.input.set`, optional policy on `panel.split`, and `panel-input-configured` receipt. Null and unknown values are invalid. Pure updates preserve membership; split applies policy only to the new Panel. Rust, TypeScript and JSON Schema consume the same conformance corpus. This adds no Desktop CLI command and requires a coordinated development cohort.

## Tab insertion boundary contract

`tab.move-index { terminalContextId, tabId, insertIndex }` extends shared canonical Tab ordering;
the original direction `tab.move` contract is unchanged. `insertIndex` is an integer in 0..256 and
denotes a zero-based boundary in the original order. After exact context/target and tree membership
composition, the Server rejects a value above its current Tab count as
`tab-move-index-out-of-bounds`. Source/source+1 boundaries are unchanged; the count boundary appends.
Both direction and indexed movement preserve pin/group metadata. Exact placement that would break
the pinned prefix or split a contiguous group returns retained `tab-move-order-conflict` without
mutation, retirement or implicit regrouping; valid placements retain complete Tab records.
It reuses `tab-moved` and `unchanged`, the shared revision, retained results and atomic commit owner,
without terminal or focus effects. The request fingerprint includes the action kind and boundary.
The corpus covers bounds, decimal integer normalization, missing/extra/conflicting fields and
negative/fraction/string/boolean/null refusal. Matching experimental cohorts activate and roll back
together; old strict decoders cannot mix, and no durable migration is added. Executable evidence
belongs to the Server/native CLI tests; the TLA asset is not executed in this parity work.

## Automatic Tab naming

The [Tab label contract](../../apps/orchbee/docs/native-tab-labels.md) owns optional `automaticLabel` metadata. New Tabs without a supplied label use their current context position; explicit numeric names and legacy Tabs remain fixed. CLI queries, label selectors and TUI display agree after move or close, and rename chooses a fixed name. Native `api snapshot` v4 exposes canonical metadata; catalog v34 contains 68 schemas. Shared protocol consumers require a matching cohort.

Client-owned Agent attention reads name 1–100 unique canonical Agent session IDs. `agent-attention` submissions and `attention-read` completions use kind 29 with a 64 KiB pre-allocation limit; responses remain kind 26. Entries preserve request order, nullable missing-session state, observation generation/sequence, semantic-change revision and seen, together with a boot-authenticatable changes cursor and nullable host focus. Workbench binds the lightweight Agent protocol DTOs without owning Agent state. The [Client attention owner](../../apps/orchbee/docs/native-client-attention.md) defines read availability, unchanged acknowledgement, bounded receipt storage and CLI cursor authentication.

Physical `terminal-agent-attention` reads instead name 1–100 unique full presentation/creation/Host
bindings; `terminal-attention-read` preserves ordered nullable sequence/seen entries, report revision
and nullable outer focus. They use the same kind-29/64-KiB submission/completion envelope and read
lifecycle. Exact binding/order checks apply to intent/result correlation; no canonical Session is
synthesized. [Physical acknowledgement ownership](../../apps/orchbee/docs/native-client-attention.md#physical-terminal-observation-and-paint)
defines the Server/Client join and paint gates.

## Retained Panel relocation

`panel.relocate` retains the exact source context/Tab/Panel tuple. Its `tab` destination accepts
optional `terminalContextId` and `targetPanelId`; absent context means the source context, and
absent Panel retains root grafting. `new-tab` accepts optional `terminalContextId` and `label`;
absent label creates an automatically numbered Tab. `new-workspace` requires `projectId` and
`label`, with optional `primaryDirectoryId`, `primaryDirectoryPath` and `tabLabel`. The directory ID and path are mutually exclusive; a path requires Worker validation and creates the Directory atomically with relocation, returned as optional `transfer.createdDirectoryId`. Optional fields are omitted; explicit
null is invalid. The directory must belong to the Project. Existing encoded destinations remain valid.

`panel-relocated` and `panel-relocated-to-new-tab` retain their existing identity fields and
optionally carry `transfer`: required `targetTerminalContextId` and `closedSourceTab`, optional
`closedSourceWorkspaceId` and `createdWorkspaceId`. It is present for a cross-context move or
source-Tab removal. New Workspace relocation uses the new-Tab result and includes its created
Workspace identity. Legacy same-context nonclosing results omit transfer. All terminal bindings
survive. The [Server relocation model](../../apps/orchbee-server/docs/workbench-authority.md#accepted-panel-relocation-model)
owns source removal, candidate bounds, publication and standalone composition; Client ratios and
focus remain outside this atomic structural result. Shared command fixtures cover the new forms,
legacy forms, omitted labels and invalid/null optional values.

## Workspace metadata

[`workbench-workspace-metadata.schema.json`](v1/workbench-workspace-metadata.schema.json) defines `workbench.workspace-metadata` and its result, OBWP kinds 31/32. Read accepts 0–8 ordered unique Workspace IDs; Report accepts 1–16 unique token patches with optional uint64 sequence and bounded TTL. Results have a separate 128 KiB pre-allocation cap; requests retain the 16 KiB intent cap. The [shared corpus](v1/workbench-workspace-metadata-conformance-v1.json) checks both language bindings. Bindings additionally enforce uint64 range, unique result identities and encoded byte limits; raw Rust decoding rejects duplicate token keys. The [owning model](../../apps/orchbee/docs/windows-workspace-metadata.md) defines permission, expiry, capacity and query consistency.

[Panel metadata schema](v1/workbench-panel-metadata.schema.json) and [conformance corpus](v1/workbench-panel-metadata-conformance-v1.json) define OBWP 33/34 reports and reads. Rust and TypeScript validate semantic row/source/order invariants beyond JSON Schema; raw Rust decoding also rejects duplicate keys.

The [Panel Agent schema](v1/workbench-panel-agent.schema.json) also covers generation-pinned
physical `observe` / `observed` reads. The shared corpus checks required nullable cursors, nonzero
Host identities and semantic change shapes; language bindings additionally enforce contiguous
sequence/head/current-state consistency. Exact cursor coverage belongs to request/result correlation.

Physical `explain` uses the same exact binding and read permission. Its `explained` result keeps
current report view separate from raw screen-rule prediction, with required nullable manifest,
winner, skip/fallback and capture fields. Rust/TypeScript enforce first-highest-priority selection,
state/flag coherence, evidence bounds and full-lifecycle skip/source consistency. Unknown providers
refuse; known providers without manifests use explicit idle fallback. The existing 512 KiB result
limit applies. This unpublished cohort changes together; canonical OBAP explanations remain separate.

Active rule provenance extends physical Explain with an optional `manifest` object: source
(`bundled`, `local-override`, `remote-cache`), required nullable actual path/cached version,
shadowing and bounded typed warnings. Missing metadata preserves the earlier shape; explicit null
is rejected. Remote source/version and warning source uniqueness are checked by both bindings.
The CLI emits v3 when metadata is present while preserving the closed v1/v2 envelopes.

Compact physical `states` reads accept 0–64 unique presentation IDs and return rows in request order,
with Workbench/report revisions and a required nullable full binding/sequence/Agent/state snapshot.
Missing membership refuses the batch; no current physical observation yields null. Empty reads fence
the report revision. Rust/TypeScript share strict shapes, unique/order checks and the conformance
corpus; Server retains current Worker and physical identity admission.

The `agent-focus` ClientView intent retains an exact context/Tab/Panel/Presentation and a canonical
Session or full physical Agent target. Bindings validate nonzero physical identity and presentation
correspondence; applied receipts require the exact Ready Client selection. It uses the existing
navigation frames and retained operation lifecycle. [Agent navigation](../../apps/orchbee/docs/native-agent-navigation.md)
owns explicit acknowledgement and rejection after identity loss.

## Installed Agent manifest operations

`workbench.agent-manifests` / `.result` use OBWP 37/38 and a 512 KiB result bound. Strict Rust,
TypeScript and JSON contracts cover installed snapshots and pending/applied/failed reload receipts.
Current and operation queries are read-only; reload uses an explicit Site-lifetime operation ID.
The shared `workbench-agent-manifests-conformance-v1.json` corpus covers accepted and refused
shapes, nullable fields, numeric versions, sources, revisions and duplicates. Semantic byte/version
and cross-field checks supplement JSON Schema. These DTOs describe actual installed state and do
not claim remote fetch/update support. The Server screen model owns installation and receipt limits.

Both Panel carriers also admit `read-presentations` and `presentations-read` for exact primary or child report/metadata observations: four unique ordered Presentation IDs, empty revision fences, required owning `panel` rows, and shared Panel IDs across different members. Rust/TypeScript enforce payload invariants and metadata field-order ownership identically to primary reads. Existing primary read/write shapes remain unchanged. The shared corpora cover exact-member bounds, strict wrappers, same-Panel members and unbound payloads; native callers additionally correlate Presentation, Panel and binding. Ship the protocol with its Server and native TUI consumers as a matched cohort.

## Panel terminal creation extension

`panel.create-terminal` names Context/Tab/Panel plus required geometry and one closed target:
`child { label }` or `unbound { presentationId }`. Optional `launch { exec, args }` and `profile`
are mutually exclusive when non-null. Optional `options` supplies an absolute `cwd` and literal
`environment` name/value tuples. The shared Rust/TypeScript/JSON corpus covers shape, identity,
program selection, byte bounds, environment restrictions and normalization. Null selectors and
empty options normalize to absent fields. Launch has 128 arguments and 30 KiB combined UTF-8 and
worst-case escaped content; profile is 512 UTF-8 bytes; environment is 32 ASCII-case-unique entries,
1024 combined bytes, and excludes managed names. This request uses the existing 96 KiB native launch
envelope. JSON Schema marks aggregate byte/uniqueness rules that bindings enforce semantically.

`panel-terminal-pending` and `panel-terminal-created` carry revision, Context/Tab/Panel/Presentation;
the final variant includes the existing terminal outcome. They reject split-only fields. The
[Server owner](../../apps/orchbee-server/docs/workbench-authority.md#terminal-creation-inside-an-existing-panel)
requires navigation and terminal creation grants and owns preparation, effects and settlement.
Existing split grammar/result fields are unchanged. Matched private cohorts roll out together;
this extension adds no persistent migration or public CLI command by itself.
