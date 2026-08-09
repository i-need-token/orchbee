----------------------- MODULE TerminalRecoveryBinding -----------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Epochs, MainProcesses, NoEpoch, ExitIdentities,
          MainQueueLimit, PendingJournalLimit, PublicationLedgerLimit,
          RejectedLimit, LiveFailureLimit, SymmetryGroupA, SymmetryGroupB

(*
This is a finite policy abstraction, not a filesystem or production-capacity
proof.  The model deliberately separates:

  - volatileQueue: the bounded Main scheduler;
  - pendingJournal: the durable pending owner written before Host-listener return;
  - publicationLedger: the durable published dedupe ledger with its own capacity;
  - rejected: durable terminal rejection/dead-letter state.

The exit-recovery portion is an owner-retention abstraction only. It does not
model session retirement, partial session delivery, sequence allocation,
renderer, transport, or filesystem behavior.

ReleaseMain clears only volatile observations and the volatile scheduler.  A
new Main must explicitly recover pendingJournal entries.  Unknown entries are
not retried by a timer: they either receive exact identity observation and are
promoted, or take the bounded terminal-rejection branch.  Atomic filesystem
replacement, write failures, transport ACK/redelivery, and exact production
capacities remain outside this finite model.

Live publication may succeed on the first attempt or after one or more
observable bounded failures.  publicationCount is a one-handoff guard; it
prevents a second success from duplicating the abstract publication.
*)

NoGeneration == 3
NoRevision == 4
LedgerStates == {
  "absent", "planned", "create-requested", "retained", "exited",
  "lost", "orphaned", "unavailable"
}
InventoryStates == {"empty", "match", "orphan"}
ExitSymmetry == {
  permutationA @@ permutationB :
    permutationA \in Permutations(SymmetryGroupA),
    permutationB \in Permutations(SymmetryGroupB)
}

VARIABLES
  mainOwner,
  ledgerState,
  attemptedEpoch,
  hostEpoch,
  inventory,
  hostCreations,
  orphanObserved,
  documentGeneration,
  openAttachmentGeneration,
  commandAttachmentGeneration,
  staleMessageRejected,
  visibleRevision,
  finalRevision,
  persistedExitRevision,
  exitPublished,
  exitRecovery

vars == <<
  mainOwner,
  ledgerState,
  attemptedEpoch,
  hostEpoch,
  inventory,
  hostCreations,
  orphanObserved,
  documentGeneration,
  openAttachmentGeneration,
  commandAttachmentGeneration,
  staleMessageRejected,
  visibleRevision,
  finalRevision,
  persistedExitRevision,
  exitPublished,
  exitRecovery
>>

Init ==
  /\ mainOwner = {}
  /\ ledgerState = "absent"
  /\ attemptedEpoch = NoEpoch
  /\ hostEpoch \in Epochs
  /\ inventory \in InventoryStates
  /\ hostCreations = 0
  /\ orphanObserved = FALSE
  /\ documentGeneration = 0
  /\ openAttachmentGeneration = NoGeneration
  /\ commandAttachmentGeneration = NoGeneration
  /\ staleMessageRejected = FALSE
  /\ visibleRevision = 0
  /\ finalRevision = NoRevision
  /\ persistedExitRevision = 0
  /\ exitPublished = FALSE
  /\ exitRecovery = [
       observed |-> {},
       volatileQueue |-> {},
       pendingJournal |-> {},
       unknown |-> {},
       identityObserved |-> {},
       presentationAccepted |-> {},
       durableHandoff |-> {},
       adapterCommitted |-> {},
       publicationPending |-> {},
       publicationLedger |-> {},
       visible |-> {},
       rejected |-> {},
       liveFailureCount |-> [identity \in ExitIdentities |-> 0],
       publicationCount |-> [identity \in ExitIdentities |-> 0]
     ]

AcquireMain(main) ==
  /\ main \in MainProcesses
  /\ mainOwner = {}
  /\ mainOwner' = {main}
  /\ UNCHANGED <<ledgerState, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

