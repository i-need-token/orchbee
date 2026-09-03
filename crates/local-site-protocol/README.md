# Local Site Protocol

Private, experimental Rust connection contract for the first same-user POSIX Server-Worker process
spine. `orchbee-server` and `orchbee-worker` are its concrete consumers. It is not a public API, a
remote protocol, a terminal protocol, or a compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

Each connection carries exactly one handshake frame from Worker to Server and one response frame
from Server to Worker, then remains open only as a process-liveness attachment. A frame contains the
fixed `OBLS` magic, unsigned big-endian protocol version, unsigned big-endian payload length, and a
payload of at most 256 bytes. Unknown magic, version, kind, enum value, missing byte, extra byte,
oversized payload, zero generation, invalid UTF-8, empty cohort, non-ASCII cohort, or a cohort over
64 bytes is rejected.

| Message          | Fields and meaning                                                                                                                                  |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `WorkerHello`    | Exact executable cohort plus a 16-byte opaque development Worker identity. It requests no work or capability.                                       |
| `WorkerAccepted` | One nonzero process-local Server generation. It acknowledges the exact connection only.                                                             |
| `Rejected`       | One typed reason: incompatible protocol, incompatible cohort, duplicate Worker, wrong message, malformed frame, or timeout. It grants no authority. |

The protocol crate encodes and decodes complete frames and provides bounded blocking I/O adapters.
It does not retry, retain a connection, choose an endpoint, bind a listener, spawn a thread or
process, or own Server admission state.

## Trust and bounds

Before either process exchanges a frame, the POSIX adapter obtains kernel Unix-socket peer
credentials through safe `nix` APIs and compares the peer's effective user to the current effective
user. Both directions perform this check. Runtime evidence covers macOS and Linux. Directory and
socket modes remain defense in depth; possession of a path or valid bytes never replaces peer
authentication.

Reads and writes use finite frame bounds and reject partial EOF. Server additionally supplies the
finite handshake timeout. After `WorkerAccepted`, no application message is admitted in this slice;
any received byte is a protocol violation and closes the Worker attachment. The current protocol
version is private and may be replaced atomically with the unshipped cohort.

## Ownership and exclusions

This crate owns only the encoded local handshake and same-user peer proof. Server owns Worker
admission and generations; Worker owns connection order; Terminal Host continues to own its
separate protocol. Client sessions, Workbench semantics, terminal creation and streaming,
activation, endpoint discovery, remote transport, encryption, enrollment, persistence, Windows
named pipes, and compatibility negotiation remain outside this crate.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-local-site-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-site-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-site-process-spine
pnpm verify
```
