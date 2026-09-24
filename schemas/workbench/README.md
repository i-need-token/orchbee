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
