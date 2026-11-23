# Rust Orchbee Server

The private Rust Server of every Session. It owns the Workbench navigation authority and projection
kernel, local Human Client and Client Connection admission, the Server-Worker process spine and its
OBCN carrier, and the terminal coordinator. The emitted executable is named `orchbee-server`. Every
release cohort ships it beside `orchbee`, `orchbee-worker` and `orchbee-terminal-host`, and the
Desktop package bundles the same cohort, checking each file against `cohort.json`
(`apps/desktop/scripts/package.mjs`). It participates in the root Cargo verification gate.

## Semantic events

The Session journal records actual accepted topology, metadata, worktree, Agent and Client changes.
An authenticated connection owns one bounded subscription; read/wait retain its cursor and probes,
while close/EOF releases the owner. Terminal output and exit use admitted Host observations, and
Client scroll/visible output use the exact read-only Client viewport. History gaps, unavailable
sources and uncertain observer cleanup remain explicit. [Event ownership and acceptance](docs/windows-events.md)
defines the lifecycle, bounds and Windows evidence; Desktop receives no new consumer here.

## Diagnostic log

The Server installs its role in the
[daily diagnostic log](../../crates/local-runtime/README.md#diagnostic-log) at startup and,
as a managed Session Server, records `started` with its version and `exited` with an outcome or
closed I/O error kind. Each [Agent catalog](#agent-catalog) lookup that ends records
`agent-catalog-lookup` with its outcome, `complete`, `failed` or `unsupported`, and how many
commands were found, missing or unavailable.

Faults the Server survives, and the ones it cannot contain, are recorded too. A record holds static
tokens, counts and flags only: never a panic message, request or terminal content, a path, or an
error's text.

| Record                    | Written when                                                                                                                                                                                                                                                                                  | Fields                                                                                                                                                                                                                                                                                                                                                              |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `listener-fault`          | The Client listener or the Agent observation endpoint fails to accept, admit or start a handler for one candidate, or its wait fails, and pauses before accepting again                                                                                                                       | `listener` (`client`, `agent-observation`), `stage`, `error` (I/O error kind or `none`), `pauseMs`, `suppressed`                                                                                                                                                                                                                                                    |
| `candidate-dropped`       | A candidate is closed before it is served: Client handshake transport setup, peer credentials, a poisoned registry, an exhausted generation, an unencodable reply or query result; an observation connection that cannot be made nonblocking or arrives over capacity                         | `operation`, `stage`, `cause` (I/O error kind, `protocol` or `none`), `suppressed`                                                                                                                                                                                                                                                                                  |
| `worker-request-failed`   | One Worker request failed while its connection kept serving: no reply within its window, an invalid whole frame, a mismatched reply, or a window that passed while it waited unwritten behind an owed reply; or a connection cleanup's caller stopped waiting while the cleanup stayed queued | `request`, `failure`, `error` (I/O error kind, protocol error or `none`), `elapsedMs`, `suppressed`                                                                                                                                                                                                                                                                 |
| `worker-reply-dropped`    | The Worker connection dropped a whole frame that answered no request and kept serving: a late reply to a request that already failed, an unmatched reply, or invalid content                                                                                                                  | `cause` (`late`, `unmatched`, `invalid`), `request` (`none` when unknown), `error`, `lateMs` for a late reply, `suppressed`                                                                                                                                                                                                                                         |
| `client-stream-failed`    | The Server closed one Client stream while that Client's connection kept serving: the reader refused it before it ran, its request did not decode, its read or intent failed, or its record could not be delivered                                                                             | `stage` (`admit`, `open`, `job-read`, `update`, `resnapshot`), `failure` (`backlog`, `streams`, the connection stage, `invalid-request`, `record-size`, `budget`), `close` (`invalid-request`, `capacity`, `outcome-unknown`), `suppressed`                                                                                                                         |
| `worker-connection-ended` | The Worker connection ends, or its thread cannot start                                                                                                                                                                                                                                        | `exit`, `failure`, `error`, `request` with `elapsedMs` since it was sent, `afterShutdown`, `workerProcess` (`not-held`, `running`, `exited`, `unknown`), `workerExitCode`                                                                                                                                                                                           |
| `worker-lost`             | The managed Server lost its Worker and decided what follows, once per lost generation                                                                                                                                                                                                         | `generation`, `cause` (`exited`, or `disconnected` when the Server ended a process still running), `exitCode` or `exitSignal` when known, `attempts` in the cap's window, `action` (`replace`, `stop`, `give-up`), and for the last two `reason` (`stop-admitted`, `worker-not-gone`, `host-gone`, `host-spawning`, `host-unverified`, `cap-reached`)               |
| `worker-replaced`         | One replacement attempt adopted the Host or failed                                                                                                                                                                                                                                            | `generation` once admitted, `outcome` (`adopted`, `failed`); when adopted the `imported`, `quarantined` and `missing` counts; when failed the `stage` (`create`, `admit`, `adopt`), the `failure` (an `AdoptHost` failure, `timed-out`, `declined`, `exited`, `protocol`, `unavailable`, or an I/O error kind) and `next` (`retry`, `stop`, `give-up`); `elapsedMs` |
| `host-adopted`            | A Windows or macOS Server started for a generation that kept its predecessor's running Host adopted that Host or failed to, once per start                                                                                                                                                    | `outcome` (`adopted`, `failed`); when adopted the `imported`, `quarantined` and `missing` counts; when failed the `failure` (`host`, `record`, an `AdoptHost` failure, `timed-out`, `declined`, `protocol`, `unavailable`); `elapsedMs`                                                                                                                             |
| `panicked`                | Any Server thread panics; the panic still ends only that thread                                                                                                                                                                                                                               | `thread` (the program's own thread names, else `other` or `unnamed`), `file` stem, `line`, `suppressed`                                                                                                                                                                                                                                                             |
| `state-poisoned`          | The Session coordinator lock is first observed poisoned                                                                                                                                                                                                                                       | `lock` (`coordinator`)                                                                                                                                                                                                                                                                                                                                              |
| `agent-change-refused`    | The Agent state refused a change as `invalid-event`, keeping its previous state                                                                                                                                                                                                               | `transition` (the refused change's kind), `suppressed`                                                                                                                                                                                                                                                                                                              |
| `agent-state-recovered`   | A fault inside the Agent state was contained and the state rebuilt from its last committed facts                                                                                                                                                                                              | `revision` (the rebuilt revision), `carrier` (whether a Worker carrier was kept), `suppressed`                                                                                                                                                                                                                                                                      |

The [automatic update schedule](#automatic-update-schedule) writes two more records. A build that
does not schedule automatic updates writes neither.

| Record                      | Written when                                                                                                                       | Fields                                                                                                                                                                                                                                       |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `automatic-update-schedule` | The Session becomes ready in a build that runs automatic updates                                                                   | `scheduled`, and when false `reason` (`outside-store`, `thread-unavailable`)                                                                                                                                                                 |
| `automatic-update`          | A run ends: a scheduled one with `update.automatic` on, or one a Client asked for; `info` for a run that answered without an error | `run` (`answered`, `unanswered`, `not-started`, `cancelled`, `timeout`, `unsettled`, `failed`), the helper's exit `status`, `reason` for `not-started`, and from its document `version`, `phase`, `error` (`none` on success) and `upToDate` |

`listener-fault`, `candidate-dropped`, `worker-request-failed`, `worker-reply-dropped`,
`client-stream-failed` and `panicked` are written at most once a minute per site, and `suppressed`
counts the records left out since the last one. The Agent state writes `agent-change-refused`
(`warning`) and `agent-state-recovered` (`error`) at most once a minute each, and their
`suppressed` counts the refusals or rebuilds left out; the
[fault recovery rule](docs/agent-canonical-model.md#fault-recovery) says what is kept. `worker-lost` is `error`, and `worker-replaced` is
`info` when adopted and `error` otherwise; neither is rate-limited, since the cap bounds them.
`host-adopted` follows the same levels and comes at most once per start.
`worker-connection-ended` is
`info` when the Server ended the connection or the Worker left after answering a shutdown, and
`error` otherwise. Only a Windows Session holds its Worker's handle after admission; there the
record waits up to two seconds for a Worker that ended its own connection to exit, so it can carry
the exit code. Without that handle the record says `not-held`. The log records nothing else about
Clients, terminals or requests.

## Client handshake and process query

Every Client connection opens with one `OBHS` frame from
[`orchbee-connection-handshake`](../../crates/connection-handshake/README.md). Server builds its
acceptor profile from its product version, startup cohort, and compiled environment, and decides an
offer in the handshake's order: connection kind, environment, generation in the release
environment or development fingerprint elsewhere, Client capacity, then state. A refusal carries
Server's facts so the Client can name the older side. A frame that is not `OBHS` gets `malformed`,
and an offer that stays silent past the two-second candidate deadline gets `timeout`. A drained or
exiting Server refuses every offer with `stopping`. After a recorded Session stop, offers are still
answered, so the stop's status and retry exchanges reach its owner, and the following Client hello
gets the typed stopping rejection; so does the hello of a Client that did not offer `reconnect`
while a [restart](#server-and-worker-restart) holds the Session.

A `query` request, on every platform, checks only the environment and replies with this Server
first, then the Worker it started, then each Terminal Host connected to that Worker, each with
product version, build, environment, PID, state (`draining` after a drain, `stopping` after a
recorded stop or an exit), terminal count, and connections; `restartKeepsTerminals` is false. The
Server and Worker count the terminals whose Host reported a start and that no Close has disposed
since, so a closed terminal stops counting once its Close disposes it. The
Server asks its Worker for the Hosts with one OBCN Host query and waits for the answer until one
second after accepting the connection, leaving the rest of the two-second candidate deadline for the
reply. Each Host's entry is the one it reports for itself, listed without capabilities and at most
256 of them. A Host that has not answered, and every Host when the Worker is absent, late, or between
generations, is left out rather than listed with guessed facts; a Server that did not start its
Worker lists neither. Its `extensions` carry `clients` and, on Windows while a Worker is listed,
`background-programs` ([messages](../../schemas/connection/README.md#messages)), which a restart's
check reads. The query
creates no Connection generation or Workbench/viewer state, acquires no shutdown record lease, and
closes after one reply, including when Client capacity is full. These are diagnostic facts, never an operation grant or proof of
future availability. The [runtime status contract](../orchbee/docs/native-runtime-status.md) owns
CLI behavior and evidence; the [local Client protocol](../../crates/local-client-protocol/README.md)
owns the exchange.

`drain` and `exit` are checked like `query` and served only by the managed Windows lifecycle owner;
any other Server answers both `unsupported`, and `handover`, which the same owner serves for a
[Server and Worker restart](#server-and-worker-restart), too. `drain` is repeatable and answers
`drain-result` with the state. It refuses new Clients and new intents, while records already
admitted run to their result and existing connections keep their reads. Each
application record is either a read, which observes the Session or keeps the requester's own
observation going (queries, snapshots, subscriptions and their continuations, viewer attachment,
demand and acknowledgements, heartbeats, and reading an admitted operation's result), or an intent,
which asks the Session to change shared state, a terminal, an Agent, or user storage; every request
variant states its effect in `application_connection/effect.rs`. A refused intent never takes
effect. On a multiplexed connection its stream closes with `stopping` and every other stream
continues. A direct connection has no per-record refusal, so it ends at the refused intent, as it
does at any record after a Session stop; its Client reports the connection lost. With no new intent,
the Worker receives no new work, so no separate Worker message drains it. A Session stop or an
`exit` refuses reads too. `exit` closes admission, waits for admitted records, stops the Server's
background jobs, publishes the final checkpoint, and sends the Worker a Keep shutdown that releases
its Host connection without inventory, containment or disposal, then waits for the Worker to exit.
With no terminal left, the Server also waits up to three seconds for the Host to end on its own.
`exit-result` is written before the process ends, within a 30-second settlement budget; a settling
failure is reported and the process still ends. The Host and every terminal keep running. `exit`
publishes no Stopped record or receipt. Once its Worker is gone it writes the generation's
activation identifier to `extensions.exited` in `session.json`, so the next activation recovers the
Session without counting a crash or a fallback failure; a write that fails, or an `exit` that ends
before its Worker does, leaves that end counted as a crash. Recovery still needs the recorded
Server and Worker gone, and adopts the running Host only through the binding record
([cold adoption](#cold-adoption)). A queued Session stop goes before a pending `exit`;
either one ending the process first answers waiting `exit` requesters.

## Durable Windows session lifecycle

For native default/named scopes, the CLI activation owner publishes an exact private Active catalog record before the managed
Server starts. Before Worker effects, the Server admits and retains this record alongside its
checkpoint store and lifetime owner. After confirmed child exit and listener cleanup, shutdown
publishes durable Stopped before completing the runtime activation record. A publication failure
leaves recovery uncertainty and never grants a successful clean-stop receipt. The runtime crate
owns catalog encoding, path admission and replacement; the Server owns when shutdown is proven.

## Automatic update schedule

A managed Windows or macOS Server schedules the
[automatic updates](../../proposals/platform/automatic-updates.md#who-checks-and-installs) of its
release store when its own folder is a store version folder (`versions/<version>/`) and its build
runs them: never a development build, a test build only while `ORCHBEE_UPDATE_FEED` names a feed,
and a release build only once the
[automatic-update switch](../../crates/local-runtime/README.md#automatic-update-switch) is on. The
Server reads that variable only to tell whether it is set. The first run comes one minute after the
Session admits Clients, and the next ones every hour on that grid while the Server runs. Each run
reads `update.automatic` from the Session's settings first and does nothing while it is off, so a
change applies at the next run.

A run finds the store root by the `ORCHBEE_INSTALL_DIR` rule and the active version by
`active.json`, pins that version's folder and Client as Session start does, and runs
`<stem> --automatic-update-v1` with the Server's environment, so proxy variables and a feed reach
it, null standard input, and standard output and error each kept to 64 KiB. On Windows the helper
runs from the system folder in a Job this process holds, at below-normal priority, which everything
it starts inherits, and the Job ends it with the Server however the Server ends. On macOS it runs
through `/usr/bin/nice -n 10` from the Server's working folder, in a process group of its own, with
an empty signal mask and `SIGPIPE` at its default action, so it and everything it starts run at
that lower priority. The run waits on the schedule's own thread and never on Session work, and it
is ended after seven hours, past the helper's own longest run. A run that lasts past the next hour
skips that moment rather than overlapping or catching up. A failed run only writes its
`automatic-update` record, and the next hour runs again. A Session stop or `exit` stops the schedule
first, and the stop never fails because of it. On Windows it does not wait: a running helper is
ended through its Job within its settlement budget. macOS has no such Job, so the stop waits up to
six seconds while the helper's group settles: `SIGTERM`, which the helper answers by settling its
own downloads, then `SIGKILL` two seconds later. A macOS Server ended by `SIGKILL` leaves a running
helper to finish or fail on its own, and the next run cleans up. Installing, verifying and cleaning
up stay with the
[helper](../orchbee/docs/windows-cli-configuration.md#windows-release-installation).

An admitted Client's
[`workbench.update-check`](../../crates/workbench-protocol/README.md#update-check) reaches the same
schedule through one job the Server creates before its Client listener opens. `start` queues a run
unless one is queued or running, and answers `checking` at once. The schedule's thread runs it next
as it runs a scheduled one, with the same helper, record and stop, whatever `update.automatic`
says, and the hourly grid stays where it was. A request made while a run is queued or running joins
that run. `read` never waits: it answers `checking`, the last run's result, scheduled runs included,
or `idle` before any run finished. Only a helper that exits 0 with error `none` finishes a run, as
`up-to-date` or, when it made a newer version active, `installed`; every other end is `failed`. A
Server that schedules nothing, on Linux, outside a store version folder or in a build that does not
run automatic updates, answers `unavailable`, as does one whose schedule stopped or whose thread
ended, and drops a request it had queued. Another Session's ID is `rejected`, and the Workbench
service, which never reaches the job, answers `denied`. The Server lists the `update-check`
capability in every `client-server` handshake answer, whether or not it schedules.

## Idle move

A managed Windows or macOS Server that schedules automatic updates
[moves its Session](../../proposals/platform/automatic-updates.md#how-the-session-moves) once it is
older than the store's active version and the Session has stayed idle for one unbroken minute; the
switch and the store version folder therefore gate it as they gate the schedule, and
`update.automatic` does not. The stop loop looks once a second. Idle means no terminal that `query`
counts and no launch that has not settled (creating, awaiting its start, starting, or with an
unknown outcome), no helper run, and no pending Workbench operation: a terminal Tab or Panel
creation, a Worktree operation, a Workspace group close, a Tab set close or a Tab duplication.
Anything found starts the minute again. A connected Client does not keep the Session: it finds the
version the Session moved to in the stop's record and starts that version.

After the minute the Server compares the name of the version folder it runs from with the store's
active version, found by the same rule as a run; only an older Server goes on. It then asks its
Worker how many plugin jobs and configured commands still run, its Terminal Host's included, with
[OBPJ action 6](../../crates/capability-node-protocol/README.md#configured-background-commands).
Anything but a correlated answer of zero counts as running: no Worker, backpressure, a refusal, and
a reply that is lost, late or uncorrelated. Nothing about the query ends the Worker or its carrier.
Last, the Server looks again and records its own stop only if no record lease is held, under the
lock every Client admission passes first, so a Client admitted afterwards is refused with
`stopping`. A check that fails waits a new minute, so the store and the Worker are read at most once
a minute.

The stop is the ordinary Session stop under a fresh identity of the Server's own. Nothing runs, so
it asks nothing and disposes nothing, and its final capture keeps the layout for the next start,
which runs the active version. Its Stopped record carries `moved_to`, the active version the check
found, which `status server` reports as `movedTo`; a person's stop or a crash leaves none. A failed attempt is retried at each tick, since a recorded stop
refuses every Client until it ends; ten seconds after its first failure the Server ends as for an
[`exit`](#managed-windows-session-shutdown) instead. A plugin hook job started in the moment between the Worker's
answer and the stop goes unseen and ends with the Worker. The countdown rule names no platform type;
only the Windows Session wires it.

## Managed Windows Session shutdown

The managed Server automatically saves committed Workbench changes after a 500ms debounce and
refreshes cached directories every 30 seconds. It retains three
valid snapshots and excludes terminal screens. Startup restores the newest valid snapshot;
unreadable copies are retained and reported to the Client, including an explicit empty-workspace
notice if every copy is invalid. The [checkpoint contract](docs/workbench-checkpoint.md) owns
capture, physical-directory fences and restoration; Session-owner admission remains a runtime decision.
Each capture also retains supported Agent session references under the
[Agent session resume contract](docs/agent-session-resume.md). After a restart, the first Client
view lets the Server type each retained Agent's own resume command once into its restored idle
shell.

One stop identity owns the remaining managed Server lifetime. Accepting it atomically closes new
application-record admission; existing admitted operations and result writes settle before cleanup.
Duplicate start and status requests observe the retained attempt; a different identity is busy.
Only explicit retry of the same failed identity starts another attempt, up to 64. Failure never
reopens ordinary admission or authorizes an uncertain Host mutation replay. Each attempt has one
60-second cooperative deadline, excluding uninterruptible operating-system calls.

An admitted Client records a stop with `workbench.session-stop`, an intent, so a drain or an
existing stop refuses it before anything is recorded. The lifecycle owner records it exactly as a
`server stop` start, under the Client's identity and only for the Session the Server holds; the
record's lease holds the attempt back until its answer is ready, and the recorded stop then refuses
every later record, that connection's included. Status and retry use the unadmitted stop exchange
with the same identity. On a multiplexed connection the answer can still lose the race with the
stop's Client shutdown, so a requester treats a missing answer as possibly recorded and reads the
status. A Server without the lifecycle owner answers `unmanaged`, and the Workbench service, which
holds no stop authority, answers `denied`. Desktop's Quit All takes this path.

The owner retains the exact Worker child, drains and joins its single checkpoint saver, joins the
periodic process sampler and the retained Worktree/Workspace-group owners, then stops Close and
graphics maintenance. It takes the fresh final capture/publication only after those writers and
samplers settle, freezes the Worker carrier to shutdown-only traffic (settling queued unsent work as unavailable), and sends one bounded Worker
shutdown request. Its budget is the attempt's remaining time, at most 45 seconds, which leaves at
least 15 seconds of the attempt for the stages before and after it; the carrier waits for the
reply for that budget plus five seconds, never less than its ordinary 20 seconds. At the 34–38 ms
per terminal measured on Windows, 45 seconds dispose about 1,200 terminals. Uncertain sampler viewer cleanup remains owned until the matching Worker/Host stop and actual Worker exit. A known no-Host/Host-stopped result and exact Worker exit must precede stopping
Agent observation and Client listeners, waking and joining retained Client handlers, and publishing
the stopped activation record. An activation guard excludes concurrent restart during publication
and lifetime-lock release. Pipe absence or lost replies cannot prove completion. Failed attempts
remain queryable while the control listener survives; after listener closure, the strict activation
record remains a positive receipt. A recorded stop refuses every Client, so on Windows a stop whose
first attempt failed at least ten seconds ago, with no retry pending or running, becomes an `exit`:
every terminal keeps running, the failed status stays readable until the process ends, and the
next Client's Server adopts the Host. macOS runs this owner with the checkpoint saver and final
capture, and Linux without them; [their startup](#managed-linux-and-macos-session-startup)
describes what differs.

A Worker that already exited can neither sample a directory nor stop its Host, and the final
capture keeps each terminal's last cached directory. The stop then ends that Worker's Host itself,
on Windows exactly as on Linux and macOS: after the final capture it sends the Host's `exit`
control request, which ends every terminal the Worker would have disposed, and continues once the
retained record observes the Host's registered identity gone, within 45 seconds. A failed or
unanswered request fails the attempt in its host-exit phase; the Host is never ended by force. On
Windows the stop then ends each Keeper the record names, checked by its recorded process ID and
creation time, since a Keeper may hold the last handles of a program whose Host ended; a Keeper gone
or never started counts as ended, and one not ended by the deadline fails the attempt in the same
phase. A
stop is therefore never refused only because its Worker is gone.
A Server whose Worker was lost with no replacement serving, whose Host is gone and, on Windows, whose
Keeper is gone or never started, with no stop admitted, begins its own stop under a fresh identity,
checking once a second;
[Managed Worker replacement](#managed-worker-replacement) says when a loss comes to that. It
publishes the same Stopped record, and a later start restores from its final capture.
The first look that finds the Worker exited with no stop recorded, on every platform, first
records a Session failure of the Server's own generation for the
[fallback after a failed move](../../proposals/platform/automatic-updates.md#what-counts-and-who-records-it),
synchronously within the recorder's 2 s lock bound; a Server outside a store version folder, or
long after a move, records nothing. A Worker that exits during a stop counts nothing.
A Worker whose Host failed to start sends that Launch's pre-handoff failure. It keeps serving when
the failed start left no Host process, and otherwise exits after that answer (see the
[Worker's lazy Host startup](../orchbee-worker/README.md#managed-linux-lazy-host-startup)). The
failure settles as a definite `terminal-runtime-unavailable` even when the registry already dropped
the Worker, because no Host received the Launch, so the request is answered before this stop begins.

## Managed Worker replacement

The managed Server supervises its Worker alike on Windows, macOS and Linux. It sees a loss when the
Worker connection ends, at once: the ended connection's thread wakes the lifecycle owner instead of
waiting for its once-a-second check. A Worker that misses a reply's response window but keeps its
connection is not lost. It then proves the Worker gone through the handle it started that Worker with: up to 2
seconds for its exit, then ended through that handle and up to 5 seconds more, and the lost
connection's thread, which retires the generation, must finish within the same wait. The first
action that applies follows:

| Condition                                                  | Action                            |
| ---------------------------------------------------------- | --------------------------------- |
| A stop is admitted                                         | The stop goes on without a Worker |
| The Worker cannot be proven gone, or the Host not observed | Give up                           |
| The Host is still spawning, or gone with no Keeper running | The Server stops itself           |
| The cap allows no attempt                                  | Give up                           |
| Otherwise                                                  | Replace the Worker                |

On Windows and macOS the retained record observes each Host slot by its registered process ID and
creation or start time, and `AdoptHost` names each running Host in its slot, with the epoch its
Worker recorded; a Session that never started a Host is adopted as no Host. Linux
registers no Host and runs one, in the first slot, so a Host lock no process holds reads as gone.
On Linux, which runs no Keeper, a gone first Host stops the Server only when the lost Worker held a
settled binding; otherwise the Server replaces the Worker with no Host. On macOS a gone Host in
any slot that no binding the lost Worker settled names by its epoch, or, with no epoch recorded, a
gone Host while no binding settled, leaves nothing to keep: the Server frees that Host's registered
slot, removes the socket a killed Host left, and replaces the Worker without it. A Host still spawning never ran. A stop whose Worker is lost sends `exit` to each Host the record
names running.

On Windows and macOS a Host gone while the record names a running Keeper, which kept that Host's
programs, is replaced rather than stopped: `AdoptHost` names a Recovery slot with the Keeper's recorded process,
and the replacement [recovers those programs](../orchbee-worker/README.md#replacement-adoption) into
a recovery Host in that slot. An attempt after a failed one adopts the Hosts the record names
running, the recovery Host a failed recovery left among them, and recovers again each one gone while
the Keeper runs; with no Host left, or one gone and no Keeper, the Server stops itself. A lone Host
whose Worker recorded no epoch is named with the epoch its bindings share, and once every slot names
an epoch, a binding of an epoch no slot serves is not sent. Linux starts no Keeper.

The cap allows 3 attempts in any trailing 10 minutes of one Server process, and an attempt counts
when it starts. Attempts run one at a time, the first at once and each later one a second after the
previous failure. A replacement starts with `--worker-id` and the Session's Worker id, on Windows
through the Session record's replacement transition, which keeps the Host's registration. The
Worker listener stays open after the first admission; while a replacement is being started it
admits only a Worker naming that id, within 20 seconds of its creation. The registry holds an
admitted replacement out of every job until it adopts, so `AdoptHost`, sent in pages of at most 256
bindings under a fresh adoption id, is its first request. Each binding of the lost generation that
awaits its start or started goes with the Close that issued a step to it; a binding whose launch or
start is unsettled stays behind and settles as stale.

Once the last page answers `adopted`, one step under the registry and Session coordinator guards
that every operation takes moves each imported binding into the replacement's generation with its
operation, fence and lease, moves the retained Close targets with it, makes each missing binding
unavailable, and lets the jobs use the replacement. Title reads and the Agent drain follow the
serving carrier, process observation drops cleanup owed to the lost generation, and the checkpoint
saver keeps the same Worker id. `query` lists no Worker from the loss until the replacement serves.
The [terminal-runtime state](#terminal-runtime) reads `Reconnecting` from the loss itself, before
the Worker is proven gone, unless a stop is admitted; `Serving` once a replacement serves, and
`GivenUp` on giving up.

On Windows a replacement that serves, and a Server that [adopted](#cold-adoption) its
predecessor's Hosts, sends its Worker a move turn every 2 seconds from its own thread, on the lane
behind every person's request
([H3 design](../../proposals/platform/host-handover.md#h3-design)); until H4 only test builds
move, and the Worker answers the first turn `Done`. A `Moved` answer rebinds the binding that still
names the old Host terminal to the new one, keeping its operation, fence and lease, so titles and
every fact read through it follow; a binding closed meanwhile changes nothing. The binding record
is written before and right after every turn, so it names each Host a turn started and each
terminal it moved: a Server that ends between turns leaves no running Host it lacks, which would
block [cold recovery](#cold-adoption), and the next turn, which ends the old copy, finds it naming
the new one. Unchanged bytes are not written again, and a write before a turn that waits for a Host
holds that turn. A binding the adoption answers as moved is rebound the same
way once the replacement serves. The turns end at `Done`, at a turn declined or left unanswered,
and when the Session stops, before its final capture; a restart the Server takes back starts them
again. A Close begun before a rebinding still names the old Host terminal and retires the binding
that moved from it.

`host-gone` makes the Server stop itself; `host-unverified`, `host-mismatch` and `host-refused`
give up at once; every other failure, `recovery-failed` among them, a timeout, a decline, and a
creation or admission failure count against the cap. A replacement that cannot be proven gone after
its failure gives up. A stop admitted, or an `exit` received, during a replacement ends it at the
next step boundary through the Server's handle, and the stop goes on without a Worker. Giving up
lasts until the Session stops; a Host that ends meanwhile makes the Server stop itself once no Keeper
runs either, so the programs a Keeper keeps after a give-up run on under `GivenUp` until a stop ends
them. The supervision never ends a Host or a Keeper.

## Cold adoption

On Windows and macOS a Server can start for a generation that kept its predecessor's running
Terminal Host:
the Server and Worker ended, by a crash or an `exit`, while its Hosts kept their terminals. Crash
recovery admits that generation only when the Session's
[binding record](docs/workbench-checkpoint.md#binding-record) names each running Host, and
publishes it with those Hosts' registrations, on Windows each Keeper slot's while its Keeper runs, and no
Server or Worker
([cold recovery](../../crates/local-runtime/docs/windows-crash-recovery.md#cold-recovery-transitions)),
so the adopting Worker adopts that Keeper instead of starting a second one.
The Server learns it at startup from that record: each registered Host it observes running is
adopted, and one gone, still spawning or not observable fails the start, so the next activation
decides again.

The Server decodes the whole binding record and requires an entry naming each registered Host's
process. It starts its Worker with the record's Session Worker id, admits it as a replacement, and
before any job sends `AdoptHost` for the recorded Hosts, each in its slot with its recorded epoch,
in the pages
[Worker replacement](#managed-worker-replacement) sends, with each recorded terminal whose
Presentation the restored Workbench holds. A Close that was in flight is not carried: its Tab stays,
and a later Close works. Once the last page answers `adopted`, the coordinator imports each
confirmed binding into the new Worker's generation with its recorded fence, its awaiting or started
state and the facts the record keeps, under a fresh operation and lease, and allocates identities
from the record's next identity plus 2^64, above any the ended Server issued after its last write.
Each such Presentation shows its running terminal again: nothing is launched, no kept output is
shown and no Agent resume is typed, and its Agent reference stays for the next capture. A recorded
terminal the Host no longer runs reads unavailable, one the adoption answers as moved is rebound to
the Host terminal it runs as, a terminal the Host runs that the record does not name stays
quarantined in the Host, and every other Presentation restores as at any start. The binding writer
keeps the adopted record's `extensions`, and on Windows the Server's
[move turns](#managed-worker-replacement) start once it serves.

A start makes one attempt. A record that does not decode or misses a kept Host, or an `AdoptHost`
that does not answer `adopted`, fails the start; its Worker then shuts down with its terminals kept,
never disposed, so the Host keeps every terminal, those a restore launched in that start included,
and the next activation tries again. `host-adopted` records the attempt.

## Server and Worker restart

On Windows and macOS the managed lifecycle owner serves `handover`, the old side of a
[Server and Worker restart](../../proposals/platform/versioning-and-upgrade.md#server-and-worker-restart),
and its `query` answer reports `restartKeepsTerminals` true; every other Server reports false and
answers `handover` `unsupported`. `orchbee server restart` requests a restart, and the
[successor](#successor) sends the steps. The owner answers each step within a 40-second window or
not at all, so the successor waits longer for `prepared`; a step that arrives while another waits is
answered `busy`. The decisions the code does not state:

- `prepare` holds the Session only for the ticket of a `successor` entry in `session.json` that
  names the successor's Worker as well as its Server, and only once the owner has opened both on
  their recorded process ID and creation time; otherwise it refuses `not-recorded`. Every way a hold
  ends can then end both through those handles, so no successor process outlives a take-back.
- `prepare` refuses rather than cut work short: `background-programs` while the Worker itself runs
  a plugin job or configured command, since the hand-over ends it, while one its Terminal Host runs
  outlives the hand-over; and `busy` while admitted
  intents or operations do not settle. A Worker that neither releases its Host nor ends through its
  handle in time does not stop `prepared`: the Host admits one Worker session, so neither the
  successor's Worker nor a take-back replacement can adopt beside it, as for any hung Worker.
- `prepare` also refuses `clients` while a connected Client, or one being admitted, did not offer
  `reconnect`. The successor checked the same before it ran, but a Client may connect after that
  check, so the owner judges again under the lock that enters the hold and admits Client
  connections: such a Client is either seen there or refused while held.
- While held, terminal requests see no Worker, as after a Worker loss, and every other intent is
  refused as during a drain, since a change after the exact capture would be lost at the commit;
  reads and new connections of Clients that offer `reconnect` go on. Any other Client is refused
  with `stopping`, which TUI and Desktop relays retry, rather than admitted and cut off at the
  commit with no way back. The [terminal-runtime state](#terminal-runtime) reads `Reconnecting` only
  from the point of no return, once the store is released, so a refused `prepare` shows Clients
  nothing.
- The record decides how a hold ends. `commit`, `abort`, the end of the 30-second lease and the exit
  of either successor process are judged under the record's writer lock: a record that names the
  successor makes the owner leave; otherwise it ends the successor's Worker and then its Server,
  removes the entry and takes the Session back with a
  [replacement Worker](#managed-worker-replacement), lifting the hold first, since the replacement
  adopts the Host over later ticks and may give up as P1a does: an intent admitted before it serves
  fails as during a Worker loss. An `exit` while held is judged first and, kept, goes on without a
  Worker. Leaving publishes no Stopped record, receipt or `exited`, since the record belongs to the
  successor. While the record stays unreadable or unwritable, the owner does not guess: after a
  bound it ends the successor and leaves without the store, and the next start adopts the Host
  through [cold adoption](#cold-adoption).
- A stop is never accepted by a Server that may leave without carrying it out. Entering the hold
  refuses `busy` once a stop is recorded, and while held a `server stop` start records nothing and
  answers `not-recorded`, both under the lock that admits stops; the hold already refuses a Client's
  stop as an intent. The person stops the Session again once the restart ends, from whichever
  Server then serves it.
- Holding nothing, the owner clears an abandoned entry, one whose Server is gone or whose ticket
  never reached `prepare`, ending what it names by verified identity, the Worker first; a start after
  both Servers ended does the same before it recovers
  ([successor entry](../../crates/local-runtime/docs/windows-crash-recovery.md#successor-entry)).

### Successor

`--runtime-directory <dir> --handover <ticket>`, with the configuration arguments an activation
freezes, starts the successor of the restart whose `successor` entry names that ticket and this
process; the requester names it there before it runs. On Windows and macOS it:

1. Checks what `prepare` would refuse, and what only it can judge, such as a connected Client whose
   `client-server` range this build does not meet and a Host whose `worker-host` range it does not
   meet, so a refusal ends the restart before anything is held.
2. Creates its Worker suspended, records it in the entry, then resumes it, so the replaced Server
   and any later start can end it by its identity. The Worker gets `--ticket <ticket>` and connects
   to the Worker endpoint that ticket names, since the replaced Server still serves the Session's
   own; it keeps that endpoint for the Session's life, and a later Worker replacement passes the
   same `--ticket`.
3. Sends `prepare`. From `prepared` it has 25 seconds, inside the replaced Server's lease: it takes
   the store, requiring the newest readable checkpoint to be the capture the replaced Server
   published; adopts the Host as cold adoption does; rewrites the binding record; and commits under
   the writer lock.
4. Sends `commit`, waits for the replaced Server to exit, ending it through its handle after a
   bound, and opens the Client endpoint.

On macOS the successor first catches stop signals and leaves the requester's session, as a start
does. Its Worker endpoint is a socket in the Session's socket folder named by the ticket's first
four bytes; it removes that socket when it fails before it serves, and its stop removes it later.
Once the replaced Server has exited, it removes the socket files that Server left, under the
lifetime lock, before it binds the Client endpoint. It binds no Agent observation endpoint.

Before `prepared` its exit code names its refusal, `orchbee_local_runtime::RestartRefusal`, which
the requester reads; any other failure exits 1. A failure before the commit sends `abort`, and the
replaced Server takes the Session back; one after it leaves the Host to the next start's cold
adoption. The `handover` and `handover-successor` log events record the steps.

`SessionRestartHandover.tla/.cfg` claims that within one restart only the recorded Server serves and
holds the store while its Worker holds the Host, the old Server exits only after the record names
the successor, every live successor process stays named by the entry or as the recorded pair, no
start adopts the Host beside one, and every run with no step left ends with one pair serving, the
next start's pair, or a failure the restart did not add. Tests cannot reach the orders of crashes,
hangs and lost messages across the four processes those claims depend on. Its actions follow
`managed/stop/handover.rs`, `managed/successor.rs`, the local-runtime successor record and the CLI's
`server_restart/workflow.rs`; it bounds one restart with one Host session to four faults and one
take-back replacement. It assumes that a hung process acts no more and ends only through a handle
opened on its verified identity, and leaves the requester's clear optional, since the requester may
be gone. Run `node scripts/tla/run-models.mjs --models session-restart-handover`.

## Workbench projection store

Sidebar deletion can submit `workspace.close-if-idle` or `project.dissolve-if-idle` with distinct
full fingerprints. The [conditional admission owner](docs/terminal-tab-close.md#conditional-idle-admission)
proves the whole captured scope under the Session gate, including pending reservations and child
bindings. A runtime-backed or unresolved scope never becomes empty from Client observations.
Closing a Workspace, Tab or Panel submits `workspace.close-if-idle`, `tab.close-if-idle` or
`panel.close-if-idle`; for each the Server reads every live terminal's own shell report and process
sample within one budget and asks unless that evidence shows an idle shell, so every Client and
every scope follows the same rule.
Conditional Workspace Close installs the ordinary retained fence; conditional Project dissolution
removes organization only and preserves member resources. A definite `confirmation-required`
refusal permits a new reviewed decision, never automatic unconditional resubmission. Existing
explicitly confirmed actions and receipts keep their semantics.

`WorkbenchAuthority` owns one in-memory Session navigation document and a bounded request/result
ledger. Navigation stores global, identity-sorted `directories`, `projects` and `workspaces` arrays
and independent `projectOrder` and `workspaceOrder` permutations. A Project contains only its identity and label;
each Workspace has a required nullable `projectId`, so ungrouped Workspaces are ordinary members
of the same document. Directories are global resources independent of Project membership.
The graph admits at most 65,536 Directories, 1,024 Workspaces and 256 Workspaces per Project,
subject to the snapshot byte bound.

Admitted local Clients can create or rename Projects and Workspaces through OBWP.
`workspace.create` accepts a nullable Project and commits one empty Terminal Context with its
reference. `workspace.set-project` groups or ungroups an existing Workspace;
`workspace.create-project` atomically creates a Project and moves that Workspace into it;
`project.dissolve` removes the Project metadata and ungroups its members. These operations preserve
Workspace identities, directory references, contexts, Tabs, Panels, terminal bindings and working
directories. They perform no filesystem access or terminal creation. Workspace rename, Close and
primary-directory acceptance target the stable Workspace identity without a Project parameter.
The independently authorized `workspace.primary-directory.accept` validates a local Worker directory
and atomically adds a global Directory selected by that empty Workspace. The
[directory contract](docs/workspace-terminal-directory.md) owns validation, replay and failures.
The `worktree-list` read re-verifies one stable Directory identity against the committed
snapshot, then dispatches one fenced OBCN discovery job to that directory's owning Worker over
the existing carrier; the answer is the Worker's bounded raw `git worktree list --porcelain -z`
bytes parsed at the service boundary — membership and generation checks run under the locks,
request I/O never does.
The local `tab.create` command appends a configured Workspace Tab, jointly commits
membership and retained terminal intent, and dispatches one launch. Exact replay returns the same
pending or final result. A later Close may remove that membership without rewriting the historical
Create result. A Client can attach a viewer using a current snapshot's exact Session/Worker/Terminal
binding through OBCS. The [authority model](docs/workbench-authority.md) defines authorization, revision conflicts,
semantic replay, allocation, retirement and evidence. Reads clone the committed snapshot under the
same gate as writes; response I/O occurs after the gate is released.

`tab.rename` changes the shared Tab label. `tab.move` permutes complete simple Tabs within a Workspace context without
changing terminal bindings or taking the Worker registry lock. Both can complete while a launch is
pending; exact Move replay retains its original order-change result and revision. The native Move
interaction remains a separate Client integration boundary.

`navigation.configure` atomically reorders Projects or moves one Workspace within/between Project
and ungrouped lists. The protocol's pure `configure_navigation` validates stable-ID anchors,
capacity and no-op behavior; the authority owns revision CAS, exact replay, one aggregate publication
and retained checkpoint order. It allocates no identity, accesses no filesystem and starts no terminal.
Project creation appends to explicit Project order and dissolution removes only that order member.

`tabs.configure` atomically changes group/pin metadata and order through the same authority gate.
It allocates only a new group identity, rejects unfinished context Close operations, and has no
Worker effects. The [native group contract](../orchbee/docs/native-tab-groups.md) defines its exact
transition and receipt checks; the shared conformance corpus compares Rust and GUI ordering.

`panel.split` is the first tree mutation: it jointly commits the source leaf, a new leaf, split
orientation and both slot identities before dispatching the second immediate terminal launch. The
Server retains exact tree membership, and aggregate Close now disposes every leaf in a captured Tab
before publishing one removal. The Linux process witness creates two real terminals, observes the
Split projection, closes the Tab and verifies that the Host has no remaining terminal or exits after
the Worker disconnects. The Linux native Client now projects that canonical split tree and moves
local focus across its validated leaves; it still attaches one viewer at a time and does not submit
`panel.split` from a TUI shortcut.

Local Workspace `tab.close`, leaf-scoped `panel.close`, status Read and Resume use
the retained lifecycle authority and one process-lifetime maintenance owner. A Panel close captures
one exact leaf, disposes and cleans it through the same lifecycle, and publishes the surviving Tab
with the leaf's parent Split collapsed into the sibling subtree; a Tab's only leaf closes as an
aggregate Close. Known Host disposal and exact Worker/Server cleanup precede
canonical removal; Client exit does not cancel accepted Close. The owner retries a failed step
within each attempt and renews an expired attempt twice itself, so a Close reports a parked status
only once the Server has given up; Resume or a fresh confirmed Close restarts it from there. Linux carrier/process checks cover
thirty-two live Client connections, idle control-slot release and same-connection reuse while an
unrelated shell survives. The [Close contract](docs/terminal-tab-close.md) owns the bounds and
separately evidenced native TUI integration and remaining production recovery and release obligations.

`panel.move` is the lifecycle-free tree operation: it relocates one leaf beside another leaf in the
same Tab, collapsing the source parent Split and committing one fresh Split with two fresh Slots in
a single revision while the moved leaf keeps its Panel, Presentation and terminal binding exact.

Invalid, oversized, semantically inconsistent, or wrong-kind inputs fail through the protocol
boundary. Requests are bounded to 16 KiB before decoding; the snapshot bound remains 1 MiB.
OBWP owns the request/result envelope. The application library remains unpublished and is consumed
only by its own executable. Complete TUI/Desktop/CLI Workbench integration remains unfinished.

## In-memory advancement and publication

The authority's private `WorkbenchProjectionStore::advance_projection` accepts one complete
internally supplied snapshot. It requires the same Session and a strictly increasing revision, then
derives navigation replacement followed by changed contexts in canonical identity order. It returns
`WorkbenchPublication::Delta` within the 1,024-change and 1 MiB encoded bounds, or a complete
snapshot otherwise. An empty delta may advance revision without visible projection changes.

Before changing state, the store validates and serializes the publication. An applied delta must
reproduce the candidate exactly. Parsing, Session, revision, refinement or serialization failure leaves
state unchanged. Only the authority advances this private component. Clients cannot replace
snapshots or submit deltas. The carrier provides read-after-write; every committed publication is
retained in a 64-entry journal and served to readers through `workbench.publication.read` under the
same read grant, so a Client holding a baseline revision fast-forwards with ordered publications
instead of full snapshots. A journal that cannot connect the caller's revision answers with one
complete snapshot boundary publication. Push subscription remains unfinished.

## Managed Linux and macOS Session startup

The private `--runtime-directory <absolute-runtime-directory>` entry consumes the
[shared runtime contract](../../crates/local-runtime/README.md).
First, Server catches `SIGTERM`, `SIGINT` and `SIGHUP`, even one it inherited as ignored, and
hands each to one signal thread. It catches them rather than blocking them, because a blocked mask
passes to the Worker, the Host and every terminal process, while a caught signal starts at its
default action in each of them. It verifies the four-artifact cohort, runtime, and immutable activation marker
before taking the exclusive lifetime guard, holds the socket folder's lock, retains the active
activation record and the durable Session record, and removes the socket files a Session that ended
without its stop left behind: no listener can serve them while this Server holds the lifetime lock,
and a Host socket stays while that Host holds its own lock. It then detaches from the Client
terminal session with `setsid`. On Linux it binds the 0600 Worker socket with no-replace
publication; on macOS it binds the final socket path inside the private runtime directory, applies
mode `0600`, and verifies the path is a single-link socket owned by the effective user. It starts
one exact cohort Worker. After registration it permanently closes Worker admission, so no
replacement generation can enroll, then atomically publishes the permission-complete Client
listener, the same listener Windows retains. Server startup has its own 20-second budget; the
Client retains its independent total startup deadline. This readiness means registered Server +
Worker, not an existing Host or terminal. Managed standard streams are null; this mode emits no
explicit-endpoint stdout readiness marker.

Client exit does not end the Session, and neither does `SIGHUP`. `server stop`, an admitted
Client's `workbench.session-stop`, `SIGTERM` and `SIGINT` all reach the
[managed stop owner](#managed-windows-session-shutdown), so a logout or a system shutdown, which
sends `SIGTERM`, stops the Session normally. The owner runs as on Windows without plugin hooks, and
on macOS the [idle move](#idle-move) stops the Session under the same identity its signals use; on
Linux it runs also without automatic updates, the idle move, Agent resume, the checkpoint saver, final capture and
kept terminal output: the Worker shutdown request disposes every terminal, and after the Client
listeners stop the owner removes the Session's socket files before it publishes the Stopped record,
which is the receipt the next start consumes. A macOS start then relaunches every terminal from the
final capture in its saved directory, showing its kept output first while
`terminal.restore_after_restart` is on, and resumes each retained Agent conversation in its managed zsh or bash under the
[Agent session resume contract](docs/agent-session-resume.md).
Every stop the Server begins itself, on a signal or once its Worker exited and its Host is gone,
uses one identity, and a later loss check retries that identity once its attempt failed. A later
signal retries the recorded stop once its attempt failed, a Client's stop included. After its
Worker exited, a stop, like an `exit`, publishes the final capture and then ends that Worker's Host
with the Host's `exit` control request, which ends every terminal the Worker would have disposed,
and continues once the Host's lock is free, within 45 seconds. A stop attempt that fails after a
signal, with no Worker left, ends the process without the Stopped record, so a signal always ends
the Server within one attempt; the next start then recovers the Session as after a crash. An
`exit` request also disposes the terminals: no later Linux or macOS Server can adopt a Host that
keeps them, and that Host's lock would refuse every recovery while it runs. On macOS its final
capture relaunches them at the next start.

A Server ended by `SIGKILL` or a crash leaves an Active record and stale socket files. The next
activation recovers that record to Crashed under the activation and lifetime locks once the Host's
lock is free, and starts the Session again; socket files never count as a live process. On macOS
that start restores the newest background save, so terminals return with the layout and
directories saved at most 30 seconds, or half a second after a layout change, before the end. A
Host that still holds its lock keeps the Session's terminals and refuses recovery until it ends, so
a restore never starts beside them. This entry
implements no service-manager installation or automatic restart. The ordinary explicit-endpoint
test entry retains the behavior below.

Linux and macOS compile the same Workbench dispatch as Windows. A Windows-only capability runs
until its platform adapter or an owner this startup never initializes refuses it with that
owner's typed outcome: plugin stores and Worker plugin jobs answer `Unsupported`, and
configuration-backed requests answer `Unavailable`. Startup starts the process observer as on
Windows: these Hosts sample each terminal's POSIX session, so Agent identity, screen states, close
evidence, directory reads, installed Agent manifests and external settings edits work here too. A
POSIX package manager installs a JavaScript Agent as a bin link that its shebang hands the runtime,
where npm's Windows shim hands it the package entry. So before identity, the Server replaces a
runtime script whose shared or generic name alone leaves its row unverified with the file it
resolves to, read against the process's working folder, and keeps that path only when it proves
the provider. One retained thread does those reads: a sample waits 25 ms for its answer, skips at
once while an earlier read is unanswered, and otherwise keeps the Host's row, so a stuck network
mount never holds a terminal's routing; answers serve the same path for ten seconds. A relayed
sample carries the same path, so Clients read the rows the Server read. On
macOS a catalog download runs `/usr/bin/curl` with the Windows arguments, attempts and limits;
Linux admits no system curl and fails it as a process failure before any network effect. Startup
also configures Panel graphics file frames: on macOS each producer gets a private 0700 frame
directory under the Session runtime, read only through descriptors that never follow a link (see
[macOS private frame directories](docs/windows-graphics.md#macos-private-frame-directories)), and
Linux compiles the same directory but never reaches it. Agent names
resolve through the same guarded path as on Windows; terminal name owners come from Agent reports
and process observation. Client view, semantic event and window title
requests are admitted as on Windows: their state lives in Server memory for the connection or its
subscription, and nothing of it is saved. Panel Agent and Panel metadata requests are admitted the
same way, so the sidebar and Panel titles read the Agent states process observation and reports
publish. A macOS Session also serves the Agent view, the Session-scoped Agent source override
that the TUI's machine Agent list reads; a plugin source answers `source-unavailable` there
because the plugin store is Windows-only. It serves configured commands too, running them in the
login shell as [custom commands](../orchbee/docs/custom-commands.md#macos) describes, and Panel
graphics. Workspace metadata and notification requests, and the Agent view, configured commands
and Panel graphics on Linux, keep their earlier refusal at admission until their behavior off
Windows is decided.
A macOS Session keeps the [checkpoint](docs/workbench-checkpoint.md#managed-startup-and-clean-stop)
and [kept terminal output](docs/workbench-checkpoint.md#terminal-output-replay) in the Session
folder under its POSIX data root and holds a Panel whose saved directory is gone until
`panel.restore-terminal`; a Linux Session keeps neither yet, which takes the checkpoint policy
in `managed/checkpoint.rs` and the restore-notice reader in the workbench Client plus a decision on
logouts that leave user processes running. Handoff remains Windows-only because only the Windows
startup constructs its owner.

## Local Worker service

On macOS and Linux, the private development invocation
`orchbee-server --local-endpoint <absolute-worker-path>` binds one Unix-domain Worker socket without
deleting an existing path. The additive form appends
`--client-endpoint <absolute-client-path>` to bind the separate Client socket. Selected sockets are
restricted to mode `0600`; only after every selected listener is ready does Server write the
machine readiness marker `ready\n` to stdout. Malformed explicit-endpoint invocation remains silent and exits with
status `1`. This is an internal additive process contract, not public CLI grammar or a production
service-installation interface.

The Server and Worker mutually require the peer's effective operating-system user through kernel
Unix-socket credentials. A credential mismatch or handshake transport-setup failure closes the
candidate without a protocol response. One listener admits at most one compatible Worker at a time
and assigns a strictly increasing process-local generation. A duplicate, malformed, timed-out,
wrong-message, protocol-incompatible, or cohort-incompatible attempt does not change the active
Worker. Although OBLW decodes its Worker identity as an opaque 16-byte value, Server rejects the
all-zero value as `MalformedFrame` before registry admission or `WorkerAccepted`: that value cannot
map to OBCN's nonzero Worker identity, and the rejection consumes no generation. Handshakes are read
synchronously under a finite timeout. After Server completely writes `WorkerAccepted`, it gives
that same unbuffered stream to exactly one generation-scoped OBCN wire owner and exposes a cloneable
request handle through the Worker registry. The owner, not a caller or the registry, serializes every
write/read exchange; no registry lock is held across wire I/O.

The handle rejects a request for another Worker or generation before touching the wire. It admits
submissions into two bounded FIFO lanes under one queue lock — high for interactive and
cleanup work, low for viewer observation — each capped at eight requests behind the one request
possibly in flight. Ordinary submissions try-admit and receive immediate `Backpressure` when
their lane is full — high-lane submissions also back off while a cleanup waiter is queued;
only the two connection-cleanup producers wait FIFO with priority over later try-admits, for at most one response window; a cleanup still queued then is written in order and its result discarded. The owner dispatches at most four consecutive
high-lane requests while the low lane is non-empty, then serves one low-lane request.
`Unavailable` and `Backpressure` both mean that the addressed request did not cross the
complete-write boundary. The [carrier scheduling model](docs/worker-carrier-model.md) owns
lane classification, admission, dispatch bounds, drain and failure settlement. The owner writes one complete OBCN Launch,
Start, or viewer request before reading exactly one result. It accepts only the matching result direction and
kind, operation ID, complete authorization echo, and, for Start, target identity. A reply that
misses its window, a whole frame with invalid content, and a mismatched reply fail only that
request, and the connection keeps serving the queue, after a missed window only once the late
reply has arrived. Only a lost frame boundary, a closed or
failed transport, an incomplete request write, or a failed handoff or shutdown retires that Worker
generation and makes queued work unavailable.

Close uses the lifecycle-only `submit_lifecycle` boundary to distinguish successful queue admission
from later settlement. Validation, frame encoding and completion storage preparation happen before
the existing `try_send`; backpressure returns no receipt. Success returns one owned
`PendingLifecycleResult`, even if the carrier retires immediately afterward. Its
`admitted_observed_at()` is sampled after successful submission, not the exact enqueue instant.
The continuation owner may use that later observation for conservative query spacing and must
charge the admitted step even if owner-gate reacquisition happens after the attempt deadline.
`wait(self)` consumes the receipt and returns the exact typed result or original carrier error;
lost completion still distinguishes pre-write Unavailable from post-write OutcomeUnknown using the
existing complete-write marker. Dropping a receipt does not cancel the submitted operation.
This boundary adds no queue, thread, timeout, retry, step counter or scheduling policy, and does not
change the existing blocking `request()` path. Held-owner queue, timestamp bounds, backpressure,
route rejection, completion-loss and real carrier result tests cover the admission/settlement split;
the Close owner models and integrated runtime own deadline and continuation behavior.

An attached carrier remains the active registry entry for the wire owner's entire lifetime. The
only callable admission cancellation rejects attached carriers; neither the request handle nor the
registry exposes independent active-generation deactivation. Thus no replacement can be admitted
while the old owner can still write, read, validate, or publish an exact result. Owner settlement
gates retirement: on normal exit, completion-receiver loss, thread-spawn failure, or panic
unwinding, owner destruction first marks the handle inactive, conservatively settles any in-flight
request from its complete-write marker, and drains the submitted slot. Only then does its RAII
registration guard remove the matching generation; that removal is the retirement linearization
point at which replacement becomes admissible. A stale guard cannot remove a replacement.

Each complete request write has a two-second deadline, and each result has twenty seconds to start
arriving. A result frame that has started has at least two seconds to finish, even past that
window: time is not evidence, so a late reply is late, not corrupt. Partial progress does not
restart either deadline. Failure before the request frame is completely written is reported as
`NotDelivered` and is known unchanged: strict Worker decoding cannot dispatch an incomplete frame,
and the owner retires that connection after a failed write, while a request that expires before
its write leaves the connection serving. After the complete write, any failure is the distinct
`PostDeliveryOutcomeUnknown` carrier error: Server neither fabricates the semantic OBCN
`OutcomeUnknown` value nor retries the request. An exact Worker result, including a Worker-reported
semantic unknown outcome, is preserved; the carrier does not interpret launch capability-version or
lifecycle outcome variants. Viewer results additionally match the attachment identity and
action/outcome kind. A Worker decline that echoes the request completes it with the distinct
`Declined` error, known unchanged and recorded with failure `declined`; callers treat it as not
delivered, the Worker being unavailable for that request, and never as an unknown outcome. A
Worker that cannot decode a whole request answers it with `WorkerRequestUnreadable`, which echoes
nothing and so answers by position: it declines the request in flight, or settles the reply owed.
A request that failed while the connection kept serving is recorded as `worker-request-failed`.
The owner remembers the last eight such requests: a reply that echoes one of them, whether it
arrives while another request waits or between requests, is dropped and recorded as a late
`worker-reply-dropped`, never taken as another request's result. The owner
writes a request only while the Worker owes no reply, because the Worker ends its session if it sees
the next request while it still runs one. After a missed window it writes nothing until the late
reply arrives, and a request queued meanwhile fails unwritten with `NotDelivered` once its window
passes, as the [failure scope](docs/worker-carrier-model.md#failure-scope) describes. Between
requests the Worker may only hint or note; any other whole frame is dropped and recorded, and only
bytes that do not begin a frame, or a frame that stops arriving, retire the connection. Dropping
the whole registry requests private shutdown but cannot admit a replacement; the owner's queue wake
reaches it at once, or it observes it after the bounded current exchange, which has at most its
remaining write deadline plus one result window and the two seconds a started frame may take.
There is no unbounded connection thread or message queue.

The endpoint path is supplied by the controlled development witness. Discovery, stale-endpoint
recovery, a default runtime directory, singleton activation across candidate paths, daemonization,
Windows named pipes, production service supervision, and shutdown signaling remain outside this
slice. Abrupt process termination or a post-bind setup failure may leave the supplied socket path
for its external owner to remove.

When selected, Client listener bind or listener-thread spawn failure occurs before readiness and
fails Server startup. After readiness, a failure costs at most the candidate it touches: a failed
accept or a handler thread that cannot start drops only that candidate, writes a `listener-fault`
record, and pauses the listener for 100 ms, doubling while faults repeat within five seconds, up to
one second, before it accepts again. The managed Windows listener treats an exhausted connection
identity and a stream it cannot clone the same way. The listener never ends on its own, so admitted
Connections and the Agent runtime it owns stay until Session stop.

The private [`orchbee-local-worker-protocol`](../../crates/local-worker-protocol/README.md) crate owns
Worker framing, handshake, cohort rejection, and bounds;
[`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) owns kernel
same-user proof. The private
[`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) crate owns OBCN
semantic values and frame encoding. Server owns listener lifetime, Worker admission state,
generations, bounded submission, serialized carrier validation, and generation-matched cleanup. The
[`orchbee-worker`](../orchbee-worker/README.md) application owns the connecting process lifecycle;
its current runtime consumes authorized Launch/Start and read-only viewer requests through the Host broker.

## Local Client Connection service

When the private Client endpoint is selected, every same-user compatible Human Client connection
creates one independent process-local Client Connection generation. TUI, Web, Desktop, mobile, tab,
window, and renderer identity are intentionally absent: the endpoint admits one peer Client model.
Server accepts up to thirty-two active Connections and at most forty-eight total
candidate/connection handler threads. The offer and the hello each have a two-second read/write timeout. One silent candidate therefore cannot
block another connection while a handler slot remains, and saturation closes excess candidates
without allocating more threads. Handler slots and admitted-Connection registrations are
lifetime-owned guards, so normal completion, faults, and unwinding release both bounds.

An authenticated candidate receives a handshake rejection for its offer, or a typed `OBLC`
rejection for a malformed or wrong message, timeout, or a recorded stop after its answer. After completely writing
`ClientAccepted`, Server removes handshake timeouts and reads strict OBCS or OBWP on that same unbuffered
stream. The private service grants its single local effective-user subject default-shell Create and
Start, plus viewing and explicit acquisition of an exclusive input/resize controller for those terminals. It reads one request, invokes the Server-owned service, and writes the complete
correlated result before reading another request from that Connection. The result is committed before
Client output; a stalled writer holds only its Connection handler, never a coordinator or Worker lock.
Idle reads have no deadline until the first byte; thereafter the whole record has two seconds,
including large `OBCQ` paste records. Each
complete Client write also has two seconds, including large OBSR records. Expiry retires that Connection.

Create selects the current local Worker and validates any exact Worker constraint. Start resolves
a successful deferred Create by its request ID across Connections of this same subject. Server retains
the exact Worker generation and complete Host identity. Worker loss or replacement never authorizes
retargeting, re-creation, or replay. EOF, malformed, wrong-direction, oversized, truncated, read, or
write failure closes only the affected Connection; generation-scoped cleanup preserves terminal state.

Failures no peer can receive — a handler that cannot spawn, handshake transport setup or
peer-credential refusal, a poisoned registry, an exhausted generation, an undeliverable stop result,
and a Connection that ends on a fault — go through the service's private `diagnostic` exit: one
locale-neutral standard-error line naming `operation`, `stage`, an I/O error-kind or `protocol`
`cause`, and the Connection generation when one exists. A multiplexed handler failure that ends its
Connection keeps its own stage; one that closes only its stream is written to the daily log as
`client-stream-failed` instead. Session stop and a departed peer (end of stream, reset, abort,
broken pipe) are ordinary ends and are not reported. The line carries no record bytes, terminal content, paths, credentials,
environment values, or nested error text, and changes no peer-visible outcome. Managed launch binds
the Server's standard error to the null device, so only a harness that owns it reads these lines.
A failure that drops a candidate before it is served, or stops the listener's accept, also writes
the same tokens to the [daily diagnostic log](#diagnostic-log) as `candidate-dropped` or
`listener-fault`, at most once a minute per operation and stage; a Connection that ends on a fault
stays a standard-error line only.

The private [`orchbee-local-client-protocol`](../../crates/local-client-protocol/README.md) owns
Client framing and its connecting adapter. The independent
[`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.md) binding owns OBCS
validation and frame I/O. Server owns subject policy, placement, operation/lease/fence issuance,
terminal binding state, bounded submission and correlated semantic results. Bare Linux `orchbee` consumes this path; persistence and production ingress remain outside this slice.

## Local Worker process-spine modeling record

| Field        | Current contract                                                                                                                                                                                                                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem      | The verified native artifacts had no real process relationship. This Worker sub-spine must prove that a separate Server can admit and retain one separate same-user compatible Worker without creating a terminal, activation policy, or durable state.                                                                                                                              |
| Inputs       | An untrusted private argument vector, an absolute endpoint path supplied by the development owner, kernel peer credentials, and one bounded framed Worker handshake. Absence, malformed bytes, timeout, wrong message, wrong user, version mismatch, cohort mismatch, duplicate Worker, and disconnect are explicit.                                                                 |
| Data         | Server phase, optional active Worker identity plus nonzero process-local generation, next generation, and one candidate connection. Server is the sole writer. No value survives process exit.                                                                                                                                                                                       |
| Outputs      | Successful bind emits one `ready\n` marker. Worker admission returns its generation; an authenticated protocol-level rejection returns a typed bounded reason when the stream permits it. Peer-authentication and transport-setup failures close silently. Process setup or listener failure exits status `1` without a human diagnostic.                                            |
| Invariants   | One endpoint has at most one listener; one Server has at most one active Worker; active generations are nonzero and strictly increase; rejection cannot replace active authority; generation-scoped cleanup cannot remove a replacement; Worker disconnect does not stop Server; Server loss closes the Worker connection.                                                           |
| Decisions    | Invocation validity, peer-user equality, frame validity, protocol and cohort equality, expected Worker message kind, active-Worker absence, and matching cleanup generation completely determine the result. No current directory, Project, Environment, Worktree, Agent, or Terminal fact participates.                                                                             |
| Model        | A deterministic Worker registry owns admission and generation-scoped disconnect. A thin synchronous Unix-socket workflow authenticates, decodes, admits or rejects, acknowledges, and monitors the accepted connection.                                                                                                                                                              |
| Transitions  | `absent -> listening`; candidate `none -> pending -> admitted/rejected/timed-out`; Worker `none -> active(generation) -> none`; Server `listening -> stopped`. A second bind is rejected, and stale cleanup is an idempotent no-op against another generation.                                                                                                                       |
| Dependencies | Rust standard-library process, filesystem, Unix socket, timeouts, threads, and I/O plus the private local-worker protocol and kernel peer-credential crates. No terminal, child process, network socket, async runtime, persistence, environment lookup, or randomness.                                                                                                              |
| Verification | Pure codec and registry tests; native same-user socket-pair tests on macOS and Linux; real Server and Worker process tests for bind, readiness, admission, duplicate and incompatible rejection, replacement after disconnect, Server loss, malformed peers, exact output, and cleanup bounds. These implementation traces do not constitute an exhaustive admission protocol proof. |
| Limits       | This Worker evidence covers macOS and Linux under the same effective user. It does not establish Windows behavior, cross-user negative execution, production endpoint discovery, crash cleanup, service activation, Client Connection semantics, terminal retention, or remote security.                                                                                             |

Within the three-generation exploration bound, `AdmissionUsesPreviousCursor` and
`GenerationCursorChangesOnlyOnAdmission` independently check that each Worker admission copies and
advances the process-local cursor. Generation exhaustion and implementation refinement remain
deterministic Rust evidence. Named rejection actions retain the abstract classification and
authority-preservation proof; exact typed or silent wire outcomes remain protocol and process-test
evidence, so the model keeps no accumulated rejection history.

## Local Worker OBCN carrier model and runtime refinement record

This model-first record fixes the smallest Server-owned carrier contract after the accepted OBLW
response is completely written. The model is the accepted refinement target; the service
implementation above supplies its runtime evidence but specifies no OBCS business policy.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | After OBLW admission, Server must reuse that exact Worker generation's stream for bounded OBCN Launch and Start exchange without letting the registry, request submitters, or another task become a second wire owner.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Inputs       | One current Worker identity and process-local generation; three valid request candidates comprising two same-kind Launch operations with distinct operation IDs and one Start; stale-handle submission while active or inactive; complete-frame write or its absence; finite monotonic per-I/O deadline ticks; disconnect; and an actual incoming response value. The response domain carries direction, result kind, Terminal ID, operation ID, Worker ID/generation, fence, lease ID/expiry, capability, Start target, and the exact outcome payload, including two distinct definite payloads and the Host epoch in Launch `OutcomeUnknown`.                                                                                                                                                                                       |
| Data         | The model keeps one `activeRef`, one generation-scoped `requestHandleRef`, one scalar `wireOwnerRef`, one capacity-one `submittedSlot`, at most one `wireRequest`, its `wirePhase`, `now` and `wireExpiry`, an environment-owned `incomingResponse`, and attempt/completion/publication audit state. The checked configuration uses two Worker identities, two generations, two Launch operations, and one Start operation. The submitted slot may coexist with one wire request, but only the latter can be in flight.                                                                                                                                                                                                                                                                                                               |
| Outputs      | Rejection before a complete request-frame write is `known-unchanged`; an exact OBCN result retains the actual validated incoming value and payload, including Worker `OutcomeUnknown`; timeout, disconnect, or invalidity after a complete write becomes carrier-level `effect-unknown`. The latter is not a fabricated OBCN result. Retirement drains the queued slot as known unchanged/unavailable, inactive stale handles are unavailable without a write, and no class causes automatic retry.                                                                                                                                                                                                                                                                                                                                   |
| Invariants   | At most one current generation and one stream owner exist; the registry stores only the request handle and never the stream; the submitted and wire slots are independently bounded to one; each operation receives at most one complete write; the deadline clock is monotonic; exact publication equals a previously observed response whose direction, kind, operation, every authorization field, Start target, and outcome payload match; every mismatch class and invalid unknown payload cannot publish exact; unknown effects are non-retryable; replacement waits for old completion and publication settlement; stale activation and generation-matched cleanup cannot replace or remove the current generation.                                                                                                            |
| Decisions    | Wrong-Worker and wrong-generation routing reject before enqueue without retiring the owner. A stale handle, including while no generation is active, returns known unchanged/unavailable. Capacity is one submitted request plus one wire request; a fresh third valid request records backpressure without changing either occupied slot. `CompleteRequestWrite` is the ambiguity boundary and installs a fresh result-read deadline after the request-write deadline. Local time advancement and expiry settle a silent peer; response and disconnect arrival have no fairness assumption. `RetireCarrier` closes the generation gate: a later write or newly observed exact response is impossible, while an exact result observed first remains publishable. Replacement requires every old completion and publication to settle. |
| Model        | The carrier state owner separates registry identity, cloneable request-handle authority, queue occupancy, sole wire ownership, write completion, actual response arrival, local validation, deadline expiry, publication, retirement, and cleanup. `ReceiveExactWorkerResult` validates and preserves `incomingResponse`; it does not construct a matching result. The model refines the post-OBLW boundary and OBCN semantic contract rather than reimplementing either byte codec.                                                                                                                                                                                                                                                                                                                                                  |
| Transitions  | `ActivateCarrier` publishes one generation; `SubmitFirstLaunch`, `SubmitSecondLaunch`, and `SubmitStart` exercise distinct candidates; `DequeueRequest` moves one value to the owner; `CompleteRequestWrite` crosses the ambiguity boundary; `AdvanceMonotonicTime` enables pre- and post-write expiry; exact arrivals and one named arrival action per mismatch class populate `incomingResponse`; `ReceiveExactWorkerResult` or `RejectMismatchedWorkerResult` validates it; disconnect actions cover idle, before-write, after-write, queued-after-write, and after-result orderings; publication, late-result discard, replacement, stale rejection, and generation-matched cleanup settle retained state.                                                                                                                        |
| Dependencies | The target composes the existing Server Worker registry and post-`WorkerAccepted` stream with the private local-worker and capability-node protocol contracts. The local-worker protocol owns OBLW framing, the capability-node protocol owns OBCN values and strict framing, and the carrier composes them. No OBCS mapping, authorization decision, Host dispatch, terminal retention, persistence, endpoint discovery, or remote ingress participates.                                                                                                                                                                                                                                                                                                                                                                             |
| Ownership    | Server's registry owns current Worker generation and request-handle publication. One `WorkerWireOwner` equivalent exclusively owns stream reads and writes plus queue draining; request-handle clones can only submit. The codec owns byte validity, Worker owns the semantic OBCN result, and a later Server policy owner—not this carrier—owns authorization, reconciliation, or any permitted retry of a separately proven known-unchanged operation.                                                                                                                                                                                                                                                                                                                                                                              |
| Verification | Carrier tests cover exact correlation, response-field/payload mismatch, expiry on each write side, disconnect ordering, stale handles, replacement gating, cleanup and occupied queue capacity. Deterministic clocks check local settlement; no separate carrier TLA proof is retained.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Limits       | This is bounded formal evidence, not runtime-refinement evidence. It abstracts the complete-frame write as one boundary and uses one-tick finite write and read deadlines rather than a concrete duration; it does not prove partial-I/O code, system clock implementation, thread scheduling, panic cleanup, OS behavior, codec correctness, OBCS-to-OBCN policy, Host effects, durable recovery, or whether another authorized owner should retry a known-unchanged request. The two sequential phase deadlines correspond to the configured write timeout plus response timeout.                                                                                                                                                                                                                                                   |

The implemented runtime refinement maps `WorkerRequestHandle` to `requestHandleRef` plus `submittedSlot`
and maps the sole `WorkerWireOwner` to `wireOwnerRef`, `wireRequest`, `wirePhase`, `now`, and
`wireExpiry`. The scalar expiry is phase-indexed: `DequeueRequest` starts the total write deadline,
and `CompleteRequestWrite` replaces it with the total read deadline. `WrongWorker` and
`WrongGeneration` never enter `submittedSlot`; `Backpressure`
leaves both occupied slots unchanged; `Unavailable`, `NotDelivered`, and queued-owner shutdown map
to `known-unchanged`; and `PostDeliveryOutcomeUnknown` maps only to carrier `effect-unknown` after
`CompleteRequestWrite`. Deactivation must linearize with the owner: before complete write it is
known unchanged, after complete write it is unknown, and `ReceiveExactWorkerResult` can win only
before `RetireCarrier`. Checking an active flag only before an exchange is insufficient because it
would permit a write or newly accepted exact result after retirement. An already observed exact
result may still publish before replacement; replacement activation must wait for that publication,
matching the authorization model's pending-result guard. The carrier may not convert any transport
classification into a successful or failed OBCN result. The single submitted-slot occupancy this
record explored has since been superseded by the dual bounded lanes in the
[carrier scheduling model](docs/worker-carrier-model.md); the wire-phase, ambiguity-boundary and
settlement obligations are unchanged.

## Local Client Connection and OBCS modeling record

The Connection model explores three Clients, two active Connections, three handlers and bounded
generations. OBCS stream-fault and correlation configurations include coalesced handshake/request,
partial frames, stalled output, malformed direction, duplicate and distinct requests. Neither model
proves terminal authorization, Worker dispatch, cancellation, replay, persistence or live relay.
The coordinator model and process tests below own the added effect boundary.

## Local Client viewer relay refinement

The accepted local-terminal milestone now connects the Server's Client Connections to the Worker's
existing read-only viewer loop. The staged native LocalClient adapter consumes the viewer protocol. A process witness proves
two independently admitted Clients receiving real Host projections through Server and Worker. The controller refinement below adds input/resize; bare Linux startup now consumes this relay.

Each logical lane of an admitted Connection owns at most eight transient viewers on the Connection's ordered carrier. Attach
references an accepted terminal Create in the same local subject; AttachBinding supplies an exact
canonical binding selector. Server resolves the retained Host identity and current Worker/generation,
allocates fresh request, attachment and cleanup identities, and pins that Worker handle. Clients
cannot allocate authoritative runtime identities; every subsequent viewer/control action must echo
one Server-issued attachment ID owned by the issuing Connection. Each poll moves one complete opaque
Host record; Server validates its envelope, selected attachment and exact feedback scope without
decoding terminal presentation. Every viewer owns its own projection flight. Equal counters on two
flights do not authenticate content: the explicit selector determines feedback ownership. A Poll also
drains the viewer's kind-53 `HistoryChanged` event lane: the Worker keeps a bounded per-viewer event
FIFO beside the frame record slot and alternates the two sources through an explicit
next-preference, so a queued event is deliverable while a frame flight is unsettled and a
continuously ready source overtakes the other at most once. A kind-53 record is a valid result only
for `Poll`; surfacing one to any other action is a strict route error that never reaches
`projection.receive`. Event delivery never marks the frame flight delivered, and `HistoryObserved`
rides its own acknowledgement flight rather than settling `FrameApplied`. Event FIFO or byte-budget
saturation closes only that viewer, and frame plus event bytes are released exactly once on every
teardown path; a valid kind-53 event for an already-retired viewer is swallowed like a late frame.
Attach commits before Client publication. Explicit detach releases only the selected viewer and its
controller; Connection disconnect or failed publication cleans all its viewers. Once Server releases
a viewer's slot, whether for exact disposal, a changed Worker generation or a failed Worker exchange,
a Detach of it answers Detached, because a retry would find it absent; any other action on that slot
answers TerminalRuntimeUnavailable, and the released slot owns its remaining Worker cleanup. A
Worker `HostRejected` answer to FrameApplied, NeedSnapshot, Poll or a read also releases the slot,
because the Worker may have closed that viewer while containing the Host's answer; that request
answers `host-rejected`, and the Client attaches again. After demand, history, rows-applied and
selection actions the slot stays and the request answers `host-rejected`. These operations never
terminate or retarget retained terminals. Unknown exchanges are not replayed. Reattachment receives
a fresh attachment identity and requires a snapshot.

Poll also admits Host terminal settlement records (kinds 15 and 19) for its exact Host epoch,
terminal and creation identity. They preserve the projection flight and require no frame
acknowledgement. Worker seeds reattached viewers from retained inventory; Client keeps final
output and downgrades control as described by [terminal settlement](../orchbee/docs/native-panel-lifecycle.md#terminal-settlement).

No coordinator or registry lock spans Worker or Client I/O. Drop cleanup uses blocking admission to
the existing one-entry Worker request queue, without waiting for a Client response or adding an
unbounded queue. Retiring handlers retain their existing bounded Client Connection slots
until cleanup admission completes. The wire owner observes completion internally. A stopped carrier
closes the broker and all its transient attachments. Worker responses allow twenty seconds for up
to sixteen pending Host viewer/control detaches, two Attach exchanges, and result delivery; control
writes retain their two-second limit. The one queued plus one in-flight bound requires a forty-six-second launch
lease. Client record reads are idle until their first byte, then use a total two-second deadline;
Client writes also have a total two-second deadline and retire only that Connection on expiry.

An exact viewer/control ServerBusy response establishes known-unchanged terminal, viewer and
controller effects. Carrier queue rejection precedes dispatch; a Worker `HostBusy` answer, a viewer
request the Worker had no room to start, keeps the viewer slot and answers ServerBusy, except that a
Detach still releases the slot. An Attach Worker capacity rejection
may follow an OBCN exchange but inserts no viewer and leaves no Host attachment: an Attach whose
inventory walk could not finish detaches before it answers. Input/Resize
may already have consumed the Client sequence, so admitted Client backpressure handling must use
both a fresh request ID and fresh effect sequence under the original logical-operation deadline.
Other unavailable, rejected or unknown results grant no authority to attempt again.

## Local terminal controller refinement

The accepted milestone extends the Connection viewer with explicit acquisition of one combined input
and resize controller per retained Host terminal. Server reserves the grant before Worker I/O;
other Connections cannot acquire while that grant is pending, active, or being cleaned up. A fresh
Server-issued controller identity scopes a strictly increasing sequence shared by input and resize.
Client-supplied identities must equal the selected viewer's grant in the issuing Connection. Rejected non-controllers and stale
sequences never dispatch; valid submitted sequences are consumed even if delivery fails. Unknown
effects are reported without replay, and authority loss never retargets a retained terminal.

Worker maps the grant to a distinct Host attachment with input and resize enabled, read disabled,
and the resize lease requested. Existing viewer attachments remain read-only and keep their frame
baseline. Worker also enforces one grant per Host terminal, its exact viewer/grant scope, and
increasing sequences. Only the Host's exact capability revision authorizes raw input. Input contains
1 through 4,096 bytes; acceptance means admission to Host's bounded input queue, not shell execution.
Resize acknowledges the exact complete geometry. No new Host wire contract or VT owner is added.

Host cleanup releases the control attachment before the viewer. Explicit OBCN Detach performs that order inside Worker; the later Server lease Drop sends an idempotent Release to settle its registry. Connection Drop submits OBCN Release before Detach. The Server reservation
remains occupied until generation-pinned cleanup completes or that Worker becomes unavailable;
cleanup admission uses the existing bounded request queue, with no lock held across I/O. A stale
cleanup cannot clear a new grant. A failed Host exchange ends the Worker connection, and the Server
reports an unknown control outcome after complete delivery; a contradictory Host answer to an
effect refuses only that request as `NotController` and releases the grant. The lease mirrors the
Worker's grant: the Server forgets it, which also frees its reservation, exactly when the Worker
refuses an effect as `NotController` or `ViewerUnavailable`, the answers that mean the Worker
released the grant on a contradictory answer, on a demand its cleanup could not set, or with a
viewer it closed. Such an effect may have reached the Host; the viewer's next effect meets
`NotController` without reaching the Worker, nothing is replayed, and its next Acquire or Takeover
asks the Worker again. Every other refusal, `HostRejected` included, leaves both the grant and the
lease in place. The Server learns of a release only from a refusal, so a Takeover by the
viewer that holds the lease asks the Worker to confirm that grant instead of answering from the
lease: the Worker's `Acquired` returns the same grant and leaves the lease, and its refusal drops
the lease as an effect's `NotController` does. No other viewer is promoted.
Explicit takeover now follows the [controller takeover model](docs/terminal-controller-takeover.md):
Pending reservations fence both competing takeover and stale effects; the candidate owns old/new
cleanup until acknowledged transfer. Independent release while retaining the Viewer remains later work.

At most eight viewers imply at most eight control attachments. Cleanup can therefore perform sixteen
Host exchanges before the next request, all inside that request's fifteen-second
[Worker Host bound](../orchbee-worker/README.md#integrated-authorized-request-loop). The Worker
response budget is twenty seconds,
launch leases last forty-six seconds, and native Client submissions allow fifty seconds. Request
writes and Client record reads/writes retain their existing two-second bounds. These are local
implementation limits, not release performance budgets.

## Local terminal send

An admitted Client may submit one non-interactive input send against an exposed Workbench terminal
it could already close. The selector names the exact session, worker, and terminal binding the Server
validates at that admission; it is not an implicit incarnation lock taken from an earlier list
result. Send is a new explicit orchestration-input authority, not a read-only query and not a
zero-permission change; it adds no remote exposure and bypasses no cohort, peer, or Host capability
check.

Send admission resolves Workbench binding exposure, a unique retained binding, the current Worker
placement and the binding's recorded Worker generation, a Started terminal phase, an Unfenced
lifecycle, and no accepted close. A not-yet-started, closed, fenced, or stale-generation target
rejects before dispatch and consumes no Server-issued identity; resolution never rebinds a stale
incarnation. On admission the Server mints a fresh operation identity and a fresh input-only
attachment identity and pins the exact worker id, generation, Host epoch, and Host terminal id into
exactly one OBCN request; the Worker never re-resolves a logical identifier. If the pinned Worker
exits or changes generation before dispatch, the request fails unavailable or unknown and is never
resent.

The Worker runs one ordered attach, input, detach sequence on its existing Host broker; no viewer,
controller, or resize grant is borrowed. Acceptance means the input entered the Host's bounded
input queue; it says nothing about the child process reading or executing it. A typed Host
rejection still runs the one detach, and so does an intact answer that contradicts the request,
which keeps the Worker-Host connection. After a transport failure or a malformed answer, the Worker
closes that connection — whose teardown reclaims the temporary attachment — and never continues a
new exchange on a possibly desynchronized stream.
Every result keeps two independent facts: input status (not-submitted, rejected, accepted, or
unknown) and cleanup status (not-needed, confirmed, or unconfirmed). A post-acceptance cleanup
failure preserves the accepted byte count and never reports the input as unsent. The Client
contract has no unknown input with confirmed cleanup, so a Worker's unknown input after a confirmed
detach reaches the Client as unconfirmed cleanup with its confirmed prefix.

Each send submits one OBCN request whose identity correlates exactly one request and response; no
persistent dedupe ledger is added, and a manually retried send is a new request that may deliver
input twice. Sends serialize on the single Worker carrier alongside controller exchanges, so input
order equals arrival order. The existing twenty-second Worker response budget, two-second write
bounds, and fifty-second Client submission window all hold; the three Host exchanges share the
Worker's fifteen-second request bound, which fits inside them without enlargement.

## Terminal Client relay delivery

Each viewer has its own frame and history queues and one in-flight record per lane. An overflowing
viewer closes without clearing its peer's work. Feedback must name the exact delivered record,
history delivery preserves the frame flight, and a stalled viewer cannot stop Worker progress.
`local_terminal_service/viewer_connection` and the Worker carrier own these rules; their tests check
exact bytes, capacity, request IDs, attachment generations and codecs over named traces.

## Connection Viewer routing

Viewer identities are immutable and never reused. A controller grant stays reserved while its owner
closes; controller release precedes viewer retirement, and a stale cleanup cannot release a
successor's grant. One grant stands for both the Server lease and the Worker grant. A Worker refusal
meaning the Worker holds no such grant drops an active viewer's lease through the same exact cleanup,
so a grant another viewer's Takeover installed meanwhile stays. Control admission validates
Connection, target, viewer and Worker epoch together. Target disposal or Worker replacement closes
affected bindings.

`local_terminal_service/viewer_connection`, `controller_connection` and Worker carrier ownership
implement these rules. The service unit tests and the `tests/client_process` traces check them,
including capacity, candidate attach failures, frame and history routing, wire validation and
compound operation results; no model explores every interleaving of this core.

## Accepted Connection Viewer history read modeling record

This accepted slice adds one one-shot retained-history read to an existing admitted viewer. It
extends the same Connection/Server/Worker/Host routing spine with a single correlated exchange and no
new long-lived state. The [OBCS contract](../../schemas/client-server/README.md) owns the wire
representation; the OBCN viewer section of the capability-node README owns the tag-6 encoding.

| Field          | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | A one-shot consumer (the native CLI) needs bounded retained scrollback plus visible rows without keeping a viewer, consuming the live feed, or inventing a second read path.                                                                                                                                                                                                                                                                                                     |
| Inputs         | `read-rows` action: owned `attachmentId`, absolute `startRow`, `rowCount` 1–4096, exact pinned `revision`/`generation`, positive `requestSequence`, `maximumBytes` 112–8,388,608 bounding the OBRW payload. The Client computes `startRow`/`rowCount` from one validated frame (`K = min(N, S, 262144 / columns)`, `start = S - K`); Server/Worker treat the action as untrusted and re-validate every field.                                                                    |
| Data and owner | `LocalTerminalConnection` still owns the viewer entry and its pinned Worker handle. Server signs one fresh Worker `OperationId` per dispatch; that identity alone is the inner Host correlation and is never equated with the outer OBCS `requestId`. No per-request history state survives the exchange.                                                                                                                                                                        |
| Outputs        | Exactly one of: complete correlated OBTP response record through OBNR/OBSR, `RowsRejected` with one of the nine closed reasons, or the existing explicit `Rejected`/`Unavailable` outcomes. A byte-budget overrun is a typed failure, never a partial success. `Empty` never answers `read-rows`.                                                                                                                                                                                |
| Routing        | Server resolves the selector within the issuing Connection (unowned → `terminal-unavailable`), revalidates the pinned Worker/generation/current-worker, then dispatches OBCN tag 6. The response record's Host correlation must equal this dispatch's Worker `OperationId`; role, kind, encoding, Host epoch, and attachment must match the request scope. The record bypasses frame-only `projection.receive`. Interleaved Host events still route to their owning attachments. |
| Settlement     | Host `TypedFailure` maps to the closed `RowsRejected` reason set; a legal `RowsRejected` does not retire a healthy viewer. `RowsRejected` on a non-`ReadRows` operation, or a response `Record` in an invalid phase, is a protocol violation. No acknowledgement (`RowsApplied`) is sent for buffer reads, and no implicit retry, resend, or retargeting follows any failure.                                                                                                    |
| Lifetime       | The exchange is bounded by the Worker's ten-second Host exchange bound inside the caller's deadline. Viewer slot release and detach/drop stay with the existing Connection cleanup; nothing here extends viewer lifetime.                                                                                                                                                                                                                                                        |

The action is additive and matched-cohort only: an older Worker rejects tag 6, and a same-cohort
violation (wrong inner correlation, response record in the wrong phase, `RowsRejected` on another
action) fails closed rather than degrading to a guessed interpretation. Worker additionally rejects
a successful response whose actual OBRW payload length exceeds this request's effective budget
(`min(requested, negotiated row-window, record carrier)`); the 64-byte OBTP header counts against
the carrier bound, while `maximumBytes` bounds only the OBRW payload.

## Local terminal coordinator modeling record

The accepted local milestone grants the Server's single effective-user subject default-shell
Create and Start through admitted local Human Client Connections. OBLC alone remains only transport
admission; this private service explicitly selects that policy after peer and cohort validation.
No grant is inferred for another endpoint, remote identity, Worker peer, or renderer kind.

| Field        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Connect admitted Client Create/Start intent to the existing Worker carrier without transferring placement authority, replaying effects, or losing retained bindings when a Client disconnects.                                                                                                                                                                                                                                                                                           |
| Inputs       | Strict OBCS intent, the current admitted Worker identity/generation, and an adapter-supplied absolute lease expiry. No Client field becomes an operation, terminal, lease, generation, or fence.                                                                                                                                                                                                                                                                                         |
| Data         | One Server-owned in-memory map of at most 1,024 create references for its single local subject; each retains exact placement, last authorization, launch mode, and pending, awaiting-start, started, unknown, or unavailable state. A binding leaves the map only by retirement, below; there is no other eviction. Only a Windows or macOS Server that adopts its predecessor's Host restores bindings, from the binding record ([Cold adoption](#cold-adoption)).                      |
| Outputs      | A correlated OBCS result or one exact OBCN command. Missing Worker and unsatisfied constraints are effect-free. Start resolves a successful deferred Create across Connections; unknown/missing creates cannot dispatch.                                                                                                                                                                                                                                                                 |
| Invariants   | A checked monotonic 128-bit allocator never reuses operation identity across terminals or Worker generations. Each dispatched Start advances its terminal fence and uses a fresh lease. One terminal has at most one pending operation. No synchronous lock spans Worker I/O or Client output.                                                                                                                                                                                           |
| Decisions    | Known create-reference collisions reject; full retained capacity and concurrent operations return server-busy, and a Tab, Workspace launch or Panel creation refused at full capacity answers `terminal-limit-reached` to a Client that negotiated it. Same-geometry Start after confirmed deferred Start returns already-started without another command; different geometry rejects. Immediate creates reject Start. Worker replacement never retargets a retained binding.            |
| Model        | A pure prepare/complete state machine reserves a binding before effects and commits through exact results or explicit delivery failures. Independent Connection output cannot block another Connection’s next operation.                                                                                                                                                                                                                                                                 |
| Transitions  | Prepare reserves state before carrier submission. Exact current success commits Host identity and geometry before Client publication. Known pre-delivery Start failure restores awaiting-start; ambiguous delivery or contradictory success freezes the binding as unknown. Previously confirmed Host identity survives failed or unknown Start. Client loss cannot cancel the submitted operation or remove the binding.                                                                |
| Dependencies | The pure coordinator consumes protocol values and explicit time/placement facts. A private service owns short mutex sections, wall-clock sampling, Worker-handle selection and one carrier call. The existing carrier owns wire serialization and complete-write classification.                                                                                                                                                                                                         |
| Ownership    | Server owns subject policy, create references, placement, operation/lease/fence issuance and result commitment. Worker owns launch defaults, admission and Host effects. Host owns PTY/VT and retained execution. No new public or cross-workspace contract.                                                                                                                                                                                                                             |
| Verification | Deterministic coordinator traces check allocation, pending exclusion, exact result commitment, unknown quarantine, capacity, geometry, and replacement. Socket/process traces check cross-Connection Start, serialization, malformed peers and Client loss. The registered launch-authorization and coordinator models check their finite safety/progress claims.                                                                                                                        |
| Limits       | Additive unshipped POSIX local authority only. The 1,024-record bound follows the accepted Worker generation limit; retirement frees records as Closes end terminals. Lease lifetime is forty-six seconds, derived from the carrier's one queued plus one in-flight exchange with a two-second write and twenty-second result deadline each; expiry can reject without retry. No replay ledger, fine-grained permissions, UI activation, restart reconciliation, or production recovery. |

The coordinator's actual consumer is the post-admission OBCS handler. Its integrated witness is a
Client issuing Create and Start through the real Server, Worker, and Host processes. The next
missing user-visible boundary is TUI activation.

[`LocalTerminalCoordinator`](formal/tla/LocalTerminalCoordinator.tla) checks one binding slot and
two peer Connections in three configurations: `replacement` has three operation identities, two
Worker generations, one terminal identity and no Close, `refill` has two operation identities,
one Worker generation, two terminal identities and Closes, so a retired slot admits a fresh
terminal, and `cold-adoption`, below, has two operation identities, two Worker generations, one
terminal identity and Closes. Retirement reads no Worker generation, so splitting them keeps both within the TLC
budget; the generation bound on the Worker's `Retire` is runtime evidence. Prepare maps to
IssueCreate/IssueStart, complete maps to exact/not-delivered/rejected/unknown/stale settlement, and each
Connection's independent publication can interleave another Connection's operation. Progress assumes
bounded carrier completion and weak fairness for settlement and publication/abandonment; it does
not promise delivery to a disconnected Client. Geometry, capacity, type conversion and real PTY
behavior belong to deterministic tests and the process witness. `ImportBinding` moves a settled
binding, awaiting or started, into a replacement Worker's generation with its fence and issued
operations; a binding whose operation was in flight at the loss stays behind and settles as stale.
`ExactCommitWasIssuedToCurrentWorker` keeps any operation from committing against a generation other
than the one it was issued to.
`DisposeTerminal`, `PublishClose`, `RetireBinding` and `ForgetRetired` model the Close that ends a
terminal, the Server's retirement of its binding and the Worker's retirement of its Disposed record.
A message naming a retired terminal may arrive in any later state, so
`RetiredIdNeverDeliveredToLive`, which holds no retired terminal live in any state, keeps it from
reaching a live terminal. `RetiredIdNeverReissued` keeps a retired terminal identity out of the
slot, and `WorkerCountWithinServerCount` keeps the Worker's live count within the Server's binding
count.

In `cold-adoption` the Server and its Worker may end together at any step (`CrashServer`) while the
Host keeps every terminal it runs, unserved. The record the ended Server left is any value its saver
may have written: none, the slot's terminal as it stood at a settled moment, or a terminal retired
since. The next Server serves nothing until its adoption settles: `ImportRecorded` imports the
recorded binding only once the Host confirmed it still runs that terminal, `AdoptMissing` binds a
recorded terminal the Host no longer runs as unavailable, and `AdoptWithoutRecord` leaves every
terminal quarantined. `SettledTerminalIsLive` keeps every settled binding no Close disposed on a
terminal its Worker serves, `RetiredIdNeverReissued` lets a retired identity return only as
unavailable, and `FreshIdsAboveHeld` keeps every identity a Worker or the Host holds below the next
one issued.

`RejectConcurrent` uses the other peer Connection's one-outstanding-request slot as the bounded
correlation surrogate, queues an explicit `server-busy` result, and preserves the original
reservation. `PublicationCopiesExactPredecessor` requires publication to copy and clear that exact
Connection's queued result while preserving every other Connection. Exact OBCS request-ID echo remains
deterministic runtime and codec evidence.

### Terminal binding retirement

A Workbench binding retires once nothing can address its terminal again: a retained Close disposed
exactly the Host terminal the binding started (or its create definitively made none and the Close
published), and no committed Presentation, including a bound snapshot member a Restart could
reuse, names it. The Session checks after each Close publication and on idle Close maintenance
turns, so a member that was rebound or published away retires too. Retirement removes the binding;
a later OBCS start, viewer or send naming its create reference reads it as unknown, exactly as
one never issued, and the monotonic allocator never issues its IDs again. A binding that is
creating, starting or unknown is never retired. Popup bindings keep their own release path.

The Worker retires second. It stops counting a binding against its own 1,024 bound as soon as it
records the terminal Disposed or its Launch definitively failed, before the Server can learn
either; the Server counts the binding until it retires it. The Server's count therefore never
falls below the Worker's, so the Worker never refuses a Launch the Server admitted. Each retired
binding whose Worker holds a Disposed record queues one OBCN `Retire` lifecycle step (tag 7),
which idle Close maintenance sends to the Worker generation that disposed it; the Worker answers
`Retired` (tag 10) and forgets the record, and any later request naming the terminal reads
`BindingUnavailable`. A lost or refused `Retire` only leaves an uncounted record. The queue keeps
at most 1,024 retirements and drops the oldest past that; it drops a retirement whose Worker
generation was replaced.

## Ownership

This application owns local native Workbench authority, Workspace membership, private projection, Worker and Client registries, the OBCN carrier, terminal-intent coordination, and Connection viewer/controller relay. The retained local Desktop composition has its own Workbench writer. Production Browser ingress remains a future capability with no replacement service introduced by legacy retirement.

The importable authority boundary is `src/lib.rs`; `src/main.rs` composes the private binary.
The exhaustive lateral directions are `local_worker_service -> worker_registry`,
`worker_obcn_carrier`, `local_client_service::start` and `local_terminal_service`;
`local_client_service -> client_connection_registry`, `local_terminal_service`, `workbench_service`
and its private `application_connection`; `workbench_service -> local_terminal_service`;
`local_terminal_service -> session_coordinator`, `WorkbenchAuthority`, `terminal_coordinator` value types, `worker_registry`
and `worker_obcn_carrier`; `session_coordinator -> WorkbenchAuthority` and `terminal_coordinator`;
`workbench_authority -> workbench_projection`; and `worker_registry -> worker_obcn_carrier`.
The service constructs the Workbench authority outside locks. SessionCoordinator owns Workspace terminal creation, exact binding selection and shared commit boundary; it composes the two cores without
taking over their facts.

The terminal coordinator's private `outcomes` child owns Worker-observation projection.
SessionCoordinator's private `tab_create` child composes
joint command preparation and settlement with origin-bound, consuming owner tokens;
`terminal_selection` composes canonical exposure, retained target resolution and attachment identity
preparation. WorkbenchAuthority's `terminal_binding_selection` child owns the read-only canonical exposure query;
`terminal_tab_create` owns command admission, prepared membership, reserved ledger bytes and settlement.
The `workspace_directory` children own directory admission and canonical commit in WorkbenchAuthority,
its Session grant composition, and the service's lock-free Worker observation followed by revalidation.
`terminal_tab_move` owns Move admission and prepared permutation; Session checks composition before
target/no-op evaluation. Both share command preflight; Move reuses the authority's prepared
settled-command publication.
LocalTerminalService's parent owns shared entropy acquisition; its `tab_create` child pins Worker
placement under registry-then-Session locks, dispatches outside both, and settles both owners before returning the retained command result. Shared helpers are defined in parents; children do not import siblings.
The service's `viewer_connection` owns attachment lifetime; `controller_connection` owns terminal
reservation, Connection grants, effect sequences and exact release; `projection` owns routing and
feedback. Client `record_stream` owns post-admission I/O deadlines. Reverse and other lateral imports
remain forbidden; tests do not widen production visibility.

The carrier owns I/O, exact echo validation and generation retirement, not OBCS or product policy.
Deterministic cores import no transport, clock or process adapter. Terminal service operations take
registry then Session locks, or registry then controller-map locks for grants. Workbench access takes
only the Session gate after external OBWP policy checking for reads and pure navigation. The opaque
`WorkbenchReadOrNavigationRequest` excludes `tab.create`, `tab.move`, `tab.close`,
`panel.split`, `panel.close`, `panel.move` and
`workspace.primary-directory.accept`, which need their explicit Session composition paths.
Move requires the navigation grant and only the Session gate; `tab.create` additionally requires
the terminal-create grant and uses the composed path above. Binding viewer selection holds registry
and Session locks, then samples the handle's active state; its internal canonical query uses OBCS
subject admission and preserves Workbench-first failure precedence. I/O releases every gate; no automatic retry is added.
The `application_connection` runner dispatches exact OBCS/OBWP prefixes to separate semantic handlers
and imports only their contracts and standard-library I/O. A postcommit coordination failure closes
the mixed carrier without fabricating a command result. The unpublished application library
exposes the authority, Session grants and typed internal membership/exposure/preparation results; whole-projection writing
remains private.

## Multiplexed Client delivery

A TUI promotes its admitted OBLC v1 connection into one OBWP v1 carrier. Request and
subscription stream IDs are connection-local, nonzero and never reused. They are independent
of graphics producer IDs. Logical lane IDs name resource scopes, not additional authenticated
Clients. Terminal viewers and popup owners use the same `(connection generation, lane ID)`;
closing one lane releases only its attachments/popups. The final lane owner and explicit
cancellation use the same cleanup. A completion already executing may finish before cancellation,
but queued lane closure then reclaims that exact owner; it cannot reclaim a sibling lane.
The first successful ClientView Synchronize binds its publishing lane. Only that lane may
synchronize, and its closure retires the view and settles pending control receipts. Synchronize
is never a reconstructable subscription; stable invalidations wake one-shot domain-owned exchanges.

Each carrier has one standard-library reader and one writer/dispatcher. The reader queues input
for the dispatcher in arrival order; the dispatcher executes finite requests and writes outside
mailbox and business locks. A request that waits on a Worker job (`WorkspaceGit`, `WorktreeQuery`,
`WorktreeListRead`) would hold every other stream behind it, so one job-read thread per connection
runs those in arrival order and hands each finished result back to the dispatcher, the only writer.
There are at most 64 active streams and 64 explicit lanes, plus root lane 1. After a `drain`, a
request stream carrying an intent closes with `stopping` before the intent takes effect, and the
Client reports that request as not submitted.

The reader refuses a new stream it cannot take now, and only that stream: once 64 input entries
wait for the dispatcher (`backlog`), while the connection holds 64 streams (`streams`), or when
the request's reservation does not fit the budget (`budget`). It drops the request bytes at once,
and the dispatcher, in arrival order, closes the stream with `capacity` before it runs and logs
`client-stream-failed` at stage `admit`. Cancellations and lane frames still queue behind a full
backlog, up to 1,024 waiting entries; past that the Client has stopped reading its answers and the
connection ends with `capacity`. A `capacity` close before a stream's first frame therefore leaves
its request without effect: either it never ran, or it was a read.

One stream's failure closes only that stream, in a way its Client already handles, and every other
stream and lane keeps serving. A request record that does not decode inside an intact frame closes
with `invalid-request`. A read whose execution fails, or whose record is larger than one frame or
than the connection's remaining budget, closes with `capacity`; a failed subscription update or
resnapshot also releases that subscription's listener. The failure's class goes to the daily log as
`client-stream-failed`, never to the Client. An intent that fails after it ran, or whose record
cannot be delivered, has an outcome nobody can report: its stream closes with `outcome-unknown`,
which the Client reports as unknown and never as not submitted, and the connection keeps serving.
The Server sends that reason only when the handshake answer lists `stream-outcome-unknown`; a
Client without it reads every other close as a request that never ran, so for it such an intent
still ends the connection. Only the carrier itself ends a connection: a message a Client may
not send, a reused stream ID, an unknown lane, 1,024 waiting input entries or a full budget for a
frame that opens no stream, a lane limit, root lane closure,
sustained resnapshots, retained records over budget, a maintenance failure, a Session stop, or a
failed socket write.

A Worker's `TerminalViewerRecordReady` becomes this connection's `ViewerRecordReady`. A viewer
connection registers its attachment in `ViewerReadyRoutes` before it sends Attach, and dropping the
route withdraws it. The wire owner delivers each hint to the route's connection, which wakes the
dispatcher; the dispatcher writes pending hints at the top of its loop. A hint for an attachment with
no route is dropped, which is how a Server-owned sampling viewer stays silent. Hints carry no
authority, stream or reservation, and Poll stays the only way to take a frame.

A Worker's `TerminalTitlesChanged` note reaches the Session title registry (`terminal_titles`) for
the note's own generation. One read pump, started by Session composition, reads the
[physical title changes](../../crates/capability-node-protocol/README.md#physical-title-changes):
at most one read starts per 250 ms, an incomplete page continues at once, a reset rereads from 0,
and a failed read retries 250 ms later. Each read waits in the carrier's idle lane, so it never goes
ahead of input. The registry keeps the last title of at most 1024 terminals of the registered
generation, projected by `project_terminal_title`; registering a new generation or retiring the
carrier makes every title unavailable until the next fresh read. `presentation-titles` answers each
Presentation with its committed Workbench binding and this title, observed only when the retained
physical identity and generation match. Ingesting and publishing titles never commits, advances the
Workbench or report revision, writes the semantic journal, or raises unread state, notifications or
Agent state; a change wakes only the streams that follow titles.
Retained request/response reservations share a 32 MiB budget; they conservatively include
encoded copies. A delivered record reserves three times its length, and that reservation can never
exceed the budget itself (`MAXIMUM_MULTIPLEX_BYTES`), so a record of 11,184,811 bytes or more is
never deliverable. A shorter record is delivered only while the budget has room for its reservation
after the claims the connection already holds, including its own request's. A length whose
reservation overflows is refused as `budget` too. Individual inner records are at most 24 MiB plus
their 12-byte header; a longer one fails as `record-size`. Decoder
storage and one currently decoded record have separate bounded transient allocations; the
reservation count is not an exact process heap bound. Incremental decoder input is capped at
32 MiB with capped geometric capacity growth; after a large frame leaves, an empty or at-most-16-KiB
suffix replaces excess capacity above 64 KiB. The large buffer is not retained as idle scratch.
A Client command queue and output queue
also have fixed bounds. A full budget produces a typed Capacity failure, never unbounded buffering.

Within that total, the Client reserves one stream and 256 KiB for application-progress validation;
ordinary consumers admit at most 63 streams and 32 MiB minus 256 KiB of retained records. After
five seconds without accepted application progress, the sole actor queues one existing read-only
`TerminalLaunchRead`. Its valid nonzero target need not exist; even a Found result is bounded by
the OBCS maximum of 63 KiB payload plus eight header bytes. The Server charges its existing
three-times-record reservation (at most 193560 bytes for this result). Decoder and temporary
parsed values remain separately bounded; this reservation is not a whole-heap measurement.
Stream allocation and FIFO enqueue share the user-command lock, preserving increasing open IDs.
The two-second total deadline starts at probe creation, including queue and write time. Only its
matching stream ID, request identity, actual handoff generation and valid complete Result settle
it. Other streams and retired probe replies cannot renew that deadline. Before a probe is pending,
accepted application frames postpone the next idle probe. Failure means progress cannot be confirmed:
withdraw projections and use ordinary connection recovery without replaying submitted mutations.
Server capacity exhaustion still returns typed Capacity; the control reservation does not promise
passage through an overloaded peer. SSH keepalive alone is not application progress. This is one
connection liveness sample, not per-feature business-state polling.

The commit signal is registered before snapshot capture. A commit concurrent with capture leaves
a newer signal revision, so the dispatcher reevaluates before sleeping. State streams send a
snapshot followed by changed full values; the Client coalesces pending state to one value.
Invalidation streams send the initial semantic snapshot then coalesced eight-byte commit hints.
A hint counts commits only: a terminal output frame wakes just the streams that deliver semantic
values, so a printing terminal never makes a Client reread its checkpoints. A published title change
wakes only `presentation-titles` streams, which follow commits and title changes but never output.
Event streams retain at most 32 pending records per Client stream. Semantic journal subscriptions
retain their Subscribe definition separately from the Read continuation, immediately drain pages
whose `more` flag is set, and use the existing journal epoch/cursor. A live stream exclusively
owns that semantic cursor; competing streams or one-shot Read/Close cannot consume it. Physical
Host probes share one connection-wide capture permit, retained through cancellation and join;
adding subscriptions does not add simultaneous Host reader jobs. Every failed subscription read,
including source resolution before capture sampling, cancels and joins the retained job before
returning so a sibling can acquire the permit. Journal loss or Client inbox
overflow sends Gap before a fresh snapshot; lost events are never presented as complete history.
The third consumer resnapshot within a rolling 30-second sequence terminates that connection as
SlowConsumer. Each output record has a two-second write bound. An unresponsive socket is closed;
a typed disconnect is delivered only when the socket can still accept a complete frame. Other
Clients and commit publication do not wait for that socket. Long semantic Wait requests are
unsupported on the multiplexed dispatcher; direct CLI Wait retains its existing bounded contract.

Workspace metadata expiry supplies an exact next-owner deadline to that same dispatcher. Under
the Session gate, maintenance samples monotonic time after acquiring the lock, commits due expiry,
and returns the next deadline. The dispatcher evaluates resulting invalidations before waiting;
it holds no business lock across socket I/O or the wait. A Report's generic commit notification
recomputes the deadline even when only TTL changes and token values remain equal. Missing
Workbench state has no deadline; retirement or maintenance failure closes the carrier rather than
retrying a past deadline. Pure metadata consumers therefore need no periodic read to drive expiry.

Settings and configuration subscriptions keep no deadline of their own. The Session's one sampler
observes `config.toml` on Windows, and a change it sees publishes through the same commit
notification as an edit or reload, so ordinary carrier frames and Host sampling neither advance nor
postpone delivery. Invalidation mode carries Server commit hints only. No Client polling timer or
additional connection is involved.

Each creation reads the Session's last valid settings once before admission, outside the Session
locks, and its plan attaches that
[launch preference capture](../../crates/capability-node-protocol/README.md#launch-preference-capture)
to the Worker request: scrollback, terminal default colors and, for a default launch, the selected
profile's command. Before startup installs configuration, or for a program beyond the selection
budget, the capture is `Unavailable`, which refuses only that creation. A launch whose capture would not fit one OBCN frame
fails when its terminal is planned, before any Worker dispatch; a Workbench creation then reports
runtime-unavailable `ServerBusy`, as an oversized plugin environment does. The Presentation
publishes the program its binding's launch requests as `launchProgram` in the binding's own
revision: the explicit executable, the custom default program, or the System shell file name that
the admitting Worker generation reported in its hello. The Worker registry attaches that name to
the generation's request handle when its carrier registers, and a creation takes its Worker
placement and the name from the one handle it plans against, so a launch never names another
generation's shell. A saved change reaches only later creations, and running terminals keep their
capture. Startup installs the configuration before any terminal launches, so terminals restored
from a checkpoint capture settings exactly as later creations do.

The Client actor polls its carrier and a local command wake stream together. It owns physical
reconnect, cancellation, IDs and subscription reconstruction. A request cancelled before handoff
is not submitted. Once its write may have begun, transport loss is OutcomeUnknown and the request
is never replayed. Reply correlation uses the generation at actual write handoff, including
requests first queued offline. Reconnect discards the old decoder/queues and recreates only subscriptions;
stream cancellation fences queued and in-flight delivery. Typed overload disconnects are terminal,
so reconnect cannot evade the limit. Logical terminal attachments are not reconstructed by replaying
Attach. One-shot CLI connections keep their direct protocol path.

A subscription delivers a snapshot before any update and a gap before a replacement snapshot; the
connection generation and stream cancellation fence queued and in-flight frames, and publication
never waits on a Client queue. A stream follows one monotone counter chosen by its mode: commits for
an invalidation stream, commits plus title changes for a published-title read, every other
publication otherwise.

Controlled implementation traces in `local_client_service/multiplex/tests.rs` check a commit during
capture, an undrained peer alongside a progressing peer, exclusive event cursors, and late lane
cleanup preserving a sibling, idle metadata expiry after a same-value earlier-TTL report,
maintenance failure without a past-deadline retry loop, a terminal output frame waking a value
stream without an invalidation hint, and a title change waking only the published-title read.
Client transport traces check cancelled late replies, actual event overflow/gap/snapshot delivery
and aggregate reservation refusal. A controlled idle-probe trace keeps unrelated pushes and an
old probe reply flowing until the fixed deadline, then verifies subscription-only recovery and no
replay of an unsettled Reload. Configuration recovery checks that an uncertain reload is not
replayed. Control traces race a changed UI sample with delivery, consume its exact completion,
and admit the next command. A source-loss trace holds a completed capture through the real
subscription read failure and verifies that a sibling can start; it uses a controlled job, not
an OS Host integration witness. View lifecycle traces distinguish sibling and publisher cleanup.
The protocol codec tests reject untrusted lengths, versions, identifiers and tags.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-worker --locked
cargo test -p orchbee-server --all-targets --all-features --locked
cargo clippy -p orchbee-server --all-targets --all-features --locked -- -D warnings
pnpm verify
```

Viewer routing tests execute all 64 generated seeds and require their declared boundary coverage.
Every trace round-trips through the replay codec; one seed also replays against a fresh IPC fixture.
Successful traces are otherwise executed once. A failure retains its original seed and trace and
shrinks against the same failure fingerprint.

The Server's two TLA+ models, `local-terminal-coordinator` and `session-restart-handover`, run only
when they change, as [TLA+ models](../../docs/VERIFICATION.md#tla-models) describes.

## Workspace terminal creation

The Session starts with an empty Workbench. Every terminal-bearing Context has one real Workspace owner; Project membership may be null. Explicit `workspace.launch` or `tab.create` prepares canonical membership and the runtime reservation together, commits once, and launches through that Workspace's admitted Worker/directory. Exact replay returns the retained pending/final result without another launch.

Clients read canonical membership and attach its exact binding. There is no `terminal.open` endpoint, singleton first-terminal reference or Workspace-less fallback. Empty Workspaces remain navigable; an empty Workbench creates no terminal implicitly. See [Workspace membership](docs/workbench-authority.md#workspace-terminal-membership-model), [Tab creation](docs/workbench-authority.md#workspace-tab-creation-model) and [directory admission](docs/workspace-terminal-directory.md).

## Windows native Session

The managed Windows Session keeps the existing in-memory authority and lifecycle guard. It accepts current-token-user named-pipe peers and starts one exact sibling worker, `orchbee-worker.exe` in the release cohort or `<stem>-worker.exe` in an environment cohort, with handle inheritance disabled, so CLI output capture finishes independently of the retained Session. Runtime path, ACL, lock, marker, and cohort validation remain in `orchbee-local-runtime`. The Windows observation ingress accepts one bounded complete JSON value without Unix half-close; malformed connections do not terminate other clients. This ingress adapter does not port the Unix provider hooks. See the [Client quick start](../orchbee/README.md#native-windows-quick-start) for the usable surface and remaining limits.

## Directory Workspace launch

The native unpublished `workspace.launch` command creates a Workspace, accepts its absolute directory and creates its first terminal under one request identity. Placement is `ungrouped`, an existing `project`, a `new-project`, or `alongside` a source Workspace. Alongside uses the source's current Project at final admission; if it is ungrouped, only that source and the new Workspace enter a newly created Project. A closed or closing source is refused. Navigation, directory association and terminal creation grants are all required. Explicit argv, environment and Launch Profiles use the existing bounded launch contracts. Client focus is outside this operation.

The Server checks admission, releases its gates for a read-only Worker directory probe, then rechecks the request, revision, source membership and exact Worker generation under registry-to-Session locking. It prepares six structural identities and any required Project identity in one candidate projection, reserves the retained result and terminal admission, and commits Project placement, Workspace, Directory, Context and first Tab/Panel/Presentation together at one revision. Failed admission publishes no partial Workspace or Project and cannot regroup the source. Terminal dispatch occurs only after commit; pending or failed launch retains all committed identities for inspection and explicit Close. The receipt carries the resulting nullable Project. Exact replay never dispatches again. A lost response is outcome unknown and must not be automatically retried.

This refinement reuses the Tab creation commit and settlement owner. Verification obligations cover grant separation, stale revision and Worker generation across the probe, capacity and identity bounds, exact replay, postcommit launch failure, and a Windows cwd/environment process witness. It does not claim durable restart recovery, Desktop support, focus changes or TLA execution.

## Retained Worktree continuation

Windows and macOS serve Worktree operations, source queries and Workspace group Close; Linux refuses them before any effect. `workbench.worktree.operation` admits Start, pure Read and attempt-fenced Resume. The Workbench authority retains at most 64 operations, one Pending globally, and canonical Workspace association; a lazy maintenance owner continues after Client disconnect. Registry-to-Session admission and publication recheck the source and exact Worker incarnation; OBCN IPC runs outside both gates. Create/Open observe Worker checkout facts before reusing or launching a Workspace. A Create's optional `agentStart` travels to the launched Workspace's first shell as that launch's `agentStart`, under a new Tab's fallback; a reused Workspace launches nothing and starts nothing, and a Resume launches with the retained intent's selector. Remove checks dirty state before Close, reserves new Workspace/directory admissions, holds canonical publication and waits for terminal disposal plus cleanup before Git removal. A known Git failure verifies the original checkout and restores fresh default shells into the retained presentation slots; independent bounded restoration counts preserve failed and unknown launches. Unknown deletion keeps the layout fenced without launching shells. Known success releases publication; final Close retries never repeat Git. Explicit ordinary Close after a failed Worktree can release the hold without Git. Explicit Session stop joins this owner before Close maintenance and Worker carrier freeze. See the [CLI/continuation contract](../orchbee/docs/windows-worktrees.md) and [shared protocol](../../schemas/workbench/v1/workbench-worktree.schema.json) for limits and recovery.

Worktree Start has a required nullable `projectId`: only a Directory source may request an
existing Project; Workspace sources and Remove require `null`. Retained outcomes preserve that
original request metadata. When Workspace-based Create/Open needs a new Workspace after
asynchronous preparation, it uses Alongside placement against the source's current membership and refuses a closed or closing
source. Verified Git associations name stable Workspace/Directory identities independently of
Project membership, so regrouping does not discard retained checkout facts.

## Read-only Worktree source queries

`workbench.worktree.query` resolves an explicit Workspace or literal directory without requiring a Project. It captures the source and Worker incarnation under registry-to-Session gates, runs one bounded discovery job outside the gates, then rechecks before joining current Workspace labels/identities. Multiple associations stay explicit; no arbitrary first match, focus, Workspace creation or mutation receipt is involved. The shared porcelain parser accepts absolute Windows drive paths as well as its existing path forms. Source changes, Worker loss, malformed metadata and expired budgets produce typed failures. See the [owning contract](../orchbee/docs/windows-worktrees.md#source-based-discovery).

## Retained Workspace group Close

Ordinary `workspace.close` removes only the selected Workspace and its terminal context, even when other Workspaces share its Project or Git repository. It does not cascade to physical Git peers or delete Project metadata.

`workbench.workspace-group-close` explicitly closes all members of one `projectId`. Admission captures at most 256 members in current Workspace display order, across any repositories, and composes their ordinary Workspace Close receipts. Workspaces in another Project are excluded even when they share a repository; later members are not added to the retained attempt. Empty Projects are valid targets. The result retains the Project identity, attempt, status and each captured Workspace's identity, label and Close result. Project metadata remains until a separate `project.dissolve`. Group and Worktree fresh mutations reserve each other while Pending. A shared maintenance lifetime adapter owns separate wake/thread/join instances; Session stop joins both submitters before Close maintenance freezes. The [Project close-all contract](../orchbee/docs/windows-worktrees.md#project-close-all) defines limits, partial failure and recovery; [the wire schema](../../schemas/workbench/v1/workbench-workspace-group-close.schema.json) owns public bytes. Group closure does not invoke Git removal.

## Retained Tab set Close

`workbench.tab-set-close` closes several Tabs of one terminal context, in their current order, as one retained operation. Admission freezes each member's bound Presentations; an `if-idle` Start reads every member's terminals once and retains nothing when any member asks. One owner thread submits each member's own Tab Close in turn and records it; a gone Tab counts as done, and the first member that cannot close stops the set while its own Close stays retained for that Tab's Resume or a fresh confirmed Close. Windows Session stop joins this owner before Close maintenance freezes. The [Tab set close contract](docs/terminal-tab-close.md#tab-set-close) defines admission, limits and failure.

## Retained Tab duplication

`workbench.tab-duplicate` copies several Tabs of one terminal context, in their current order, as one retained operation. One owner thread copies each member through an ordinary duplicate Tab creation, right after its source, and records the copy and its terminal outcome; the first member that is gone, refused or whose terminal does not start stops the operation, and nothing is replayed. Windows Session stop joins this owner while the Worker still runs. The [Tab duplication contract](docs/workbench-authority.md#tab-duplication-operation) defines admission, limits and failure.

## Windows Client view routing

Transport admission remains surface-neutral. An admitted Windows Connection may additionally publish one ephemeral interactive view under its Server-assigned generation identity. `client_view_registry` owns bounded routing, freshness and retained delivery receipts; `WorkbenchClientConnection` supplies authenticated ownership and disconnect cleanup. Canonical Workbench authority and terminal carrier ownership stay separate. The [owning Client control contract](../orchbee/docs/windows-client-control.md) defines the wire, post-draw confirmation, lifecycle and evidence; deterministic Client-view lifecycle tests cover the implementation.

## Workspace metadata reports

The [Workspace metadata model](../orchbee/docs/windows-workspace-metadata.md) owns transient token values, per-source sequence fences and monotonic TTL under the existing Session gate. The separate OBWP 31/32 RPC uses read permission for queries and navigation permission for reports, without creating topology ledger entries, changing canonical revision or requiring a terminal Worker. Reports are atomic across token/source/resource limits; stale sequences do not refresh expiry. State is absent after restart, and closed Workspace identities are pruned at the next metadata operation. Reports and expiry publish to the semantic journal; admitted dispatchers wake at the earliest owned TTL deadline.

Panel presentation reports use OBWP 33/34 under the Session gate, with read/navigation permissions and exact primary-binding guards. The [state model](docs/workbench-authority.md#panel-presentation-metadata) owns transient per-terminal sources, field priority, tokens and expiry.

## Native plugin action admission

Workbench plugin execution requires read, terminal-creation and lifecycle grants before source resolution. Local terminal service checks Session/Worker identity, enabled manifest availability, action ambiguity and Windows platform support, freezes argv/cwd/context and submits once to Worker. It does not own subprocesses. Read pages remain bound to Worker generation; lost mutation replies preserve uncertainty. A managed Windows Session also runs enabled plugins' startup commands once per start and their event hooks for journaled semantic events through the same Worker jobs; one hook owner per Session stops before the Worker. [Native plugin behavior](../orchbee/docs/windows-plugins.md) owns the full contract, including delivered events and dropped overlaps.

Plugin Panel Tab/split admission resolves catalog selectors after grant, revision and replay guards, then freezes an explicit program, plugin-root fallback cwd and protected display environment. Both terminal planning and completion corroborate that environment with the frozen launch, cwd, ordinary environment and Worker generation. Resolution refusal precedes canonical reservation; retained requests replay without reading a changed manifest. Batch entrypoints are refused until a literal Windows shell-wrapper contract is available.

Plugin Panel membership is prepared against the candidate canonical snapshot and committed with its terminal reservation. The bounded ephemeral map follows exact Presentation/binding identity; read-only PanelGet prunes lost bindings and returns revision-correlated facts, including when the Worker is unavailable. A retired authority refuses the read.

## Agent integration admission

`workbench.agent-integration` inspection (`list`, `explain`), poll, and cancel require the read
grant. Starting `disable` additionally requires the terminal-creation and lifecycle grants, the
same set as mutating plugin actions. Starting `enable` needs the same grants and is then withheld
in every build and state: the Server answers `worker-unavailable` before the Session check and
dispatches no Worker job, because the user-facing Agent integration was removed. Non-mutating
requests trigger no generic Workbench invalidation. An unauthorized request answers `denied`.

An authorized request must name the current Session, or it answers `session-mismatch`. The Server
relays it to the current Worker as OBCN kind 31 under a fresh operation ID and pairs the kind 32
result by operation ID, job ID, and action. A Start's job ID is its own operation ID, so the
Workbench job `{workerId, generation, jobId}` names exactly one Worker job. A poll or cancel whose
job names another Worker or generation, or a Worker fencing refusal, answers `worker-changed`.
Worker `Busy` and carrier backpressure answer `busy`; a missing Worker, `SpawnThreadFailed`, or
`Dead` answers `worker-unavailable`. A delivered disable start whose result is lost answers
`abandoned`, because its effects are unknown. The Server decodes a completed report against the
contract before answering; an undecodable report answers `malformed`.

## Agent catalog

`agent_catalog::AgentCatalog` holds the Session's Agent catalog: one row per startable provider in
catalog order with its product name, start command line and state, the lookup status, when the
last lookup completed, the Recent list, and a revision that starts at 1 and advances whenever any
of them changes. Recent holds at most 8 provider ids, newest first, each once. A provider moves to
the front when the Session accepts a creation carrying `agentStart` (`tab.create`,
`workspace.launch`, the Worktree driver's first shell, `panel.split`, `panel.create-terminal`), at
the commit before the launch is dispatched, and when a run of a startable provider is first
published, so an Agent typed by hand counts too. The checkpoint saves the list as `recentAgents`
and a restored Session hands it back before any Client connects
([Recent Agents](docs/workbench-checkpoint.md#recent-agents)). Rows and the rule for which providers
can be checked come from `StartableAgent` in
[`orchbee-agent-observation-protocol`](../../crates/agent-observation-protocol/README.md#native-process-input-rules);
a provider whose start command is a generic alias is `cant-check`, and its command is never sent.
`startable` is true on Windows and macOS, the platforms that start Agents.

One pump thread, started by Session composition, keeps the catalog current through the registered
Worker's [Agent probe job](../orchbee-worker/README.md#agent-probe-job), OBCN kinds 37 and 38. Each
exchange takes a fresh random operation ID and waits in the carrier's low lane with discardable
completion. Each Worker generation is looked up as soon as it registers: a Start naming the
checkable commands, then a Poll every 100 milliseconds. A refresh starts another lookup unless one
is running or the last ended under two seconds ago. A lookup fails on any other answer, a lost
exchange, the loss of its Worker, or after ten seconds; a failed lookup the Worker had accepted is
cancelled once. A failed later lookup keeps the last completed rows, and a failed first lookup
leaves checkable rows `unknown`, as does a Linux Worker's `Unsupported`. The module comment holds
the schedule's states and transitions. A Windows Session stop joins the pump before Close
maintenance stops, without waiting for a Worker job, which the Worker ends at its own deadline or
exit.

`workbench.agent-catalog` (OBWP 75/76) is a read-granted query of the catalog. `current` reads it
and `refresh` asks for a lookup under the rule above; both answer at once with the catalog as it
stands. A request naming another Session answers `denied`; before the first Worker registers and
once the Session stops, it answers `unavailable`. It is not withheld elsewhere: a Linux Session
answers `startable: false` with status `unsupported`, and a drained Server still answers it,
`refresh` included. Only `current` opens a State stream, which follows the catalog's own change
count rather than commits or terminal output. The catalog wakes its watching Client connections,
held weakly, after each revision and when it becomes readable or stops being readable; each
connection then reads it once and sends a frame only when the answer changed, so a slow Client gets
the latest catalog and never a backlog. Publishing never waits on a watcher, and a closed
connection leaves nothing to release.

## Terminal runtime

`terminal_runtime::TerminalRuntime`, owned by `LocalTerminalService` and reached through
`terminal_runtime()`, holds whether the Session's terminals are `Serving`, `Reconnecting` while the
Server replaces a lost Worker, or `GivenUp` once it stops trying until the Session stops. It starts
`Serving` at revision 1; `set` advances the revision by one only when the state changes and then
wakes its watchers, held weakly and woken after its lock is released, so a change never waits on a
Client connection. The Worker replacement is its writer, and on Windows and macOS a restart's `prepare` sets
`Reconnecting` once it releases its Worker ([Server and Worker restart](#server-and-worker-restart)).

The read-only [`workbench.terminal-runtime`](../../schemas/workbench/README.md#session-terminal-runtime-reads)
answers from it: `current` for the Session's own Workbench, `denied` for another Session and
`unavailable` before the Workbench exists. Its State streams follow only this state's revisions,
never commits or terminal output; a change wakes every watching connection, which rereads the state
once and sends a frame only when the bytes changed, so a slow Client receives the latest state
rather than a backlog. The Server advertises the read as the `client-server` capability
`terminal-runtime`.

## Agent start delivery

Every Tab, split, Workspace or Worktree creation that carried `agentStart` hands its started
terminal to one Session thread, which makes sure the start happens without any Client typing it:
managed PowerShell, and on POSIX the Host's managed zsh or bash, runs it as its first command, and
a cmd the Host marks gets the bare line once through the prompt gate. A start that did not happen
publishes a one-shot `agentStartFailure` on the Presentation; a `not-found` also marks that
provider's checked [catalog](#agent-catalog) row `not-found` until the next completed lookup
answers again. The thread shares resume's prompt readiness rule, process reads and gated send. The
[start delivery contract](docs/agent-start-delivery.md) owns the states, guards and outcomes.

## Workspace Git observations

`workbench.workspace-git` is a read-granted query of a canonical Workspace on the
current Worker incarnation. It reads the latest exact-physical directory sample of
the Workspace's first terminal, otherwise the committed directory; it never attaches
a viewer for this read. Session, committed directory, canonical revision and Worker
generation are checked before and after the discovery job; no topology or metadata
report is mutated. Status headers supply branch and local upstream counts, with
explicit absence; Git exit 128 is a nonrepository observation without branch facts. The [Workspace sidebar model](../orchbee/docs/machines.md#workspace-sidebar-model)
owns the complete consumer and evidence limits.

Discovery exchanges use nonblocking high-lane admission and absolute-deadline
waits. WorktreeJob and AgentDrain replies remain checked on the sole wire owner,
but an abandoned read completion does not retire a healthy carrier. Effectful
operations retain required delivery. The status reader never resubmits a timed-out
start and queues one nonblocking cancel for its known job identity; physical TTL,
reaping and retained receipt remain Worker-owned if cleanup cannot be admitted.
The read budget does not preempt mutex acquisition or process suspension, and
cancel admission is not confirmed physical cleanup.

### Discovery admission and receipt ownership

`local_terminal_service::worktree_list::DiscoveryGate` coordinates all three
read-only starts: committed Directory lists (`Start`), Workspace/literal-directory
queries (`Start` with `trust_repository`) and sidebar Git observations (`StartStatus`). The Worker
still owns one physical discovery job and one retained receipt. A new accepted
start replaces that receipt, so Server admission covers the entire job lifetime,
including intervals between polls and terminal-result consumption.

The gate retains one active reader and at most one waiting foreground reader.
This represents a physical owner and its single successor, without increasing
Worker capacity or creating a background queue. Sidebar observations are
opportunistic and return their existing typed `Unavailable` when admission is
occupied. A registered foreground list/query owns the next turn; further
foreground contenders receive `ServerBusy` without dispatch.

| State/event                                              | Transition and ownership                                                                                         |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Idle; valid status/list/query                            | Acquire the sole RAII permit before any discovery Start.                                                         |
| Active; first foreground contender                       | Register one successor and wait on the condition variable under the original absolute deadline.                  |
| Active or foreground waiting; status contender           | Refuse immediately; no Worker request or retained status waiter.                                                 |
| Foreground already waiting; another foreground contender | Refuse without replacing the registered successor.                                                               |
| Waiting reader's deadline                                | Remove only that waiter; leave the active job and its permit intact.                                             |
| Active reader consumes a terminal result                 | Release its permit; the registered foreground successor has priority over new status reads.                      |
| Active reader abandons an admitted job                   | Queue one exact-job Cancel without waiting, then release the permit; Worker TTL owns cleanup if admission fails. |

No Session, registry or gate mutex is held across IPC, polling delays or admission
waits. Source, Session and Worker generation fences still surround observation;
organization or terminal resources are never changed by discovery. The gate is
ephemeral and does not survive a Server restart.

All readers keep their original five-second deadline, including admission and
reply waits. The sole wire owner consumes and validates late replies even after
the reader returns. An admitted Start keeps its exact operation/job identity for
Cancel even when acceptance arrives late. Foreground readers may retry only a
definite `Busy` Start refusal while an abandoned physical job settles, with fresh
operation IDs and no deadline renewal. Each retry or poll waits an eighth of the time
the reader has run, from 2 ms up to 50 ms, and 100 ms once it has run two seconds, so a
Git job of a few hundred milliseconds is seen within an eighth of its time. Starts and
polls share a 120-request bound within the reserved 128-identity block, with room for
cancellation; the schedule spends about 97 of them across the five seconds. Accepted starts, unknown outcomes and Git mutations are never
replayed. Physical cleanup and progress still depend on the existing Worker TTL,
carrier and process scheduling; this policy does not promise successful reads
after a Worker loss or a deadline exhausted by an earlier job.

`worktree_list::tests::discovery_*` checks bounded admission, foreground priority
and waiter expiry. The Windows real-LocalStream `worktree_query::tests` traces
cover six sidebar readers competing with both source forms between polls,
committed Directory list/query sharing, active-status handoff, admission timeout,
late Start/Poll replies, exact Cancel and definite-Busy recovery. These controlled
implementation traces cover this gate; the coordinator TLA model does not model
discovery receipt ownership or claim this fix. Live terminal
load and deployed-cohort behavior require separate observation.

## Workspace current directory reads

`workbench.current-directory` (OBWP 65/66) is a read-granted query of where a Workspace is
working, asked by the TUI and the Web and Desktop Server Clients before New Workspace and before
Worktree discovery, from the sidebar or a shortcut, so every Client reads Git in the same directory. A request
naming another Session answers `denied` and a missing Workspace `not-found`. Without `panel` the
Server reads the Workspace's first terminal, the primary terminal of its first Tab's leading Panel.
With `panel` it reads the terminal the asking Client shows in that Panel: the named Presentation,
primary or child, of a Panel in any of the Workspace's Tabs. A Panel the Workspace does not hold, or
a Presentation that Panel does not hold, answers `not-found`; an unbound Presentation, like a
Workspace without terminals, leaves the Workspace's own directory to answer. The Server finds that
terminal among the current unfenced observation targets, then releases the Session lock before
sampling. The
process observer samples that terminal on demand through the same temporary Viewer and
sole pending-cleanup owner as its periodic samples: a valid terminal-reported directory, otherwise
the root process directory, never a child's. The answer drops a trailing separator except at a
drive root. Without a sample the Workspace's own directory answers, and a Workspace without one
answers `unavailable`.

The observer keeps one executing and one waiting read for this kind, apart from Agent explanation
diagnostics, so neither waits behind the other's queue. A waiting read ends the current sampling
turn after its next sample and is served before periodic work resumes; a read still queued after
two seconds attaches nothing and falls back. The read runs on the multiplexer's job-read thread,
changes no Workbench state, and never feeds the [followed directory](../orchbee/docs/native-location-labels.md#automatic-workspace-names),
which keeps its periodic owner. `process_observer::on_demand::tests` checks queue admission,
exact completion, expiry, cancellation and shutdown. On Windows, a real window and a real TUI each
started New Workspace in the shell's directory after `cd`, and the window's read was answered in
14 ms.

## Panel image ownership

The accepted model and current implementation boundary on Windows and macOS are maintained in [Panel graphics](docs/windows-graphics.md). Canonical commits retire invalid layers immediately; authenticated connections own producers, and the shared temporary-viewer observer retains uncertain PNG-read cleanup. UI pixels, capability observations, CLI producers and final composed acceptance remain separately required.

Canonical split ratios are shared by native Clients and retained in checkpoint v6, guarded by catalog v5. The [checkpoint contract](docs/workbench-checkpoint.md) owns strict migration from versions 1–5; the [shared ratio contract](docs/shared-split-ratios.md) owns ratio mutation, projection and compatibility.

## Settings from `config.toml`

The Session owns one `config.toml` location, frozen at startup, and serves it on
`workbench.settings`: `read` answers the current snapshot and `edit` writes one batch of changes
in a single file replace through the settings editor. Every reader the Session runs, including
terminal creation, launch capture, scrollback, restore-output and Agent resume, takes its values
from the same last valid settings, so a committed change reaches only later creations.

The configuration and settings snapshots take their revisions from one counter, and each keeps
the revision of its own last change. A committed edit, or an external edit the sampler observes,
publishes exactly one settings snapshot with a higher revision; a refused edit publishes
nothing. A file that stops parsing or cannot be read keeps the last valid settings: terminal
creation is not blocked, the snapshot reports `syntax-error` with its line and column or
`unreadable`, and an edit against it is refused without writing. Turning
`terminal.restore_after_restart` off removes kept output at once on Windows and macOS, whichever
editor turned it off.
