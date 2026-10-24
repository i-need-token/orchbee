# Native Agent Observation Projection

| Field             | Value                                                                                                                                                                                                                                                                                                                                                           |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Design input under review. This document records verified current-state facts, locates the exact native wire gap, and proposes an admissible projection boundary for the native TUI and CLI. Nothing in this document is accepted authority until the product and architecture decision owner accepts it; every option below remains a proposal, not a decision |
| Decision owner    | The active requester acting as Orchbee product and architecture decision owner                                                                                                                                                                                                                                                                                  |
| Semantic owners   | Agent observation domain, Server semantic ownership, and native Client projection boundaries                                                                                                                                                                                                                                                                    |
| Current authority | [`agent-runtime-observation.md`](agent-runtime-observation.md), [`native-runtime-topology.md`](../platform/native-runtime-topology.md), [`native-command-surface.md`](../cli/native-command-surface.md)                                                                                                                                                         |
| Exit condition    | The projection boundary is accepted, the owning slices are implemented with evidence, the maintained contract moves into the owning package and application guides and tests, and this proposal and its translation are deleted                                                                                                                                 |

Simplified Chinese review translation:
[native-agent-projection.zh-CN.md](native-agent-projection.zh-CN.md). English remains canonical.

## Change authorization

| Field         | Proposed authorization                                                                                                                                                                                                                                                                                                                                                                                             |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Tier          | Modeled because the boundary owns ordered state, cursor semantics, retention overflow, and multi-client projections; Consequential because it introduces a new cross-process contract between Server and native Clients                                                                                                                                                                                            |
| Problem       | The native TUI and CLI cannot observe Agent sessions at all. The canonical observed-session model and its volatile change feed exist only in the TypeScript runtime behind Desktop-owned ingress, a Desktop-owned renderer protocol, and a Desktop-owned local semantic-command endpoint. The Rust Server has no Agent module, and the native wire families carry no Agent document                                |
| Phase outcome | One authorized, read-only, content-free Agent observation projection reaches the native TUI and CLI from the Server-owned canonical model: baseline snapshot first, bounded ordered change pages behind an opaque cursor, explicit reset-required on retention overflow, and truthful freshness; TUI renders it through its own presentation and CLI exposes the query family through the accepted command surface |
| Non-goals     | Agent launch, stop, resume, scheduling, orchestration, prompt submission, question answering, approval, or any mutation; terminal-content or process scraping; durable history; remote relay; changing Workbench publication semantics; migrating Server-side canonical state in this slice; deciding wire schema names, numeric bounds, or product vocabulary                                                     |
| Compatibility | Terminals, Workbench snapshots and publications, existing Client sessions, and CLI commands behave identically when Agent observation is absent, unsupported, stale, or refused. A Client that does not request the family sees no change                                                                                                                                                                          |
| Delivery      | The wire contract and schema precede TUI and CLI consumers. Each consumer slice keeps its own presentation ownership and evidence boundary                                                                                                                                                                                                                                                                         |
| Recovery      | Withdraw the projection family without touching Workbench projection; absence degrades every Agent indicator to hidden or `unknown` and never invents working, attention, success, or failure                                                                                                                                                                                                                      |

## Verified current state

These statements are verified facts at the base commit, not proposals.

### Canonical model and feed (TypeScript)

- `packages/agent` owns the canonical observed-session model (`AgentSessionSnapshot`, `AgentRuntimeSnapshot`, `AgentTurnState`, `AgentAttentionRequest`, `AgentTurnOutcome`, `AgentObservation`), the semantic projection (`needs-attention`, `failed`, `succeeded`, `canceled`, `interrupted`, `working`, `idle`, `unknown`, `ended`), and the volatile change journal (`AGENT_RUNTIME_CHANGE_RETENTION` is 512 applied transitions, `MAX_AGENT_RUNTIME_CHANGE_PAGE_SIZE` is 100 ordered records per read) inside `AgentRuntime`.
- The journal is content-free by contract: no prompts, transcripts, commands, question text, tool content, filesystem paths, environment values, provider-private payloads, or screen evidence. Retention overflow returns an explicit reset-required result and directs the consumer to a fresh canonical snapshot.
- Desktop owns ingress and brokering: `apps/desktop/src/main/agent-observation/` hosts the `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` Unix-socket listener, joins it with the private APC correlation half forwarded as a `TerminalApplicationSignal` on channel `orchbee.agent.v1`, and dispatches joined transitions into the runtime. `DesktopAgentObservationBroker` publishes snapshot-change and semantic-event notifications to the renderer over `desktop-client-protocol`.
- The Web Workbench retains `AgentSnapshotSource` consumption through `useAgentSnapshot`, agent navigation, and notification hooks. The old Desktop semantic endpoint and TypeScript CLI consumers have retired; native Agent behavior is maintained by its native owners.

### Native path (Rust)

