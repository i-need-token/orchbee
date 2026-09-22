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
`set_nonblocking` and the standard `AsFd` view let that same sole owner implement an absolute
readiness deadline without extracting, cloning, or replacing the admitted stream.
For an admission-only caller, `wait_for_server_loss` observes EOF and rejects any post-admission
byte; a caller using a semantic phase reads that phase directly instead.

The adapter does not discover an endpoint, activate a service, retry, spawn a process or thread,
render a TUI, or supervise Server lifetime. Bare Linux `orchbee` consumes the deadline-aware adapter; help and version remain effect-free.

## Deadline-bounded Linux admission

The accepted lazy-activation workflow needs a caller-owned deadline across connection and
handshake. The additive Linux-only ClientSessionConnection::connect_before(endpoint, cohort,
deadline) supplies that boundary; apps/orchbee consumes it through private LocalClient::connect_before.
Existing connect retains its separate two-second handshake timeouts. Neither entry discovers or
activates a Site. macOS deadline adaptation and native verification remain unfinished; this
addition leaves the legacy connect entry unchanged.

Only `ConnectFailed` reports failure before connection completion, preserving dial-error provenance.
Managed startup may treat its NotFound/ConnectionRefused as absence; an identically numbered
error after connection belongs to `Io` and cannot authorize activation. Peer, handshake, rejection,
and protocol failures never become absence. Neither variant permits this adapter to retry.

### Admission modeling record

| Field        | Contract                                                                                                                                                                                                                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A silent or incrementally responding local Server must not renew the Client bootstrap admission budget. This is one connection attempt, not complete Site activation.                                                                                                                                                                     |
| Inputs       | Caller-selected endpoint and absolute monotonic Instant; cohort validated before effects; untrusted kernel connect/readiness/peer observations and one OBLC response.                                                                                                                                                                     |
| Data         | One owned nonblocking socket, one unchanged deadline, one hello, decoder progress and typed result. No socket clone, process, lock, retry ledger or product state.                                                                                                                                                                        |
| Outputs      | Exact accepted generation and the same blocking, unbuffered stream; existing typed IO/protocol/rejection error. Expiry carries TimedOut. Failure drops only the candidate connection.                                                                                                                                                     |
| Invariants   | One connect syscall; hello follows same-effective-user proof; partial progress never renews time; acceptance requires exact response, successful blocking restoration and a final now < deadline; failure releases the candidate.                                                                                                         |
| Decisions    | Invalid cohort precedes effects. An expired valid request never opens a socket. Only EINPROGRESS waits for connection completion; readiness requires zero SO_ERROR. Backlog EAGAIN, other connect errors, wrong peer, IO errors and invalid responses fail without reconnecting.                                                          |
| Model        | [LocalClientAdmission](formal/tla/LocalClientAdmission.tla) models one attempt, two abstract hello chunks and two response chunks, with independent external completion and deadline events.                                                                                                                                              |
| Transitions  | Validated request → connecting → peer proof → writing → reading → decoded response → blocking restoration → accepted. Invalid input, IO, peer or response failure, or observed expiry ends in failed and releases the candidate.                                                                                                          |
| Dependencies | Linux atomic SOCK_CLOEXEC/SOCK_NONBLOCK, existing pinned Nix socket/poll wrappers, kernel peer credentials, monotonic clock and the unchanged OBLC codec.                                                                                                                                                                                 |
| Ownership    | This crate owns admission and candidate cleanup; the caller owns deadline selection and the accepted stream. Server owns its Session and observes connection loss independently.                                                                                                                                                          |
| Verification | Deterministic partial-I/O and deadline-boundary tests; real Linux Unix socket acceptance, rejection, silence/EOF, backlog rejection and next-phase preservation; old connect regressions; registered TLC safety, eventual settlement and every semantic action.                                                                           |
| Correctness  | AD-01–AD-04 below connect the bounded model to adapter witnesses. Only deadline expiration and observation are weakly fair; external connection/readiness/response events have no fairness assumption.                                                                                                                                    |
| Limits       | One Linux admission, not service readiness, activation, OS process scheduling or Server cleanup timing. Kernel calls and scheduling cannot be preempted by this adapter; it checks the deadline at controllable boundaries and before success. macOS and Windows deadline adapters are excluded. No production timeout value is selected. |

| Claim                                                                                           | Implementation and executable evidence                                                                         | Model evidence                                                                                           |
| ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| AD-01: invalid or already expired admission has no socket effect; one attempt never reconnects. | connect_before; invalid/expired and Linux backlog tests.                                                       | InvalidCohortHasNoSocket, NoReconnect, SettledNeverReenters; BeginConnect is the only connection action. |
| AD-02: peer proof precedes one hello and one exact, unbuffered response.                        | Existing credential boundary and codec; same-stream OBCS and coalesced next-phase tests.                       | HelloRequiresVerifiedPeer, ResponseFollowsOneHello; byte preservation is runtime evidence only.          |
| AD-03: one total deadline covers partial progress and final acceptance.                         | Deadline-aware poll/read/write and final checks; stepping-clock write/read and final-boundary tests.           | DeadlineNeverRenews, AcceptedIsExactAndTimely, AdmissionEventuallySettles.                               |
| AD-04: failure closes only the candidate; success returns the accepted blocking stream.         | Silent-Server EOF, typed negative responses and blocking-mode tests; existing callers retain stream ownership. | FailureReleasesCandidate, ReturnConnection, ObserveDeadline, IoFails.                                    |

### Cancellable admission boundary

`connect_before_cancellable(endpoint, cohort, deadline, wake)` is the same admission plus one
optional borrowed wake descriptor. `None` is exactly `connect_before`, and the absolute deadline is
never renewed. The descriptor is a wake-up capability only: this crate never owns, duplicates,
reads, drains, or closes it, never changes signal masks or dispositions, and never interprets what
made it readable. `POLLIN` is only a wake candidate; `POLLERR`, `POLLHUP`, or `POLLNVAL` without
readable bytes report a broken wake channel, and a wake event is decided before any simultaneous
socket readiness. The private observations surface at the public boundary as
`ConnectClientError::CancellationReady` and
`ConnectClientError::CancellationSourceFault(error)`. Neither is a cancellation verdict and neither
may be read from marker text; the caller's own signal owner performs the sole arbitration read and
decides the outcome, including an unreachable or faulted wake source. `connect` and
`connect_before` cannot produce these variants.

## Server admission and bounds

Server assigns every accepted connection an independent generation and admits up to eight active
Client Sessions. It reserves at most sixteen total Client candidate/session threads, so silent
handshakes cannot create unbounded work; a connection beyond that transport bound is closed. A
candidate within the transport bound receives a typed capacity rejection after the eight-session
admission limit. Every handshake has a two-second read/write timeout.

The delivered private Server retains the accepted unbuffered stream as the Session carrier and
passes its next-phase bytes to the separately owned
[`orchbee-client-server-protocol`](../client-server-protocol/README.md) OBCS decoder. This crate
neither defines nor decodes that phase. The Server's local subject policy, fault handling, and
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
node scripts/tla/run-models.mjs --models local-client-admission,local-client-sessions,local-client-sessions-obcs
pnpm verify
```

## Windows admission

On Windows, the same OBLC messages run over `orchbee-local-ipc` local named pipes. The adapter proves the peer token user before sending the hello, retains the caller's absolute deadline across connect/read/write, and observes cancellation through a borrowed readiness source. It never starts or retries a Site. Pipe names and permissions belong to the runtime and transport owners; OBCS/OBWP framing is unchanged.
