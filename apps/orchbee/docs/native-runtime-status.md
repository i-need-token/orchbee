# Native runtime status and doctor

## Authority and grammar

This reference owns `status`, which reads the running Session, and `doctor`, which checks whether
Orchbee works on this computer, with the two collectors both reuse: `status client` reads this
executable and its installation files, and `status server` asks the selected Session for its
processes. `status client` and `status server` are unlisted: help, completion and the Agent guide
never name them, but they stay accepted and answer as described here, because `machine add` runs
them on a remote computer that may have an older build (see [Machines](machines.md)). `doctor` is
the command a person runs.

| Path                           | Needs a Session | Contacts                                              | Schema                  |
| ------------------------------ | --------------- | ----------------------------------------------------- | ----------------------- |
| `status [--format text\|json]` | yes             | one Workbench snapshot and one handshake `query`      | `orchbee.status`        |
| `doctor [--format text\|json]` | no              | the selected Session's handshake `query` when present | `orchbee.doctor`        |
| `status client [--format ...]` | no              | nothing                                               | `orchbee.status.client` |
| `status server [--format ...]` | no              | the selected Session's handshake `query` when present | `orchbee.status.server` |

None of them starts a Server, creates a runtime directory or lock file, repairs a marker, attaches
a terminal, changes focus, creates a Client Connection or writes a file. `doctor` and the two
collectors are local reads with local-runtime provenance and are not routed with `--machine`.

## `status`