- `apps/orchbee-server/src` contains no Agent module, schema, or ownership seam; `apps/orchbee-server/src/lib.rs` exports only Workbench authority and projection types.
- `crates/workbench-protocol` `Snapshot`, `Delta`, `Presentation`, `DeltaChange`, `WorkbenchProtocolDocument`, and `WorkbenchPublicationDocument` carry only Workbench navigation and terminal-context data. `WorkbenchRequest` admits exactly `workbench.snapshot.read`, `workbench.command`, `workbench.tab-close.read`, `workbench.tab-close.resume`, and `workbench.publication.read`. The Server publication journal in `apps/orchbee-server/src/workbench_projection.rs` retains at most `MAXIMUM_RESULT_PUBLICATIONS` (64) committed publications.
- `apps/orchbee-server/src/local_client_service/application_session.rs` demultiplexes the local Client connection on a four-byte prefix: `OBCS` (terminal control frames), `OBCQ` (paste records), and `OBWP` (Workbench requests). Any other prefix terminates the session with `UnknownProtocol`. There is no Agent family branch.
- `apps/orchbee-worker` decodes the `orchbee.agent.v1` terminal application signal in `host_connection/viewers.rs` only to fence it against the terminal epoch and drop it; no Agent runtime, join, or forwarding exists. `apps/orchbee-server`, `apps/orchbee-worker`, `crates/capability-node-protocol`, and `crates/client-server-protocol` contain no Agent references.
- `apps/orchbee` (native TUI/CLI executable) polls Workbench state through `snapshot.read` baseline plus `publication.read` cursor reads, with a 250 ms publication probe piggybacked on ordinary polls; hints become `PublicationDebt` refresh demand. `tui_render.rs` draws a static localized `Agents` sidebar line that consumes no state. The native CLI surface admits no Agent command.

## Precise wire gap

A native Client cannot express any of the following on the current wire:

1. Read an Agent observation snapshot — no request kind exists; `Snapshot` has no Agent collection or per-presentation Agent field.
2. Follow a bounded change feed — `PublicationReadIntent`/`WorkbenchPublicationOutcome` is a Workbench-revision journal, not an Agent journal; its 64-entry retention and Workbench revision space cannot express the accepted 512-entry Agent journal, the ≤100 page bound, the signed opaque cursor, or the reset-required outcome.
3. Receive a reset-required signal for Agent state — `WorkbenchPublicationOutcome` has `Current`, `Published`, `Denied`, and `Unavailable` only.
4. Map Agent presentation attachments to native presentations — the canonical model stores `presentationAttachments[].presentationId` as an opaque 1–256 character string, while the native Workbench `TerminalPresentationId` is exactly 32 lowercase hexadecimal characters. No mapping contract exists on either side.
5. Name the observation family — the four-byte demux has no Agent magic, and neither `ClientMessage`/`ServerMessage` (OBCS) nor `WorkbenchRequest` (OBWP) admits an `agent.*` kind.

## Projection boundary options

| Option                                                                                         | Shape                                                                                                                                                  | Consequence                                                                                                                                                                                                                                                                                                           |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A. Embed Agent fields in the Workbench `Snapshot`/`Delta`                                      | Add a session collection or per-presentation markers to `crates/workbench-protocol` documents                                                          | Fuses two authorities and two revision spaces: the Workbench journal (64 committed publications, Workbench revisions) would carry unrelated Agent churn (512-transition journal, own cursor and reset semantics), and every Workbench consumer would parse a domain it does not own. Rejected                         |
| B. Admit `agent.observation.*` request and outcome kinds inside the existing OBWP request enum | Extend `WorkbenchRequest` and the response enum with Agent intents on the same framed connection                                                       | Reuses the framed connection, request identity, versioning, and dispatch with no new demux branch; but the enum and crate are named and owned as the Workbench projection protocol, so Agent kinds either misname the family or force a rename/generalization. Viable only if the owner accepts the namespace tension |
| C. Dedicated Agent-observation protocol family                                                 | A new four-byte magic and crate on the same authenticated local Client connection, with its own request/outcome documents, journal, and revision space | Matches the repository's one-family-per-domain demux (`OBCS`, `OBCQ`, `OBWP`, `OBCN`, `OBLS`), keeps ownership unambiguous, and leaves Workbench untouched; costs one new demux branch, one crate, and its own conformance evidence                                                                                   |
| D. Agent reads inside `ClientMessage`/`ServerMessage` (OBCS)                                   | Extend the terminal-control frame family                                                                                                               | OBCS currently carries terminal attachment and control messages, not projection reads; mixing a domain projection into the control family repeats the Option B ownership problem on a second carrier. Rejected                                                                                                        |

### Recommendation

Prefer Option C, with Option B as the acceptable lighter-weight fallback. The decisive properties are not the byte-level carrier but the semantic boundary: one Agent-observation family with its own versioned documents, its own revision and cursor space, its own bounded journal, and no shared sequence with Workbench. Option C states that boundary most honestly; Option B reaches the same semantics if the owner prefers fewer protocol families and accepts the `agent.*` namespace living beside `workbench.*` kinds. Whichever carrier is chosen, the family must not embed Agent state inside `Snapshot`, `Delta`, or `Presentation`, and must not reuse the Workbench publication journal, revision, or cursor space.

