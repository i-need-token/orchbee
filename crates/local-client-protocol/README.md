# Local Client Protocol

Private, experimental Rust connection contract and concrete connecting adapters for same-user
Client ↔ Server connections on Linux, macOS, and Windows. It is not a public API, Worker protocol,
terminal relay, TUI, activation mechanism, or remote protocol; its compatibility comes from the
handshake, not from this crate.

## Opening a connection

Every connection opens with one `OBHS` frame from
[`orchbee-connection-handshake`](../connection-handshake/README.md): an offer for a Client
Connection or one control request. The caller builds the offer; the adapters send it and read one
reply within the handshake's two-second candidate deadline, inside the caller's own deadline.

| Reply               | Result                                                                                                                                                            |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Answer              | The `OBLC` phase below starts on the same stream.                                                                                                                 |
| Rejection           | `ConnectClientError::Refused` keeps the Server's reason, generation range, product version, and environment, so the caller can name the older side.               |
| Not an `OBHS` frame | `ConnectClientError::PredatesHandshake`: the Server is older than the handshake. An old Server answers the unknown frame with its own `OBLC` malformed rejection. |

Which offer the Server accepts, and in which order it checks connection kind, environment,
generation or fingerprint, capacity, and state, belongs to the handshake crate and the
[Server guide](../../apps/orchbee-server/README.md).

## Framing and messages

After an answer, `OBLC` frames carry fixed magic, an unsigned big-endian protocol version, an
unsigned big-endian payload length, and at most 256 payload bytes. Unknown magic, version, kind,
enum value, missing or extra bytes, an oversized payload, or a zero generation is rejected. The
Client phase consumes no byte of the next protocol phase; the same stream may then remain only as
the Client Connection liveness attachment or carry a separately owned semantic protocol.

| Message              | Fields and meaning                                                                                                                             |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `ClientHello`        | Kind 1 with no payload: enter the Client phase. Kernel peer credentials provide local human identity; the offer already stated the build.      |
| `ClientAccepted`     | One nonzero process-local Server Client Connection generation assigned to this exact transport. It grants no terminal or Workbench capability. |
| `Rejected`           | Capacity exceeded, wrong message, malformed frame, timeout, or stopping. Rejection changes no admitted Client Connection.                      |
| `SessionStopRequest` | Kinds 4 and 5 below.                                                                                                                           |

Human Client and Worker contracts remain separate. This crate contains no Worker identity,
Worker capability advertisement, lease, fence, Host, terminal, renderer, Workspace, Tab, Panel, or
semantic-command field.

## Process query

`exchange_query_before(endpoint, environment, deadline, wake)` sends one `query` request carrying
the caller's environment and returns the Server's `query-result`: the Server, then its Worker and
Hosts. The connection never enters the Client phase and closes before returning, so it allocates no
Client Connection, generation, or stop lease. It runs on Linux, macOS, and Windows under the same
peer proof and deadlines as an offer. A rejection, an old Server, and EOF after connection are never
absence. The [CLI status contract](../../apps/orchbee/docs/native-runtime-status.md) owns the public
output.

## Managed Session stop control

Kinds 4 and 5 carry same-user Session stop requests and status responses after an answered offer,
within the 256-byte bound. A request contains a nonzero 128-bit stop identity and a closed
start/status/explicit-retry action. Responses echo that identity and distinguish not-recorded,
pending, failed, another active identity and unmanaged mode. Pending/failed states retain a positive
attempt (at most 64), a closed phase, and failures retain their reason and known disposal count for
that attempt. A response never claims final process exit: the admitted Windows activation record is
the completion receipt after the managed owner settles its resources. Duplicate start never retries
effects; only explicit retry of the same failed identity may advance the attempt. A Client hello
after a recorded stop receives the typed stopping rejection. Unknown tags, zero identities, invalid
attempts and trailing payload bytes fail decoding. The codec itself neither starts nor stops a
process, and only Windows has a stop adapter.

## Connecting adapter

