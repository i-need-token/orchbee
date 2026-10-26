# Local Worker Protocol

Private, experimental Rust connection contract for the first same-user POSIX Server-Worker process
spine. `orchbee-server` and `orchbee-worker` are its concrete consumers. It is not a public API, a
remote protocol, a terminal protocol, or a compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

The OBLW portion of each connection carries exactly one handshake frame from Worker to Server and
one response frame from Server to Worker. After acceptance, a local Server adapter may retain that
same stream as a separately owned OBCN carrier; no further OBLW frame is valid. An OBLW frame
contains the fixed `OBLW` magic, unsigned big-endian protocol version, unsigned big-endian payload
length, and a payload of at most 256 bytes. Unknown magic, version, kind, enum value, missing byte,
extra byte, oversized payload, zero generation, invalid UTF-8, empty cohort, non-ASCII cohort, or a
cohort over 64 bytes is rejected.

| Message          | Fields and meaning                                                                                                                                  |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `WorkerHello`    | Exact executable cohort plus a 16-byte opaque development Worker identity. The codec admits all bit patterns; it requests no work or capability.    |
| `WorkerAccepted` | One nonzero process-local Server generation. It acknowledges the exact connection only.                                                             |
| `Rejected`       | One typed reason: incompatible protocol, incompatible cohort, duplicate Worker, wrong message, malformed frame, or timeout. It grants no authority. |

The protocol crate encodes and decodes complete frames and provides bounded blocking I/O adapters.
It does not retry, retain a connection, choose an endpoint, bind a listener, spawn a thread or
process, or own Server admission state.

## Trust and bounds

Before either process exchanges a frame, each consumer uses
[`orchbee-local-peer-credentials`](../local-peer-credentials/README.md) to compare the kernel
Unix-socket peer's effective user with its own. Both directions perform this check. Runtime
evidence covers macOS and Linux. Directory and socket modes remain defense in depth; possession of
a path or valid bytes never replaces peer authentication.

Reads and writes use finite frame bounds and reject partial EOF. Server additionally supplies the
finite handshake timeout. After `WorkerAccepted`, OBLW admits no further local-worker frame. The
Server adapter currently permits only its serialized OBCN request/result exchange while one request
is outstanding; unsolicited bytes or another protocol violate that carrier and close the Worker
attachment. This crate does not parse or own the post-OBLW phase. The current protocol version is
private and may be replaced atomically with the unshipped cohort.

The current Server adapter must map the OBLW Worker identity into OBCN's nonzero Worker identity.
It therefore rejects an all-zero `WorkerHello` identity as `MalformedFrame` before admission or
`WorkerAccepted`, without consuming a Server generation. This is an adapter compatibility
precondition, not a new OBLW codec restriction.

## Ownership and exclusions

This crate owns only the encoded local Worker handshake. Server owns Worker admission, generations,
and its post-acceptance carrier; Worker owns connection order;
`orchbee-local-peer-credentials` owns same-user proof; Terminal Host and Human Clients retain
separate protocols. Client Sessions, Workbench semantics, terminal creation and streaming,
activation, endpoint discovery, remote transport, encryption, enrollment, persistence, Windows
named pipes, and compatibility negotiation remain outside this crate.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-local-worker-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-worker-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-session-startup
pnpm verify
```
