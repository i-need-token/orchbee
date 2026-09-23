# orchbee-agent-observation-protocol

Independent Rust binding for the version-1 Agent observation contract governed by
[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json).
The crate provides bounded typed wire DTOs, strict semantic validation, the OBAP frame codec, and
the opaque list page-cursor wire shape. Its actual consumers are the native `orchbee` CLI and the
`orchbee-server` Agent service — consuming both reads and names.

## Scope

- Requests: the closed `AgentRequest` union — the `agent.session.list/get/explain` first-read
  family (flat closed objects with a nonzero 32-hex `requestId`; `includeEnded`/`limit` defaults
  `false`/`50` applied at parse) plus the `AgentNamesRequest` family
  (`resolve`/`names.list`/`name.assign`/`name.release` over `AgentSessionSelector`).
- Results: the closed `AgentResult` union — read `outcome` unions
  (`listed`/`found`/`rejected`/`unavailable`) nested under
  `{kind, requestId, result:{outcome}}`; `runtime-unavailable` is the only v1 unavailability
  reason — plus the names outcome envelopes.
- Entities mirror the canonical Agent model exactly; the only wire-layer narrowing is the native
  `presentationId` profile (32 lowercase hex, nonzero).
- Framing: 12-byte big-endian `OBAP` header, version `1`, request kinds `1..=13` odd
  (read `1/3/5`, names `7/9/11/13`) with results on the corresponding even kinds `2..=14`;
  per-kind and per-direction bounds are enforced before allocation; header/body `kind` agreement
  is verified after parse; small `rejected`/`unavailable` bodies are bounded at 1,024 bytes after
  typed construction.
- Cursor: the `v1.<59>.<43>` opaque token shape and the 44-byte big-endian payload layout
  (`version|operation|flags|instance|target|revision|position`). Tag creation and verification are
  the Server's private concern and never enter this crate.

## Delivery status

Both families are live: native Agent reads and `agent names/resolve/rename` use the
Server's `application_session` demux and `AgentService`. The published schema covers
kinds 1–14. Names are runtime-scoped aliases owned by the Server cell; the protocol
owns only their bounded DTOs and validation. Direct names in get/explain/wait remain
outside the current CLI grammar; see
[`proposals/agent/session-names.md`](../../proposals/agent/session-names.md).

Names lists contain at most 512 live bindings, sorted by unique name, with unique
session identities. A name-in-use rejection must identify its incumbent; other
assignment refusals carry null. Result kinds 8/12/14 reject bodies above 1,024 bytes
before allocating or reading the body; kind 10 has the 256 KiB bounded-list ceiling.
Semantic invariants on canonical read entities remain enforced on every parse and
serialize path, independently of JSON Schema.

## Non-goals

Canonical Agent state, session lifecycle, ingress, mutations, wait or change streams, persistence,
socket or timeout lifecycle, authentication, authorization, cursor secrets, deadlines, rendering,
CLI grammar, and platform effects.

## Verification

`cargo test -p orchbee-agent-observation-protocol` covers typed round-trips, frame header and
direction rejections, per-kind length ceilings, strict JSON closeness, semantic invariants,
collection caps, safe-integer bounds, cursor shape and payload layout, and derived worst-case
encoded sizes asserted against the wire ceilings.