`ClientConnection::connect(endpoint, offer)` connects one caller-selected Unix socket, proves the
Server has the same effective user through `orchbee-local-peer-credentials`, applies two-second
read and write timeouts, sends the offer and then one hello, requires one accepted response, then
removes the timeouts while retaining the stream. The accepted connection implements `Read` and
`Write` so its caller can become the sole owner of the next protocol phase without opening a second
socket or creating a second reader. Exact, unbuffered reads preserve next-phase bytes even when a
Server writes them together with `ClientAccepted`. Dropping the value closes only that connection.
`set_nonblocking` and the standard `AsFd` view let that same sole owner implement an absolute
readiness deadline without extracting, cloning, or replacing the admitted stream.
For an admission-only caller, `wait_for_server_loss` observes EOF and rejects any post-admission
byte; a caller using a semantic phase reads that phase directly instead.

The adapter does not discover an endpoint, activate a service, retry, spawn a process or thread,
render a TUI, or supervise Server lifetime. Bare Linux and macOS `orchbee` consume the deadline-aware adapter; help and version remain effect-free.

## Deadline-bounded POSIX admission

The accepted lazy-activation workflow needs a caller-owned deadline across connection and
handshake. POSIX ClientConnection::connect_before(endpoint, offer, deadline) supplies that
boundary on Linux and macOS; apps/orchbee consumes it through private LocalClient::connect_before.
Existing `connect` retains its separate two-second handshake timeouts. Neither entry discovers or
activates a Session. Native macOS verification exercises the same descriptor, poll, peer-proof, and
handshake path; the legacy `connect` entry remains unchanged.

Only `ConnectFailed` reports failure before connection completion, preserving dial-error provenance.
Managed startup may treat its NotFound/ConnectionRefused as absence; an identically numbered
error after connection belongs to `Io` and cannot authorize activation. Peer, handshake, rejection,
and protocol failures never become absence. Neither variant permits this adapter to retry.

### Admission modeling record

| Field        | Contract                                                                                                                                                                                                                                                                                                                     |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A silent or incrementally responding local Server must not renew the Client bootstrap admission budget. This is one connection attempt, not complete Session activation.                                                                                                                                                     |
| Inputs       | Caller-selected endpoint and absolute monotonic Instant; a valid offer; untrusted kernel connect/readiness/peer observations, one OBHS reply and one OBLC response.                                                                                                                                                          |
| Data         | One owned nonblocking socket, one unchanged deadline, one offer, one hello, decoder progress and typed result. No socket clone, process, lock, retry ledger or product state.                                                                                                                                                |
| Outputs      | Exact accepted generation and the same blocking, unbuffered stream; existing typed IO/protocol/rejection error. Expiry carries TimedOut. Failure drops only the candidate connection.                                                                                                                                        |
| Invariants   | One connect syscall; the offer follows same-effective-user proof and the hello follows an answer; partial progress never renews time; acceptance requires exact response, successful blocking restoration and a final now < deadline; failure releases the candidate.                                                        |
| Decisions    | An expired request never opens a socket. The answer must arrive within the candidate deadline. Only EINPROGRESS waits for connection completion; readiness requires zero SO_ERROR. Backlog EAGAIN, other connect errors, wrong peer, rejections, IO errors and invalid responses fail without reconnecting.                  |
| Model        | One deadline-bound admission attempt moves through connect, peer proof, partial hello/response I/O and blocking restoration. Controlled implementation tests inject completion and expiry at each boundary.                                                                                                                  |
| Transitions  | Request → connecting → peer proof → offer → answer → hello → decoded response → blocking restoration → accepted. A rejection, IO, peer or response failure, or observed expiry ends in failed and releases the candidate.                                                                                                    |
| Dependencies | POSIX atomic SOCK_CLOEXEC/SOCK_NONBLOCK, existing pinned Nix socket/poll wrappers, kernel peer credentials, monotonic clock, the handshake codec and the OBLC codec.                                                                                                                                                         |
| Ownership    | This crate owns admission and candidate cleanup; the caller owns deadline selection and the accepted stream. Server owns its Client Connection and observes connection loss independently.                                                                                                                                   |
| Verification | Deterministic partial-I/O and deadline-boundary tests, Linux and macOS Unix-socket acceptance/rejection/silence/EOF/backlog cases, next-phase byte preservation and existing connect regressions.                                                                                                                            |
| Correctness  | AD-01–AD-04 map to the concrete adapter witnesses below. Local deadline observation provides bounded failure without assuming external connect/readiness/response progress. No separate admission TLA model is retained.                                                                                                     |
| Limits       | One POSIX admission, not service readiness, activation, OS process scheduling or Server cleanup timing. Kernel calls and scheduling cannot be preempted by this adapter; it checks the deadline at controllable boundaries and before success. Windows uses its named-pipe adapter. No production timeout value is selected. |

