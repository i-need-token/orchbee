# Client-Server Protocol Rust Binding

## Scope

`orchbee-client-server-protocol` is the independent Rust binding for the language-neutral OBCS
version-1 contract under [`schemas/client-server`](../../schemas/client-server/README.md). It owns
validated Rust values, strict direction-specific JSON decoding, deterministic encoding, and bounded
frame I/O. It has no dependency on the OBLC local admission binding, the OBCN Server-Worker
authorization binding, Terminal Host types, or Workbench types.

The crate does not connect a transport, authenticate a Client, authorize a request, choose a Worker,
store request identity, execute a terminal effect, cancel work, reconcile an unknown outcome, or
project Workbench state.

## Public API

| API                                              | Contract                                                                                              |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| `RequestId`, `WorkerId`                          | Parse nonzero opaque 128-bit lowercase hexadecimal identities.                                        |
| `WorkingDirectory`                               | Validate an absolute POSIX UTF-8 path of at most 4,096 encoded bytes with no NUL.                     |
| `Geometry`                                       | Construct complete geometry only when all scalar and product bounds hold.                             |
| `TerminalCreateRequest`, `TerminalStartRequest`  | Construct validated Client requests; start rejects equal start/create request identities.             |
| `CreateOutcome`, `StartOutcome` and result types | Represent only the stable Client-visible result combinations admitted by OBCS v1.                     |
| `encode_client_frame`, `decode_client_frame`     | Encode or decode exactly one complete in-memory Client-to-Server frame.                               |
| `encode_server_frame`, `decode_server_frame`     | Encode or decode exactly one complete in-memory Server-to-Client frame.                               |
| `read_client_message`, `read_server_message`     | Read exactly one frame from a partial-read-capable ordered stream and leave a following frame unread. |
| `write_client_message`, `write_server_message`   | Write a complete frame through partial writes without flushing the caller.                            |

All decoded objects and enums are closed. Missing explicit nullable create fields, unknown fields or
tags, duplicate keys, BOM, invalid UTF-8, trailing JSON values, direction mismatches, invalid
identities, paths, or geometry, and frames outside the exact length or 32 KiB payload bound are
rejected. Protocol and geometry numbers use the schema-owned finite binary64 safe-integer
refinement, so equivalent integral decimal and exponent spellings decode to the same value.
Canonical encoding uses compact deterministic JSON with ordinary integer tokens.

## Request boundary

The crate models no replay lookup, historical duplicate detection, or runtime idempotency ledger.
`RequestId` is request/result correlation only; a Client generates a fresh value for every submitted
request and does not reuse it within one active Server authority and authenticated subject.
`terminalCreateRequestId` identifies a deferred create within that same scope, independently of the
particular Client Session transport. A Client submits a mutating intent at most once and must not
automatically resend it after timeout, EOF, Session loss, or caller abandonment. None of those
events encodes cancellation or proves that an effect did not commit; ambiguous completion is left
to a future read-only projection or observation boundary.

## Verification

Run from the repository root:

```sh
CARGO_TARGET_DIR="$PWD/target" cargo fmt --all -- --check
CARGO_TARGET_DIR="$PWD/target" cargo check -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

The conformance tests consume the canonical cross-language corpus and cover frame boundaries,
strict payload decoding, construction invariants, partial I/O, and no-flush behavior. They do not
prove a transport, Server state machine, authorization policy, Worker or Host mapping, terminal
lifecycle, or end-to-end execution.
