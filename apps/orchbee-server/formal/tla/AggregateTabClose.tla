------------------------ MODULE AggregateTabClose ------------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(*
   A bounded aggregate Tab Close with two exact terminal targets and one
   unrelated Tab.  The operation owns one accepted command/revision credit,
   while each captured target owns its lifecycle progress, pending flight,
   disposal proof and local cleanup fact.  One target may be in flight at a
   time, but a parked sibling never changes another target's evidence.

   This is an owner model, not a host protocol model.  A known Disposed state
   abstracts an exact Worker receipt and local cleanup is a separate step.
   Unknown is a terminal quarantine: it cannot be resumed or published.
   Ordinary label/order/create writes may rebase the eventual publication, but
   they cannot change the captured A/B membership.
*)

CONSTANTS A, B, C, None, MaxRevision, MaxGeneration, MaxSteps
ASSUME /\ Cardinality({A, B, C, None}) = 4
       /\ MaxRevision = 3
       /\ MaxGeneration = 3
       /\ MaxSteps = 2

Targets == {A, B}
Identities == {A, B, C}
Controls == {"one", "two", "three"}

EmptyTarget == [phase |-> "absent", active |-> FALSE, pending |-> FALSE,
                expired |-> FALSE, disposed |-> FALSE, cleanup |-> FALSE,
                steps |-> 0]
EmptyClose == [phase |-> "absent", acceptedRevision |-> -1,
               generation |-> 0, credit |-> FALSE,
               targets |-> [t \in Targets |-> EmptyTarget]]

VARIABLE s
vars == <<s>>

Members == {s.tabs[i] : i \in 1..Len(s.tabs)}
Accepted == s.close.phase # "absent"
Closed == s.close.phase = "closed"
Credits == IF s.close.credit THEN 1 ELSE 0
FlightTarget == s.flight
AllDisposed == \A t \in Targets:
  s.close.targets[t].disposed
AllCleaned == \A t \in Targets:
  s.close.targets[t].cleanup
AllPublishable == AllDisposed /\ AllCleaned /\ ~s.retired
CanWrite == ~s.retired /\ s.revision + Credits + 1 <= MaxRevision
CanCharge == s.entries < 4
RemoveTab(tabs) == SelectSeq(tabs, LAMBDA member: member # A /\ member # B)

Init == s =
  [tabs |-> <<A, B>>, created |-> FALSE, labels |-> [i \in Identities |-> "created"],
   revision |-> 0, writes |-> 0, entries |-> 0, moved |-> FALSE,
   renamed |-> FALSE, retired |-> FALSE, close |-> EmptyClose,
   alias |-> None, flight |-> None, flightGeneration |-> 0,
   prepared |-> FALSE, preparedRevision |-> -1, preparedTabs |-> <<>>]

AdmitClose ==
  /\ ~Accepted
  /\ ~s.retired
  /\ A \in Members
  /\ B \in Members
  /\ CanCharge
  /\ s.revision + Credits + 1 <= MaxRevision
  /\ s' = [s EXCEPT !.close = [EmptyClose EXCEPT
       !.phase = "accepted", !.acceptedRevision = s.revision,
       !.generation = 1, !.credit = TRUE,
       !.targets = [t \in Targets |-> [EmptyTarget EXCEPT
          !.phase = "closing", !.active = TRUE]]],
       !.entries = @ + 1]

JoinAlias ==
  /\ s.close.phase = "accepted"
  /\ s.alias = None
  /\ CanCharge
  /\ s' = [s EXCEPT !.alias = A, !.entries = @ + 1]

ReplayClose == /\ s.close.phase = "accepted" /\ UNCHANGED vars
ConflictClose == /\ s.close.phase = "accepted" /\ UNCHANGED vars

RefuseCapacity == /\ ~CanCharge /\ UNCHANGED vars
RefuseRevision == /\ s.revision + Credits + 1 > MaxRevision /\ UNCHANGED vars

OrdinaryCreate ==
  /\ ~s.created
  /\ CanWrite
  /\ CanCharge
  /\ s' = [s EXCEPT !.tabs = Append(@, C), !.created = TRUE,
       !.revision = @ + 1, !.writes = @ + 1, !.entries = @ + 1]