Bare `status` reads the Workbench snapshot, then sends the handshake `query` under the snapshot's
network deadline. Text lists the Session state, the Client version, one line per process labelled
by role, whether Restart Session keeps terminals, and the Workbench counts. `orchbee.status` carries
the processes in the required `result.server` member with `restartKeepsTerminals` and `processes`
as described under [Query and compatibility](#query-and-compatibility). With no running Session the
command fails with the usual unavailable failure (exit 4); a refused handshake is exit 6; a query
that fails after the snapshot fails the command with that failure, never a partial document. Where
the platform cannot ask a Session for its processes, `result.server` is `{"kind":"unsupported"}`.

## `doctor`

`doctor` runs six fixed checks in order. Each folds the facts one existing collector gathers into a
status, a closed reason and those facts unchanged; each collector keeps its own bound: five seconds,
and for the Session query five seconds with two for the reply.

| Key            | Label (en-US / zh-CN)       | Collector                                                                                                   | Result                                                                                                                                                                      |
| -------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `installation` | Installation / 安装         | `status client`                                                                                             | fail: installation files `missing`, `invalid`, `denied`, `timed-out` or `io-error`; warning `delivery-unknown`: the installed release could not be verified                 |
| `settings`     | Settings / 设置             | the `user-config` row of `config check`                                                                     | that row's status; the reason is the failure's, or the first diagnostic's for a warning                                                                                     |
| `records`      | Saved data / 已保存数据     | the `records` row of `config check`                                                                         | warning with the first record's problem (`newer-format` or `not-updated`)                                                                                                   |
| `runtime`      | Runtime folder / 运行时目录 | the runtime folder report with the `runtime-base`, `runtime-directory` and `sockets` rows of `config check` | fail: an unsafe or unreadable path (`absent` explicit base, `invalid-path`, `io-error`, `unsafe`, `address-too-long`); warning: the first advisory while no Session answers |
| `session`      | Session / 会话              | `status server`                                                                                             | pass: running; warning: the Server `draining` or `stopping`; not-applicable: not running; fail: the refusal's reason, `timed-out`, or the failure kind                      |

The runtime
advisories (`activation-recorded`, `socket-residue`, `pipe-name-present`, `unrecognized-records`)
describe what a running Session also leaves, so they warn only when the Session check is
not-applicable. `recovery-blocked` leads them when the Session's record names a registered process
whose exact identity is still alive, read by the observation `session list` uses for `recoverable`,
with no Server contacted: the runtime details then carry `blockingProcess` (`role`, `program`, the
program's file name, and `processId`), and the text line is the sentence a refused start shows. A
check whose collector the platform lacks is `{"kind":"unsupported"}` in JSON and has no text line;
on Linux that is `installation` and `session`.

Text starts with the verdict, then one line per check: the status word (`ok`, `warn`, `fail`, or
`-` for not-applicable; zh-CN `通过`, `警告`, `失败`, `-`), the label, and one fact. A warning or a
failure shows its reason instead of the fact and may add one indented line with the command to run.
The last line points to JSON:

```text
Orchbee OK: all 5 checks passed.
ok    Installation        0.1.0-beta.1 · stable · C:\Users\me\AppData\Local\Programs\Orchbee\orchbee.exe
ok    Settings            C:\Users\me\.orchbee\config.toml
ok    Saved data
ok    Runtime folder      C:\Users\me\.orchbee\internal\runtime\default
ok    Session             running · 0.1.0-beta.1
Details: orchbee doctor --format json
```

The verdicts are "Orchbee OK: all N checks passed.", "Orchbee usable with N warning(s)." and
"Orchbee has problems: N check(s) failed." (zh-CN "Orchbee 正常：N 项检查全部通过。", "Orchbee 可用，有 N 项警告。"
and "Orchbee 有问题：N 项检查失败。"), counting the checks this platform makes. A settings warning or
failure adds `orchbee config check`.

`orchbee.doctor` v1 is `{schema, version, source:{kind:"local-runtime"}, result:{ok, checks}}`.
`checks` has the five keys, each `{status, reason, details}` or `{"kind":"unsupported"}`; `status`
is `pass`, `warning`, `fail` or `not-applicable`; `reason` is a closed kebab-case token, null for a
pass or a not-applicable Session; `details` are the collector's existing shapes moved in unchanged:
the `orchbee.status.client` result, the `user-config` and `records` row details of
`orchbee.config.check`, the runtime folder report, the `orchbee.status.server` result (or, for a
failed Session check, the failure's details object). `ok` is false only when a check failed.

Exit codes: 0 when no check failed, warnings and a Session that is not running included; 1 when a
check failed, with the document still on standard output and one line on standard error, as
`config check` does; 2 for usage; 130 when interrupted. A Session that is too old or of another
environment is a failed check (exit 1), not exit 6: `doctor` completed its own read.

## `status client`

`status client` reads this executable's compiled version, operating system, architecture and
lossless executable path, and independently validates its sibling native cohort. A missing,
invalid, denied, timed-out or unreadable cohort is explicit unavailable metadata. With a valid
cohort, a missing `release.json` means development delivery; an installed release must have an
admitted manifest whose SHA-256 matches the immutable directory name, whose version matches the
Client, and whose `cohort.json` digest binds the independently verified cohort. This yields the
installed channel, version, buildId and manifest identity. Corrupt, moved or inaccessible metadata
yields unknown delivery and no guessed channel. The configured channel preference is independent.

Unavailable cohort metadata carries a bounded I/O diagnostic: `ioKind` from the closed lowercase
vocabulary `not-found`, `permission-denied`, `invalid-input`, `invalid-data`, `timed-out`,
`unsupported`, `interrupted`, `unexpected-eof`, `would-block`, `out-of-memory` and `other` (unmapped
or future kinds use `other`), signed 32-bit `osError` or null, and the exact missing executable name
only when opening that declared sibling returned `NotFound`. Text omits these raw fields.

## `status server`

`status server` probes only the selected implicit local Session with one handshake `query`. Its
successful response lists the processes the responding same-user Server reports, not values copied
from the querying Client or inferred from a pipe name. `orchbee.status.server` holds the scope,
runtime directory, endpoint, the querying Client's `clientVersion`, and `server`: `not-running`
with reason `runtime-absent` or `endpoint-absent`, or `running` with `restartKeepsTerminals` and
`processes`. On Windows the scope is the Session name, the name of its runtime folder, and the read
does not activate a missing named Session.

| Field      | Contract                                                                                                                                                                                                               |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inputs     | Typed scope, format, local runtime location, this build's environment, one monotonic deadline, cancellation and one untrusted same-user `query-result`.                                                                |
| State      | One read attempt with optional admitted rendezvous, candidate stream and complete reply; no persistent operation record.                                                                                               |
| Invariants | Peer proof before request; one completed connection and one request; no Client hello, generation allocation, Workbench Session, activation, lock creation, process launch or mutation.                                 |
| Decisions  | Missing selected runtime/endpoint before connection can be absence; refusals, a reply from a Session that predates the handshake, partial replies, permission errors, busy/deadline, malformed replies and EOF cannot. |
| Ownership  | Local Session runtime owns private path admission; the handshake crate owns the request and reply; the local Client protocol owns the exchange; Server owns its process facts; CLI owns presentation.                  |
| Lifetime   | Candidate drops after the single reply or any failure, before presentation. Cancellation releases its wait source through the existing emit finalizer.                                                                 |
| Limits     | An observation at the selected scope, not proof that all Server processes are absent, a global health check or an atomic snapshot of process state.                                                                    |

`not-running` means there is no reachable Server through the inspected local scope at that
observation. A runtime directory without an endpoint can be absent even if stale marker files
remain; no read removes them. A Server may stop immediately after a successful reply. If the probe
exhausts its budget, the result is timeout, never a fabricated absence or late success.

## Query and compatibility

The request is the handshake's `query`, carrying this build's environment; the reply lists the
Server first, then the Worker it started, then each Terminal Host connected to that Worker (at most
256), each with role (`server`, `worker`, `terminal-host` or `unknown`), product version, build,
environment, PID, state (`ready`, `draining`, `stopping` or `unknown`), terminal count and
connections, plus whether Restart Session keeps terminals. Each Host states its own facts; a Host
that has not answered by the time the reply is due is left out, never listed with guessed facts.
The [local Client protocol](../../../crates/local-client-protocol/README.md#process-query)
owns the exchange on every platform, and the [Server guide](../../orchbee-server/README.md) owns
the reply. The Server answers before Client admission, after a `drain` (reporting `draining`), and
while a Session stop or an `exit` is underway (reporting `stopping`); the request grants no
capability and cannot enter a Client phase.

A Session in another environment refuses the request: `status` and `status server` report
`incompatible` with reason `environment-mismatch` (exit 6), and `doctor` fails its Session check
with that reason. A Session older than the handshake answers with its old OBLC rejection, read as
`incompatible` with reason `session-too-old` and no Session version. Neither is reported as
not-running. Version compatibility for joining a Session is decided by the handshake when a Client
offers, not by these reads, and no updater, restart, channel change or named/remote routing is
performed.

## Output and evidence

Paths use the lossless platform path representation; human strings and paths are escaped through
the common text renderer; JSON bytes do not depend on locale. The `status client` and
`status server` failures keep their established usage, denied, incompatible, protocol/transport,
timeout (exit 7) and cancellation (exit 130) codes and add the closed detail
`{stage:"runtime-status"}` to timeout and cancellation. A `denied` query prints the shared refusal
copy instead of the generic query failure: on Windows, a Client below High integrity refused with OS
error 5 learns that the Session runs as administrator and to run `orchbee server stop` in an
administrator terminal; any other refusal says the local Session refused this connection.

Evidence: the reply's process order, versions and unknown role or state; Hosts after the Worker, at
most 256 and without capabilities, and a late Worker read as no Host; the Windows named-pipe request
with environment refusal, an old Session's rejection, wrong replies, silence, expiry and
cancellation; Server query with full Session capacity and while stopping without allocating a
generation; offline Client, help, schema and completion; `status` with no Session failing
unavailable; and `doctor` with no Session exiting 0 with `session` not-applicable while an
unreadable configuration file exits 1. Review is performed by the implementation author with
implementation tests; neither independent review nor unexecuted formal proof is claimed.
