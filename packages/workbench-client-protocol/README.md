# @orchbee/workbench-client-protocol

Strict, runtime-neutral data contracts for one Orchbee Workbench page client.

## Scope

The package owns validated messages for a shared Workbench client session, terminal projection control, and the projected attachment stream opened for one visible terminal viewport. Electron, ordinary Browser pages, and future mobile clients may carry these messages through different transports without changing their Workbench-client meaning.

The package also supplies the independent TypeScript binding for the additive version-1 read-only
Workbench projection kernel. The language-neutral authority and canonical conformance corpus live
under [`schemas/workbench`](../../schemas/workbench/README.md). The binding parses one strict
`workbench.snapshot.read` intent, complete bounded snapshots, and revision-chained aggregate deltas;
`applyWorkbenchDelta` applies a delta atomically or returns one typed recovery reason. This contract
contains Project, Directory, Workspace, Tab, Panel, and Terminal Presentation product semantics,
including an exact bounded POSIX path and required nullable primary Directory reference, but
excludes Environment, Directory discovery or mutation policy, Worktree, Agent, Launch Profile,
terminal content, persistence, transport, and every Client-local view fact. No current
client-session consumer uses this new contract yet.

The separate command binding validates `workbench.command` and correlated command/snapshot results.
It also covers the revision-cursor `workbench.publication.read` intent and its
`workbench.publication.result`, whose `published` outcome carries one through 64 ordered
`workbench.snapshot`/`workbench.delta` documents under the shared snapshot-result byte bound.
It includes Project and Workspace changes, `workspace.primary-directory.accept`, canonical
`tab.rename`, `tab.move` within standalone or Workspace contexts, and
`tab.create` in standalone or configured Workspace contexts with its committed terminal result variants. Move carries exactly
`terminalContextId`, `tabId`, and `direction: previous | next | first | last`; `tab-moved` carries
exactly `revision`, `terminalContextId`, and `tabId`.
The Server owns ordering/no-op semantics; consuming adapters own request/result correlation and
action compatibility. The bindings and both runtime binaries activate and roll back together as one
local unpublished OBWP v1 cohort; old closed decoders reject Move. It reuses projection identities,
labels, revision and
snapshot invariants, rejects unknown fields, and checks encoded request/result limits. The native
Server consumes the independent Rust binding over OBWP; this package owns decoded values and does
not implement OBWP transport, duplicate-key decoding, retries, shared-state mutation or
authorization policy.

Workspace primary Directory acceptance carries only Project, Workspace, and exact path input.
Its accepted result carries revision plus the Server-selected Directory and Worker identities.
The binding keeps `workspace-directory-unavailable` and `workspace-directory-conflict` in the
rejection domain and `directory-unavailable` in the command-unavailable domain. It validates the
closed wire and projection values; filesystem observation, Worker selection, authorization,
set-once admission, commit, and replay remain Server responsibilities.