OrdinaryRename ==
  /\ ~s.renamed
  /\ B \in Members
  /\ CanWrite
  /\ CanCharge
  /\ s' = [s EXCEPT !.labels[B] = "renamed",
       !.renamed = TRUE, !.revision = @ + 1,
       !.writes = @ + 1, !.entries = @ + 1]

OrdinaryMove ==
  /\ ~s.moved
  /\ Len(s.tabs) >= 2
  /\ CanWrite
  /\ CanCharge
  /\ s' = [s EXCEPT !.tabs = Append(Tail(@), Head(@)),
       !.moved = TRUE, !.revision = @ + 1,
       !.writes = @ + 1, !.entries = @ + 1]

SubmitStep(t) ==
  /\ s.close.phase = "accepted"
  /\ t \in Targets
  /\ s.close.targets[t].phase = "closing"
  /\ s.close.targets[t].active
  /\ ~s.close.targets[t].expired
  /\ ~s.close.targets[t].pending
  /\ s.flight = None
  /\ s.close.targets[t].steps < MaxSteps
  /\ s' = [s EXCEPT !.flight = t,
       !.flightGeneration = s.close.generation,
       !.close.targets[t].pending = TRUE,
       !.close.targets[t].steps = @ + 1]

ExpireAttempt(t) ==
  /\ t \in Targets
  /\ s.close.targets[t].active
  /\ ~s.close.targets[t].expired
  /\ s' = [s EXCEPT !.close.targets[t].expired = TRUE]

ParkBudget(t) ==
  /\ t \in Targets
  /\ s.close.targets[t].active
  /\ s.close.targets[t].expired
  /\ s.flight # t
  /\ s' = [s EXCEPT !.close.targets[t].active = FALSE,
       !.close.targets[t].phase = IF s.close.targets[t].disposed
          THEN "disposed" ELSE "blocked"]

CompleteBlocked(t) ==
  /\ s.flight = t
  /\ s.flightGeneration = s.close.generation
  /\ s.close.targets[t].pending
  /\ s' = [s EXCEPT !.flight = None, !.flightGeneration = 0,
       !.close.targets[t] = [@ EXCEPT !.phase = "blocked",
          !.active = FALSE, !.pending = FALSE]]

CompleteUnknown(t) ==
  /\ s.flight = t
  /\ s.flightGeneration = s.close.generation
  /\ s.close.targets[t].pending
  /\ s' = [s EXCEPT !.flight = None, !.flightGeneration = 0,
       !.close.targets[t] = [@ EXCEPT !.phase = "unknown",
          !.active = FALSE, !.pending = FALSE]]

CompleteDisposed(t) ==
  /\ s.flight = t
  /\ s.flightGeneration = s.close.generation
  /\ s.close.targets[t].pending
  /\ s' = [s EXCEPT !.flight = None, !.flightGeneration = 0,
       !.close.targets[t] = [@ EXCEPT !.phase = "disposed",
          !.disposed = TRUE, !.active = ~s.close.targets[t].expired,
          !.pending = FALSE]]

WrongTargetSettlement ==
  /\ s.flight \in Targets
  /\ UNCHANGED vars

Resume(t) ==
  /\ s.close.phase = "accepted"
  /\ t \in Targets
  /\ s.flight = None
  /\ s.close.targets[t].phase \in {"blocked", "disposed"}
  /\ ~s.close.targets[t].active
  /\ s.close.targets[t].phase # "unknown"
  /\ s.close.generation < MaxGeneration
  /\ s' = [s EXCEPT !.close.generation = @ + 1,
       !.close.targets[t] = [@ EXCEPT !.phase = "closing",
          !.active = TRUE, !.expired = FALSE,
          !.pending = FALSE]]

ResumeReplay == /\ s.close.phase = "accepted" /\ UNCHANGED vars
ResumeStale == /\ s.close.phase = "accepted" /\ UNCHANGED vars
ResumeFuture == /\ s.close.phase = "accepted" /\ UNCHANGED vars
ResumeNotResumable == /\ s.close.phase = "accepted" /\ UNCHANGED vars
ResumeExhausted == /\ s.close.generation = MaxGeneration /\ UNCHANGED vars

