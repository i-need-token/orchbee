# Private local IPC

Windows byte-stream transport for the native Client, Server, and Worker. This owner
adapts local named pipes to bounded reads, writes, cancellation, and readiness without
changing OBLC, OBWP, OBAP, OBCS, OBCN, or Terminal Host framing. It does not launch
processes, select a runtime, own protocol state, or grant terminal authority.

## Windows transport refinement

The user-authorized Windows native delivery retains the existing process and protocol
owners. Both ends prove the peer process's token user before exchanging protocol bytes.
Listeners reject remote connections, use a protected current-user DACL, and exclude
handle inheritance. A name alone is never authorization. The existing Terminal Host
keeps its independently owned endpoint naming and peer validation.

| Field        | Contract                                                                                                                                                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs       | Validated local pipe name, current token user, nonblocking pipe results, optional monotonic deadline, explicit shutdown                                                                                                                             |
| State        | One shared connection owner, at most one zero-byte read watch, closed flag, caller-selected read/write timeout                                                                                                                                      |
| Output       | Exact ordered bytes, would-block, EOF, timeout, or typed OS error; no protocol retry                                                                                                                                                                |
| Ownership    | Clones share one connection; shutdown closes every clone's transport and signals its read watch, which wakes any poller; final drop closes the handle                                                                                               |
| Invariants   | Readiness consumes no bytes; no pipe call waits on the peer, because reads and writes stay within counted bytes and quota and a zero quota is tested in nonblocking mode; no detached I/O thread; no application bytes before identity verification |
| Transitions  | Connect/accept → identity proof → admission; readiness waits on a zero-byte read; shutdown/final drop → closed                                                                                                                                      |
| Deadline     | A timeout bounds each transport call; protocol owners retain their existing absolute whole-operation deadline across calls                                                                                                                          |
| Dependencies | `interprocess` supplies safe named-pipe bindings, `winsafe` supplies safe process-token inspection and search-handle RAII, and `widestring` encodes descriptors and query names as UTF-16; Windows-only, no async runtime                           |
| Evidence     | Native two-process round trip, first-instance/name rejection, exact byte ordering across readiness probes, wake on peer bytes and closure, full-buffer timeout, shutdown, and handle release                                                        |

Readiness is advisory: writable notification permits a bounded nonblocking attempt,
not a promise that the complete record fits. Pipe writes never count as protocol ACKs.
Close abandons unacknowledged bytes and does not spawn a flush thread; each protocol
owner must retain its existing unknown-outcome behavior. Kernel scheduling is outside
the monotonic budget's real-time guarantee. The existing local submission/relay models
continue to own request identity, at-most-once submission, and feedback semantics.

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

A zero write quota cannot refuse a write by itself, so `write_once` cancels this end's read watch and
tests the write in `PIPE_NOWAIT` mode. `ERROR_PIPE_BUSY` from that mode change proves queued bytes and
maps to would-block. Native tests in `src/windows/tests.rs` wake a waiting poll on peer bytes and on
peer closure, and keep the byte order and the full-buffer timeout across those waits.

`ChildInputWriter::pipe` pairs a nonblocking parent writer with blocking child stdin;
`ChildOutputReader::pipe` provides the reverse direction. The terminal stream relay
uses both without changing inherited user handle modes. The process owner must bound
reads/writes, close pipes and stop/join its child; transport writes never imply consumption.