| Claim                                                                                   | Implementation and executable evidence                                                                         |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| AD-01: already expired admission has no socket effect; one attempt never reconnects.    | connect_before; expired and Linux/macOS backlog tests.                                                         |
| AD-02: peer proof precedes one offer, one hello and one exact, unbuffered response.     | Existing credential boundary and codecs; same-stream OBCS and coalesced next-phase tests.                      |
| AD-03: one total deadline covers partial progress and final acceptance.                 | Deadline-aware poll/read/write and final checks; stepping-clock write/read and final-boundary tests.           |
| AD-04: failure closes only the candidate; success returns the accepted blocking stream. | Silent-Server EOF, typed negative responses and blocking-mode tests; existing callers retain stream ownership. |

### Cancellable admission boundary

`connect_before_cancellable(endpoint, offer, deadline, wake)` is the same admission plus one
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

Server assigns every accepted connection an independent generation and admits up to thirty-two active
Client Connections. It reserves at most forty-eight total Client candidate/connection threads, so silent
handshakes cannot create unbounded work; a connection beyond that transport bound is closed. A
candidate within the transport bound receives a `capacity` rejection to its offer after the
thirty-two-connection admission limit. The offer and the hello each have a two-second read/write
timeout; a silent offer receives a `timeout` rejection.

The delivered private Server retains the accepted unbuffered stream as the Client Connection carrier and
passes its next-phase bytes to the separately owned
[`orchbee-client-server-protocol`](../client-server-protocol/README.md) OBCS decoder. This crate
neither defines nor decodes that phase. The Server's local subject policy, fault handling, and
generation-scoped cleanup are owned by its
[`apps/orchbee-server` guide](../../apps/orchbee-server/README.md).

## Ownership and exclusions

This crate owns only Client framing, the exchange of one offer or request through the handshake
crate, the connecting adapters, and transfer of the accepted byte stream to its caller. It neither defines nor decodes the next semantic phase.
Server owns listener and Client Connection state. `orchbee-local-peer-credentials` owns kernel
same-user proof. `orchbee-local-worker-protocol` separately owns the Worker handshake. Endpoint
discovery, lazy activation, public `orchbee` integration, Workbench snapshots and commands,
terminal relay, controller policy, persistence, Windows named pipes, remote authentication, and
production service lifecycle remain outside this slice.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-local-client-protocol --all-targets --all-features --locked
cargo clippy -p orchbee-local-client-protocol --all-targets --all-features --locked -- -D warnings
node scripts/tla/run-models.mjs --workspace apps/orchbee
node scripts/tla/run-models.mjs --models local-session-startup
pnpm verify
```

## Windows admission

On Windows, the same offer, request, and OBLC messages run over `orchbee-local-ipc` local named pipes. The adapter proves the peer token user before sending the offer, retains the caller's absolute deadline across connect/read/write, and observes cancellation through a borrowed readiness source. It never starts or retries a Session. Pipe names and permissions belong to the runtime and transport owners; OBCS/OBWP framing is unchanged.

The 10 ms cancellation cadence limits only a pending kernel connection wait. It is not
the authenticated connection's deadline: a pipe that opens and completes peer proof
after that slice, but before the caller's original deadline, proceeds to one offer.
Only the transport's typed preconnection pending result permits another wait. Dial
errors remain `ConnectFailed`; errors after opening a pipe remain `Io`, so neither
authentication failure nor connected EOF can be reinterpreted as preconnect absence.
Cancellation and total expiry after authentication release the candidate before the offer,
without another dial or any renewal of the total budget. Query and Session stop use the
same connection boundary. No request or effect is retried.

`src/connection/windows_admission/tests.rs` controls completion order and clock progress
across slice crossing, pending cancellation, connected failure, late completion and
candidate cleanup. Native handshake and stop tests exercise the real pipe adapter. These
named traces complement the existing protocol models; they do not model or bound
Windows kernel scheduling.
