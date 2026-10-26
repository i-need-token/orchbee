# Local Client Protocol

## Read-only Windows Server metadata

OBLC v1 kinds 6/7 carry one metadata request/response under the unchanged 256-byte payload bound.
The request is just kind 6. Kind 7 is: version length (`u8`), 1–64 graphic ASCII version bytes,
cohort length (`u8`), the existing validated ASCII cohort bytes, nonzero PID (`u32` big-endian),
nonzero OBCS terminal version (`u8`), mode (`0` unmanaged, `1` managed-ready, `2` managed-stopping),
then diagnostic feature bits (`u32` big-endian). Known bits are render-row reads (1), Client view
control (2), Agent observation (4) and live handoff (8); unknown bits remain uninterpreted data.
Invalid lengths/values/modes and trailing bytes fail decoding. Old message encodings are unchanged.

The Windows deadline adapter verifies the peer through the existing named-pipe transport, writes
one request, reads one exact response and drops the candidate. It never sends a Client hello or
enters an application phase. The caller owns the five-second total and cancellation arbitration;
partial progress never extends its deadline. Only connection failures preserve pre-connect absence
provenance; EOF after connection is never absence. The Windows Server answers after same-user
proof and before cohort equality/Client Connection admission, under existing candidate and handshake bounds.
This observation grants no operation capability, creates no Client Connection and changes no shutdown state.
No Linux/macOS runtime adapter is added. The [CLI status contract](../../apps/orchbee/docs/native-runtime-status.md)
owns public comparison/output semantics and evidence, including old-Server incompatibility.

## Managed Session stop control

Private OBLC v1 additionally carries kinds 4/5 for same-user, exact-cohort Session stop requests and
status responses, within the existing 256-byte bound. A request contains a validated cohort, nonzero
128-bit stop identity and closed start/status/explicit-retry action. Responses echo that identity and
distinguish not-recorded, pending, failed, another active identity and unmanaged mode. Pending/failed
states retain a positive attempt (at most 64), a closed phase, and failures retain their reason and
known disposal count for that attempt. A response never claims final process exit: the admitted
Windows activation record is the completion receipt after the managed owner settles its resources.
Duplicate start never retries effects; only explicit retry of the same failed identity may advance
the attempt. New Client hellos during shutdown receive the typed stopping rejection. Unknown tags,
zero identities, invalid attempts and trailing payload bytes fail decoding. The codec itself neither
starts nor stops a process and does not introduce a Linux or macOS shutdown runtime.

Private, experimental Rust connection contract and concrete connecting adapter for the first
same-user POSIX Human Client-to-Server process spine. It is not a public API, Worker protocol,
terminal relay, TUI, activation mechanism, remote protocol, or compatibility promise.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Framing and messages

Each connection carries exactly one handshake frame from Client to Server and one response frame
from Server to Client. Acceptance ends the `OBLC` phase without consuming bytes from the next
protocol phase; the same stream may then remain only as the Client Connection liveness attachment or
be used by its caller for a separately owned semantic protocol. An `OBLC` frame contains fixed
magic, an unsigned big-endian protocol version, an unsigned big-endian payload length, and at most
256 payload bytes. Unknown magic, version, kind, enum value, missing or extra bytes, oversized
payload, zero generation, invalid UTF-8, empty cohort, non-ASCII cohort, or a cohort over 64 bytes
is rejected.

| Message          | Fields and meaning                                                                                                                             |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `ClientHello`    | Exact executable cohort. Kernel peer credentials provide local human identity; renderer form and Client-local view state are absent.           |
| `ClientAccepted` | One nonzero process-local Server Client Connection generation assigned to this exact transport. It grants no terminal or Workbench capability. |
| `Rejected`       | Incompatible protocol/cohort, capacity exceeded, wrong message, malformed frame, or timeout. Rejection changes no admitted Client Connection.  |

Human Client and Worker contracts remain separate. This crate contains no Worker identity,
Worker capability advertisement, lease, fence, Host, terminal, renderer, Workspace, Tab, Panel, or
semantic-command field.

## Connecting adapter

`ClientConnection::connect` is the real Rust Client-side consumer. It validates the cohort
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
render a TUI, or supervise Server lifetime. Bare Linux and macOS `orchbee` consume the deadline-aware adapter; help and version remain effect-free.

## Deadline-bounded POSIX admission