CleanupSuccess(t) ==
  /\ t \in Targets
  /\ s.close.targets[t].phase = "disposed"
  /\ s.close.targets[t].active
  /\ ~s.close.targets[t].cleanup
  /\ s' = [s EXCEPT !.close.targets[t].cleanup = TRUE]

CleanupFailure(t) ==
  /\ t \in Targets
  /\ s.close.targets[t].phase = "disposed"
  /\ s.close.targets[t].active
  /\ ~s.close.targets[t].cleanup
  /\ s' = [s EXCEPT !.close.targets[t].active = FALSE]

PreparePublication ==
  /\ s.close.phase = "accepted"
  /\ AllPublishable
  /\ ~s.prepared
  /\ A \in Members
  /\ B \in Members
  /\ s' = [s EXCEPT !.prepared = TRUE,
       !.preparedRevision = s.revision, !.preparedTabs = RemoveTab(s.tabs)]

PreparationFailure ==
  /\ s.close.phase = "accepted"
  /\ AllPublishable
  /\ ~s.prepared
  /\ UNCHANGED vars

ReleasePreparedGate ==
  /\ s.prepared
  /\ UNCHANGED vars

RejectStalePreparation ==
  /\ s.prepared
  /\ s.preparedRevision # s.revision
  /\ s' = [s EXCEPT !.prepared = FALSE, !.preparedRevision = -1,
       !.preparedTabs = <<>>]

CommitRemoval ==
  /\ s.close.phase = "accepted"
  /\ s.prepared
  /\ s.preparedRevision = s.revision
  /\ AllPublishable
  /\ s.close.credit
  /\ s' = [s EXCEPT !.tabs = s.preparedTabs,
       !.revision = @ + 1, !.writes = @ + 1,
       !.close = [@ EXCEPT !.phase = "closed", !.credit = FALSE],
       !.prepared = FALSE, !.preparedRevision = -1,
       !.preparedTabs = <<>>]

Retire ==
  /\ ~s.retired
  /\ s' = [s EXCEPT !.retired = TRUE]

StatusRead == UNCHANGED vars

ObserveTwoTargetAcceptance == /\ Accepted /\ UNCHANGED vars
ObserveOneTargetDisposed ==
  /\ Accepted
  /\ \E t \in Targets: s.close.targets[t].disposed
  /\ UNCHANGED vars
ObserveUnknownQuarantine ==
  /\ \E t \in Targets: s.close.targets[t].phase = "unknown"
  /\ UNCHANGED vars
ObserveLateReceipt ==
  /\ s.flight \in Targets
  /\ \E t \in Targets: s.close.targets[t].expired
  /\ UNCHANGED vars
ObserveWrongTargetOrdinal == /\ s.flight \in Targets /\ UNCHANGED vars
ObserveRebasedPublication ==
  /\ Closed
  /\ s.close.acceptedRevision < s.revision
  /\ UNCHANGED vars
ObserveUnrelatedTabSurvives ==
  /\ Closed
  /\ C \in Members
  /\ UNCHANGED vars
ObserveExactOneRevision ==
  /\ Closed
  /\ s.close.credit = FALSE
  /\ UNCHANGED vars

