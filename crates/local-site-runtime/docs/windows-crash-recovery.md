# Windows crash recovery

This contract owns process registration and cold recovery for native Windows default and named
Sites. It covers cooperating current-user cohort processes and ordinary process interruption.
The Client owns activation; Server and Worker register their directly created Worker and Terminal
Host respectively. Workbench checkpoint interpretation remains owned by Server.

## State and inputs

A catalog generation has a nonzero activation identity and one registration for each of Server,
Worker and Host. Registration is `NotStarted`, `Spawning`, or `Identified` with a nonzero Windows
PID and nonzero creation FILETIME. A PID alone is never identity. Windows process observation
returns exact identity alive, exact identity gone, or unknown. A reused PID means the old identity
is gone; it provides no termination authority over the replacement process.

The private record writer lock serializes generation, child registration and recovery publication.
Activation additionally holds the namespace, activation and Site lifetime guards. Effects derive
paths from admitted scope and current-user inputs, never from serialized paths. Checkpoint storage
must be idle and all Client, Worker, Host and observation endpoints absent before cold recovery.

## Registration transitions

| Transition             | Guard                                                                                       | Publication and effect                                                |
| ---------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Reserve child          | Active generation; slot NotStarted; exact registered parent, or activation owner for Server | Flush Spawning before CreateProcess                                   |
| Create child           | Same reserved generation and writer guard                                                   | Create suspended, detached child without inherited handles            |
| Identify child         | Exact retained child handle                                                                 | Flush PID and creation FILETIME before execution                      |
| Release child          | Identity publication succeeded                                                              | Resume its retained initial thread; release writer guard              |
| Failure or parent loss | Any incomplete phase                                                                        | Preserve uncertain registration; never infer absence from missing PID |

A successfully created child is never released when identity publication fails. Its creator tries
to terminate and confirm exit through the exact owned handle. Unconfirmed cleanup remains an
uncertain generation. This startup cleanup authority does not authorize terminating an orphan or
a process discovered by PID. Server/Worker effects must not bypass registered creation.

## Cold recovery transitions

| Catalog and observed facts                                                                                                         | Result                                                                                 |
| ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Active v2–v5; no Spawning; all identified identities gone; all guards and endpoint/storage checks pass                             | Publish Crashed for the old activation; reconcile its marker; begin a fresh generation |
| Crashed v2–v5 with matching old marker, or marker absent after runtime loss                                                        | Resume the same guarded reconciliation before a fresh generation                       |
| Any exact identity alive, identity query unknown, Spawning, unsafe storage, conflicting receipt after creation or endpoint present | Refuse and retain data                                                                 |
| Active v1, malformed or unsupported record                                                                                         | Refuse; absence of legacy identity is not death evidence                               |
| Valid Stopped v1 or v2 with its legacy plain primary                                                                               | Existing clean-stop restart path; next activation writes v5                            |
| Valid Stopped v3–v5 with current checkpoint evidence                                                                               | Existing clean-stop restart path; next activation writes v5                            |

Recovery never terminates old processes, fabricates a clean-stop receipt, deletes checkpoint data,
or resumes an Agent into a still-owned terminal. A live orphan Host remains protected and requires
the separately owned Host-adoption protocol. Refusal offers read-only diagnosis and an alternate
named Site; it does not claim an unknown-owner cleanup capability.

Crashed publication precedes runtime reconciliation. A failure after durable publication is a
partial committed result, retried for the same activation identity under the same guards. New
activation never reuses the old identity. Current record contents, rather than an early retained
copy, carry subsequently registered Host identity into a clean-stop publication.

Catalog publication and runtime marker replacement are separate writes. After a fresh Active v5
record is published, a complete prior Stopped or Crashed receipt may remain if activation is
interrupted before marker replacement. Recovery admits this boundary only when every role is
NotStarted: the registration-before-create rule proves that the new generation has created no
child, and publishing that generation already required the previous one to be closed. Once any role
is reserved or identified, a conflicting prior receipt is refused. Recovery first publishes Crashed
for the unstarted generation, truncates and flushes the old receipt, then writes its own crash marker.
An interruption leaves either the complete prior receipt or an empty/matching-prefix crash marker;
the same guarded reconciliation can retry without inventing a clean-stop result.

## Compatibility and durability

Catalog v1 and v2 remain readable. A valid Stopped legacy record can take the clean-stop activation
path only with its plain primary. Active or Crashed v2 retains its exact-identity cold-recovery
path: after every identity is proved gone and the old generation is reconciled, the fresh
activation publishes v5 and may then read either the legacy plain primary or an absent store.
Active v1 still refuses because it has no death proof. Reading alone never migrates any record.
Catalog v3 retains the v2 exact process registry and declares the framed multi-slot
checkpoint layout; v4 also admits canonical shared ratios, and v5 admits optional Project organization
with global Workspace and Directory identities. The new Active v5 record is atomically published before marker commitment,
Server spawn or multi-slot storage effects. Earlier readers that admit only v1–v4 reject v5,
including after clean stop; there is no automatic downgrade or inferred legacy cleanup. The
[Windows checkpoint storage contract](windows-checkpoint-storage.md) owns the version table,
stopped evidence and old-cohort no-effect boundary.

Private staging, flush and atomic replacement cover ordinary process crashes; filesystem/device
power-loss durability, removal of the configured runtime base and hostile administrators are
outside the claim. A removed runtime leaf does not erase the durable catalog or checkpoint.

## Correctness evidence

| Claim                                                                 | Evidence                                                                                            |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Child execution follows identity publication                          | Windows suspended-child trace and registration transition tests                                     |
| Incomplete registration and unknown identity never authorize recovery | Parser/policy tests and WindowsSiteCrashRecovery safety checks                                      |
| Old live owner cannot coexist with a newly recovered owner            | TLA+ bounded two-Client/two-generation model and contended Windows activation traces                |
| PID reuse does not grant termination authority                        | Pure identity comparison and native identity observation; recovery exposes no termination operation |
| Recovery interruption preserves generation and checkpoint             | Publication-boundary tests and isolated process-loss restart traces                                 |
| Default and named Sites share policy                                  | Native Windows integration cases for both routes                                                    |

The abstract model retains physical processes across generation changes instead of forgetting old
owners. Suspended creation, identity publication and resume are separate actions; parent loss can
occur between them. Process-query denial, legacy records and endpoint obstruction reject recovery.
Its `legacy` state maps specifically to v1 without process identity. Its registered non-legacy
states refine v2–v5 exact registries; the model does not enumerate those four catalog
versions as separate process states.
Its finite bounds and OS/process adapter assumptions do not prove arbitrary kernel behavior or
Host adoption. Actual Windows reboot and human Windows Terminal acceptance are separate evidence.