The staged Close binding adds `tab.close`, immutable `tab-close-accepted`, effect-free
`workbench.tab-close.read` and explicit `workbench.tab-close.resume` with their strict results.
`WorkbenchTabCloseAttemptGenerationSchema` validates a nonzero canonical decimal u64 domain
separate from revision. Closed status/phase/reason unions preserve known disposal, blocked cleanup,
unknown effects and final removal as different facts. Resume failure target echoes only repeat
submitted identities, including for Denied; they do not confirm an existing operation.
The [Close wire contract](../../schemas/workbench/README.md#accepted-tab-close-wire-contract)
owns shapes and the shared oracle. These decoded-value parsers do not activate a lifecycle effect
or prove Server/Worker/native integration; that cohort must move together.

Every `terminal-projection-open-requested` command carries one complete positive
`TerminalGeometry`. It is the caller's measured mounted geometry for a visual surface, not an
optional resize hint or fallback. Request identity therefore includes presentation identity and
geometry: an exact duplicate may replay, while reuse with different geometry is conflicting control
traffic. The transport contract supplies no timeout or 80x24 default.

The projected attachment stream carries semantic bell events without choosing a sound, visual
flash, or notification policy. Each event retains the Host attachment identity and terminal-owned
sequence so the projected terminal can suppress stale or duplicate delivery.

The same stream carries a complete attachment-scoped resize-lease ownership change. A renderer
validates both attachment and terminal identities before dynamically enabling or disabling Host
resize requests; the immutable attachment bootstrap is not rewritten when ownership changes.

Live input authority uses a separate `attachment-input-capability-changed` event containing the
complete Host-committed attachment capability state and positive revision. The transport cannot
request or synthesize this mutation. A client validates the attachment and terminal identities and
applies only an exact replay or exact-next changed input value; stale delivery is inert and a
conflict or revision gap fails closed. Downgrade makes PTY Actions unavailable synchronously while
history, search, selection, and copy remain local output interactions. Upgrade carries no queued
input and grants only future input stamped with the new revision.

Feature failures retain their semantic operation and retryability across both Electron and Gateway
transports. A Host response additionally retains its exact correlation ID, while client-owned copy
and link effects retain their request sequence. These values identify consequences and diagnostics;
they never authorize automatic replay of input or another user effect.

Local filesystem effects are an optional attachment capability, absent by default. When present,
`link-open` carries a discriminated URL or file target instead of granting a generic opener. URL
targets retain the shared scheme policy; file targets are meaningful only to the owning local
Desktop broker. Browser, Gateway, archived, and source-neutral attachments do not acquire local
application-launch or filesystem-read authority from terminal text.

Image preview is a separate request-bound flow. A capable renderer may request one detected local
image path and cancel the exact request sequence. Desktop replies with either an exact bounded
`ArrayBuffer` plus one admitted raster media type, or an unavailable result that reveals no file
detail. The protocol caps preview content at 8 MiB and transports no file handle, directory listing,
cache key, or persistence instruction. Request identity lets the renderer reject late content after
hover replacement or attachment retirement; it does not authorize retry or remote file transfer.

Projection release remains correlated after the attachment stream closes. The client retains the
request identity until Main returns either `terminal-projection-released` or the exact typed cleanup
failure. The success acknowledgement transfers no port; a late cleanup result therefore settles only
its released request and cannot be mistaken for an unknown message that retires unrelated projections.
Because command delivery and asynchronous open completion are independently scheduled, a correlated
`terminal-projection-opened` may reach the client after that client requested release. The client
closes exactly the transferred attachment port, keeps the released request identity until cleanup
settles, and leaves the shared control transport and unrelated requests available. A missing or extra
transferred port, conflicting identity, or duplicate open remains invalid control traffic and retires
the affected control transport.

It contains no Electron, DOM, UI, broker, coordinator, transport lifecycle, network framing, Gateway, authentication, PTY, or VT implementation. Current shared-document and legacy TerminalPresentation semantics remain with `@orchbee/workbench` until their accepted migration; the additive version-1 projection semantics come from the language-neutral schema authority. Terminal Host wire semantics remain with `@orchbee/terminal-protocol`.

Applications parse every untrusted IPC or network value with these schemas at the first trusted boundary. A transport owner remains responsible for framing, authentication, binary transfer, backpressure, and connection retirement.

Each client-session bootstrap carries `terminalHostName`, supplied by the application runtime that
owns the projected terminal machine. The shared Web client uses that trusted value to validate OSC
7 current-directory metadata; it never substitutes the Browser URL hostname, Gateway peer name, or
renderer input. Projected frames carry the complete nullable natural title and current-directory
observation. The protocol transports those values without selecting Tab-title precedence,
persistence, or URI trust policy; `@orchbee/workbench` owns those decisions.

Projected frame and row-window events accept either an exact `ArrayBuffer` or a bounded `Uint8Array` view so in-process adapters can preserve a validated payload view. The visible payload is bounded by `maxFrameBytes` for frames and `maxRowWindowBytes` for row windows; a view's unrelated backing bytes do not count toward that limit. Before structured-cloning a view into a less-trusted capability, the transport owner must copy its visible bytes into an exact-size `ArrayBuffer`; structured clone carries the complete backing buffer, including bytes outside the view. Receivers still honor the declared byte offset and length, while transport trust and terminal-protocol validation remain separate responsibilities.

Selection export remains attachment-scoped and plain-text-only. The projected command has no format
selector. A Desktop-effect attachment receives `selection-copied` after Main commits the clipboard
effect; a client-effect attachment receives `selection-copy-content` with bounded UTF-8 bytes and
commits those bytes through its own platform clipboard before reporting local completion. Neither
event grants HTML, raw VT, or a generic clipboard capability.

Host-originated clipboard-write commit is a separate, request-bound flow. Main forwards at most one pending bounded request per attachment, and the renderer automatically returns only an exact `clipboard-write-decision` containing the original attachment, sequence, and text plus the commit result. Desktop Main rejects mismatched and duplicate decisions and performs the privileged write only for a positive Desktop-effect decision. A client-effect attachment performs its local Browser write first and uses the same exact decision only to consume Main's pending request, so Main cannot duplicate the effect.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```

## Worktree operation binding

`workbench-worktree-contract.ts` implements the [retained Worktree wire contract](../../schemas/workbench/v1/workbench-worktree.schema.json). Request/response unions include Start/Read/Resume and retained partial outcomes, with their own 96 KiB encoded budget. Strict objects, UTF-8 reference bounds, attempt bounds, checkout/path equality, Git status/diagnostic constraints and reused/launch-outcome agreement match the independent Rust binding. Both consume the [shared corpus](../../schemas/workbench/v1/workbench-worktree-conformance-v1.json). The shared `workbench-terminal-launch-contract.ts` leaf owns terminal geometry and launch outcomes; the command module keeps its existing exports without a cycle. Optional Worktree shell restoration receipts retain total/attempted/started/failed/unknown counts (0–4096) and an independent nullable recovery failure. The counts describe a serial launch prefix; final receipts have no pending launch, and unknown launches forbid Resume. Exact binding/Create results keep their existing query owners. Close publication also admits nonresumable `workspace-retained` for a quiescent Worktree Close whose canonical deletion was cancelled. No Desktop runtime command is added by this binding.

## Worktree source-query binding

`workbench-worktree-query-contract.ts` adds the separate read request/result to the shared unions, with the same 32 KiB result cap, strict nullable fields, closed failures and cross-entry invariants as Rust. It consumes the [query conformance corpus](../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json) together with the [JSON Schema](../../schemas/workbench/v1/workbench-worktree-query.schema.json). This binding adds no Desktop command or mutation authority.

## Workspace group Close binding

`workbench-workspace-group-close-contract.ts` implements the [group contract](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json) and extends request/result unions. It reuses the existing child Close status/generation schemas. The binding enforces 16 KiB requests, 512 KiB results, 128 unique captured Workspaces, 1–64 attempts, source-last and closed-prefix ordering, unique identity observations and truthful aggregate closure/recovery. Required-nullable child observations distinguish unsubmitted from accepted-but-unobserved work. The [shared corpus](../../schemas/workbench/v1/workbench-workspace-group-close-conformance-v1.json) exercises the independent Rust binding and structural schema. Runtime admission and monotonic publication remain Server-owned; this binding adds no Desktop CLI verb.

## Client view binding

`workbench-client-viewport-contract.ts` validates ordered batches of 1–16 unique Panel
viewport observations, decimal uint64 positions, source identity/geometry and the offset
equations. Unknown, pending and live/history states remain distinct; primary binding,
request membership and Ready/Busy view constraints are checked before accepting a receipt.
Snapshots are bounded to 12 KiB within the existing Client control frames. Shared corpus
cases exercise both bindings and JSON Schema without introducing a Desktop runtime.

`workbench-client-view-contract.ts` independently binds the [Client view contract](../../schemas/workbench/README.md#ephemeral-client-view-control) with strict Zod shapes and semantic checks. The [shared corpus](../../schemas/workbench/v1/workbench-client-view-conformance-v1.json) is also consumed by Rust and JSON Schema witnesses. It does not place Client-local selection in canonical Workbench snapshots or expose a Desktop CLI command.

`workbench-client-layout-contract.ts` validates bounded cell rectangles, nullable visibility/focus, unique split/slot/Panel identities, finite ratios, physical containment and independently recomputed directional neighbors. Client view validation ties a layout read to the exact target and minimum revision; selected observations also match the published Client view. The shared corpus covers valid inactive/hidden targets and rejected cross-field, bounds and reference violations.

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

Client attention snapshots reuse `@orchbee/agent` session identity validation. Their strict shapes and semantics preserve nullable focus/missing state, ordered unique batches, bounded revisions and exact intent correspondence. These wire DTOs describe Client observations; they do not move acknowledgement into canonical Workbench or Agent state.

Physical `terminal-agent-attention` reads instead name 1–100 unique full presentation/creation/Host
bindings; `terminal-attention-read` preserves ordered nullable sequence/seen entries, report revision
and nullable outer focus. They use the same kind-29/64-KiB submission/completion envelope and read
lifecycle. Exact binding/order checks apply to intent/result correlation; no canonical Session is
synthesized. [Physical acknowledgement ownership](../../apps/orchbee/docs/native-client-attention.md#physical-terminal-observation-and-paint)
defines the Server/Client join and paint gates.

## Workspace metadata contract

`workbench-workspace-metadata-contract.ts` adds strict report/read envelopes to the shared request/result unions, with 16 KiB requests and 128 KiB results. The [shared corpus](../../schemas/workbench/v1/workbench-workspace-metadata-conformance-v1.json) and [JSON Schema](../../schemas/workbench/v1/workbench-workspace-metadata.schema.json) cover token normalization, source/sequence/TTL bounds, read batches and closed failures. Rust additionally rejects duplicate raw JSON keys; the TypeScript object binding preserves legal `__proto__` and `constructor` token keys. This binding exposes neither Desktop commands nor an event subscription. Runtime ownership belongs to the [Workspace metadata model](../../apps/orchbee/docs/windows-workspace-metadata.md).

Panel metadata adds strict request/result union members, 16 KiB requests and 512 KiB results. The [shared corpus](../../schemas/workbench/v1/workbench-panel-metadata-conformance-v1.json) and [schema](../../schemas/workbench/v1/workbench-panel-metadata.schema.json) cover canonical reports and bounded source records. Bindings additionally check unique rows/sources and per-field write-order bounds/ownership. Runtime semantics belong to the [Panel metadata contract](../../apps/orchbee/docs/windows-panel-metadata.md).

Panel Agent reports add `workbench.panel-agent` / `workbench.panel-agent.result` with 16 KiB
requests and 512 KiB results. The [shared corpus](../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json)
and [schema](../../schemas/workbench/v1/workbench-panel-agent.schema.json) cover four explicit
report kinds, four report states, full-u64 decimal sequences, ordered reads and closed receipts.
Rust/TypeScript additionally enforce UTF-8 reference bounds, source/reference eligibility and view
consistency. The [Server report model](../../apps/orchbee-server/docs/agent-report-model.md) owns
physical generation, sequencing, suppression and atomic metadata exit cleanup. This decoded binding
adds no Desktop command or process producer and cannot submit process-existence claims.

Physical `observe` reads use the same Panel Agent carrier and read permission. They require an exact
presentation/Host binding and a required nullable `afterSequence`; `observed` returns current view,
terminal-local sequence and at most 128 contiguous semantic changes. Rust/TypeScript validate the
last change against the current head/state; Rust correlation additionally proves request cursor
coverage. This is transient report state, separate from canonical Agent revisions and Client `done`.

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
TypeScript and JSON contracts cover installed snapshots, optional maintenance metadata, retained
Reload/Update operations and separate cache-report/activation facts. Current/status queries are
read-only. Mutations use explicit Site-lifetime operation IDs; repeated same-kind IDs read the
retained receipt and cross-kind reuse refuses. Update reports retain per-provider attempted/cached
versions and failures, catalog/time and persistence facts; unknown effects never claim success.
The shared `workbench-agent-manifests-conformance-v1.json` corpus covers accepted/refused shapes,
nullable fields, numeric versions, sources, revisions, duplicates and cross-field consistency.
Semantic byte/version and cross-field checks supplement JSON Schema. The Server screen model owns
source selection, downloading, installation, cancellation and receipt limits.

Optional `maintenance.automatic` exposes the configured check and provenance, background/release/availability gates, effective enabled state and one latest automatic operation. Required nullable `lastOperation` retains pending, updated, failed or unknown facts; reload-only Applied is refused. Rust/TypeScript validate gate consistency and default provenance. Automatic work never consumes manual receipt IDs; scheduling belongs to the Server.
