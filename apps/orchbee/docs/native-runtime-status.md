# Native runtime status

## Authority and grammar

This reference owns Windows `status server` and `status client`, both with
`--format text|json`. Bare `status` keeps its Workbench snapshot contract. Unknown operands,
options and combinations fail before effects. Help, schema and completion stay offline.
The new command runtimes are not implemented on macOS or Linux.

`status client` reads this executable's compiled version, operating system, architecture,
development delivery identity and lossless executable path. It independently validates the sibling
native cohort. A missing, invalid (including an unsupported manifest version), denied, timed-out or unreadable cohort is explicit
unavailable metadata, not an invented identity or a reason to start a Site. The development
delivery has no configured stable/preview update channel; update-channel management remains a
separate capability. This command never contacts the Server.

`status server` probes only the selected implicit local Windows Site. It does not start a Server,
create a runtime directory or lock file, repair a marker, attach a terminal, change focus or create
a Client Session. Its successful response reports facts supplied by the responding same-user
Server, not values copied from the querying Client or inferred from a pipe name.

## Observation model

| Field      | Contract                                                                                                                                                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem    | A script needs running Server/build/protocol facts and must diagnose a different executable cohort before ordinary Client admission would reject it.                                                                           |
| Inputs     | Typed scope, format, local runtime location, current executable/cohort files, one monotonic deadline, cancellation and one untrusted same-user metadata response.                                                              |
| State      | One read attempt with optional admitted rendezvous, candidate stream, complete Server metadata and independent local cohort observation; no persistent operation record.                                                       |
| Outputs    | One bounded localized text answer or versioned JSON document, or one existing typed failure; no partial output.                                                                                                                |
| Invariants | Peer proof before request; one completed connection and one request; no ordinary Client hello, generation allocation, Workbench Session, activation, lock creation, process launch or mutation.                                |
| Decisions  | Missing selected runtime/endpoint before connection can be absence; partial replies, permission errors, busy/deadline, malformed metadata and EOF after connection cannot.                                                     |
| Ownership  | Local Site runtime owns private path admission; local Client protocol owns bounded metadata framing/transport; Server owns its build/cohort/mode/implementation facts; CLI owns comparisons and presentation.                  |
| Lifetime   | Candidate drops after the single reply or any failure, before presentation. Cancellation releases its wait source through the existing emit finalizer.                                                                         |
| Budget     | Five seconds from collection start; connection, partial I/O and settlement never renew it. File hashing checks the remaining budget between bounded reads; synchronous kernel/filesystem calls are cooperative, not preempted. |
| Evidence   | Codec counterexamples, Windows named-pipe peers, Server admission/state tests, CLI process/schema/completion tests and actual staged PowerShell 5.1/7 probes.                                                                  |
| Limits     | An observation at the selected scope, not proof that all Server processes are absent, a global health check or an atomic snapshot of local files and remote process state.                                                     |

| Phase   | Guard and result                                                                                    | Failure/cancellation                                                                                                                                            |
| ------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parse   | Recognized scope and output format; no runtime capabilities used.                                   | Existing usage failure.                                                                                                                                         |
| Locate  | Read runtime location and validate existing private directories.                                    | Missing selected runtime yields not-running; invalid/uninspectable paths remain errors.                                                                         |
| Probe   | Connect once to the admitted endpoint; peer proof precedes the metadata request.                    | Pre-connect endpoint absence yields not-running; busy, rejection and other connection errors retain their meaning.                                              |
| Observe | Decode one complete metadata reply under the original deadline, then close the candidate.           | Wrong kind, malformed payload, EOF, expiry and cancellation fail; never reconnect or fall back to filesystem liveness guesses.                                  |
| Compare | Independently inspect the current local cohort with the remaining budget.                           | Unavailable local metadata leaves cohort compatibility unknown; a known terminal-protocol mismatch still requires restart. Cancellation still cancels the read. |
| Present | Prepare the complete localized text or locale-independent JSON under the existing 8 MiB output cap. | Overflow or write failure uses existing CLI failure/output rules.                                                                                               |

`not-running` means there is no reachable Server through the inspected local scope at that
observation. A runtime directory without an endpoint can be absent even if stale marker files
remain; this command does not remove them. A Server may stop immediately after a successful reply.
Known Server facts survive unavailable optional local cohort metadata. Local and Server observations
are not claimed to be simultaneous. If the Server probe itself exhausts its budget, the result is
timeout, never a fabricated absence or late success.

## Metadata carrier and compatibility

Private OBLC v1 kinds 6/7 carry an empty metadata request and one response inside the unchanged
256-byte payload limit. Existing hello/accept/reject and stop kinds retain their exact bytes.
The response contains an ASCII graphic build-version token of 1–64 bytes, an existing validated
1–64-byte ASCII cohort token, a nonzero process ID, a nonzero OBCS terminal protocol version,
unmanaged/managed-ready/managed-stopping mode, and a 32-bit implementation feature mask.
Malformed lengths, invalid text/version/PID/mode and trailing bytes are refused. Unknown feature
bits are preserved as uninterpreted diagnostic bits, never converted into permission or an
operation. Named known bits cover render-row reads, Client view control, Agent observation and
live handoff; managed stop support comes from the actual managed lifecycle owner.

Only the Windows Server composition answers this new request. Same-user proof and existing
candidate/handshake bounds apply before dispatch. Metadata bypasses cohort equality and ordinary
Client Session admission solely to report diagnostics; it grants no application capability and
cannot enter a subsequent application phase. The Server can answer while ordinary admission is
stopping, and the metadata read never acquires a shutdown record lease or creates a stop request.
Shared codecs remain platform-neutral; this adds no macOS/Linux command runtime.

Terminal protocol compatibility compares the responding version with the compiled Client
version. Cohort compatibility compares the Server's retained startup cohort with a successfully
validated local cohort. A known mismatch requires a matching-cohort restart before use; it does
not prove that the Server executable alone changed. Unavailable comparisons stay null. No updater,
handoff, restart, channel change or named/remote routing is performed.

An older Server rejects the unknown metadata request as a malformed frame or wrong message; the CLI reports incompatible metadata
support, not not-running. Roll out or roll back the complete development cohort together. The new
commands and schemas are additive, bare `status` and `site diagnose` keep their meanings, and no
durable state or migration is introduced.

## Public output and evidence

`orchbee.status.client` and `orchbee.status.server` are v1 documents with local-runtime provenance;
the catalog becomes v46 with 72 schemas and the native inventory has 86 command paths. Required
discriminated metadata fields distinguish observed Server data, absence and unavailable local
cohort information. Paths use the existing lossless Windows path representation. Human strings
and paths are escaped through the common text renderer; JSON bytes do not depend on locale.
No-server and complete diagnostic observations exit 0. Existing usage, denied, incompatible,
protocol/transport, timeout and cancellation failures keep their established codes and envelopes;
timeout exits 7 and cancellation exits 130. Both include the closed detail
`{stage:"runtime-status"}` in the existing v1 failure envelope; older command detail shapes are unchanged.

Required evidence includes maximum/truncated/invalid/unknown-bit payloads and unchanged old-kind
goldens; same-user Windows metadata response, wrong replies, silence, expiry and cancellation;
Server metadata with full Session capacity and while stopping without allocating a generation;
local-cohort mismatch/absence and old-Server rejection; offline Client/help/schema/completion;
and actual matched-cohort PowerShell 5.1/7 probes with all owned Sites stopped afterward.
Review is performed by the implementation author. TLA execution remains deferred, and neither
independent review nor unexecuted formal proof is claimed.
