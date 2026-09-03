# Local Client Protocol

Private, experimental Rust connection contract and concrete connecting adapter for the first
same-user POSIX Human Client-to-Server process spine. It is not a public API, Worker protocol,
terminal relay, TUI, activation mechanism, remote protocol, or compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

Each connection carries exactly one handshake frame from Client to Server and one response frame
from Server to Client, then remains open only as the Client Session liveness attachment. A frame
contains fixed `OBLC` magic, an unsigned big-endian protocol version, an unsigned big-endian
payload length, and at most 256 payload bytes. Unknown magic, version, kind, enum value, missing or
extra bytes, oversized payload, zero generation, invalid UTF-8, empty cohort, non-ASCII cohort, or
a cohort over 64 bytes is rejected.

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
timeouts while retaining the liveness stream. Dropping the value closes only that connection.
`wait_for_server_loss` observes EOF and rejects any post-admission byte because this slice has no
application messages.

The adapter does not discover an endpoint, activate a service, retry, spawn a process or thread,
render a TUI, or supervise Server lifetime. It is not yet called by public `orchbee`; process tests
exercise it against the real private Server without changing bare, help, or version behavior.

## Server admission and bounds

Server assigns every accepted connection an independent generation and admits up to eight active
Client Sessions. It reserves at most sixteen total Client candidate/session threads, so silent
handshakes cannot create unbounded work; a connection beyond that transport bound is closed. A
candidate within the transport bound receives a typed capacity rejection after the eight-session
admission limit. Every handshake has a two-second read/write timeout.

Any byte after admission, including a repeated hello, closes only that Session. Disconnect removes
only the matching generation; delayed cleanup cannot remove a later Session. Client disconnect
does not stop Server, Worker, or any other Client. Server process loss closes every liveness stream.

## Ownership and exclusions

This crate owns only Client framing, handshake validation, and the connecting adapter. Server owns
listener and Client Session state. `orchbee-local-peer-credentials` owns kernel same-user proof.
`orchbee-local-site-protocol` separately owns the Worker handshake. Endpoint discovery, lazy
activation, public `orchbee` integration, Workbench snapshots and commands, terminal relay,
controller policy, persistence, Windows named pipes, remote authentication, and production
service lifecycle remain outside this slice.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-local-client-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-client-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --models local-client-sessions
pnpm verify
```