ReleaseMain ==
  /\ mainOwner # {}
  /\ mainOwner' = {}
  /\ exitRecovery' = [exitRecovery EXCEPT
        !.volatileQueue = {},
        !.identityObserved = {},
        !.presentationAccepted = {},
        !.visible = {}]
  /\ UNCHANGED <<ledgerState, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>

LoadPlanned ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "empty"
  /\ ledgerState' = "planned"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

LoadCorrupt ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "empty"
  /\ ledgerState' = "unavailable"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

ObserveOrphan ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "orphan"
  /\ ledgerState' = "orphaned"
  /\ orphanObserved' = TRUE
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

MarkCreateRequested ==
  /\ mainOwner # {}
  /\ ledgerState = "planned"
  /\ ledgerState' = "create-requested"
  /\ attemptedEpoch' = hostEpoch
  /\ UNCHANGED <<mainOwner, hostEpoch, inventory, hostCreations,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

CreateResponds ==
  /\ mainOwner # {}
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ inventory = "empty"
  /\ ledgerState' = "retained"
  /\ inventory' = "match"
  /\ hostCreations' = hostCreations + 1
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, orphanObserved,
                  documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

CreateResponseLost ==
  /\ mainOwner # {}
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ inventory = "empty"
  /\ inventory' = "match"
  /\ hostCreations' = hostCreations + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

RecoverSameEpoch ==
  /\ mainOwner # {}
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ inventory = "match"
  /\ ledgerState' = "retained"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

ChangeHostEpoch(nextEpoch) ==
  /\ mainOwner # {}
  /\ nextEpoch \in Epochs
  /\ nextEpoch # hostEpoch
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ hostEpoch' = nextEpoch
  /\ inventory' = "empty"
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostCreations,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

MarkCrossEpochLost ==
  /\ mainOwner # {}
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch # hostEpoch
  /\ ledgerState' = "lost"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

StartDocument ==
  /\ mainOwner # {}
  /\ ledgerState \in {"retained", "exited"}
  /\ documentGeneration < 2
  /\ documentGeneration' = documentGeneration + 1
  /\ commandAttachmentGeneration' = NoGeneration
  /\ staleMessageRejected' = FALSE
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  openAttachmentGeneration, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

DetachPreviousAttachment ==
  /\ mainOwner # {}
  /\ openAttachmentGeneration # NoGeneration
  /\ openAttachmentGeneration < documentGeneration
  /\ openAttachmentGeneration' = NoGeneration
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

AttachCurrentDocument ==
  /\ mainOwner # {}
  /\ ledgerState \in {"retained", "exited"}
  /\ inventory = "match"
  /\ documentGeneration > 0
  /\ openAttachmentGeneration = NoGeneration
  /\ openAttachmentGeneration' = documentGeneration
  /\ commandAttachmentGeneration' = documentGeneration
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished, exitRecovery>>

RejectStaleMessage ==
  /\ mainOwner # {}
  /\ openAttachmentGeneration # NoGeneration
  /\ openAttachmentGeneration < documentGeneration
  /\ commandAttachmentGeneration = NoGeneration
  /\ staleMessageRejected' = TRUE
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

HostExited ==
  /\ mainOwner # {}
  /\ ledgerState = "retained"
  /\ ledgerState' = "exited"
  /\ finalRevision' = 2
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

PostExitHistoryEviction ==
  /\ mainOwner # {}
  /\ ledgerState = "exited"
  /\ exitPublished
  /\ finalRevision # NoRevision
  /\ finalRevision < NoRevision - 1
  /\ persistedExitRevision = finalRevision
  /\ finalRevision' = finalRevision + 1
  /\ persistedExitRevision' = finalRevision + 1
  /\ visibleRevision' = finalRevision + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, exitPublished, exitRecovery>>

PostExitHistoryEvictionBeforePublication ==
  /\ mainOwner # {}
  /\ ledgerState = "exited"
  /\ ~exitPublished
  /\ finalRevision # NoRevision
  /\ finalRevision < NoRevision - 1
  /\ finalRevision' = finalRevision + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

