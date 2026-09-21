# orchbee-agent-observation-protocol

Independent Rust binding for the version-1 Agent observation contract governed by
[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json).
The crate provides bounded typed wire DTOs, strict semantic validation, the OBAP frame codec, and
the opaque list page-cursor wire shape. Its actual consumers are the native `orchbee` CLI and the
`orchbee-server` read service — and they consume only the read family today.

## Scope

- Requests: the closed `AgentRequest` union — the `agent.session.list/get/explain` first-read
  family (flat closed objects with a nonzero 32-hex `requestId`; `includeEnded`/`limit` defaults
  `false`/`50` applied at parse) plus the staged `AgentNamesRequest` family
  (`resolve`/`names.list`/`name.assign`/`name.release` over `AgentSessionSelector`).
- Results: the closed `AgentResult` union — read `outcome` unions
  (`listed`/`found`/`rejected`/`unavailable`) nested under
  `{kind, requestId, result:{outcome}}`; `runtime-unavailable` is the only v1 unavailability
  reason — plus the staged names outcome envelopes.
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

The read family is live end to end: the CLI submits read requests and the Server's
`application_session` demux executes them through `AgentService`. The names family is
**staged codec/DTO only** — the published schema still covers just the read kinds 1–6, the
Server holds no names table and rejects well-formed names requests with a contract fault that
ends the session, and no CLI names command exists. The candidate design and its exit condition
live in [`proposals/agent/session-names.md`](../../proposals/agent/session-names.md); names must
not be presented as usable until that slice lands.

Semantic invariants beyond JSON Schema (unique capabilities and request identities, attachment-key
uniqueness, one navigable generation per presentation, activity-hint prerequisites, ended-session
emptiness, turn-scoped request membership, collection ceilings) are enforced by
`contract::validate_*` on every parse and serialize path. These are wire-level strict checks, not
a canonical superRefinement proof.

## Non-goals

Canonical Agent state, session lifecycle, ingress, mutations, wait or change streams, persistence,
socket or timeout lifecycle, authentication, authorization, cursor secrets, deadlines, rendering,
CLI grammar, and platform effects.

## Verification

`cargo test -p orchbee-agent-observation-protocol` covers typed round-trips, frame header and
direction rejections, per-kind length ceilings, strict JSON closeness, semantic invariants,
collection caps, safe-integer bounds, cursor shape and payload layout, and derived worst-case
encoded sizes asserted against the wire ceilings.