The exact wire form — new magic versus new kinds on OBWP, schema names, numeric bounds, version negotiation, and the protocol-version ladder — remains a reserved decision owned by the wire-contract stage. This document asserts only the semantic requirements the chosen form must satisfy.

## Required consumption contract

Whatever carrier is selected, the projection must preserve the accepted feed semantics:

1. A Client first reads a canonical snapshot at a recorded revision, then follows bounded change pages behind an opaque cursor. Pages return at most 100 ordered records.
2. Retention overflow is never presented as a continuous feed. The outcome is an explicit reset-required that carries the current revision and directs the Client to reacquire a snapshot.
3. Cursors are fenced against coordinator generation, operation, and target; forged, cross-generation, cross-operation, or malformed cursors are rejected. No continuity is claimed across Server restart.
4. Records remain content-free: session identity, provider, execution location, presentation attachments, lifecycle, turn state, attention requests, outcome, and observation health (fidelity, freshness, generation, sequence, source) only.
5. The family is read-only. No request may launch, stop, resume, prompt, answer, approve, schedule, or orchestrate.
6. Long polling is a Server-side admission choice, not a wire requirement; a bounded `wait`/`changes` invocation may map onto either a Server long-poll or bounded Client re-polling.

## TUI projection positions

The TUI consumes the projection as a peer renderer; it owns presentation only.

- Sidebar: replace the static `Agents` line with an aggregate entry drawn from the projection — session count, attention count, and the highest-priority semantic status. It must degrade to the current static presentation when the family is absent or refused.
- Tab strip: a per-presentation semantic indicator is admissible only where an attachment mapping exists; it belongs on the presentation slot, not the tab label, and renders nothing when unmapped.
- Navigator: the richer surface — a grouped session list with provider, execution location, semantic status, and freshness — fits the navigator overlay better than permanently expanding compact tab rows.
- Refresh mechanics: Agent change hints track their own debt and refresh generation, separate from `PublicationDebt`. A probe may piggyback on the existing poll cadence, but it issues the Agent family read, not `publication.read`; Workbench publications remain a different domain's journal.
- Status vocabulary (`blocked`, `working`, `done`, icons, colors) is product-language and stays reserved; the wire carries the nine semantic statuses and freshness, and the renderer maps them.

## CLI query family

`native-command-surface.md` lists Agent under later domains added only by their owning accepted domain contracts; this document is that domain input for reads. The proposed admission is the five read operations as resource-first commands — `agent session list`, `agent session get`, `agent session explain`, `agent session wait`, and `agent session changes` — shaped as:

- stateless invocations that obtain a baseline at the recorded revision and print one bounded page; `wait` and `changes` are bounded single invocations, not resident subscriptions;
- versioned JSON or NDJSON on stdout, localized diagnostics on stderr, and the accepted stable exit categories, including a distinct category for reset-required rather than a silent gap;
- no TUI initialization, no Desktop dependency, and no direct reads of product state files;
- no mutation verbs; any later Agent control family is a separate decision.

## Correlation and migration notes

- Presentation attachment identity must be reconciled when the canonical model moves to Rust: the TypeScript model admits opaque strings up to 256 characters while the native `TerminalPresentationId` is 32 lowercase hexadecimal characters. The Server-owned projection should emit native presentation identities so every Client maps attachments without a per-renderer correlation rule.
- The `orchbee.agent.v1` channel name on the terminal-effect wire and the `orchbee.agent.v3;` APC prefix are ingress transport details, not Client-facing observation contracts; naming the Client family must not inherit either.
- Current TypeScript packages (`@orchbee/agent` contracts) remain current authority for their existing consumers until the accepted migration switches each consumer; they do not silently become the Rust wire contract. A language-neutral schema with cross-language conformance evidence is required before the first Rust producer and consumer.
- Server restart fencing, remote-Client reachability, and the Server-side journal are owned by the Server migration slice; this proposal consumes their outcome without deciding it.

## Reserved decisions

- Exact wire form: new magic family versus new kinds on an existing carrier, schema names, numeric bounds, compatibility negotiation, and version ladder.
- Product presentation vocabulary, iconography, colors, and where the aggregate entry appears in the sidebar ordering.
- Whether Agent ever enters the mutation domain; this document proposes reads only.
- The canonical attachment-to-native-presentation identity contract and any migration of stored identity formats.
- Journal retention, cursor signing, and reset semantics across Server restart; the 512/100 bounds are accepted for the TypeScript journal and are the reference point, not an automatic wire commitment.
- Remote Client delivery and Server network admission of the family.

## Evidence obligations for any accepted slice

- Schema fixtures for every request and outcome kind, including reset-required, denied, unavailable, and malformed-cursor rejection.
- Journal conformance evidence: ordered batching, contiguous revision pages, retention overflow, cursor fencing, and coordinator-retirement behavior, mirroring the accepted TypeScript journal tests and bounded model.
- TUI evidence that absence, refusal, and reset-required degrade truthfully without fabricated statuses; CLI evidence for machine output, exit categories, and bounded invocations.
- Bilingual documentation pairing for every repository document the slice touches.
