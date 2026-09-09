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

| API                                                                                      | Contract                                                                                              |
| ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `RequestId`, `SiteId`, `WorkerId`, `TerminalId`, `ViewerId`, `HostEpoch`, `ControllerId` | Parse separately typed nonzero opaque 128-bit lowercase hexadecimal identities.                       |
| `WorkingDirectory`                                                                       | Validate an absolute POSIX UTF-8 path of at most 4,096 encoded bytes with no NUL.                     |
| `Geometry`                                                                               | Construct complete geometry only when all scalar and product bounds hold.                             |
| `TerminalCreateRequest`, `TerminalStartRequest`                                          | Construct validated Client requests; start rejects equal start/create request identities.             |
| `CreateOutcome`, `StartOutcome` and result types                                         | Represent only the stable Client-visible result combinations admitted by OBCS v1.                     |
| `encode_client_frame`, `decode_client_frame`                                             | Encode or decode exactly one complete in-memory Client-to-Server frame.                               |
| `encode_server_frame`, `decode_server_frame`                                             | Encode or decode exactly one complete in-memory Server-to-Client frame.                               |
| `read_client_message`, `read_server_message`                                             | Read exactly one frame from a partial-read-capable ordered stream and leave a following frame unread. |
| `write_client_message`, `write_server_message`                                           | Write a complete frame through partial writes without flushing the caller.                            |

All decoded objects and enums are closed. Missing explicit nullable create fields, unknown fields or
tags, duplicate keys, BOM, invalid UTF-8, trailing JSON values, direction mismatches, invalid
identities, paths, or geometry, and JSON frames outside the exact length or 32 KiB payload bound are
rejected. Protocol and geometry numbers use the schema-owned finite binary64 safe-integer
refinement, so equivalent integral decimal and exponent spellings decode to the same value.
Canonical encoding uses compact deterministic JSON with ordinary integer tokens.

## Viewer binding

`TerminalViewerRequest`, `TerminalViewerAction`, `TerminalViewerResult`, and `TerminalViewerOutcome`
provide session-scoped read-only Attach, AttachBinding, Poll, FrameApplied, NeedSnapshot, and Detach
values. AttachBinding carries separately typed OBCS `SiteId`, `WorkerId`, and `TerminalId` values;
equal encoded bytes across those domains remain valid. The crate validates their lexical contract
but neither imports Workbench types nor decides whether a tuple is currently exposed. Server owns
that semantic resolution. `ViewerUnavailableReason::WorkbenchUnavailable` is available only in a
viewer result.

Poll, FrameApplied, NeedSnapshot, and Detach require a typed `attachment_id: ViewerId`; Attach and
AttachBinding cannot carry it. `TerminalViewerAction::attachment_id()` returns the existing-viewer
selector, or `None` for the two attachment actions. Server validates ownership within the submitting
Session; the binding only validates identity syntax.

`ViewerId` and `HostEpoch` represent Server-issued attachment scope. Feedback counters use canonical
u64 decimal strings; the JSON corpus tests every action, outcome, reason, closed field set, and
identity boundary. Record results retain the schema-owner's OBSR binary framing and
`MAXIMUM_VIEWER_RECORD_BYTES` bound; the same Server frame functions dispatch either JSON control or
an opaque binary record. Client-direction readers reject OBSR. AttachBinding adds no OBCN or OBSR
type. This crate does not decode OBTP or terminal presentation; Server and the eventual renderer own
their respective routing and presentation validation. Large-record tests cover byte equality,
partial reads, following-frame alignment, direction, and invalid header/length/identity rejection.

## Controller binding

`TerminalControlRequest`, `TerminalControlAction`, `TerminalControlResult`, and `TerminalControlOutcome`
carry explicit Acquire and grant-scoped Input/Resize. `ControllerId` is a Server-issued identity;
all three actions require `attachment_id: ViewerId`, exposed by
`TerminalControlAction::attachment_id()`. This is a required unpublished OBCS v1 shape change;
Client/Server/Worker must use an exact development cohort with no implicit-viewer fallback.
`ControlInput` validates 1 through `MAXIMUM_CONTROL_INPUT_BYTES` (4,096) bytes and serializes canonical
lowercase hex. Shared number adapters validate positive decimal u64 sequences. These controls use
the existing 32 KiB OBCS JSON framing; they introduce no binary input carrier or Host dependency.
The schema-owner's corpus checks each action/outcome/reason and malformed, maximum, and excessive
inputs. This binding grants no controller authority and never retries an unknown effect.

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

## Standalone opening binding

`TerminalOpenRequest`, `TerminalOpenResult` and `OpenOutcome` implement the unshipped
[standalone opening contract](../../schemas/client-server/README.md#standalone-terminal-opening).
The request carries only correlation and complete geometry; Opened returns the independent
Server-created reference retained by legacy viewer attachment. AttachBinding does not change the
Open result. The binding owns strict encoding and validation, while Server owns singleton selection,
creation and lifetime.
Canonical fixtures cover every open outcome and reject absent, extra, malformed and wrong-direction
messages through both the JSON schema and independent Rust decoder.

Open and AttachBinding are additive, unpublished OBCS v1 source contracts. They retain the existing
frame version and limits. The installed exact runtime cohort is Client, Server, Worker, and Host;
this Rust source binding is a contract input rather than an installed cohort artifact. Before
publication, its source contract and the runtime cohort roll back together, with no persisted state
or migration.
