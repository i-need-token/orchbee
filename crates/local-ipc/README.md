# Private local IPC

Windows byte-stream transport for the native Client, Server, and Worker. This owner
adapts local named pipes to bounded reads, writes, cancellation, and readiness without
changing OBLC, OBWP, OBAP, OBCS, OBCN, or Terminal Host framing. It does not launch
processes, select a runtime, own protocol state, or grant terminal authority.

## Agent observation channel withdrawn

Windows withdrew the Agent observation channel: its Node adapters open through `node:net`, which
sets no security quality of service, so the identification limit below never covered them, and a
corrected adapter could not reach installed copies without rewriting Agent configuration. People
who enabled an integration earlier lose its hook-reported resume and status on Windows. Bringing the
channel back needs an adapter client that opens at identification level and a way to deliver it
that writes into no Agent configuration. The
[writer crate's README](../agent-integration-writer/README.md#windows-observation-channel-withdrawn)
records the decision, what earlier enablers lose, and the second defect a revival must close.

## Windows transport refinement

The user-authorized Windows native delivery retains the existing process and protocol
owners. Both ends prove the peer's token user before exchanging protocol bytes: a client
reads its server process's token, and a listener its client process's token, or the
token the client opened with for a listener bound with `bind_admitting_elevated`.
Listeners reject remote connections, use a protected current-user DACL, and exclude
handle inheritance. A name alone is never authorization. The existing Terminal Host
keeps its independently owned endpoint naming and peer validation, and labels its pipe
and mutex by the rule below.

A listener whose creator runs at High integrity or above, as an elevated administrator or
a service does, also carries the mandatory label `S:(ML;;NWNR;;;HI)`: no process below High
can open it for reading or writing, the same user's standard processes included. Without
it, a standard process could open an elevated Session's pipe and pass that Server's peer
check, because the elevated side may open a lower peer. A creator below High keeps the
system's default label for its own level; the level is that of the token the binding
thread acts with, its process's unless it impersonates. `runs_at_high_integrity` reads the
token's level through `winsafe`, so a Client can tell that the refusal it met came from a
Session started as administrator.

The reverse direction depends on the bind. `LocalListener::bind` proves a client by
opening its process and that process's token, and an elevated process's access list grants
only Administrators and SYSTEM, which a standard token holds for deny only, so a listener
bound by a standard process refuses the same user's elevated clients; Workers bind this way.
`LocalListener::bind_admitting_elevated` keeps the same descriptor and label
but proves a client by the token it opened the pipe with: the accepting thread calls
`ImpersonateNamedPipeClient`, reads the user through the effective-token pseudo handle,
which opens no object and so meets no access check, and reverts before the comparison. The
client's identification-level open guarantees that the thread can only query that token.
Such a listener admits this user's clients at every level its label lets in, an elevated
client of a standard listener among them, and still refuses another user's. `require_same_user`
proves the peer again the way admission did. A Session's Client endpoint binds like the machine
endpoints, with `bind_admitting_elevated`, so a standard Session admits its own user's
administrator Clients, such as a Win32-OpenSSH shell, which an administrator account receives
elevated. A Session started as administrator keeps its High label, so standard Clients are still
refused at the open.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                    |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs       | Validated local pipe name, current token user and integrity level, nonblocking pipe results, optional monotonic deadline, explicit shutdown                                                                                                                                                                                                                                 |
| State        | One shared connection owner, at most one zero-byte read watch, closed flag, caller-selected read/write timeout                                                                                                                                                                                                                                                              |
| Output       | Exact ordered bytes, would-block, EOF, timeout, or typed OS error; no protocol retry                                                                                                                                                                                                                                                                                        |
| Ownership    | Clones share one connection; shutdown closes every clone's transport and signals its read watch, which wakes any poller; final drop closes the handle                                                                                                                                                                                                                       |
| Invariants   | Readiness consumes no bytes; no pipe call waits on the peer, because reads and writes stay within counted bytes and quota and a zero quota is tested in nonblocking mode; no detached I/O thread; no application bytes before identity verification                                                                                                                         |
| Transitions  | Connect/accept → identity proof → admission; readiness waits on a zero-byte read; shutdown/final drop → closed                                                                                                                                                                                                                                                              |
| Deadline     | A timeout bounds each transport call; protocol owners retain their existing absolute whole-operation deadline across calls                                                                                                                                                                                                                                                  |
| Dependencies | `interprocess` supplies Windows named-pipe listener binding, accepts, and `SecurityDescriptor`; the client end owns its overlapped `OwnedHandle` directly via `std` + `windows_native`; `winsafe` supplies process-token and search-handle RAII; `widestring` encodes descriptors and query names as UTF-16                                                                 |
| Evidence     | Native two-process round trip, first-instance/name rejection, exact byte ordering across readiness probes, wake on peer bytes and closure, full-buffer timeout, shutdown, and handle release; admission of a same-user client whose process the listener cannot open, of a higher client by a lower listener, and refusal of a lower client and of another user at the open |

Readiness is advisory: writable notification permits a bounded nonblocking attempt,
not a promise that the complete record fits. Pipe writes never count as protocol ACKs.
Close abandons unacknowledged bytes and does not spawn a flush thread; each protocol
owner must retain its existing unknown-outcome behavior. Kernel scheduling is outside
the monotonic budget's real-time guarantee. The existing local submission/relay models
continue to own request identity, at-most-once submission, and feedback semantics.

Every Rust and native client open passes `SECURITY_SQOS_PRESENT` with `SECURITY_IDENTIFICATION`, so a
server that squats the endpoint's name receives a thread token at identification level: it can read
the client's token user, as the Terminal Host's same-user check does, but it cannot impersonate the
client. Anonymous would stop that token query, so identification is the lowest level that keeps
every server check working. The Node observation adapter opens through `node:net`, which sets no
SQOS flag, which is why Windows [withdrew its channel](#agent-observation-channel-withdrawn).
`interprocess`'s built-in connect offers no SQOS
flag parameter and its `PipeStream::try_from(OwnedHandle)` reopens the pipe via `ReOpenFile`
(opening a second client connection whenever the listener already holds its next idle instance), so
`open_identified` opens with `std`'s `OpenOptions` and
`FILE_FLAG_OVERLAPPED` and wraps the handle directly in `windows_native::ClientPipe` for
overlapped I/O without reopening. The native test `client_open_limits_a_server_impersonation_to_identification`
confirms the identification level, and `parallel_client_connections_while_listener_holds_next_instance_never_break`
proves concurrent clients never trigger phantom connections or broken pipes. A busy endpoint waits
in `WaitNamedPipeW` through `windows_native::wait_for_instance` within the caller's budget. The
same-user proof still runs after the open, and the reader that `ChildInputWriter::pipe` gives a
child is opened with the same SQOS flags.

`LocalStream::connect_before_with_wait_limit` separates one kernel connection wait from
the caller's absolute admission deadline. `None` means only busy/wait exhaustion before
a pipe was opened. An opened pipe must complete the same-user proof and nonblocking
setup before the original deadline; crossing the shorter wait limit does not discard it.
`ConnectAttemptError::Dial` preserves preconnection errors, while `Admission` preserves
postconnection errors, including OS codes also used for wait timeouts. Authentication
failure or total expiry drops the candidate and never authorizes another dial. The
protocol owner checks cancellation between pending waits and before sending bytes;
kernel authentication calls remain outside a preemptive timing guarantee.

Controlled-clock tests in `src/windows/connection/tests.rs` cover slice crossing, true
expiry and cleanup, pending classification and error provenance. The adjacent native
transport tests exercise a real short-wait connection and same-user process round trip.
These are implementation traces; the existing shared models do not abstract kernel
authentication scheduling or prove its latency.

`LocalStream::server_process_id` reports the process the kernel names as the server of a
client connection's pipe, read from the connected handle rather than looked up by name. A
caller that knows which process must serve an endpoint compares it before writing any
protocol byte, as a replacement Worker does for the Terminal Host it adopts; the same-user
proof still runs first.

The direct implementation challenge considered pipe squatting, remote clients, inherited
handles, partial records, read-ahead duplication, blocked writes, and clone cancellation.
It is an explicit adversarial read, not independent review. Native evidence must pass
before this transport is claimed as delivered.

`endpoint_exists` observes one validated local pipe name with an exact `FindFirstFile`
query, without connecting. Wildcards and remote names are rejected; the extended
namespace prefix preserves literal names, including trailing periods. It does not walk
the live list of all pipes: unrelated creation/deletion during that enumeration can
skip a listener that remains open. `endpoint_exists_before` checks the caller's
monotonic deadline before and after the same single query. `Some(true)` means the name
was seen; `Some(false)` means that exact query found no match; `None` means the deadline
expired. These are separate-instant name facts, never peer identity or health. OS
query failures preserve their error codes; kernel stalls remain outside the cooperative
budget. Native tests retain listeners while unrelated pipes churn, verify no connection
is consumed, and cover case-insensitive names, busy listeners, invalid names and expiry.

The private `src/windows_native.rs` adapter is this crate's only authored unsafe
boundary, admitted by the [Rust policy](../../docs/RUST.md#lint-policy-and-enforcement).
For the name query it owns a NUL-terminated UTF-16 name without interior NULs and a correctly aligned
`WIN32_FIND_DATAW` buffer through `FindFirstFileW` and immediate `GetLastError` capture.
An `INVALID_HANDLE_VALUE` result is classified before any buffer destruction or handle
cleanup: only `ERROR_FILE_NOT_FOUND` (2) and the pipe namespace's `ERROR_NO_MORE_FILES`
(18) mean absence; every other failure retains its raw OS code. A successful search
handle moves directly into one `FindCloseGuard` and is closed once; invalid handles
are never wrapped or closed. No handle escapes the adapter, and the query never opens
a pipe connection. The adjacent safe native regression uses an executable file as a
parent directory and requires `ERROR_DIRECTORY` (267), proving errors cannot become
absence without changing public pipe-name admission. It also checks a missing sibling
in that existing directory; the pipe tests cover namespace absence.

The same adapter owns pipe readiness, so `poll` blocks on kernel events instead of a timer:

- **Counters.** `NtQueryInformationFile` receives a pipe handle borrowed for the call and writable,
  correctly aligned out values, and retains neither. A disconnected status is a closed pipe, not an
  error. The write quota is advisory, because a peer's pending read reduces it while the pipe is empty.
- **Read watch.** `ReadWatch` owns a duplicate of the pipe handle, a manual-reset event and pinned
  boxed `OVERLAPPED` storage, and starts at most one zero-byte `ReadFile`. The kernel may use that
  storage only while `pending` is set. `Drop` cancels with `CancelIoEx` and then blocks in
  `GetOverlappedResult` until the kernel has released it, before anything is freed. The event is reset
  only with no operation outstanding, so a completion cannot be lost; a zero length never writes
  through the buffer. A waiter holds the event through an `Arc`, and `Drop` signals it so a waiter on
  another clone observes the closure.
- **Wait.** `wait_any` admits at most 63 events its caller keeps alive for the call, plus one
  high-resolution waitable timer owned by the waiting thread, because every other timed wait rounds up to
  the system tick. A thread without that timer falls back to a timed wait. Nothing retains a pointer,
  and callers reprobe after every return: a wake is advisory.

The same adapter also owns client-side pipe I/O, the busy-endpoint wait, the pipe-client
impersonation, and test-only tokens:

- **Client pipe I/O.** `open_identified` opens the client handle once, with `FILE_FLAG_OVERLAPPED`, and
  `ClientPipe` keeps that handle; it is never reopened. Each `ClientPipe` owns one manual-reset event that
  no other operation shares, and the event is created before the pipe opens. `&mut self` under the
  connection lock admits one transfer at a time. Each transfer's `OVERLAPPED` is boxed and owned by its
  `OverlappedGuard`, which is bound to the pipe and event borrows and lives within the caller's buffer
  borrow. A pending operation is waited to completion before `finish` returns; if the transfer unwinds
  first, the guard cancels it with `CancelIoEx` and then waits again. A start that fails with any error
  other than `ERROR_IO_PENDING` leaves `pending` clear, so the kernel holds no `OVERLAPPED` and nothing is
  waited for.
- **Busy wait.** `wait_for_instance` passes a NUL-terminated UTF-16 name, which outlives the call, to
  `WaitNamedPipeW` and keeps no pointer. The timeout is clamped to `1..=u32::MAX - 1` milliseconds, so the
  system default (zero) and an infinite wait are never requested. The wake is advisory, and the caller
  reopens the pipe.
- **Pipe-client impersonation.** `as_pipe_client` runs one closure while the thread acts as the
  client of a borrowed, live server-side pipe, then reverts through a guard that also runs on unwind;
  a failed `RevertToSelf` aborts the process, because a thread left acting as the client would go on
  doing so. The client's identification-level open means the closure can query the thread token but
  not act as the client. The test-only `impersonated_client_level` reads the level it grants.
- **Test tokens.** Test builds only. `lowered_token` duplicates this process's token for
  impersonation and labels the copy with a lower integrity level; `as_token` and `as_anonymous` run a
  closure while the thread acts as that copy or as the anonymous logon, and revert through the same
  guard. They stand in for a process of another level or another user, which a test cannot start
  without administrator rights.

A zero write quota cannot refuse a write by itself, so `write_once` cancels this end's read watch and
tests the write in `PIPE_NOWAIT` mode. `ERROR_PIPE_BUSY` from that mode change proves queued bytes and
maps to would-block. Native tests in `src/windows/tests.rs` wake a waiting poll on peer bytes and on
peer closure, and keep the byte order and the full-buffer timeout across those waits.

`ChildInputWriter::pipe` pairs a nonblocking parent writer with blocking child stdin;
`ChildOutputReader::pipe` provides the reverse direction. The terminal stream relay
uses both without changing inherited user handle modes. The process owner must bound
reads/writes, close pipes and stop/join its child; transport writes never imply consumption.

On Linux and macOS, `ChildOutputReader::pipe` alone is available: an anonymous pipe, as a shell
pipeline gives a child, so a child that opens `/dev/stderr` or checks its output's type behaves as
it does there; a socket would refuse that open on Linux. Only the parent's read end is nonblocking, with the same read and
would-block contract; the child's write end stays blocking. The standard library opens both ends
close-on-exec, so only the child's standard stream inherits the writer. rustix supplies the
nonblocking switch the standard library lacks for a pipe.