CommitFrame ==
  /\ mainOwner # {}
  /\ commandAttachmentGeneration = documentGeneration
  /\ visibleRevision < (IF finalRevision = NoRevision THEN 2 ELSE finalRevision)
  /\ visibleRevision' = visibleRevision + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, finalRevision, persistedExitRevision,
                  exitPublished, exitRecovery>>

PublishExit ==
  /\ mainOwner # {}
  /\ ledgerState = "exited"
  /\ finalRevision # NoRevision
  /\ persistedExitRevision = finalRevision
  /\ visibleRevision >= finalRevision
  /\ ~exitPublished
  /\ exitPublished' = TRUE
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitRecovery>>

RetryExitPersistence ==
  /\ mainOwner # {}
  /\ ledgerState = "exited"
  /\ ~exitPublished
  /\ finalRevision # NoRevision
  /\ persistedExitRevision < finalRevision
  /\ persistedExitRevision' = finalRevision
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished, exitRecovery>>

ExitScalars == <<mainOwner, ledgerState, attemptedEpoch, hostEpoch, inventory,
                 hostCreations, orphanObserved, documentGeneration,
                 openAttachmentGeneration, commandAttachmentGeneration,
                 staleMessageRejected, visibleRevision, finalRevision,
                 persistedExitRevision, exitPublished>>

