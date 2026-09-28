# Agent Observation Cross-Language Contract

## Authority

[`v1/agent-observation.schema.json`](v1/agent-observation.schema.json) is the language-neutral
authority for the version-1 Agent observation read bodies exchanged inside OBAP frames. The Rust
binding in `orchbee-agent-observation-protocol` and any future binding are independent
implementations. Neither binding may add accepted values, omit invariants, or redefine field
meaning.

Version 1 uses decoded JSON values governed by JSON Schema 2020-12. Every object is closed, every
collection and string is bounded, and revision/generation/sequence numbers are nonnegative safe
integers. The session, turn, request, attachment, and explanation entities mirror the canonical
Agent model exactly; the only wire-layer narrowing is the native `presentationId` profile: exactly
32 lowercase hexadecimal characters and never the zero sentinel.

## OBAP framing

One Agent read exchanges exactly one request and one result on an already admitted local Client
Session; the frame family is selected by the 4-byte record prefix inside the existing sequential
application-record loop — OBAP never opens a listener, connection type, or authentication surface
of its own.

The frame header is 12 bytes, big-endian: 4-byte magic `OBAP`, 2-byte version `1`, 2-byte kind,
4-byte payload length. Kinds: `1` list request, `2` list result, `3` get request, `4` get result,
`5` explain request, `6` explain result. A reader rejects an unknown kind, a kind for the wrong
direction, an unsupported version, an invalid header, or a header/body `kind` mismatch before
dispatch, and rejects any declared length above the bound for that kind before allocating or
reading the body.

Encoded bounds: every request body is at most 16,384 bytes; a list result body is at most
1,048,576 bytes; a get or explain result body is at most 131,072 bytes. A `rejected` or
`unavailable` outcome body is at most 1,024 bytes, checked by the producer after typed
construction and before serialization — the header alone does not identify the outcome, so no
reader may preallocate by outcome.

## Requests and results

A request is a flat closed object: `kind`, a nonzero 32-hex `requestId`, and operation fields
(`sessionId`; or `includeEnded`, `limit`, and an optional `cursor`). Absent `includeEnded`
defaults to `false` and absent `limit` defaults to `50` at parse. A result is
`{kind, requestId, result: {outcome}}` where `outcome` is a closed union: a successful page or
lookup, `rejected` with a closed reason set, or `unavailable` with `runtime-unavailable` as the
only v1 reason. A client must require an exact `requestId` and operation match before trusting a
result; the request identity is the only correlation key.

Semantic invariants apply beyond JSON Schema validation, exactly as in the canonical model:
capabilities are unique; presentation attachment `(presentationId, generation)` keys are unique
and at most one attachment per `presentationId` is navigable; an `activityHint` requires the
`activity` capability with `terminal` observation source; pending request identities are unique; an
ended session carries no activity hint, no active turn, and no pending requests; a turn-scoped
request must name the active turn. Each binding enforces these checks itself — wire shape
validation is not a canonical superRefinement proof, and a producer constructs results only from
validated canonical state.

## Page cursor

A list page cursor is an opaque server-minted token: `v1.` + unpadded base64url of a 44-byte
payload (59 characters) + `.` + unpadded base64url of a 32-byte tag (43 characters), exactly 106
characters inside a 107 bound. The payload is big-endian:

| Bytes | Field            | Rule                                                          |
| ----- | ---------------- | ------------------------------------------------------------- |
| 1     | version          | `1`                                                           |
| 1     | operation        | `1` = list                                                    |
| 2     | flags            | bit 0 = `includeEnded`; all other bits zero                   |
| 8     | runtime instance | random nonzero `u64` minted per canonical runtime instance    |
| 16    | target           | all zero in v1                                                |
| 8     | revision         | canonical runtime revision, `0..2^53-1`                       |
| 8     | position         | index into the filtered, canonically ordered session sequence |

The tag is HMAC-SHA256 over `b"orchbee.agent.list.cursor.v1\0"` followed by the 44 payload bytes,
using a 32-byte key generated at Server boot and held only in memory; there is no durable epoch.
Verification order: length and character set, canonical payload and tag re-encode equality,
constant-time tag verification, then version, operation, instance, flag, reserved-zero, and
position checks. A cursor that fails authentication, canonical form, or an impossible position is
`cursor-invalid`; an authentic cursor from the same runtime instance whose recorded list revision
no longer equals the current revision is `cursor-stale`. Position indexes the filtered session
sequence ordered by canonical `sessionId` comparison — UTF-16 code-unit order, identical to byte
order for the ASCII identity alphabet — never insertion order, and no renormalization to wire
order. `nextCursor` is non-null only when more filtered items remain, and is null at the exact end.

## Non-goals

This contract does not own canonical Agent state, session lifecycle, ingress, mutation, wait or
change streams, socket policy, admission, permission, cursor secrets, deadlines, CLI grammar, or
rendering. The Server owns the canonical runtime and the read policy; the CLI owns invocation and
presentation.

Launch Status uses read-only kinds 25/26, exact session identity and canonical revision. Its closed management phase is unmanaged, pending with the existing reservation and blocked flag, or active with original reservation/provider/physical binding. Result size remains 1,024 bytes; only the status operation accepts a status outcome. The [Rust binding contract](../../crates/agent-observation-protocol/README.md#managed-launch-status) and Server names owner define validation and lifecycle.

TerminalStatus uses read-only request/result kinds 27/28 and the same request/small-result bounds. It requires the full fresh launch binding and returns that binding with unmanaged or active provider ownership. It carries no canonical session, revision, reservation or readiness claim; only TerminalStatus results admit this outcome. Physical membership mismatch is binding-changed. The Server Names owner retains restored management; clients correlate the exact binding and request.
