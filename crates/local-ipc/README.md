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

| Field        | Contract                                                                                                                                                                                       |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs       | Validated local pipe name, current token user, nonblocking pipe results, optional monotonic deadline, explicit shutdown                                                                        |
| State        | One shared connection owner, at most one prefetched byte, closed flag, caller-selected read/write timeout                                                                                      |
| Output       | Exact ordered bytes, would-block, EOF, timeout, or typed OS error; no protocol retry                                                                                                           |
| Ownership    | Clones share one connection; shutdown closes every clone's transport and wakes bounded polling; final drop closes the handle                                                                   |
| Invariants   | Prefetch is consumed exactly once; no blocking pipe I/O or flush; no detached I/O thread; no application bytes before identity verification                                                    |
| Transitions  | Connect/accept → identity proof → nonblocking admission; readiness may retain one byte; read consumes retained byte before pipe bytes; shutdown/final drop → closed                            |
| Deadline     | A timeout bounds each transport call; protocol owners retain their existing absolute whole-operation deadline across calls                                                                     |
| Dependencies | `interprocess` supplies safe named-pipe bindings, `winsafe` supplies safe process-token inspection, and `widestring` supplies the descriptor's UTF-16 encoding; Windows-only, no async runtime |
| Evidence     | Native two-process round trip, first-instance/name rejection, exact byte ordering across readiness probes, full-buffer timeout, shutdown, and handle release                                   |

Readiness is advisory: writable notification permits a bounded nonblocking attempt,
not a promise that the complete record fits. Pipe writes never count as protocol ACKs.
Close abandons unacknowledged bytes and does not spawn a flush thread; each protocol
owner must retain its existing unknown-outcome behavior. Kernel scheduling is outside
the monotonic budget's real-time guarantee. The existing local submission/relay models
continue to own request identity, at-most-once submission, and feedback semantics.

The direct implementation challenge considered pipe squatting, remote clients, inherited
handles, partial records, read-ahead duplication, blocked writes, and clone cancellation.
It is an explicit adversarial read, not independent review. Native evidence must pass
before this transport is claimed as delivered.

`endpoint_exists_before` observes the Windows pipe namespace without connecting.
It validates the requested name, scans at most 2048 entries and checks the caller's
monotonic deadline before each entry. `Some(true)` means a name was seen;
`Some(false)` requires completed enumeration; `None` means the budget ended before
absence could be established. These are separate-instant name facts, never peer
identity or health. OS enumeration failures stay errors; filesystem stalls remain
outside the cooperative budget. Tests retain an actual listener through inspection
and check deadline/entry-budget exhaustion without manufacturing absence.

`ChildInputWriter::pipe` pairs a nonblocking parent writer with blocking child stdin;
`ChildOutputReader::pipe` provides the reverse direction. The terminal stream relay
uses both without changing inherited user handle modes. The process owner must bound
reads/writes, close pipes and stop/join its child; transport writes never imply consumption.