Next ==
  \/ AdmitClose \/ JoinAlias \/ ReplayClose \/ ConflictClose
  \/ RefuseCapacity \/ RefuseRevision
  \/ OrdinaryCreate \/ OrdinaryRename \/ OrdinaryMove
  \/ \E t \in Targets: SubmitStep(t)
  \/ \E t \in Targets: ExpireAttempt(t)
  \/ \E t \in Targets: ParkBudget(t)
  \/ \E t \in Targets: CompleteBlocked(t)
  \/ \E t \in Targets: CompleteUnknown(t)
  \/ \E t \in Targets: CompleteDisposed(t)
  \/ WrongTargetSettlement
  \/ \E t \in Targets: Resume(t)
  \/ ResumeReplay \/ ResumeStale \/ ResumeFuture
  \/ ResumeNotResumable \/ ResumeExhausted
  \/ \E t \in Targets: CleanupSuccess(t)
  \/ \E t \in Targets: CleanupFailure(t)
  \/ PreparePublication \/ PreparationFailure
  \/ ReleasePreparedGate \/ RejectStalePreparation \/ CommitRemoval
  \/ Retire \/ StatusRead
  \/ ObserveTwoTargetAcceptance \/ ObserveOneTargetDisposed
  \/ ObserveUnknownQuarantine \/ ObserveLateReceipt
  \/ ObserveWrongTargetOrdinal \/ ObserveRebasedPublication
  \/ ObserveUnrelatedTabSurvives \/ ObserveExactOneRevision

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.tabs \in Seq(Identities) /\ Len(s.tabs) <= 3
  /\ s.created \in BOOLEAN /\ s.labels \in [Identities -> {"created", "renamed"}]
  /\ s.revision \in 0..MaxRevision /\ s.writes \in 0..MaxRevision
  /\ s.entries \in 0..4 /\ s.moved \in BOOLEAN /\ s.renamed \in BOOLEAN
  /\ s.retired \in BOOLEAN /\ s.close.phase \in {"absent", "accepted", "closed"}
  /\ s.close.acceptedRevision \in -1..MaxRevision
  /\ s.close.generation \in 0..MaxGeneration
  /\ s.close.credit \in BOOLEAN
  /\ s.close.targets \in [Targets -> [phase: {"absent", "closing", "blocked", "unknown", "disposed"},
       active: BOOLEAN, pending: BOOLEAN, expired: BOOLEAN, disposed: BOOLEAN,
       cleanup: BOOLEAN, steps: 0..MaxSteps]]
  /\ s.alias \in Targets \cup {None}
  /\ s.flight \in Targets \cup {None}
  /\ s.flightGeneration \in 0..MaxGeneration
  /\ s.prepared \in BOOLEAN
  /\ s.preparedRevision \in -1..MaxRevision
  /\ s.preparedTabs \in Seq(Identities)

MembershipExact ==
  Members = (IF Closed THEN {} ELSE Targets)
    \cup (IF s.created THEN {C} ELSE {})

CreditAccounting ==
  /\ s.close.credit = (Accepted /\ ~Closed)
  /\ s.revision + Credits <= MaxRevision
  /\ s.revision = s.writes

SingleOutstanding ==
  /\ (s.flight = None) = (s.flightGeneration = 0)
  /\ s.flight # None => s.close.targets[s.flight].pending

ClosedRequiresProof ==
  ~Closed
  \/ (AllDisposed
      /\ AllCleaned
      /\ ~s.close.credit
      /\ A \notin Members /\ B \notin Members)

KnownEffectsMonotone == [][\A t \in Targets:
  /\ (s.close.targets[t].disposed => s'.close.targets[t].disposed)
  /\ (s.close.targets[t].cleanup => s'.close.targets[t].cleanup)
  /\ (s.close.phase = "closed" => s'.close.phase = "closed") ]_vars

CapturedMembershipIsStable == [][
  /\ Accepted /\ ~Closed /\ s'.close.phase # "closed"
  => /\ A \in Members' /\ B \in Members']_vars

AcceptedAcknowledgmentFixed == [][
  s.close.acceptedRevision # -1 =>
    s'.close.acceptedRevision = s.close.acceptedRevision]_vars

OutstandingSurvivesExpiry == [][
  /\ s.flight # None /\ s'.flight = s.flight
  => s'.flightGeneration = s.flightGeneration]_vars

PublicationRemovesOnlyCapturedTab == [][
  /\ s.close.phase # "closed" /\ s'.close.phase = "closed"
  => /\ s'.tabs = RemoveTab(s.tabs)
     /\ s'.revision = s.revision + 1
     /\ s'.close.credit = FALSE
     /\ C \in Members => C \in Members']_vars

CleanupRequiresDispose == [][\A t \in Targets:
  s.close.targets[t].cleanup # s'.close.targets[t].cleanup =>
    s.close.targets[t].disposed]_vars

ResumeConsumesGeneration == [][
  /\ s'.close.generation > s.close.generation
  => /\ s'.close.generation = s.close.generation + 1
     /\ s.flight = None]_vars

=============================================================================
