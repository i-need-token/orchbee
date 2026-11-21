# Windows crash recovery

This contract owns process registration and cold recovery for native Windows default and named
Sessions. It covers cooperating current-user cohort processes and ordinary process interruption.
The Client owns activation; Server and Worker register their directly created Worker and Terminal
Host respectively. Workbench checkpoint interpretation remains owned by Server.

## State and inputs

A Session's catalog record, `sessions/<session>/session.json` in the data root, holds its current
generation. A generation has a nonzero activation identity and one registration for each of Server,
Worker and Host. Registration is `NotStarted`, `Spawning`, or `Identified` with a nonzero Windows
PID and nonzero creation FILETIME. A PID alone is never identity. Windows process observation
returns exact identity alive, exact identity gone, or unknown. A reused PID means the old identity
is gone; it provides no termination authority over the replacement process.

The private record writer lock, `session.lock` beside the record, serializes generation, child registration and recovery publication.
Activation additionally holds the namespace (`internal/sessions.lock`), activation and Session
lifetime guards. Effects derive paths from the admitted Session name and runtime folder, never from
serialized paths. Checkpoint storage
must be idle and all Client, Worker, Host and observation endpoints absent before cold recovery,
except the endpoint of a Host the next generation adopts.

## Registration transitions

| Transition             | Guard                                                                                       | Publication and effect                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Reserve child          | Active generation; slot NotStarted; exact registered parent, or activation owner for Server | Flush Spawning before CreateProcess                                                                             |
| Reserve replacement    | Active generation; exact registered Server; Worker slot registered; Host not Spawning       | Flush Worker Spawning before CreateProcess; Host registration kept                                              |
| Create child           | Same reserved generation and writer guard                                                   | Create suspended, detached child without inherited handles                                                      |
| Identify child         | Exact retained child handle                                                                 | Flush PID and creation FILETIME before execution                                                                |
| Release child          | Identity publication succeeded                                                              | Resume its retained initial thread; release writer guard                                                        |
| Failure or parent loss | Any incomplete phase                                                                        | Keep the record as last written; the writer guard ends with the attempt; never infer absence from a missing PID |

A successfully created child is never released when identity publication fails. Its creator tries
to terminate and confirm exit through the exact owned handle. This startup cleanup authority does
not authorize terminating an orphan or a process discovered by PID. Server/Worker effects must not
bypass registered creation.

