----------------------- MODULE TerminalRecoveryBinding -----------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Epochs, MainProcesses, NoEpoch

NoGeneration == 3
NoRevision == 3
LedgerStates == {
  "absent", "planned", "create-requested", "retained", "exited",
  "lost", "orphaned", "unavailable"
}
InventoryStates == {"empty", "match", "orphan"}

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
  exitPublished

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
  exitPublished
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
  /\ exitPublished = FALSE

AcquireMain(main) ==
  /\ main \in MainProcesses
  /\ mainOwner = {}
  /\ mainOwner' = {main}
  /\ UNCHANGED <<ledgerState, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished>>

LoadPlanned ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "empty"
  /\ ledgerState' = "planned"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished>>

LoadCorrupt ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "empty"
  /\ ledgerState' = "unavailable"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished>>

ObserveOrphan ==
  /\ mainOwner # {}
  /\ ledgerState = "absent"
  /\ inventory = "orphan"
  /\ ledgerState' = "orphaned"
  /\ orphanObserved' = TRUE
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, exitPublished>>

MarkCreateRequested ==
  /\ ledgerState = "planned"
  /\ ledgerState' = "create-requested"
  /\ attemptedEpoch' = hostEpoch
  /\ UNCHANGED <<mainOwner, hostEpoch, inventory, hostCreations,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, exitPublished>>

CreateResponds ==
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ inventory = "empty"
  /\ ledgerState' = "retained"
  /\ inventory' = "match"
  /\ hostCreations' = hostCreations + 1
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, orphanObserved,
                  documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, exitPublished>>

CreateResponseLost ==
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ inventory = "empty"
  /\ inventory' = "match"
  /\ hostCreations' = hostCreations + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, exitPublished>>

RecoverSameEpoch ==
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ inventory = "match"
  /\ ledgerState' = "retained"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished>>

ChangeHostEpoch(nextEpoch) ==
  /\ nextEpoch \in Epochs
  /\ nextEpoch # hostEpoch
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch = hostEpoch
  /\ hostEpoch' = nextEpoch
  /\ inventory' = "empty"
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostCreations,
                  orphanObserved, documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision, exitPublished>>

MarkCrossEpochLost ==
  /\ ledgerState = "create-requested"
  /\ attemptedEpoch # hostEpoch
  /\ ledgerState' = "lost"
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished>>

StartDocument ==
  /\ ledgerState \in {"retained", "exited"}
  /\ documentGeneration < 2
  /\ documentGeneration' = documentGeneration + 1
  /\ commandAttachmentGeneration' = NoGeneration
  /\ staleMessageRejected' = FALSE
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  openAttachmentGeneration, visibleRevision, finalRevision,
                  exitPublished>>

DetachPreviousAttachment ==
  /\ openAttachmentGeneration # NoGeneration
  /\ openAttachmentGeneration < documentGeneration
  /\ openAttachmentGeneration' = NoGeneration
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, finalRevision,
                  exitPublished>>

AttachCurrentDocument ==
  /\ ledgerState \in {"retained", "exited"}
  /\ inventory = "match"
  /\ documentGeneration > 0
  /\ openAttachmentGeneration = NoGeneration
  /\ openAttachmentGeneration' = documentGeneration
  /\ commandAttachmentGeneration' = documentGeneration
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, staleMessageRejected, visibleRevision,
                  finalRevision, exitPublished>>

RejectStaleMessage ==
  /\ openAttachmentGeneration # NoGeneration
  /\ openAttachmentGeneration < documentGeneration
  /\ commandAttachmentGeneration = NoGeneration
  /\ staleMessageRejected' = TRUE
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, visibleRevision,
                  finalRevision, exitPublished>>

HostExited ==
  /\ ledgerState = "retained"
  /\ ledgerState' = "exited"
  /\ finalRevision' = 2
  /\ UNCHANGED <<mainOwner, attemptedEpoch, hostEpoch, inventory,
                  hostCreations, orphanObserved, documentGeneration,
                  openAttachmentGeneration, commandAttachmentGeneration,
                  staleMessageRejected, visibleRevision, exitPublished>>

CommitFrame ==
  /\ commandAttachmentGeneration = documentGeneration
  /\ visibleRevision < 2
  /\ visibleRevision' = visibleRevision + 1
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  finalRevision, exitPublished>>

PublishExit ==
  /\ ledgerState = "exited"
  /\ finalRevision # NoRevision
  /\ visibleRevision >= finalRevision
  /\ ~exitPublished
  /\ exitPublished' = TRUE
  /\ UNCHANGED <<mainOwner, ledgerState, attemptedEpoch, hostEpoch,
                  inventory, hostCreations, orphanObserved,
                  documentGeneration, openAttachmentGeneration,
                  commandAttachmentGeneration, staleMessageRejected,
                  visibleRevision, finalRevision>>

Next ==
  \/ \E main \in MainProcesses: AcquireMain(main)
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
  \/ PublishExit

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
  /\ visibleRevision \in 0..2
  /\ finalRevision \in 0..NoRevision
  /\ exitPublished \in BOOLEAN

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

=============================================================================
