------------------------- MODULE TerminalExitRecovery -------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Finite owner-retention abstraction for Host exit events crossing Desktop
\* Main release/reacquisition. It models a bounded volatile queue, durable
\* pending journal, publication reservation/ledger, terminal rejection, and
\* visibility recovery. Transport delivery and filesystem atomicity remain
\* outside this model and are owned by their refinement evidence.

CONSTANTS MainProcesses, ExitIdentities, MainQueueLimit, PendingJournalLimit,
          PublicationLedgerLimit, RejectedLimit, LiveFailureLimit

ExitSymmetry == Permutations(ExitIdentities)

VARIABLES mainOwner, exitRecovery

vars == <<mainOwner, exitRecovery>>

Init ==
  /\ mainOwner = {}
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
       publicationBlocked |-> {},
       visible |-> {},
       rejected |-> {},
       rejectionUnavailable |-> {},
       liveFailureCount |-> [identity \in ExitIdentities |-> 0],
       publicationCount |-> [identity \in ExitIdentities |-> 0]
     ]

AcquireMain(main) ==
  /\ main \in MainProcesses
  /\ mainOwner = {}
  /\ mainOwner' = {main}
  /\ UNCHANGED exitRecovery

ReleaseMain ==
  /\ mainOwner # {}
  /\ mainOwner' = {}
  /\ exitRecovery' = [exitRecovery EXCEPT
       !.volatileQueue = {},
       !.identityObserved = {},
       !.presentationAccepted = {},
       !.visible = {}]

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
  /\ UNCHANGED mainOwner

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
  /\ UNCHANGED mainOwner

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
  /\ UNCHANGED mainOwner

