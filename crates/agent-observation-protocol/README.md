# orchbee-agent-observation-protocol

Independent Rust binding for the version-1 Agent observation contract governed by
[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json).
The crate provides bounded typed wire DTOs, strict semantic validation, the OBAP frame codec, and
the opaque list/change cursor wire shape. Its actual consumers are the native `orchbee` CLI and the
`orchbee-server` Agent service — consuming reads, names and launch reservations, with bounded changes available to native observers.

## Scope

- Requests: the closed `AgentRequest` union — the `agent.session.list/get/explain` first-read
  family (flat closed objects with a nonzero 32-hex `requestId`; `includeEnded`/`limit` defaults
  `false`/`50` applied at parse) plus the `AgentNamesRequest` family
  (`resolve`/`names.list`/`name.assign`/`name.release` over `AgentSessionSelector`) and
  `AgentLaunchRequest` (`reserve`/`inspect`/`complete`/`cancel`/`status`) and `AgentChangesRequest` (`agent.session.changes`).
- Results: the closed `AgentResult` union — read `outcome` unions
  (`listed`/`found`/`rejected`/`unavailable`) nested under
  `{kind, requestId, result:{outcome}}`; `runtime-unavailable` is the only v1 unavailability
  reason — plus the names and launch outcome envelopes.
- Entities mirror the canonical Agent model exactly; the only wire-layer narrowing is the native
  `presentationId` profile (32 lowercase hex, nonzero).
- Framing: 12-byte big-endian `OBAP` header, version `1`, request kinds `1..=23` odd
  (read `1/3/5`, names `7/9/11/13`, launch `15/17/19/21`, changes `23`) with results on the corresponding even kinds `2..=24`;
  per-kind and per-direction bounds are enforced before allocation; header/body `kind` agreement
  is verified after parse; small `rejected`/`unavailable` bodies are bounded at 1,024 bytes after
  typed construction.
- Cursor: the `v1.<59>.<43>` opaque token shape and the 44-byte big-endian payload layout
  (`version|operation|flags|instance|target|revision|position`). Tag creation and verification are
  the Server's private concern and never enter this crate.

## Delivery status

All three families are live: native Agent reads, `agent names/resolve/rename` and Windows `agent start` use the
Server's `application_session` demux and `AgentService`. The published schema covers
kinds 1–28. Names are runtime-scoped aliases owned by the Server cell; the protocol
owns only their bounded DTOs and validation. Get/explain/wait resolve names on the same admitted session, then read or poll the pinned session ID. The CLI owns composition and budgets.

Names lists contain live `bindings` and separate `pending` launch reservations sharing a
combined limit of 512. Each list is sorted by unique name; committed session identities,
pending reservation IDs and pending presentations are unique, with no overlapping names. A name-in-use rejection must identify its incumbent; other
assignment refusals carry null. Result kinds 8/12/14 reject bodies above 1,024 bytes
before allocating or reading the body; kind 10 retains the 256 KiB bounded-list ceiling.
Semantic invariants on canonical read entities remain enforced on every parse and
serialize path, independently of JSON Schema.

The Windows Names composition also admits exact terminal selectors and typed `nameTarget` values:
canonical session or terminal launch binding (presentation, creation generation, Host epoch and
terminal ID). `resolved-target`, `assigned-target`, `unchanged-target`, `released-target` and
`listed-targets` preserve this identity distinction. Mixed lists share the same 512-entry limit,
unique global names and sorted order; physical presentation claims cannot overlap pending launches.
`rejected-target` is exclusively name-in-use with a non-null incumbent target. All small results
remain within 1 KiB; both list variants use the existing 256 KiB ceiling. Names owner/provider
reconciliation and guarded composition belong to the Server; no canonical ID is synthesized.

## Non-goals

Canonical Agent state, session lifecycle, ingress, mutations, wait or change streams, persistence,
socket or timeout lifecycle, authentication, authorization, cursor secrets, deadlines, rendering,
CLI grammar, and platform effects.

## Verification

`cargo test -p orchbee-agent-observation-protocol` covers typed round-trips, frame header and
direction rejections, per-kind length ceilings, strict JSON closeness, semantic invariants,
collection caps, safe-integer bounds, cursor shape and payload layout, and derived worst-case
encoded sizes asserted against the wire ceilings.

