# @orchbee/workbench-client-protocol

Private package entries export only symbols consumed in this repository, enforced by Knip `includeEntryExports`. Internal contracts, generated assets and independent conformance evidence retain their owners; their presence does not automatically expand the public entry. New clients add exports for admitted uses.

The Workbench bindings live under `src/workbench-protocol/`, are production code consumed by the Server Client path, and stay verified against the shared schemas and corpora by the suites beside them. Product projection and launch schemas and the setting key table remain in `src/`. The package entry exports only what a consumer imports; the rest stays reachable from the suites.

## Project and Workspace organization

The matched-cohort projection uses global Directory, Project and Workspace registries. Project is
only an optional group: Workspace has required-nullable `projectId` and `primaryDirectoryId`, with
stable Workspace/Terminal Context identity. Identity arrays stay sorted independently of display
order. Navigation validates references, unique Context ownership, 65,536 global Directories,
256 Workspaces per Project and 1,024 globally. Nonempty `projectOrder` and `workspaceOrder` are exact
permutations of their respective registries; omission/empty uses canonical ID order.

The [Rust organization contract](../../crates/workbench-protocol/README.md#project-organization-and-stable-workspace-identity)
owns the shared semantic rules for atomic `navigation.configure` membership/order changes,
`workspace.set-project`, atomic `workspace.create-project`,
`project.dissolve` and the four `workspace.launch` placements. Launch requires an absolute cwd and
uses the 96 KiB launch class; receipts require nullable Project membership. Regrouping and dissolution
preserve physical resources and display order. Stable Workspace rename/Close/primary-directory
commands and all Close identities omit Project. The exact Close read bound is 1,000,007 bytes.
Workspace events carry membership only inside the Workspace. All bindings, schemas and raw/canonical
conformance fixtures move together within unpublished OBWP v1.

Workspace create, launch and rename command labels require a non-whitespace Unicode scalar,
matching Rust and JSON Schema. Validation preserves accepted text exactly, including surrounding
spaces, and uses the shared command corpus for ASCII and Unicode whitespace. Snapshot labels
remain readable for recovery of existing blank names; other label families keep their own rules.

Strict command bindings also admit `workspace.close-if-idle {workspaceId}`,
`project.dissolve-if-idle {projectId}`, `tab.close-if-idle {terminalContextId, tabId}` and
`panel.close-if-idle {terminalContextId, tabId, panelId}`, reject caller-supplied idle assertions,
and decode the
definite no-effect `confirmation-required` rejection. Existing confirmed action shapes and
accepted receipt shapes are unchanged; conditional and confirmed kinds are different request
fingerprints. The [Server conditional owner](../../apps/orchbee-server/docs/terminal-tab-close.md#conditional-idle-admission)
defines effect admission. The binding cannot prove idle or authorize automatic retry.

`panel.restore-terminal {terminalContextId, tabId, panelId, presentationId, directory}` admits
`directory` `saved` or `home`. A snapshot `launchFailure` with reason `directory-unavailable` must
sit on an unbound Presentation with a `null` executable and a `directory`, plus an optional
`agentResume`; `terminal-create-failed` stays bound and carries neither. A snapshot
`agentNotResumed` or `agentStartFailure` must sit on a bound Presentation; the startable Agent enum
both `agentStartFailure` and the creation commands' `agentStart` name lives in the semantic contract.

## Semantic event binding

The independent `workbench.events` binding validates 28 event kinds, closed filters, physical/Client
payloads, full-width cursors, separate probe ownership, ordered bounded pages and history gaps.
It shares the Rust/JSON event corpus and 256 KiB matched-cohort carrier bound. Optional Client
`splitRevision` reports the actual ratio-owner counter and requires a selected context; old
observations retain their existing shape. This package adds contracts, without a Desktop event
consumer. The [Windows event model](../../apps/orchbee-server/docs/windows-events.md) owns runtime behavior.

Strict, runtime-neutral data contracts for one Orchbee Workbench page client.

## Scope

The package owns validated messages for the projected attachment stream opened for one visible terminal viewport. Electron, ordinary Browser pages, and future mobile clients may carry these messages through different transports without changing their Workbench-client meaning.

The package also supplies the independent TypeScript binding for the additive version-1 read-only
Workbench projection kernel. The language-neutral authority and canonical conformance corpus live
under [`schemas/workbench`](../../schemas/workbench/README.md). The binding parses one strict
`workbench.snapshot.read` intent, complete bounded snapshots, and revision-chained aggregate deltas.
A Client on a State stream receives complete snapshots, so applying a delta stays with
`crates/workbench-protocol`. This contract
contains Project, Directory, Workspace, Tab, Panel, and Terminal Presentation product semantics,
including an exact bounded local-platform path and required nullable primary Directory reference, but
excludes Environment, Directory discovery or mutation policy, Worktree, Agent, Launch Profile,
terminal content, persistence, transport, and every Client-local view fact.

The separate command binding validates `workbench.command` and correlated command/snapshot results.
It also covers the revision-cursor `workbench.publication.read` intent and its
`workbench.publication.result`, whose `published` outcome carries one through 64 ordered
`workbench.snapshot`/`workbench.delta` documents under the shared snapshot-result byte bound.
It includes Project and Workspace changes, `workspace.primary-directory.accept`, canonical
`tab.rename`, `tab.move`, and `tabs.configure` within Workspace contexts, and
`tab.create` in configured Workspace contexts with its committed terminal result variants. Move carries exactly
`terminalContextId`, `tabId`, and `direction: previous | next | first | last`; `tab-moved` carries
exactly `revision`, `terminalContextId`, and `tabId`.
`tabs.configure` validates closed group/pin operations and the nullable group identity in
`tabs-configured`; the [native group contract](../../apps/orchbee/docs/native-tab-groups.md)
owns runtime admission and receipt rules.
Selected-set operations require 1–256 unique Tab IDs; `move-tab` carries direction and an
ephemeral `adjacentGroupCollapsed` hint. The bindings do not persist Client selection or collapse.
`move-tabs-by-drag` carries the same bounded set, an integer `insertIndex` from 0 through 256,
and a required-nullable `groupId` for one atomic arrangement.
The Server owns ordering/no-op semantics; consuming adapters own request/result correlation and
action compatibility. The bindings and both runtime binaries activate and roll back together as one
local unpublished OBWP v1 cohort; old closed decoders reject Move. It reuses projection identities,
labels, revision and
snapshot invariants, rejects unknown fields, and checks encoded request/result limits. The native
Server consumes the independent Rust binding over OBWP; this package owns decoded values and does
not implement OBWP transport, duplicate-key decoding, retries, shared-state mutation or
authorization policy.

`tab.create` and `panel.split` use the same optional launch/profile/options fields, closed
nested shapes, normalization and 96 KiB command envelope as `panel.create-terminal`. All three
preserve literal arguments and environment values; supplying both a non-null launch and profile
is invalid. Default `automaticLabel:false` on Tab creation and `rightClick:orchbee` on Split
normalize to omission, matching Rust serialization. Null `options` and null environment arrays
are invalid; null cwd and empty environment are omitted. The shared corpus checks these existing
native launch capabilities across Rust, TypeScript and JSON Schema, including extended requests
above the ordinary intent limit. This does not add Desktop commands or change Server launch,
authorization, replay or rollback ownership. Workspace launch uses the same launch fields with
required cwd and explicit placement; native-only profile catalog operations remain outside this binding.

Workspace primary Directory acceptance carries only Workspace and exact path input.
Its accepted result carries revision plus the Server-selected Directory and Worker identities.
The binding keeps `workspace-directory-unavailable` and `workspace-directory-conflict` in the
rejection domain and `directory-unavailable`, `directory-missing`, `not-a-directory` and
`directory-inaccessible` in the command-unavailable domain. It validates the
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

The projected attachment stream carries semantic bell events without choosing a sound, visual
flash, or notification policy. Each event retains the Host attachment identity and terminal-owned
sequence so the projected terminal can suppress stale or duplicate delivery.

An attachment bootstrap may carry `hostDefaultColors`, the owning runtime's configured color scheme,
foreground and background. These bounded values are distinct from the active theme, so a client can
restore Host defaults after reopening a terminal without inferring them from a previous display override.
They do not replace the current palette, cursor, theme revision or terminal output colors.

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

Feature failures retain their semantic operation and retryability across application transports. A Host response additionally retains its exact correlation ID, while client-owned copy
and link effects retain their request sequence. These values identify consequences and diagnostics;
they never authorize automatic replay of input or another user effect.

Local filesystem effects are an optional attachment capability, absent by default. When present,
`link-open` carries a discriminated URL or file target instead of granting a generic opener. URL
targets retain the shared scheme policy; file targets are meaningful only to an owning local
broker, which no composition supplies today. No current attachment bootstrap sets
`localFileEffects`, the Server path's included, so none acquires local application-launch or
filesystem-read authority from terminal text.

It contains no Electron, DOM, UI, broker, coordinator, transport lifecycle, network framing, authentication, PTY, or VT implementation. The additive version-1 projection semantics come from the language-neutral schema authority. Terminal Host wire semantics belong to [`schemas/terminal`](../../schemas/terminal/README.md); `@orchbee/terminal-protocol` is the TypeScript binding.

Applications parse every untrusted IPC or network value with these schemas at the first trusted boundary. A transport owner remains responsible for framing, authentication, binary transfer, backpressure, and connection retirement.

The composing application supplies `terminalHostName` for the machine that owns the projected
terminals; Desktop reads it from its Server connection bootstrap. The shared Web client uses that trusted value to validate OSC
7 current-directory metadata; it never substitutes the Browser URL hostname, transport peer name, or
renderer input. Projected frames carry the complete nullable natural title and current-directory
observation. The protocol transports those values without selecting Tab-title precedence,
persistence, or URI trust policy; `apps/web`'s Workbench owns those decisions.

Projected frame and row-window events accept either an exact `ArrayBuffer` or a bounded `Uint8Array` view so in-process adapters can preserve a validated payload view. The visible payload is bounded by `maxFrameBytes` for frames and `maxRowWindowBytes` for row windows; a view's unrelated backing bytes do not count toward that limit. Before structured-cloning a view into a less-trusted capability, the transport owner must copy its visible bytes into an exact-size `ArrayBuffer`; structured clone carries the complete backing buffer, including bytes outside the view. Receivers still honor the declared byte offset and length, while transport trust and terminal-protocol validation remain separate responsibilities.

Selection export remains attachment-scoped and plain-text-only. The projected command has no format
selector. A transport-effect attachment receives `selection-copied` after its transport commits the
clipboard effect, which no current transport does; a client-effect attachment receives `selection-copy-content` with bounded UTF-8 bytes and
commits those bytes through its own platform clipboard before reporting local completion. Neither
event grants HTML, raw VT, or a generic clipboard capability.

Host-originated clipboard-write commit is a separate, request-bound flow. The transport forwards at most one pending bounded request per attachment, and the renderer automatically returns only an exact `clipboard-write-decision` containing the original attachment, sequence, and text plus the commit result. A transport that performs the write itself rejects mismatched and duplicate decisions and writes only for a positive transport-effect decision; no current transport does. A client-effect attachment performs its local Browser write first and uses the same exact decision only to consume the transport's pending request, so the transport cannot duplicate the effect.

## TerminalPresentation contract

`terminal-presentation-contract.ts` owns the TerminalPresentation identity and the five lifecycle
states a Server Client can know: `running`, `exited`, `failed`, `launch-failed`, and `unbound`.
`apps/web` projects each parsed Server snapshot into the stores its Workbench holds, as its
[publication reference](../../apps/web/docs/publication-and-client-view.md) describes.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```

## Worktree operation binding

`workbench-worktree-contract.ts` implements the [retained Worktree wire contract](../../schemas/workbench/v1/workbench-worktree.schema.json). Request/response unions include Start/Read/Resume and retained partial outcomes, with their own 96 KiB encoded budget. Strict objects, UTF-8 reference bounds, attempt bounds, checkout/path equality, Git status/diagnostic constraints and reused/launch-outcome agreement match the independent Rust binding. Both consume the [shared corpus](../../schemas/workbench/v1/workbench-worktree-conformance-v1.json). The shared `workbench-terminal-launch-contract.ts` leaf owns terminal geometry and launch outcomes; the command module keeps its existing exports without a cycle. Optional Worktree shell restoration receipts retain total/attempted/started/failed/unknown counts (0–4096) and an independent nullable recovery failure. The counts describe a serial launch prefix; final receipts have no pending launch, and unknown launches forbid Resume. Exact binding/Create results keep their existing query owners. Close publication also admits nonresumable `workspace-retained` for a quiescent Worktree Close whose canonical deletion was cancelled. No Desktop runtime command is added by this binding. The entry exports `WorktreeReferenceSchema`, the Branch and Base rule, so the Web New Worktree form refuses exactly the references this binding would refuse.

## Worktree source-query binding

`workbench-worktree-query-contract.ts` adds the separate read request/result to the shared unions, with the same 32 KiB result cap, strict nullable fields, closed failures and cross-entry invariants as Rust. It consumes the [query conformance corpus](../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json) together with the [JSON Schema](../../schemas/workbench/v1/workbench-worktree-query.schema.json). This binding adds no Desktop command or mutation authority.

Start and Available receipts require nullable `projectId`, retaining the originally requested destination. Only Directory sources permit a non-null Start destination; Workspace and `workspace-directory` sources and Remove require null and derive grouping at authoritative commit. These fields do not authorize automatic replay or reinterpret unknown delivery.

Legacy `workbench-worktree-list-contract.ts` binds `workbench.worktree-list.read` and its `found`
result to global Directory identity alone, without Project scope. It preserves UTF-8/raw-byte Git
entry alternatives and the independent 32 KiB result cap. The command schema and shared command
corpus cover this direction. Worktree source query associations require nullable Project membership.

## Workspace group Close binding

`workbench-workspace-group-close-contract.ts` implements the [group contract](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json) and extends request/result unions. It reuses the existing child Close status/generation schemas. The binding enforces 16 KiB requests, 1 MiB results, up to 256 unique captured Workspaces of one Project, 1–64 attempts and closed-prefix ordering, unique identity observations and truthful aggregate closure/recovery. Start names only the Project and operation; results omit source Workspace identity and members omit redundant Project identity. Membership is frozen at admission, independent of Git repository. Empty Pending/Closed groups are valid; Closed requires every child Closed. Required-nullable child observations distinguish unsubmitted from accepted-but-unobserved work. The [shared corpus](../../schemas/workbench/v1/workbench-workspace-group-close-conformance-v1.json) exercises the independent Rust binding and structural schema. Runtime admission and monotonic publication remain Server-owned; this binding adds no Desktop CLI verb.

## Tab set Close binding

`workbench-tab-set-close-contract.ts` implements the [set contract](../../schemas/workbench/v1/workbench-tab-set-close.schema.json) and extends request/result unions. It reuses the existing child Close status/generation schemas. Start names one terminal context, 1–256 distinct Tabs, the expected revision and an `if-idle` or `confirmed` condition; Read names only the operation. The binding enforces 16 KiB requests, 1 MiB results, unique members and child IDs, absent members without a child, a finished-member prefix before any later child or absence, and Closed only when every member is closed or absent. `confirmation-required` and `rejected` carry no members because nothing was retained. The [shared corpus](../../schemas/workbench/v1/workbench-tab-set-close-conformance-v1.json) exercises the independent Rust binding and structural schema. Admission, current Tab order and monotonic publication remain Server-owned.

## Tab duplication binding

`workbench-tab-duplicate-contract.ts` implements the [duplication contract](../../schemas/workbench/v1/workbench-tab-duplicate.schema.json) and extends request/result unions. It reuses the existing terminal geometry and outcome schemas. Start names one terminal context, 1–256 distinct Tabs, the expected revision and the geometry every copy starts at; Read names only the operation. The binding enforces 16 KiB requests, 256 KiB results, unique members, copy Tab IDs unique and distinct from every member, no copy after the first member whose copy has not started, and Duplicated only when every copy started. `rejected` carries no members because nothing was retained. The [shared corpus](../../schemas/workbench/v1/workbench-tab-duplicate-conformance-v1.json) exercises the independent Rust binding and structural schema. Admission, current Tab order and monotonic publication remain Server-owned.

`workbench-layout-history-contract.ts` implements the [earlier-layout contract](../../schemas/workbench/v1/workbench-layout-history.schema.json) and extends request/result unions. `list` reads at most 48 entries newest first; `reopen` names one entry by its `savedAt` Unix milliseconds. The binding enforces safe-integer save times, strictly decreasing listings, not-open counts within their totals, and a reopen that opened something and names its first Tab whenever Tabs came back. The [shared corpus](../../schemas/workbench/v1/workbench-layout-history-conformance-v1.json) exercises the independent Rust binding and structural schema. What is kept and reopened remains Server-owned.

`workbench-session-stop-contract.ts` implements the [Session stop contract](../../schemas/workbench/v1/workbench-session-stop.schema.json) and extends request/result unions; `WorkbenchStopIdSchema` brands the stop identity the requester chooses. Outcomes carry no data. The [shared corpus](../../schemas/workbench/v1/workbench-session-stop-conformance-v1.json) exercises the independent Rust binding and structural schema. Admission and the stop itself remain Server-owned.

## Client view binding

`workbench-client-presentation-contract.ts` validates the scalar exact current-member snapshot:
required nullable selection; bound members carry exact Presentation/binding, uint64 attachment generation, uint32 pane
incarnation and installed viewport, bounded to 4 KiB. The Client view binding verifies intent,
Panel, Ready/Busy view and minimum revision; the shared corpus covers null, child, bounds and
mismatched receipts. Existing Client selection and primary viewport shapes remain closed.

`workbench-client-viewport-contract.ts` validates ordered batches of 1–16 unique Panel
viewport observations, decimal uint64 positions, source identity/geometry and the offset
equations. Unknown, pending and live/history states remain distinct; primary binding,
request membership and Ready/Busy view constraints are checked before accepting a receipt.
Snapshots are bounded to 12 KiB within the existing Client control frames. Shared corpus
cases exercise both bindings and JSON Schema without introducing a Desktop runtime.

`workbench-client-view-contract.ts` independently binds the [Client view contract](../../schemas/workbench/README.md#ephemeral-client-view-control) with strict Zod shapes and semantic checks. The [shared corpus](../../schemas/workbench/v1/workbench-client-view-conformance-v1.json) is also consumed by Rust and JSON Schema witnesses. It does not place Client-local selection in canonical Workbench snapshots or expose a Desktop CLI command.

`workbench-client-layout-contract.ts` validates bounded cell rectangles, nullable visibility/focus, unique split/slot/Panel identities, finite ratios, physical containment and independently recomputed directional neighbors. Client view validation ties a layout read to the exact target and minimum revision; selected observations also match the published Client view. The shared corpus covers valid inactive/hidden targets and rejected cross-field, bounds and reference violations.

Directional control adds `focus-direction` intent and `direction-focused` / `direction-unchanged` receipts on ordinary frames 25/26. Focus success validates an exact destination distinct from the source and its observed view. No-neighbor validates the source layout revision while allowing an unrelated current context. The shared corpus covers closed shapes, revisions, target/intent mismatches and invalid directions.

No-neighbor `direction-unchanged` may retain a Busy view because it does not dismiss the current
mode or navigate. Starting remains invalid and source revision fences still apply; successful
`direction-focused` still requires a settled view. Panel and directional focus admission delegates
cancellable-mode validation to the native Client before any exit effect.

Split adjustment adds strict `resize` and `set-split-ratio` intents and `layout-adjusted` receipts on frames 25/26. Amounts are finite 0–0.5, ratios finite 0.1–0.9, and binary split paths contain at most 16 indices. The bindings independently validate target/revision/view relationships, distinct ordered slots, the exact setter value or clamped signed resize delta, and the resize-only no-split result. Ordinary Applied cannot acknowledge either intent. The shared corpus contains 64 valid and 115 invalid documents; structural JSON Schema checks supplement independent semantic validators. Runtime lifetime, draw ordering and inactive storage belong to the Windows layout owner.

## Panel custom labels

The [Panel label contract](../../apps/orchbee/docs/native-panel-labels.md) requires nullable canonical `Presentation.customLabel` and `panel.rename` with exact context/Tab/Panel IDs. A non-null override is nonempty, trimmed and at most 512 Unicode code points; no override is explicit null. Omitted fields and whitespace-only fallback labels reject. Update `label` is optional/nullable, bounded before Unicode White_Space trimming, and normalizes blank input to null. Rust and TypeScript independently implement that normalization (NEL is whitespace; BOM is not). The creation `label` and all identities/bindings stay unchanged. This is one coordinated experimental OBWP v1 cohort; strict older readers may reject the new field/action. Shared command conformance includes canonical set/clear and normalized forms.

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

The [Tab label contract](../../apps/orchbee/docs/native-tab-labels.md) owns optional `automaticLabel` metadata. New Tabs without a supplied label use their current context position; explicit numeric names and legacy Tabs remain fixed. CLI queries, label selectors and TUI display agree after move or close, and rename chooses a fixed name. The current version-1 `api snapshot` exposes canonical metadata; the offline API catalog lists its schema. Shared protocol consumers require a matching cohort.

`Workspace` and `workspace.launch` carry the same optional `automaticLabel`; default `false`
normalizes to omission, matching Rust serialization. `Workspace` also carries optional
`terminalDirectory`, a directory path. The
[location label contract](../../apps/orchbee/docs/native-location-labels.md#automatic-workspace-names)
owns their runtime meaning.

Client attention snapshots validate Agent Session identity exactly as `AgentSessionId` in `crates/agent-observation-protocol` does. Their strict shapes and semantics preserve nullable focus/missing state, ordered unique batches, bounded revisions and exact intent correspondence. These wire DTOs describe Client observations; they do not move acknowledgement into canonical Workbench or Agent state.

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
current report view separate from raw screen-rule prediction, with a required physical sample and
required nullable manifest, winner and publication skip/fallback fields. Rust/TypeScript enforce
first-highest-priority selection, state/flag coherence and evidence bounds. Unknown providers
refuse; known providers without matching state evidence use explicit unknown fallback. The existing 512 KiB result
limit applies. This unpublished cohort changes together; canonical OBAP explanations remain separate.

Active rule provenance extends physical Explain with an optional `manifest` object: source
(`bundled`, `local-override`, `remote-cache`), required nullable actual path/cached version,
shadowing and bounded typed warnings. Missing metadata preserves the earlier shape; explicit null
is rejected. Remote source/version and warning source uniqueness are checked by both bindings.
The CLI carries this metadata in its sole version-1 envelope.

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
read-only. Mutations use explicit Session-lifetime operation IDs; repeated same-kind IDs read the
retained receipt and cross-kind reuse refuses. Update reports retain per-provider attempted/cached
versions and failures, catalog/time and persistence facts; unknown effects never claim success.
Maintenance `sourceUrl`/`sourceError` distinguish a configured catalog (URL, null error), a rejected
configuration (null URL, `invalid-source`) and no configured catalog (both null).
The shared `workbench-agent-manifests-conformance-v1.json` corpus covers accepted/refused shapes,
nullable fields, numeric versions, sources, revisions, duplicates and cross-field consistency.
Semantic byte/version and cross-field checks supplement JSON Schema. The Server screen model owns
source selection, downloading, installation, cancellation and receipt limits.

Optional `maintenance.automatic` exposes the configured check and provenance, background/release/availability gates, effective enabled state and one latest automatic operation. Required nullable `lastOperation` retains pending, updated, failed or unknown facts; reload-only Applied is refused. Rust/TypeScript validate gate consistency and default provenance. Automatic work never consumes manual receipt IDs; scheduling belongs to the Server.

Both Panel carriers also admit `read-presentations` and `presentations-read` for exact primary or child report/metadata observations: four unique ordered Presentation IDs, empty revision fences, required owning `panel` rows, and shared Panel IDs across different members. Rust/TypeScript enforce payload invariants and metadata field-order ownership identically to primary reads. Existing primary read/write shapes remain unchanged. The shared corpora cover exact-member bounds, strict wrappers, same-Panel members and unbound payloads; native callers additionally correlate Presentation, Panel and binding. Ship the protocol with its Server and native TUI consumers as a matched cohort.

Both Panel carriers additionally admit closed `report-presentation` / `presentation-reported`
variants with required Panel, Presentation and expected/actual binding identities. Legacy
`report` / `reported` remain closed and primary-only. Shared Rust/TypeScript/JSON vectors cover
exact requests/replies, required IDs and legacy rejection; native correlation also requires the
exact requested member. The [Agent report target contract](../../apps/orchbee/docs/windows-panel-agent-reports.md#exact-member-targets)
and [metadata target contract](../../apps/orchbee/docs/windows-panel-metadata.md#exact-member-targets)
own runtime semantics, submission receipts and matched-cohort compatibility.

The exact Presentation selection also accepts the strict unbound alternative
`{presentationId, selectionGeneration}` with a nonzero decimal uint64 generation. It carries no
attachment, binding or viewport, and remains distinct from a null local selection. Bound selections
retain their original shape. Rust, TypeScript and JSON Schema share positive/negative conformance
vectors for missing, mixed, extra and invalid generation fields. The native current-Panel owner
validates canonical absence of binding and brackets joins with the existing selection generation;
no read allocates an epoch or viewer. Roll out this private extension with its matched native cohort.

## Native plugin runtime contract

The exported plugin request/result schemas validate matched-cohort OBWP kinds 45/46, closed context/log shapes, UTF-8 byte bounds, process outcomes and Worker/sequence cursor consistency. Shared plugin conformance cases are checked against Rust and JSON Schema. [Native plugin behavior](../../apps/orchbee/docs/windows-plugins.md) owns execution authority, output limits and remaining integration.

Plugin Panel selectors in launch options are closed, bounded catalog identities admitted only by Tab/split creation. Normalization preserves non-null selectors and removes null/absent values. The shared command corpus covers identity bounds, unknown execution fields and launch/profile conflicts; it does not imply support for every plugin placement.

The plugin contract also carries read-only Panel membership requests and exact revision/binding responses. The shared corpus rejects zero revisions, unbound members and qualified local entrypoint names.

## Workspace Git status reads

`workbench-workspace-git-contract.ts` joins the main request/response unions with
closed identity/revision/nullability/failure semantics, independent 8 KiB frames,
1024-byte branch text and uint32 ahead/behind counts. It consumes the
[shared corpus](../../schemas/workbench/v1/workbench-workspace-git-conformance-v1.json)
and [schema](../../schemas/workbench/v1/workbench-workspace-git.schema.json) with the
independent Rust binding; semantic byte/cross-field checks supplement JSON Schema.
Configuration snapshots require separate 32 KiB `spacesSidebarProfile` alongside
the Agent profile, with a 256 KiB encoded budget. Runtime installation remains
Client-owned and atomic. This binding adds no Desktop command or mutation permission.

## Workspace current directory reads

`workbench-current-directory-contract.ts` joins the main request/response unions with the closed
observed/failed outcome, the shared directory-path rules and the 26 KiB result budget. It consumes
the [shared corpus](../../schemas/workbench/v1/workbench-current-directory-conformance-v1.json) and
[schema](../../schemas/workbench/v1/workbench-current-directory.schema.json) with the independent
Rust binding. The read changes no state; the [Server](../../apps/orchbee-server/README.md#workspace-current-directory-reads)
owns which directory answers.

## Agent catalog reads

`workbench-agent-catalog-contract.ts` joins the main request/response unions with
`workbench.agent-catalog`, its `current` and `refresh` actions and its `current`/`failed` outcomes,
and the 32 KiB result budget. Row provider ids use `WorkbenchAgentStartProviderSchema`, whose order
is the catalog order every result's rows must follow; product names, commands and states come from
the wire, so the package holds no Agent table. The entry exports the `WorkbenchAgentCatalog` type,
`WorkbenchAgentStartProviderSchema` and its `WorkbenchAgentStartProvider` type for the GUI's Start
Agent dialog, which reads the catalog through the parsed response union. It consumes the
[shared corpus](../../schemas/workbench/v1/workbench-agent-catalog-conformance-v1.json) and
[schema](../../schemas/workbench/v1/workbench-agent-catalog.schema.json) with the independent Rust
binding. The [Server](../../apps/orchbee-server/README.md#agent-catalog) owns the catalog. Its test
also checks that the
[Agent start rows corpus](../../schemas/workbench/README.md#cross-client-parity-sources) is well
formed against the catalog schema; the row model that must produce its rows belongs to the GUI.

## Agent integration jobs

`workbench-agent-integration-contract.ts` joins the main request/response unions with the job
direction and binds the [Agent integration operation](../../schemas/agent-integration/README.md)
it embeds: closed vocabularies, all three node path forms, the 4,096-byte text path, and the
10 KiB request and 36 KiB result budgets. It consumes the
[shared corpus](../../schemas/workbench/v1/workbench-agent-integration-conformance-v1.json) and
[schema](../../schemas/workbench/v1/workbench-agent-integration.schema.json) with the independent
Rust binding. It rebuilds no path and grants no mutation; the Worker's writer decides every
operation. `AGENT_INTEGRATION_PROVIDERS` exports the provider vocabulary in registry order, which
Web Settings lists as its rows.

## Plugin overlay contract

The closed plugin union additionally validates exact read-only `panel-observe`/`panel-observed` membership and boolean settlement. Client `overlay` intent pins created and return member identities, expected binding, required nullable return binding and prior zoom. Self-return identities and Applied without zoom/multiple leaves fail semantic validation. The shared Rust/JSON Schema corpus covers these additive shapes; runtime ownership stays with the [Windows native Client and Server](../../apps/orchbee/docs/windows-plugins.md#overlay-lifecycle-model), with no Desktop placement or mutation permission. Use a matched native cohort.

Panel launch options additionally carry optional bounded `pluginContext` only with `pluginPanel` on Tab/split creation. The shared command corpus covers null normalization, unknown fields, UTF-8/total context bounds and unsupported creation kinds. Context is display data: the Server validates source membership, supplies canonical labels and fixes invocation source/correlation before freezing protected plugin environment. It never selects executable, launch directory or Worker authority. Omission preserves legacy admission. Rust redacts the context from launch Debug; TypeScript shares its context schema without a launch/plugin import cycle and preserves the existing plugin export. Runtime semantics and acceptance scope belong to [Pane launch source context](../../apps/orchbee/docs/windows-plugins.md#panel-launch-source-context).

Launch options also accept nullable `agentStart`, a canonical provider id from the closed startable Agent catalog, on `tab.create`, `panel.split` and `workspace.launch`. It is refused beside `pluginPanel` and on `panel.create-terminal`, and normalizes away when null. The Server resolves the command a new shell runs first; the field never carries command text. A Worktree Create intent accepts the same id as optional, non-nullable `agentStart` for its new Workspace's first shell, matching the Rust binding's refusal of explicit null.

## Configured Windows command contracts

The configured-command request/result binding and Client `command-popup-open` intent share one strict popup specification. The native installed keys profile owns opaque IDs; a catalog entry's `display` repeats the sanitized command text only for people to read, and no decoded value is executable input. Four action receipts preserve Worker spawn, canonical Pane creation, plugin invocation and Client popup control as distinct results. Optional plugin selection pins both row/column endpoints and the exact terminal content revision, requires a bound current source and limits capture to 4096 rows and 8192 UTF-8 bytes. Requests/results remain bounded at 256 KiB. The sole version-1 Client control contract admits the configured popup intent. Server resolution, temporary observer cleanup, physical jobs and TUI execution are described in [Windows custom commands](../../apps/orchbee/docs/windows-custom-commands.md).

## Panel image binding

The Panel graphics binding shares the closed image, layer, chunk and producer contracts with Rust. It checks encoded and decoded byte bounds, canonical base64, resource ordering, damage rectangles and full-width sequence strings. Metadata catalogs contain no pixels. Renderer capability and visibility fields are observations supplied by an actual Client; nullable fields do not imply a supported renderer. The shared graphics conformance corpus tests TypeScript, Rust and JSON Schema boundaries.

The semantic and command boundaries independently validate canonical split ratios and the identified `layout.set-split-ratio` CAS. Native [shared ratio behavior](../../apps/orchbee-server/docs/shared-split-ratios.md) owns commit, persistence and draw acknowledgement; Client geometry remains viewport-specific.

## Settings binding

`workbench-settings-contract.ts` binds the closed `workbench.settings` request and result to the shared command unions: a `read`, or an `edit` of up to 64 changes, each a value or `restore: true` for one dotted key; and the `current`, `edited` and `refused` outcomes. A snapshot carries its decimal revision, the file path, platform, scalar values, the keys the file sets, the launch-profile catalog, bounded diagnostics, and the file state, discriminated on `status`: `ok`, `missing`, `syntax-error` with line and column, or `unreadable`. A refusal names one of fourteen closed reasons. The [shared corpus](../../schemas/workbench/v1/workbench-settings-conformance-v1.json) is consumed by strict Zod, Rust and JSON Schema.

`setting-keys.ts` mirrors the registry's [key table](../../schemas/workbench/v1/setting-keys-v1.json) as an `as const` value, and a unit test keeps the two equal; copy the regenerated file rather than editing the table by hand. Its helpers give each key's default and read a key's value from a snapshot, falling back to the default for a value the table does not admit. The binding grants no file-write authority, transport retry or live application; the [settings file owner](../../crates/local-runtime/README.md#user-settings) validates and writes the file.

## Retained operation identity

The closed `CloseOperationReadSchema` and `CloseOperationResultSchema` join the request/result union and command conformance corpus. They mirror [retained close identity lookup](../../crates/workbench-protocol/README.md#retained-close-identity-lookup) without introducing Client-owned lifecycle state.
