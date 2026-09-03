# Client-Server Terminal Control Contract

## Authority and boundary

[`v1/client-server.schema.json`](v1/client-server.schema.json) and this document are the
language-neutral authority for version 1 of the Orchbee Client-Server terminal-control protocol
(`OBCS`). OBCS carries terminal semantic control between an authorized Client Session and Server
over any ordered byte stream. The Rust crate `orchbee-client-server-protocol` is an independent
binding and may not add values, omit invariants, or redefine field meaning.

OBCS has these adjacent boundaries:

| Boundary             | Responsibility                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| `OBLC`               | Same-user local Client connection admission and liveness only; it grants no terminal operation. |
| `OBCS`               | Carrier-neutral Client-to-Server terminal semantic requests and stable Client-visible results.  |
| `OBCN`               | Server-to-Worker placement authorization, operation, generation, lease, and fence facts.        |
| Workbench projection | Read-only authorized product projection; it is not a terminal-control writer.                   |

Version 1 admits only `terminal.create`, `terminal.create.result`, `terminal.start`, and
`terminal.start.result`. It defines no attach, input, resize-after-start, close, cancel, retry
transport, authentication, authorization policy, Server state store, Worker operation, Host launch,
or Workbench mutation.

## Frame

| Field          | Encoding                                               |
| -------------- | ------------------------------------------------------ |
| Magic          | Four bytes `OBCS`                                      |
| Frame version  | Unsigned 16-bit big-endian integer `1`                 |
| Payload length | Unsigned 16-bit big-endian byte length, at most 32 KiB |
| Payload        | Strict UTF-8 JSON object with `protocolVersion: 1`     |

A decoder validates the complete eight-byte header before allocating payload storage, reads exactly
the declared payload, and rejects an invalid magic, unsupported frame version, payload over 32 KiB,
truncation, or extra bytes when decoding one complete in-memory frame. A stream reader consumes one
frame at a time so another complete frame may follow. Readers and writers must support partial I/O;
a frame writer does not flush its caller's stream.

The payload rejects a UTF-8 BOM, invalid UTF-8, duplicate object keys at any depth, a trailing second
JSON value, non-object roots, unknown fields, unknown message or enum tags, and a message in the
wrong direction. Whitespace, object-key order, and equivalent JSON string escapes are not semantic.
Canonical encoding is deterministic compact JSON; equality and request replay compare decoded
values, not raw payload bytes.

## Identity, path, and geometry

`requestId`, `terminalCreateRequestId`, and `workerId` are opaque, nonzero 128-bit values encoded as
exactly 32 lowercase hexadecimal characters. A `terminal.start` request uses distinct `requestId`
and `terminalCreateRequestId` values.

An explicit `cwd` is an absolute POSIX UTF-8 path whose encoded path value contains 1 through 4,096
bytes and no NUL. `/` is valid. OBCS does not normalize the path, resolve `.` or `..`, inspect the
filesystem, infer Git meaning, or establish that the selected Worker accepts it. `cwd: null` asks
the selected Worker boundary to resolve its authorized default; omission is invalid.

Every geometry contains all four integer fields and satisfies every invariant:

| Field or product                                       | Range                     |
| ------------------------------------------------------ | ------------------------- |
| `columns`, `rows`, `cellPixelWidth`, `cellPixelHeight` | 1 through 4,096 inclusive |
| `columns * rows`                                       | At most 262,144           |
| `columns * cellPixelWidth`                             | At most 65,535            |
| `rows * cellPixelHeight`                               | At most 65,535            |

## Client requests

`terminal.create` contains `protocolVersion`, `requestId`, explicit `workerConstraint`, explicit
`cwd`, and `startMode`.

| Field              | Admitted value                                                                  |
| ------------------ | ------------------------------------------------------------------------------- |
| `workerConstraint` | `null`, or `{ "kind": "exact", "workerId": ... }`                               |
| `cwd`              | `null`, or one valid absolute POSIX path                                        |
| `startMode`        | `{ "kind": "immediate", "geometry": ... }`, or exactly `{ "kind": "deferred" }` |

Deferred creation carries no geometry or fallback. Immediate creation carries one complete valid
geometry. A Client never supplies a Terminal identity, Worker generation, operation, lease, fence,
Host identity or epoch, executable, argument vector, environment or overlay, Project, Workspace,
Panel, Worktree, Client generation, profile, or configuration fact.

`terminal.start` contains `protocolVersion`, a new `requestId`, the distinct
`terminalCreateRequestId` that identifies the deferred create request, and one complete valid
`geometry`. It carries no terminal identity or fallback geometry.

## Server results

Every result contains `protocolVersion`, the exact request's `requestId`, and one closed `outcome`.
The result exposes no internal Terminal, Worker, operation, lease, fence, Host, or placement fact.

| Create outcome    | Closed detail                                                                                      |
| ----------------- | -------------------------------------------------------------------------------------------------- |
| `accepted`        | `state` is `awaiting-start`, or `started` with the accepted complete `geometry`.                   |
| `rejected`        | `not-authorized`, `worker-constraint-unsatisfied`, `cwd-not-accepted`, or `request-id-conflict`.   |
| `unavailable`     | `no-compatible-worker`, `terminal-runtime-unavailable`, or `server-busy`.                          |
| `failed`          | `terminal-create-failed`.                                                                          |
| `outcome-unknown` | No additional field. The Client must reconcile rather than infer success or retry as a new effect. |

| Start outcome     | Closed detail                                                                                                                  |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `accepted`        | The accepted complete `geometry` and `transition` of `started` or `already-started`.                                           |
| `rejected`        | `not-authorized`, `terminal-create-unavailable`, `terminal-not-awaiting-start`, `geometry-conflict`, or `request-id-conflict`. |
| `unavailable`     | `terminal-runtime-unavailable` or `server-busy`.                                                                               |
| `failed`          | `terminal-start-failed`.                                                                                                       |
| `outcome-unknown` | No additional field. The Client must reconcile rather than infer a transition.                                                 |

## Request and cancellation obligations

Within one active Server authority, a request identity names one semantic request. Repeating the
same identity with a semantically equal decoded request must reconcile with that request rather than
authorize another effect; reusing it for a different decoded request returns `request-id-conflict`.
This contract defines the boundary obligation and result vocabulary, not an idempotency ledger or
Server implementation.

Version 1 has no cancel message. Transport loss, Client Session loss, or caller abandonment does not
encode cancellation and must not be interpreted as proof that a terminal effect did not commit.

## Conformance and limits

[`v1/client-server-conformance-v1.json`](v1/client-server-conformance-v1.json) is the deterministic
cross-language oracle. It includes every message, result category, reason, accepted-state variant,
and accepted-start transition, plus malformed and semantically invalid payloads. Repeated-value
recipes compactly materialize exact byte-boundary, multibyte, and worst-case JSON path inputs.
Bindings consume the corpus without deriving expected results from their own codec.

Run from the repository root:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

These checks establish the schema-to-binding corpus agreement and deterministic frame behavior.
They do not establish authentication, authorization decisions, same-active-Server idempotency
storage, cancellation, terminal execution, persistence, recovery, or any live transport integration.
