# orchbee-agent-observation-protocol

Independent Rust binding for the version-1 Agent observation read contract governed by
[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json).
The crate provides bounded typed wire DTOs, strict semantic validation, the OBAP frame codec, and
the opaque list page-cursor wire shape. Its actual consumers are the native `orchbee` CLI and the
`orchbee-server` read service.

## Scope

- Requests: `agent.session.list/get/explain` first-read slice only — flat closed objects with a
  nonzero 32-hex `requestId`; `includeEnded`/`limit` defaults (`false`/`50`) applied at parse.
- Results: closed `outcome` unions (`listed`/`found`/`rejected`/`unavailable`) nested under
  `{kind, requestId, result:{outcome}}`; `runtime-unavailable` is the only v1 unavailability
  reason — no draining variant is reserved before a real owner exists.
- Entities mirror the canonical Agent model exactly; the only wire-layer narrowing is the native
  `presentationId` profile (32 lowercase hex, nonzero).
- Framing: 12-byte big-endian `OBAP` header, version `1`, kinds `1..=6`; per-kind and
  per-direction bounds are enforced before allocation; header/body `kind` agreement is verified
  after parse; small `rejected`/`unavailable` bodies are bounded at 1,024 bytes after typed
  construction.
- Cursor: the `v1.<59>.<43>` opaque token shape and the 44-byte big-endian payload layout
  (`version|operation|flags|instance|target|revision|position`). Tag creation and verification are
  the Server's private concern and never enter this crate.

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
