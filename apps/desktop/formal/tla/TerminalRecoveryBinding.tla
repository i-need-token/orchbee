----------------------- MODULE TerminalRecoveryBinding -----------------------
EXTENDS FiniteSets, Naturals, TLC

\* Finite abstraction of Desktop Main ownership, Host creation recovery,
\* renderer-document generations, attachment replacement, and final-revision
\* publication. Exit-event admission and durable replay are independently
\* owned by TerminalExitRecovery so neither state space can mask the other.

CONSTANTS Epochs, MainProcesses, NoEpoch

NoGeneration == 3
NoRevision == 4
LedgerStates == {
  "absent", "planned", "create-requested", "retained", "exited",
  "lost", "orphaned", "unavailable"
}
InventoryStates == {"empty", "match", "orphan"}
PostExitMutationPhases == {"before-publication", "after-publication"}

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
  rejectedPostExitMutations

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
  rejectedPostExitMutations
>>

Init ==
  /\ mainOwner = {}
  /\ ledgerState = "absent"
  /\ attemptedEpoch = NoEpoch
  /\ hostEpoch \in Epochs
  /\ inventory \in {"empty", "orphan"}
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
  /\ rejectedPostExitMutations = {}

AcquireMain(main) ==
  /\ main \in MainProcesses
  /\ mainOwner = {}
  /\ mainOwner' = {main}
  /\ UNCHANGED <<ledgerState, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

ReleaseMain ==
  \* This is an Electron Main restart. The durable binding/exit facts and the
  \* external Host inventory survive; Main-owned document and attachment
  \* authority do not.
  /\ mainOwner # {}
  /\ mainOwner' = {}
  /\ documentGeneration' = 0
  /\ openAttachmentGeneration' = NoGeneration
  /\ commandAttachmentGeneration' = NoGeneration
  /\ staleMessageRejected' = FALSE
  /\ UNCHANGED <<ledgerState, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, visibleRevision,
                  finalRevision, persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

LoadPlanned ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "empty"
  /\ ledgerState' = "planned"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

LoadCorrupt ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "empty"
  /\ ledgerState' = "unavailable"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

MarkCreateRequested ==
  /\ mainOwner # {}
  /\ ledgerState = "planned"
  /\ ledgerState' = "create-requested"
  /\ attemptedEpoch' = hostEpoch
  /\ UNCHANGED <<mainOwner, hostEpoch, inventory, hostCreations,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, persistedExitRevision,
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

MarkCrossEpochLost ==
  /\ mainOwner # {}
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch # hostEpoch
  /\ ledgerState' = "lost"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

DetachPreviousAttachment ==
  /\ mainOwner # {}
  /\ openAttachmentGeneration # NoGeneration
  /\ openAttachmentGeneration < documentGeneration
  /\ openAttachmentGeneration' = NoGeneration
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  persistedExitRevision, exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

HostExited ==
  /\ mainOwner # {}
  /\ ledgerState = "retained"
  /\ ledgerState' = "exited"
  /\ finalRevision' = 2
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, persistedExitRevision,
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

RejectPostExitMutationAfterPublication ==
  /\ mainOwner # {}
  /\ ledgerState = "exited"
  /\ exitPublished
  /\ finalRevision # NoRevision
  /\ "after-publication" \notin rejectedPostExitMutations
  /\ rejectedPostExitMutations' =
       rejectedPostExitMutations \cup {"after-publication"}
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>

RejectPostExitMutationBeforePublication ==
  /\ mainOwner # {}
  /\ ledgerState = "exited"
  /\ ~exitPublished
  /\ finalRevision # NoRevision
  /\ "before-publication" \notin rejectedPostExitMutations
  /\ rejectedPostExitMutations' =
       rejectedPostExitMutations \cup {"before-publication"}
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  persistedExitRevision, exitPublished>>

CommitFrame ==
  /\ mainOwner # {}
  /\ commandAttachmentGeneration = documentGeneration
  /\ visibleRevision < (IF finalRevision = NoRevision THEN 2 ELSE finalRevision)
  /\ visibleRevision' = visibleRevision + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, finalRevision, persistedExitRevision,
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  persistedExitRevision>>
  /\ UNCHANGED rejectedPostExitMutations

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
                  exitPublished>>
  /\ UNCHANGED rejectedPostExitMutations

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
  \/ CommitFrame
  \/ RetryExitPersistence
  \/ PublishExit
  \/ RejectPostExitMutationBeforePublication
  \/ RejectPostExitMutationAfterPublication

Spec == Init /\ [][Next]_vars

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
  /\ rejectedPostExitMutations \subseteq PostExitMutationPhases

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

ReleasedMainOwnsNoDocumentState ==
  mainOwner = {} =>
    /\ documentGeneration = 0
    /\ openAttachmentGeneration = NoGeneration
    /\ commandAttachmentGeneration = NoGeneration
    /\ ~staleMessageRejected

RejectedPostExitMutationIsDiagnostic ==
  rejectedPostExitMutations # {} =>
    /\ ledgerState = "exited"
    /\ finalRevision # NoRevision

FinalRevisionFreezeStep ==
  ledgerState # "exited" \/ finalRevision' = finalRevision

FinalRevisionFrozenAfterExit == [] [FinalRevisionFreezeStep]_vars

PersistedRevisionMonotonic ==
  [] [(persistedExitRevision' >= persistedExitRevision)]_vars

=============================================================================