RecoverDurablePending(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.pendingJournal
  /\ identity \notin er.volatileQueue
  /\ identity \notin er.unknown
  /\ identity \notin er.publicationLedger
  /\ Cardinality(er.volatileQueue) < MainQueueLimit
  /\ exitRecovery' = [er EXCEPT !.volatileQueue = @ \cup {identity}]
  /\ UNCHANGED mainOwner

ObserveIdentity(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in ExitIdentities
  /\ identity \notin er.identityObserved
  /\ exitRecovery' = [er EXCEPT !.identityObserved = @ \cup {identity}]
  /\ UNCHANGED mainOwner

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
  /\ UNCHANGED mainOwner

PromoteObservedUnknown(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.unknown
  /\ identity \in er.identityObserved
  /\ identity \notin er.rejectionUnavailable
  /\ Cardinality(er.volatileQueue) < MainQueueLimit
  /\ exitRecovery' = [er EXCEPT
        !.unknown = @ \ {identity},
        !.volatileQueue = @ \cup {identity}]
  /\ UNCHANGED mainOwner

RejectUnknown(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.unknown
  /\ identity \notin er.identityObserved
  /\ identity \notin er.rejectionUnavailable
  /\ Cardinality(er.rejected) < RejectedLimit
  /\ exitRecovery' = [er EXCEPT
        !.unknown = @ \ {identity},
        !.pendingJournal = @ \ {identity},
        !.rejected = @ \cup {identity}]
  /\ UNCHANGED mainOwner

AcceptPresentation(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.volatileQueue
  /\ identity \in er.pendingJournal
  /\ identity \notin er.rejected
  /\ exitRecovery' = [er EXCEPT !.presentationAccepted = @ \cup {identity}]
  /\ UNCHANGED mainOwner

PersistAssociatedExit(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.volatileQueue
  /\ identity \in er.presentationAccepted
  /\ exitRecovery' = [er EXCEPT
        !.volatileQueue = @ \ {identity},
        !.durableHandoff = @ \cup {identity}]
  /\ UNCHANGED mainOwner

CommitAdapterPublication(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ exitRecovery' = [er EXCEPT
        !.durableHandoff = @ \ {identity},
        !.adapterCommitted = @ \cup {identity}]
  /\ UNCHANGED mainOwner

ReservePublication(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.durableHandoff
  /\ identity \in er.presentationAccepted
  /\ identity \notin er.publicationLedger
  /\ identity \notin er.publicationPending
  /\ exitRecovery' = [er EXCEPT !.publicationPending = @ \cup {identity}]
  /\ UNCHANGED mainOwner

MarkPublished(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ identity \in er.adapterCommitted
  /\ identity \notin er.publicationBlocked
  /\ Cardinality(er.publicationLedger) < PublicationLedgerLimit
  /\ exitRecovery' = [er EXCEPT
        !.publicationLedger = @ \cup {identity}]
  /\ UNCHANGED mainOwner

ClassifyPublicationLedgerFull(identity) ==
  \* The committed exit remains durably pending. This safety classification
  \* makes no promise that external ledger capacity will later become free.
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ identity \in er.adapterCommitted
  /\ identity \notin er.publicationLedger
  /\ identity \notin er.publicationBlocked
  /\ Cardinality(er.publicationLedger) >= PublicationLedgerLimit
  /\ exitRecovery' = [er EXCEPT
        !.publicationBlocked = @ \cup {identity}]
  /\ UNCHANGED mainOwner

FailClosedRejectionUnavailable(identity) ==
  \* A full rejection ledger cannot erase the already durable unknown exit.
  \* The model records the terminal failure without assuming later recovery.
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.unknown
  /\ identity \notin er.identityObserved
  /\ identity \notin er.rejectionUnavailable
  /\ Cardinality(er.rejected) >= RejectedLimit
  /\ exitRecovery' = [er EXCEPT
        !.rejectionUnavailable = @ \cup {identity}]
  /\ UNCHANGED mainOwner

PublishVisible(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationLedger
  /\ identity \in er.presentationAccepted
  /\ identity \notin er.visible
  /\ exitRecovery' = [er EXCEPT
        !.visible = @ \cup {identity}]
  /\ UNCHANGED mainOwner

LivePublicationFailure(identity) ==
  LET er == exitRecovery IN
  /\ mainOwner # {}
  /\ identity \in er.publicationPending
  /\ identity \in er.publicationLedger
  /\ identity \in er.visible
  /\ er.liveFailureCount[identity] < LiveFailureLimit
  /\ exitRecovery' = [er EXCEPT
        !.liveFailureCount = [@ EXCEPT ![identity] = @ + 1]]
  /\ UNCHANGED mainOwner

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
  /\ UNCHANGED mainOwner

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
  /\ UNCHANGED mainOwner

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
  /\ UNCHANGED mainOwner

Next ==
  \/ \E main \in MainProcesses: AcquireMain(main)
  \/ ReleaseMain
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
  \/ \E identity \in ExitIdentities: ClassifyPublicationLedgerFull(identity)
  \/ \E identity \in ExitIdentities: FailClosedRejectionUnavailable(identity)
  \/ \E identity \in ExitIdentities: PublishVisible(identity)
  \/ \E identity \in ExitIdentities: LivePublicationFailure(identity)
  \/ \E identity \in ExitIdentities: LivePublicationSuccess(identity)
  \/ \E identity \in ExitIdentities: RecoverPublishedVisibility(identity)
  \/ \E identity \in ExitIdentities: CompactPending(identity)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ mainOwner \subseteq MainProcesses
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
  /\ exitRecovery.publicationBlocked \subseteq ExitIdentities
  /\ exitRecovery.visible \subseteq ExitIdentities
  /\ exitRecovery.rejected \subseteq ExitIdentities
  /\ exitRecovery.rejectionUnavailable \subseteq ExitIdentities
  /\ exitRecovery.liveFailureCount \in [ExitIdentities -> 0..LiveFailureLimit]
  /\ exitRecovery.publicationCount \in [ExitIdentities -> Nat]
  /\ Cardinality(exitRecovery.volatileQueue) <= MainQueueLimit
  /\ Cardinality(exitRecovery.pendingJournal) <= PendingJournalLimit
  /\ Cardinality(exitRecovery.publicationLedger) <= PublicationLedgerLimit
  /\ Cardinality(exitRecovery.rejected) <= RejectedLimit

SingleMainOwner == Cardinality(mainOwner) <= 1

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

PublicationCapacityFailureRetainsDurableOwnership ==
  LET er == exitRecovery IN
  /\ er.publicationBlocked \subseteq
       er.pendingJournal \cap er.publicationPending \cap er.adapterCommitted
  /\ er.publicationBlocked \cap
       (er.publicationLedger \cup er.visible \cup er.rejected) = {}
  /\ (er.publicationBlocked # {} =>
        Cardinality(er.publicationLedger) = PublicationLedgerLimit)

RejectionCapacityFailureRetainsDurableOwnership ==
  LET er == exitRecovery IN
  /\ er.rejectionUnavailable \subseteq er.unknown \cap er.pendingJournal
  /\ er.rejectionUnavailable \cap
       (er.presentationAccepted \cup er.durableHandoff \cup
        er.adapterCommitted \cup er.publicationPending \cup
        er.publicationLedger \cup er.visible \cup er.rejected) = {}
  /\ (er.rejectionUnavailable # {} =>
        Cardinality(er.rejected) = RejectedLimit)

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
  /\ PublicationCapacityFailureRetainsDurableOwnership
  /\ RejectionCapacityFailureRetainsDurableOwnership
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
  \A identity \in ExitIdentities:
    exitRecovery.publicationCount[identity] <= 1

ReleasedMainOwnsNoVolatileState ==
  mainOwner = {} =>
    /\ exitRecovery.volatileQueue = {}
    /\ exitRecovery.identityObserved = {}
    /\ exitRecovery.presentationAccepted = {}
    /\ exitRecovery.visible = {}

=============================================================================