AdmitExitToVolatile(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in ExitIdentities
  /\ identity \notin er.observed
  /\ Cardinality(er.pendingJournal) < PendingJournalLimit
  /\ Cardinality(er.volatileQueue) < MainQueueLimit
  /\ exitRecovery' = [er EXCEPT
        !.observed = @ \cup {identity},
        !.pendingJournal = @ \cup {identity},
        !.volatileQueue = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

AdmitExitToDurablePending(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in ExitIdentities
  /\ identity \notin er.observed
  /\ Cardinality(er.pendingJournal) < PendingJournalLimit
  /\ Cardinality(er.volatileQueue) >= MainQueueLimit
  /\ exitRecovery' = [er EXCEPT
        !.observed = @ \cup {identity},
        !.pendingJournal = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

RejectExitAdmission(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in ExitIdentities
  /\ identity \notin er.observed
  /\ Cardinality(er.pendingJournal) >= PendingJournalLimit
  /\ Cardinality(er.rejected) < RejectedLimit
  /\ exitRecovery' = [er EXCEPT
        !.observed = @ \cup {identity},
        !.rejected = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

RecoverDurablePending(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.pendingJournal
  /\ identity \notin er.volatileQueue
  /\ identity \notin er.unknown
  /\ identity \notin er.publicationLedger
  /\ Cardinality(er.volatileQueue) < MainQueueLimit
  /\ exitRecovery' = [er EXCEPT !.volatileQueue = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

ObserveIdentity(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in ExitIdentities
  /\ identity \notin er.identityObserved
  /\ exitRecovery' = [er EXCEPT !.identityObserved = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

ClassifyUnknown(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.volatileQueue
  /\ identity \in er.pendingJournal
  /\ identity \notin er.presentationAccepted
  /\ identity \notin er.durableHandoff
  /\ identity \notin er.adapterCommitted
  /\ identity \notin er.publicationPending
  /\ identity \notin er.publicationLedger
  /\ identity \notin er.visible
  /\ exitRecovery' = [er EXCEPT
        !.volatileQueue = @ \ {identity},
        !.unknown = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

PromoteObservedUnknown(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.unknown
  /\ identity \in er.identityObserved
  /\ Cardinality(er.volatileQueue) < MainQueueLimit
  /\ exitRecovery' = [er EXCEPT
        !.unknown = @ \ {identity},
        !.volatileQueue = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

RejectUnknown(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.unknown
  /\ identity \notin er.identityObserved
  /\ Cardinality(er.rejected) < RejectedLimit
  /\ exitRecovery' = [er EXCEPT
        !.unknown = @ \ {identity},
        !.pendingJournal = @ \ {identity},
        !.rejected = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

AcceptPresentation(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.volatileQueue
  /\ identity \in er.pendingJournal
  /\ identity \notin er.rejected
  /\ exitRecovery' = [er EXCEPT !.presentationAccepted = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

PersistAssociatedExit(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.volatileQueue
  /\ identity \in er.presentationAccepted
  /\ exitRecovery' = [er EXCEPT
        !.volatileQueue = @ \ {identity},
        !.durableHandoff = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

CommitAdapterPublication(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ exitRecovery' = [er EXCEPT
        !.durableHandoff = @ \ {identity},
        !.adapterCommitted = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

ReservePublication(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.durableHandoff
  /\ identity \in er.presentationAccepted
  /\ identity \notin er.publicationLedger
  /\ identity \notin er.publicationPending
  /\ exitRecovery' = [er EXCEPT !.publicationPending = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

MarkPublished(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ identity \in er.adapterCommitted
  /\ Cardinality(er.publicationLedger) < PublicationLedgerLimit
  /\ exitRecovery' = [er EXCEPT
        !.publicationLedger = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

PublishVisible(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationLedger
  /\ identity \in er.presentationAccepted
  /\ identity \notin er.visible
  /\ exitRecovery' = [er EXCEPT
        !.visible = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

LivePublicationFailure(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ identity \in er.publicationLedger
  /\ identity \in er.visible
  /\ er.liveFailureCount[identity] < LiveFailureLimit
  /\ exitRecovery' = [er EXCEPT
        !.liveFailureCount = [@ EXCEPT ![identity] = @ + 1]]
  /\ UNCHANGED ExitScalars

LivePublicationSuccess(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ identity \in er.publicationLedger
  /\ identity \in er.visible
  /\ er.publicationCount[identity] = 0
  /\ exitRecovery' = [er EXCEPT
        !.publicationPending = @ \ {identity},
        !.publicationCount = [@ EXCEPT ![identity] = @ + 1]]
  /\ UNCHANGED ExitScalars

RecoverPublishedVisibility(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationLedger
  /\ identity \in er.pendingJournal
  /\ identity \notin er.visible
  /\ exitRecovery' = [er EXCEPT
        !.presentationAccepted = @ \cup {identity},
        !.publicationPending = @ \cup {identity},
        !.visible = @ \cup {identity}]
  /\ UNCHANGED ExitScalars

CompactPending(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.visible \cup er.rejected
  /\ (identity \in er.rejected \/ (identity \in er.visible /\ er.publicationCount[identity] > 0))
  /\ identity \in er.pendingJournal
  /\ exitRecovery' = [er EXCEPT
        !.pendingJournal = @ \ {identity},
        !.volatileQueue = @ \ {identity},
        !.unknown = @ \ {identity},
        !.durableHandoff = @ \ {identity},
        !.publicationPending = @ \ {identity}]
  /\ UNCHANGED ExitScalars

ExitRecoveryNext ==
  \/ \E identity \in ExitIdentities: AdmitExitToVolatile(identity)
  \/ \E identity \in ExitIdentities: AdmitExitToDurablePending(identity)
  \/ \E identity \in ExitIdentities: RejectExitAdmission(identity)
  \/ \E identity \in ExitIdentities: RecoverDurablePending(identity)
  \/ \E identity \in ExitIdentities: ObserveIdentity(identity)
  \/ \E identity \in ExitIdentities: ClassifyUnknown(identity)
  \/ \E identity \in ExitIdentities: PromoteObservedUnknown(identity)
  \/ \E identity \in ExitIdentities: RejectUnknown(identity)
  \/ \E identity \in ExitIdentities: AcceptPresentation(identity)
  \/ \E identity \in ExitIdentities: PersistAssociatedExit(identity)
  \/ \E identity \in ExitIdentities: CommitAdapterPublication(identity)
  \/ \E identity \in ExitIdentities: ReservePublication(identity)
  \/ \E identity \in ExitIdentities: MarkPublished(identity)
  \/ \E identity \in ExitIdentities: PublishVisible(identity)
  \/ \E identity \in ExitIdentities: LivePublicationFailure(identity)
  \/ \E identity \in ExitIdentities: LivePublicationSuccess(identity)
  \/ \E identity \in ExitIdentities: RecoverPublishedVisibility(identity)
  \/ \E identity \in ExitIdentities: CompactPending(identity)

Next ==
  \/ \E main \in MainProcesses: AcquireMain(main)
  \/ ReleaseMain
  \/ LoadPlanned
  \/ LoadCorrupt
  \/ ObserveOrphan
  \/ MarkCreateRequested
  \/ CreateResponds
  \/ CreateResponseLost
  \/ RecoverSameEpoch
  \/ \E epoch \in Epochs: ChangeHostEpoch(epoch)
  \/ MarkCrossEpochLost
  \/ StartDocument
  \/ DetachPreviousAttachment
  \/ AttachCurrentDocument
  \/ RejectStaleMessage
  \/ HostExited
  \/ PostExitHistoryEvictionBeforePublication
  \/ PostExitHistoryEviction
  \/ CommitFrame
  \/ RetryExitPersistence
  \/ PublishExit
  \/ ExitRecoveryNext

Spec == Init /\ [][Next]_vars

ExitRecoveryNextWithAcquire ==
  \/ \E main \in MainProcesses: AcquireMain(main)
  \/ ExitRecoveryNext

(*
The default recovery configuration starts with the Main-acquisition step and
then explores only ExitRecoveryNext.  The combined Spec remains available for
targeted experiments, but composing the independent lifecycle and recovery
actions creates a combinatorial state space that is not a useful default gate.
*)
ExitRecoverySpec == Init /\ [][ExitRecoveryNextWithAcquire]_vars

TypeOK ==
  /\ mainOwner \subseteq MainProcesses
  /\ ledgerState \in LedgerStates
  /\ attemptedEpoch \in Epochs \cup {NoEpoch}
  /\ hostEpoch \in Epochs
  /\ inventory \in InventoryStates
  /\ hostCreations \in 0..1
  /\ orphanObserved \in BOOLEAN
  /\ documentGeneration \in 0..2
  /\ openAttachmentGeneration \in 0..NoGeneration
  /\ commandAttachmentGeneration \in 0..NoGeneration
  /\ staleMessageRejected \in BOOLEAN
  /\ visibleRevision \in 0..3
  /\ finalRevision \in 0..NoRevision
  /\ persistedExitRevision \in 0..NoRevision
  /\ exitPublished \in BOOLEAN
  /\ exitRecovery.observed \subseteq ExitIdentities
  /\ exitRecovery.volatileQueue \subseteq ExitIdentities
  /\ exitRecovery.pendingJournal \subseteq ExitIdentities
  /\ exitRecovery.unknown \subseteq ExitIdentities
  /\ exitRecovery.identityObserved \subseteq ExitIdentities
  /\ exitRecovery.presentationAccepted \subseteq ExitIdentities
  /\ exitRecovery.durableHandoff \subseteq ExitIdentities
  /\ exitRecovery.adapterCommitted \subseteq ExitIdentities
  /\ exitRecovery.publicationPending \subseteq ExitIdentities
  /\ exitRecovery.publicationLedger \subseteq ExitIdentities
  /\ exitRecovery.visible \subseteq ExitIdentities
  /\ exitRecovery.rejected \subseteq ExitIdentities
  /\ exitRecovery.liveFailureCount \in [ExitIdentities -> 0..LiveFailureLimit]
  /\ exitRecovery.publicationCount \in [ExitIdentities -> Nat]
  /\ Cardinality(exitRecovery.volatileQueue) <= MainQueueLimit
  /\ Cardinality(exitRecovery.pendingJournal) <= PendingJournalLimit
  /\ Cardinality(exitRecovery.publicationLedger) <= PublicationLedgerLimit
  /\ Cardinality(exitRecovery.rejected) <= RejectedLimit

SingleMainOwner == Cardinality(mainOwner) <= 1

NoAmbiguousRecreate == hostCreations <= 1

UnavailableLedgerFailsClosed ==
  ledgerState = "unavailable" => hostCreations = 0

OrphanInventoryIsUntouched ==
  orphanObserved => hostCreations = 0

CurrentDocumentOwnsCommands ==
  commandAttachmentGeneration # NoGeneration =>
    /\ commandAttachmentGeneration = documentGeneration
    /\ openAttachmentGeneration = commandAttachmentGeneration

OnePhysicalAttachment ==
  openAttachmentGeneration = NoGeneration \/ openAttachmentGeneration \in 0..2

FinalRevisionBeforeExit ==
  exitPublished => finalRevision # NoRevision /\ visibleRevision >= finalRevision

DurableExitRevision ==
  /\ persistedExitRevision <= finalRevision
  /\ exitPublished => persistedExitRevision = finalRevision

ExitRecoveryObservedOwnership ==
  LET er == exitRecovery IN
  er.observed = er.pendingJournal \cup er.publicationLedger \cup er.rejected

ExitRecoveryVolatileOwnership ==
  LET er == exitRecovery IN
  er.volatileQueue \subseteq er.pendingJournal

ExitRecoveryUnknownOwnership ==
  LET er == exitRecovery IN
  er.unknown \subseteq er.pendingJournal

ExitRecoveryHandoffOwnership ==
  LET er == exitRecovery IN
  er.durableHandoff \subseteq er.pendingJournal

ExitRecoveryPublicationReservationOwnership ==
  LET er == exitRecovery IN
  er.publicationPending \subseteq er.pendingJournal

ExitRecoveryAdapterOwnership ==
  LET er == exitRecovery IN
  er.adapterCommitted \subseteq er.observed

ExitRecoveryLedgerOwnership ==
  LET er == exitRecovery IN
  er.publicationLedger \subseteq er.adapterCommitted

ExitRecoveryVisibleOwnership ==
  LET er == exitRecovery IN
  /\ er.visible \subseteq er.publicationLedger
  /\ er.visible \subseteq er.presentationAccepted

ExitRecoveryDisjointOwnership ==
  LET er == exitRecovery IN
  /\ er.pendingJournal \cap er.rejected = {}
  /\ er.publicationLedger \cap er.rejected = {}
  /\ er.volatileQueue \cap er.unknown = {}

ExitRecoveryOwnership ==
  /\ ExitRecoveryObservedOwnership
  /\ ExitRecoveryVolatileOwnership
  /\ ExitRecoveryUnknownOwnership
  /\ ExitRecoveryHandoffOwnership
  /\ ExitRecoveryPublicationReservationOwnership
  /\ ExitRecoveryAdapterOwnership
  /\ ExitRecoveryLedgerOwnership
  /\ ExitRecoveryVisibleOwnership
  /\ ExitRecoveryDisjointOwnership

MainExitQueueBound == Cardinality(exitRecovery.volatileQueue) <= MainQueueLimit

PendingJournalAdmissionBound ==
  Cardinality(exitRecovery.pendingJournal) <= PendingJournalLimit

PublicationLedgerAdmissionBound ==
  Cardinality(exitRecovery.publicationLedger) <= PublicationLedgerLimit

RejectedAdmissionBound ==
  Cardinality(exitRecovery.rejected) <= RejectedLimit

ExitPublicationAfterDurability ==
  exitRecovery.visible \subseteq exitRecovery.publicationLedger

ExitPublicationNoDuplicate ==
  [] (\A identity \in ExitIdentities:
        exitRecovery.publicationCount[identity] <= 1)

FinalRevisionMonotonic ==
  [] [((finalRevision = NoRevision) \/ (finalRevision' = NoRevision)
       \/ (finalRevision' >= finalRevision))]_vars

PersistedRevisionMonotonic ==
  [] [(persistedExitRevision' >= persistedExitRevision)]_vars

ReleaseClearsVolatileState ==
  [] (mainOwner = {} => exitRecovery.volatileQueue = {})

=============================================================================