The accepted lazy-activation workflow needs a caller-owned deadline across connection and
handshake. POSIX ClientConnection::connect_before(endpoint, cohort, deadline) supplies that
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
| Inputs       | Caller-selected endpoint and absolute monotonic Instant; cohort validated before effects; untrusted kernel connect/readiness/peer observations and one OBLC response.                                                                                                                                                        |
| Data         | One owned nonblocking socket, one unchanged deadline, one hello, decoder progress and typed result. No socket clone, process, lock, retry ledger or product state.                                                                                                                                                           |
| Outputs      | Exact accepted generation and the same blocking, unbuffered stream; existing typed IO/protocol/rejection error. Expiry carries TimedOut. Failure drops only the candidate connection.                                                                                                                                        |
| Invariants   | One connect syscall; hello follows same-effective-user proof; partial progress never renews time; acceptance requires exact response, successful blocking restoration and a final now < deadline; failure releases the candidate.                                                                                            |
| Decisions    | Invalid cohort precedes effects. An expired valid request never opens a socket. Only EINPROGRESS waits for connection completion; readiness requires zero SO_ERROR. Backlog EAGAIN, other connect errors, wrong peer, IO errors and invalid responses fail without reconnecting.                                             |
| Model        | One deadline-bound admission attempt moves through connect, peer proof, partial hello/response I/O and blocking restoration. Controlled implementation tests inject completion and expiry at each boundary.                                                                                                                  |
| Transitions  | Validated request → connecting → peer proof → writing → reading → decoded response → blocking restoration → accepted. Invalid input, IO, peer or response failure, or observed expiry ends in failed and releases the candidate.                                                                                             |
| Dependencies | POSIX atomic SOCK_CLOEXEC/SOCK_NONBLOCK, existing pinned Nix socket/poll wrappers, kernel peer credentials, monotonic clock and the unchanged OBLC codec.                                                                                                                                                                    |
| Ownership    | This crate owns admission and candidate cleanup; the caller owns deadline selection and the accepted stream. Server owns its Client Connection and observes connection loss independently.                                                                                                                                   |
| Verification | Deterministic partial-I/O and deadline-boundary tests, Linux and macOS Unix-socket acceptance/rejection/silence/EOF/backlog cases, next-phase byte preservation and existing connect regressions.                                                                                                                            |
| Correctness  | AD-01–AD-04 map to the concrete adapter witnesses below. Local deadline observation provides bounded failure without assuming external connect/readiness/response progress. No separate admission TLA model is retained.                                                                                                     |
| Limits       | One POSIX admission, not service readiness, activation, OS process scheduling or Server cleanup timing. Kernel calls and scheduling cannot be preempted by this adapter; it checks the deadline at controllable boundaries and before success. Windows uses its named-pipe adapter. No production timeout value is selected. |

| Claim                                                                                           | Implementation and executable evidence                                                                         |
| ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| AD-01: invalid or already expired admission has no socket effect; one attempt never reconnects. | connect_before; invalid/expired and Linux/macOS backlog tests.                                                 |
| AD-02: peer proof precedes one hello and one exact, unbuffered response.                        | Existing credential boundary and codec; same-stream OBCS and coalesced next-phase tests.                       |
| AD-03: one total deadline covers partial progress and final acceptance.                         | Deadline-aware poll/read/write and final checks; stepping-clock write/read and final-boundary tests.           |
| AD-04: failure closes only the candidate; success returns the accepted blocking stream.         | Silent-Server EOF, typed negative responses and blocking-mode tests; existing callers retain stream ownership. |

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

Server assigns every accepted connection an independent generation and admits up to thirty-two active
Client Connections. It reserves at most forty-eight total Client candidate/connection threads, so silent
handshakes cannot create unbounded work; a connection beyond that transport bound is closed. A
candidate within the transport bound receives a typed capacity rejection after the thirty-two-connection
admission limit. Every handshake has a two-second read/write timeout.

The delivered private Server retains the accepted unbuffered stream as the Client Connection carrier and
passes its next-phase bytes to the separately owned
[`orchbee-client-server-protocol`](../client-server-protocol/README.md) OBCS decoder. This crate
neither defines nor decodes that phase. The Server's local subject policy, fault handling, and
generation-scoped cleanup are owned by its
[`apps/orchbee-server` guide](../../apps/orchbee-server/README.md).

## Ownership and exclusions

This crate owns only Client framing, handshake validation, the connecting adapter, and transfer of
the accepted byte stream to its caller. It neither defines nor decodes the next semantic phase.
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
node scripts/tla/run-models.mjs --models local-site-startup
pnpm verify
```

## Windows admission

On Windows, the same OBLC messages run over `orchbee-local-ipc` local named pipes. The adapter proves the peer token user before sending the hello, retains the caller's absolute deadline across connect/read/write, and observes cancellation through a borrowed readiness source. It never starts or retries a Session. Pipe names and permissions belong to the runtime and transport owners; OBCS/OBWP framing is unchanged.

The 10 ms cancellation cadence limits only a pending kernel connection wait. It is not
the authenticated connection's deadline: a pipe that opens and completes peer proof
after that slice, but before the caller's original deadline, proceeds to one hello.
Only the transport's typed preconnection pending result permits another wait. Dial
errors remain `ConnectFailed`; errors after opening a pipe remain `Io`, so neither
authentication failure nor connected EOF can be reinterpreted as preconnect absence.
Cancellation and total expiry after authentication release the candidate before hello,
without another dial or any renewal of the total budget. Metadata and Session-stop use the
same connection boundary. No request or effect is retried.

`src/connection/windows_admission/tests.rs` controls completion order and clock progress
across slice crossing, pending cancellation, connected failure, late completion and
candidate cleanup. Native metadata/stop tests exercise the real pipe adapter. These
named traces complement the existing protocol models; they do not model or bound
Windows kernel scheduling.