The names family additionally accepts `{kind:"presentation", presentationId}`. Resolution, assignment and release inspect the current canonical snapshot inside one Agent fold. Only live sessions with a matching navigable attachment qualify, including hidden attachments; attachment generations are owned by the canonical model. Zero matches refuses as not-found (session-not-found for mutation), multiple sessions refuse as ambiguous-target, and one resolves with resolvedBy presentation. A duplicate attachment never counts a session twice. Refusals leave name bindings unchanged and stay within the 1,024-byte result bound. Workbench Panel IDs and primary-presentation selection remain CLI composition, outside the Agent cell; no socket read occurs under its lock. Same-cohort delivery gates the additive wire vocabulary; rollback replaces the cohort and requires no persistent-state migration.

Launch results are bounded at 1,024 bytes before allocation. Reservations pin a native
presentation, creation generation, Host epoch and physical terminal. Reserve request IDs
also identify reservations. TTL is 1–300000 milliseconds; settleRemainingMs is 0–3000.
Inspect's candidateSessionId is required and nullable. Only the matching operation's
success outcome is valid. Names resolve/assign/release explicitly refuse launch-pending.
Deadline, readiness, baseline observations, exact binding reconciliation and atomic name
commit remain Server responsibilities in the [canonical model](../../apps/orchbee-server/docs/agent-canonical-model.md).

## Bounded changes contract

Changes uses request/result kinds 23/24 and the usual correlated outcome envelope. Omit cursor to obtain a current-head checkpoint; explicit null is invalid. The page limit defaults to 50 and accepts 1–100. Results are changes/current/reset-required/rejected/unavailable. A page has contiguous positive revisions, a tail equal to observedThroughRevision, a sampled runtimeRevision at least that large, and hasMore exactly when the tail is below the head. Every record contains the canonical journal's session projection, without synthetic attachments or execution location. The result ceiling is 1 MiB before allocation; non-page outcomes remain at most 1,024 bytes.

`AgentChangesCursor` shares the list token's bounded shape but uses operation 2 and a separate Server HMAC domain. Authentication, boot identity and retention checks belong to the Server. A lost-retention token produces reset-required with a new head checkpoint; invalid or prior-boot tokens produce cursor-invalid. Neither is uninterrupted observation. The [Server model](../../apps/orchbee-server/docs/agent-canonical-model.md#native-bounded-change-reads) owns preparation, snapshot/journal coherence and recovery. The extension requires matching cohort binaries and no persistent migration. Native consumer workflows and their final acceptance are separate from codec coverage.

## Managed launch status

Read-only launch Status uses request/result kinds 25/26 and the existing 16 KiB request/1,024-byte result ceilings. The status outcome carries exact sessionId, canonical revision and a closed management phase: unmanaged; pending with a validated reservation and blocked boolean; active with reservationId, providerId and exact launch binding. Only Status results admit this outcome. Session existence and lifecycle, completed startup retention and read authorization belong to the Server names owner. Consumers must correlate the Session and distinguish completed startup from current prompt readiness. This additive native extension requires matching cohort binaries.

TerminalStatus uses read-only request/result kinds 27/28 and the same request/small-result bounds. It requires the full fresh launch binding and returns that binding with unmanaged or active provider ownership. It carries no canonical session, revision, reservation or readiness claim; only TerminalStatus results admit this outcome. Physical membership mismatch is binding-changed. The Server Names owner retains restored management; clients correlate the exact binding and request.

## Report input values

`report_values` owns report-label normalization and full-u64 source sequences. It has no report
state, permissions, provider execution or I/O. Recognized labels use the report form (`claude`,
`agy`, `copilot`, `qwen`), independently of canonical provider IDs. Unknown labels retain trimmed
original content; normalization never removes internal controls or truncates identity. The exact
catalog lookup does not trim, lowercase, strip suffixes or recognize bare `agent`; callers own those
policies. Report normalization strips one recognized suffix before taking the last nonempty path
component. Native process lookup retains its separate suffix and bare-`agent` policy.

All report sources are custom data and confer no provider Session or lifecycle authority. Sequences
encode the full unsigned 64-bit domain as canonical decimal strings, independently of JavaScript-safe
canonical Agent revisions. Ordering belongs to the Server. Existing tests cover alias/custom-label
normalization, process recognition and full-u64 serialization boundaries.

## Native process input rules

`identify_agent_process` and `select_agent_process` now serve both native CLI diagnostics and the Windows Server Host adapter. Recognition uses the actual image and a supported runtime entrypoint, never a name mentioned in arbitrary arguments; a forged runtime argv[0] cannot replace the image. Native executable suffixes and the `agent` alias remain separate from report-label normalization. The pure candidate selector requires one outer recognized Agent chain, rejects cycles/reused-parent creation order and ambiguous siblings, and preserves explicit absence. These helpers do not collect processes, parse Windows command-line text, grant foreground/control authority or create canonical Agent Sessions. Each Windows caller uses the existing OS parser; only the Server adapter admits proven Host samples under its identity and timing fences.