A creator still running whose termination stays unconfirmed returns the failure and closes its
handles without terminating the child; a creator lost before the release loses its handles to the
kernel. Either way the child is a suspended orphan: it holds no Session role, lock, pipe or
inherited handle, and it never runs, because only its creator held the handle to the initial thread
that would resume it. Its slot keeps what the attempt last wrote. An attempt that ended before the
identity write leaves the slot Spawning, which cold recovery reads as not started under the
[writer-lock rule](#the-writer-lock-rule), so the orphan blocks no later generation. An attempt that
ended after it, by its creator's loss or a failed resume or deadline, leaves the orphan's exact
identity, which recovery observes alive and refuses until the orphan ends.

Orchbee places registered children in no Job of its own: a child inherits its creator's Jobs, apart
from a Server created outside its activator's Job. A kill-on-close Job ends its processes only when
its last handle closes, and an orphan's creator holds no handle to a Job it merely runs in, so the
creator's exit closes none. The launcher's kill-on-close Job ends the Client with the launcher, but
every process the Client starts leaves it, so an orphaned Server is never in it. An orphan therefore
lasts until a kill-on-close Job it inherited closes, such as the Win32-OpenSSH connection's Job
around a Server that could not leave it, until the computer restarts, or until the person ends it.
Until then it keeps its program file in use, so that file cannot be replaced or deleted; an update
still installs, because it publishes the new version beside the old one rather than replacing its
files.

A replacement Worker reuses the generation's Worker slot. Only the recorded Server reserves it, and
only after it has proved the recorded Worker gone through the handle it created that Worker with,
or knows a failed replacement left no process; the record holds no such proof, so the Server
carries it. The adopted Host keeps its identity, and a Host still Spawning refuses the reservation.
The record therefore admits a Spawning Host only under an identified Worker, and an identified Host
under any Worker registration: a registered one during a replacement, and none in a generation that
adopts its predecessor's Host ([cold adoption](#cold-adoption)). Cold recovery reads a replacement still Spawning like any other
Spawning slot; a reader that predates the rule refuses it as malformed.
[`SessionCrashRecovery`](../formal/tla/SessionCrashRecovery.tla) checks the replacement: its
`replacement` configuration gives one generation two Worker incarnations, and `SingleOwner` and
`OnlyCurrentWorkerRuns` hold only while a replacement waits for its predecessor's proven exit. An
admitted stop starts no replacement, and a stopped Session runs no Worker or Host.

Its `handover` configuration checks the record rules of a restart that keeps terminals, with one
activator and every observation known. A requester reserves a successor Server in the record; the
recorded Server releases the Session to it and ends its Worker while the Host keeps running; the
successor commits by one publish that names itself, its Worker and the kept Host under a fresh
generation, and only while the record still names that successor and the recorded Server. Until
then the recorded Server may take the Session back, and only while the record still names it. Cold
adoption starts a fresh generation that keeps the recorded Host on the same proof recovery takes,
and an `exit` records its end only once its Worker is gone. `ServingIsRecorded` checks that a
Server serving the Session is the one the record names, `SingleOwner` that a released Server the
record no longer names is the only other live Server, until it exits, and `ExitedRunsNoWorker` that
a recorded exit never hides a running Worker. Of these, only the recorded `exit` runs today; the
rest follow the
[Server and Worker restart](../../../proposals/platform/versioning-and-upgrade.md#server-and-worker-restart)
proposal.

## Cold recovery transitions

| Catalog and observed facts                                                                                                     | Result                                                                                                                                                              |
| ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Active; all identified identities gone, a Spawning slot counting as not started; all guards and endpoint/storage checks pass   | Publish Crashed for the old activation; reconcile its marker; begin a fresh generation; the old activation counts as a crash unless the record names it `exited`    |
| Crashed with matching old marker, or marker absent after runtime loss                                                          | Resume the same guarded reconciliation before a fresh generation                                                                                                    |
| Active or Crashed; Server and Worker proven gone, the registered Host alive and named by the binding record; other checks pass | As the first row, but the fresh generation keeps the Host's registration, with Server and Worker NotStarted, for its Server to adopt                                |
| Any state; the record carries a readable [successor entry](#successor-entry)                                                   | First end every live process the entry names, the Worker first; then the row that applies. One that cannot be opened or ended refuses this attempt and retains data |
| Any other exact identity alive, identity query unknown, unsafe storage, conflicting receipt after creation or endpoint present | Refuse and retain data                                                                                                                                              |
| Malformed or unsupported record                                                                                                | Refuse as InvalidData; retain bytes                                                                                                                                 |
| Valid Stopped with current checkpoint evidence                                                                                 | Existing clean-stop restart path; next activation writes version 1                                                                                                  |

A Spawning slot counts as not started only under the [writer-lock rule](#the-writer-lock-rule).

A Server `exit` leaves its generation Active and, once its Worker is gone, writes that generation's
activation identifier to the record's `extensions.exited`. Recovery proves the generation dead by
the same rules but answers no activation to count, so the activator records no crash and no
fallback failure for it. A fresh generation starts without the member, and an `exited` that names
another activation counts nothing.

## Successor entry

A [Server and Worker restart](../../../proposals/platform/versioning-and-upgrade.md#server-and-worker-restart)
records its successor in `session.json` as `extensions.successor`: the ticket, the successor
Server's process ID and creation time, the activation identifier it commits, and, as `worker`, the
process ID and creation time of the Worker that Server created suspended, written under the writer
lock before the Worker is resumed. The commit removes the entry in the publish that names the
successor, so an entry a later reader finds names processes that never became the recorded pair.
Whoever removes it first ends what it names by that identity, the Worker first, inside the same
writer-lock hold and before its publish: `RetainedSessionRecord::fence` for the Server holding the
restart, `clear_successor` for an abandoned entry. An identity that matches no live process is gone
and nothing is ended. An entry this build cannot read is no entry.

Recovery holds the lifetime lock, so both Servers are gone. Under the writer lock, before any
verdict and whatever the record's state, it ends every live process the entry names, the Worker
first, waiting up to 5 seconds for each. A live successor process therefore never refuses recovery,
and no generation adopts the Host beside one; only a process that cannot be opened or ended refuses,
as `RecoveryRequired`, and only that attempt. A fresh generation's record starts without the entry.

## Cold adoption

A generation whose Server and Worker are proven gone while its registered Host still runs is
recovered only when the Session's binding record names that Host: `bindings.json` reads as version
1 with exactly one Host entry whose process ID and creation time equal the registration. Recovery
reads only those members, under the storage guard, and leaves the rest of the record to the
Server. The fresh generation then keeps the Host's Identified registration with Server and Worker
NotStarted, its Host endpoint need not be absent, and its Server adopts the Host from the record
([cold adoption](../../../apps/orchbee-server/README.md#cold-adoption)); the Host's epoch is not
registered, so that adoption checks it. Without such a record the live Host blocks recovery as
before, and the refusal names it. `begin_activation` keeps the Host only from a Crashed predecessor
whose Host it observes running: a Host gone, NotStarted or Spawning starts a generation without
one, and an unknown observation refuses. An adopting generation keeps its predecessor's receipt
across an interrupted activation as an unstarted one does, since it created no process. A reader
that predates the rule refuses the adopting record as malformed.

Recovery never terminates a registered process, fabricates a clean-stop receipt, deletes checkpoint
data, or resumes an Agent into a still-owned terminal; it ends only what a
[successor entry](#successor-entry) names. A live Host is never ended: a binding record that
names it lets the next generation adopt it, and otherwise it blocks recovery. Refusal offers read-only diagnosis: a refusal whose
verdict observed a registered identity alive carries that `RecoveryBlocker`, the first such role in
Server, Worker, Host order and its PID, so the surface that reports the refusal can name the process
whose end, or a restart, clears it. An unknown observation names nothing, and naming grants no
termination authority. A retained record
refuses only the Session it names or whose location it occupies, so an alternate named Session
remains available; this grants no unknown-owner cleanup capability.

A running Server may read its own retained generation's Host registration to learn whether that
Host is absent: NotStarted, or an Identified identity observed gone. Spawning and an unknown
observation are not absence, and the answer grants no termination authority. The Server's
[managed stop](../../../apps/orchbee-server/README.md#managed-windows-session-shutdown) uses it
after Worker loss.

Crashed publication precedes runtime reconciliation. A failure after durable publication is a
partial committed result, retried for the same activation identity under the same guards. New
activation never reuses the old identity. Current record contents, rather than an early retained
copy, carry subsequently registered Host identity into a clean-stop publication.

Catalog publication and runtime marker replacement are separate writes. After a fresh Active version 1
record is published, a complete prior Stopped or Crashed receipt may remain if activation is
interrupted before marker replacement. Recovery admits this boundary only when every role is
NotStarted: the registration-before-create rule proves that the new generation has created no
child, and publishing that generation already required the previous one to be closed. Once any role
is reserved or identified, a conflicting prior receipt is refused. Recovery first publishes Crashed
for the unstarted generation, truncates and flushes the old receipt, then writes its own crash marker.
An interruption leaves either the complete prior receipt or an empty/matching-prefix crash marker;
the same guarded reconciliation can retry without inventing a clean-stop result.

## The writer-lock rule

Cold recovery reads a Spawning slot as not started by one rule that every later change must keep. A
registered child's creator holds the record writer lock from before its Spawning write until after
its identity write and resume, or its failure cleanup; only the attempt's return or its creator's
loss releases it. Every cold-recovery verdict is taken under that same lock:
`SessionCatalog::prepare_activation`, `SessionCatalog::recover_inactive`,
`NamedSessionEntry::cold_recoverable` and `NamedSessionEntry::recovery_blocker` each read the record
and observe its identities while holding it, and `ProcessSnapshot::verdict` takes the held lock as
an argument. A verdict therefore sees a
Spawning slot only once its attempt has ended without an identity, whose child, if one was created,
was never resumed and never runs; the identified roles, among them the slot's creator, decide alone.
A reader without the lock, such as the running Server's Host presence above, cannot tell an attempt
in progress from an ended one and never reads Spawning as absent. A change that releases the lock
anywhere inside that window makes reading Spawning as not started unsound.

## Format and durability

Only current version 1 records with all fields and the exact process registry are admitted.
Other versions and shapes are InvalidData; reads never migrate or delete them. Existing records
from the preceding version 5 writer are also incompatible. Such records remain visible as
`unreadable` under their folder's name; explicit confirmed deletion retains all existing owner/storage
safety checks. Unsupported data grants no process authority. The
[Windows checkpoint storage contract](windows-checkpoint-storage.md) owns stopped evidence and
current storage framing.

Private staging, flush and atomic replacement cover ordinary process crashes; filesystem/device
power-loss durability, removal of the configured runtime base and hostile administrators are
outside the claim. A removed runtime leaf does not erase the catalog record or checkpoint, which
live in the Session folder.

A Session folder takes the Session name, and names compare without letter case, so one name has
one record, one runtime folder, one set of pipes and one store. A record names the runtime folder
it used: after `ORCHBEE_RUNTIME_BASE` changes, a Stopped Session starts through the new folder,
while an Active or Crashed one refuses until its own folder recovers it, so two runtime folders
never own one Session. Two folders spelling one name, which only a case-sensitive directory can
hold, refuse that name. A folder whose name is no Session name, the inventory bound and the deadline
refuse every activation. Diagnosis reports each refusing record with its kind, reason and path. Rendezvous also refuses
directories containing `site-lifetime.lock`, preventing a new
`session-lifetime.lock` from bypassing an existing owner. Neither boundary dual-reads, repairs
or migrates; identity replacement belongs to a recovery workflow that owns the old data and processes.

## Correctness evidence

| Claim                                                                                | Evidence                                                                                            |
| ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| Child execution follows identity publication                                         | Windows suspended-child trace and registration transition tests                                     |
| Live or unknown identity never authorizes recovery                                   | Parser/policy tests and SessionCrashRecovery `ProofIsSound`                                         |
| An attempt that ended without an identity never refuses recovery                     | Policy tests, the native failed-start regression and SessionCrashRecovery `RecoveryNeverStuck`      |
| Old live owner cannot coexist with a newly recovered owner                           | TLA+ bounded two-Client/two-generation model and contended Windows activation traces                |
| PID reuse does not grant termination authority                                       | Pure identity comparison and native identity observation; recovery exposes no termination operation |
| Recovery interruption preserves generation and checkpoint                            | Publication-boundary tests and isolated process-loss restart traces                                 |
| Default and named Sessions share policy                                              | Native Windows integration cases for both routes                                                    |
| A record refuses only the Session it names                                           | Windows catalog regression cases for unreadable and other-runtime records; letter-case is untested  |
| A live Host is recovered only by adoption, and only when the binding record names it | The catalog cold-adoption test                                                                      |
| A recorded `exit` counts no crash and hides no running Worker                        | The `durable_sessions` exit-recovery test and SessionCrashRecovery `ExitedRunsNoWorker`             |

The abstract model retains physical processes across generation changes instead of forgetting old
owners. Suspended creation, identity publication and resume are separate actions; parent loss can
occur between them. A creation attempt holds the writer lock as `pending` until it ends; its
creator's loss or an unconfirmed cleanup ends it and turns an unidentified suspended child into an
orphan that never runs (`OrphansNeverRun`). Recovery decides only with no attempt pending:
`ProofIsSound` checks that it never admits a generation that may still run a process, and
`RecoveryNeverStuck` that it admits every Active generation that can run none once no observation
is unknown or obstructed. Process-query denial and endpoint obstruction reject recovery. The model
has one current exact-registry state space; unsupported records are rejected before entering it.

Its finite bounds and OS/process adapter assumptions do not prove arbitrary kernel behavior or
Host adoption. Actual Windows reboot and human Windows Terminal acceptance are separate evidence.
