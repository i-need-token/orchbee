# Local Client Protocol

Private, experimental Rust connection contract and concrete connecting adapter for the first
same-user POSIX Human Client-to-Server process spine. It is not a public API, Worker protocol,
terminal relay, TUI, activation mechanism, remote protocol, or compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

Each connection carries exactly one handshake frame from Client to Server and one response frame
from Server to Client. Acceptance ends the `OBLC` phase without consuming bytes from the next
protocol phase; the same stream may then remain only as the Client Session liveness attachment or
be used by its caller for a separately owned semantic protocol. An `OBLC` frame contains fixed
magic, an unsigned big-endian protocol version, an unsigned big-endian payload length, and at most
256 payload bytes. Unknown magic, version, kind, enum value, missing or extra bytes, oversized
payload, zero generation, invalid UTF-8, empty cohort, non-ASCII cohort, or a cohort over 64 bytes
is rejected.

| Message          | Fields and meaning                                                                                                                          |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `ClientHello`    | Exact executable cohort. Kernel peer credentials provide local human identity; renderer form and Client-local view state are absent.        |
| `ClientAccepted` | One nonzero process-local Server Client Session generation assigned to this exact transport. It grants no terminal or Workbench capability. |
| `Rejected`       | Incompatible protocol/cohort, capacity exceeded, wrong message, malformed frame, or timeout. Rejection changes no admitted Session.         |

Human Client and Worker contracts remain separate. This crate contains no Worker identity,
capability advertisement, lease, fence, Host, terminal, renderer, Workspace, Tab, Panel, or
semantic-command field.

## Connecting adapter

`ClientSessionConnection::connect` is the real Rust Client-side consumer. It validates the cohort
before effects, connects one caller-selected Unix socket, proves the Server has the same effective
user through `orchbee-local-peer-credentials`, applies a two-second read and write handshake
timeout, sends exactly one hello, requires one accepted response, then removes the handshake
timeouts while retaining the stream. The accepted connection implements `Read` and `Write` so its
caller can become the sole owner of the next protocol phase without opening a second socket or
creating a second reader. Exact, unbuffered handshake reads preserve next-phase bytes even when a
Server writes them together with `ClientAccepted`. Dropping the value closes only that connection.
`set_io_timeout` lets the sole caller apply one read/write timeout to its owned next phase without
changing the handshake policy.
`set_nonblocking` and the standard `AsFd` view let that same sole owner implement an absolute
readiness deadline without extracting, cloning, or replacing the admitted stream.
For an admission-only caller, `wait_for_server_loss` observes EOF and rejects any post-admission
byte; a caller using a semantic phase reads that phase directly instead.

The adapter does not discover an endpoint, activate a service, retry, spawn a process or thread,
render a TUI, or supervise Server lifetime. It is not yet called by public `orchbee`; process tests
exercise it against the real private Server without changing bare, help, or version behavior.

## Server admission and bounds

Server assigns every accepted connection an independent generation and admits up to eight active
Client Sessions. It reserves at most sixteen total Client candidate/session threads, so silent
handshakes cannot create unbounded work; a connection beyond that transport bound is closed. A
candidate within the transport bound receives a typed capacity rejection after the eight-session
admission limit. Every handshake has a two-second read/write timeout.

The delivered private Server retains the accepted unbuffered stream as the Session carrier and
passes its next-phase bytes to the separately owned
[`orchbee-client-server-protocol`](../client-server-protocol/README.md) OBCS decoder. This crate
neither defines nor decodes that phase. The Server's deny-only runtime, fault handling, and
generation-scoped cleanup are owned by its
[`apps/orchbee-server` guide](../../apps/orchbee-server/README.md).

## Ownership and exclusions

This crate owns only Client framing, handshake validation, the connecting adapter, and transfer of
the accepted byte stream to its caller. It neither defines nor decodes the next semantic phase.
Server owns listener and Client Session state. `orchbee-local-peer-credentials` owns kernel
same-user proof. `orchbee-local-site-protocol` separately owns the Worker handshake. Endpoint
discovery, lazy activation, public `orchbee` integration, Workbench snapshots and commands,
terminal relay, controller policy, persistence, Windows named pipes, remote authentication, and
production service lifecycle remain outside this slice.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-local-client-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-client-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions,local-client-sessions-obcs
pnpm verify
```
