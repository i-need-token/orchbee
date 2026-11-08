# Native runtime status

## Authority and grammar

This reference owns Windows `status server` and `status client` reads this executable's compiled version, operating system, architecture and lossless executable path, and independently validates its sibling native cohort. A missing, invalid, denied, timed-out or unreadable cohort is explicit unavailable metadata. With a valid cohort, a missing `release.json` means development delivery; an installed release must have an admitted manifest whose SHA-256 matches the immutable directory name, whose version matches the Client, and whose cohort.json digest binds the independently verified cohort. This yields actual installed channel, version, buildId and manifest identity. Corrupt, moved or inaccessible metadata yields unknown delivery and no guessed channel. The configured channel preference is independent. This read never contacts a Server or updates files.

`status server` probes only the selected implicit local Windows Session with one handshake
`query` request. It does not start a Server, create a runtime directory or lock file, repair a
marker, attach a terminal, change focus or create a Client Connection. Its successful response
lists the processes the responding same-user Server reports, not values copied from the querying
Client or inferred from a pipe name.

## Observation model

| Field      | Contract                                                                                                                                                                                                               |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem    | A script or person needs the running Session's processes and versions, and must learn that the Session is too old or from another environment rather than absent.                                                      |
| Inputs     | Typed scope, format, local runtime location, this build's environment, one monotonic deadline, cancellation and one untrusted same-user `query-result`.                                                                |
| State      | One read attempt with optional admitted rendezvous, candidate stream and complete reply; no persistent operation record.                                                                                               |
| Outputs    | One bounded localized text answer or versioned JSON document, or one existing typed failure; no partial output.                                                                                                        |
| Invariants | Peer proof before request; one completed connection and one request; no Client hello, generation allocation, Workbench Session, activation, lock creation, process launch or mutation.                                 |
| Decisions  | Missing selected runtime/endpoint before connection can be absence; refusals, a reply from a Session that predates the handshake, partial replies, permission errors, busy/deadline, malformed replies and EOF cannot. |
| Ownership  | Local Session runtime owns private path admission; the handshake crate owns the request and reply; the local Client protocol owns the exchange; Server owns its process facts; CLI owns presentation.                  |
| Lifetime   | Candidate drops after the single reply or any failure, before presentation. Cancellation releases its wait source through the existing emit finalizer.                                                                 |
| Budget     | Five seconds from collection start, and two seconds for the reply; connection, partial I/O and settlement never renew either.                                                                                          |
| Evidence   | Handshake codec tests, Server query and admission tests, Windows named-pipe peers, CLI process/schema/completion tests and staged probes.                                                                              |
| Limits     | An observation at the selected scope, not proof that all Server processes are absent, a global health check or an atomic snapshot of process state.                                                                    |

| Phase   | Guard and result                                                                                    | Failure/cancellation                                                                                                          |
| ------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Parse   | Recognized scope and output format; no runtime capabilities used.                                   | Existing usage failure.                                                                                                       |
| Locate  | Read runtime location and validate existing private directories.                                    | Missing selected runtime yields not-running; invalid/uninspectable paths remain errors.                                       |
| Probe   | Connect once to the admitted endpoint; peer proof precedes the `query` request.                     | Pre-connect endpoint absence yields not-running; busy, refusal and other connection errors retain their meaning.              |
| Observe | Decode one complete `query-result` under the original deadline, then close the candidate.           | Wrong reply, malformed frame, EOF, expiry and cancellation fail; never reconnect or fall back to filesystem liveness guesses. |
| Present | Prepare the complete localized text or locale-independent JSON under the existing 8 MiB output cap. | Overflow or write failure uses existing CLI failure/output rules.                                                             |

`not-running` means there is no reachable Server through the inspected local scope at that
observation. A runtime directory without an endpoint can be absent even if stale marker files
remain; this command does not remove them. A Server may stop immediately after a successful reply.
If the Server probe itself exhausts its budget, the result is timeout, never a fabricated absence
or late success.

## Query and compatibility

The request is the handshake's `query`, carrying this build's environment; the reply lists the
Server first, then the Worker it started, each with role, product version, build, environment, PID,
state, terminal count and connections, plus whether Restart Session keeps terminals. Hosts join the
reply once the Worker reports them. The [local Client protocol](../../../crates/local-client-protocol/README.md#process-query)
owns the exchange on every platform, and the [Server guide](../../orchbee-server/README.md) owns
the reply. The Server answers before Client admission, after a `drain` (reporting `draining`), and
while a Session stop or an `exit` is underway (reporting `stopping`); the request grants no
capability and cannot enter a Client phase.

A Session in another environment refuses the request: the CLI reports `incompatible` with reason
`environment-mismatch` (exit 6). A Session older than the handshake answers with its old OBLC
rejection, which the CLI reports as `incompatible` with reason `session-too-old` and no Session
version. Neither is reported as not-running. Version compatibility for joining a Session is decided
by the handshake when a Client offers, not by this read, and no updater, restart, channel change or
named/remote routing is performed.

## Public output and evidence

`orchbee.status.client` emits v1 with a closed installation observation and a bounded I/O
diagnostic on unavailable cohort metadata. The diagnostic
contains `ioKind` from the closed lowercase vocabulary `not-found`, `permission-denied`,
`invalid-input`, `invalid-data`, `timed-out`, `unsupported`, `interrupted`, `unexpected-eof`,
`would-block`, `out-of-memory`, and `other`; unmapped or future I/O kinds use `other`. It also
contains signed 32-bit `osError` or null, and the exact missing executable name only when opening
that declared sibling returned `NotFound`. Text output omits these raw diagnostic fields.
`orchbee.status.server` stays v1 under the pre-release rule: a running Server's `server` member
holds `restartKeepsTerminals` and `processes`, each process with `role` (`server`, `worker`,
`terminal-host` or `unknown`), `version`, `processId`, `state` (`ready`, `draining`, `stopping` or
`unknown`) and `terminals`, beside the querying Client's `clientVersion`. Text lists one line per
process, labelled by role. Both use local-runtime
provenance; the schema catalog is version 1, which also includes the Windows Agent projection contracts. Required
discriminated fields distinguish observed processes, absence and unavailable local installation
information. Paths use the existing lossless Windows path representation. Human strings
and paths are escaped through the common text renderer; JSON bytes do not depend on locale.
No-server and complete diagnostic observations exit 0. Existing usage, denied, incompatible,
protocol/transport, timeout and cancellation failures keep their established codes and envelopes;
timeout exits 7 and cancellation exits 130. Both include the closed detail
`{stage:"runtime-status"}` in the existing v1 failure envelope; older command detail shapes are unchanged.

Required evidence includes the reply's process order, versions and unknown role or state; the
Windows named-pipe request with environment refusal, an old Session's rejection, wrong replies,
silence, expiry and cancellation; Server query with full Session capacity and while stopping without
allocating a generation; and offline Client/help/schema/completion.
Review is performed by the implementation author. This workflow uses implementation tests, and neither
independent review nor unexecuted formal proof is claimed.

The Server status scope accepts both the default `orchbee-native-` identity
(32 hex digits) and the named Windows `orchbee-session-` identity (47 hex digits),
as produced by `SessionName::scope_for_user`. This observation does not
activate a missing named Session. The version-1 schema admits both existing forms.
