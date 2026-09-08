------------------- MODULE WorkbenchTabCloseCoordination -------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(* Exact-known target identities A and B; C abstracts one ordinary Create.
   The initial revision is an offset, not a claim that real bootstrap is free.
   Worker Disposed is the exact target/correlation and Worker-cleanup cut of
   WorkerTerminalLifecycleStep. Server cleanup remains a separate transition.
   Deadline expiry does not settle or discard an outstanding Worker receipt.
   Local WF expires and parks attempts; no Worker-response/cleanup fairness.
   Config variants split control histories from current-snapshot mutations. *)
CONSTANTS A, B, C, None, Mode, MaxRevision, MaxGeneration,
          MaxEntries, MaxBytes, PrimaryCharge, OtherCharge
ASSUME /\ Cardinality({A, B, C, None}) = 4
       /\ Mode \in {"credits", "resume"}
       /\ MaxGeneration = 3 /\ MaxEntries = 3
       /\ PrimaryCharge = 3 /\ OtherCharge = 1 /\ MaxBytes = 7
       /\ MaxRevision = 3
Targets == {A, B}
Identities == {A, B, C}
Controls == {"x", "y", "z"}
ControlTargets == IF Mode = "resume" THEN {A} ELSE Targets
EmptyReceipt == [id |-> "none", expected |-> 0, generation |-> 0]
EmptyOperation ==
  [phase |-> "absent", acceptedRevision |-> -1, credit |-> FALSE,
   generation |-> 0, receipt |-> EmptyReceipt, active |-> FALSE,
   expired |-> FALSE, submitted |-> FALSE, disposed |-> FALSE,
   cleanup |-> FALSE, closedRevision |-> -1]
EmptyPrepared == [target |-> None, revision |-> -1, tabs |-> <<>>,
                  labels |-> [o \in Identities |-> "created"]]
EmptyAlias == [target |-> None, base |-> -1, acceptedRevision |-> -1]
VARIABLE s
vars == <<s>>
Members == {s.tabs[i] : i \in 1..Len(s.tabs)}
Accepted == {t \in Targets: s.op[t].phase # "absent"}
Credits == Cardinality({t \in Targets: s.op[t].credit})
Closed == {t \in Targets: s.op[t].phase = "closed"}
AliasCount == IF s.alias.target = None THEN 0 ELSE 1
Entries == Cardinality(Accepted) + AliasCount + s.ordinaryLedger
Bytes == PrimaryCharge * Cardinality(Accepted)
         + OtherCharge * (AliasCount + s.ordinaryLedger)
CanCharge(charge) == Entries < MaxEntries /\ Bytes + charge <= MaxBytes
CanWrite == ~s.retired /\ s.revision + 1 + Credits <= MaxRevision
Remove(tabs, target) == SelectSeq(tabs, LAMBDA member: member # target)
Resumable(t) ==
  /\ s.op[t].phase \in {"blocked", "disposed"}
  /\ ~s.op[t].active /\ s.outstanding # t
ResumeDecision(t, id, expected) ==
  CASE t \notin Accepted -> "operation-unavailable"
    [] s.op[t].receipt.id = id ->
         IF s.op[t].receipt.expected = expected THEN "replay" ELSE "conflict"
    [] expected < s.op[t].generation -> "stale"
    [] expected > s.op[t].generation -> "future"
    [] ~Resumable(t) -> "not-resumable"
    [] s.op[t].generation = MaxGeneration -> "exhausted"
    [] OTHER -> "resumed"

Init == s =
  [tabs |-> <<A, B>>, labels |-> [o \in Identities |-> "created"],
   revision |-> 0, writes |-> 0, moved |-> FALSE, renamed |-> FALSE,
   created |-> FALSE, op |-> [t \in Targets |-> EmptyOperation],
   alias |-> EmptyAlias, ordinaryLedger |-> 0, retired |-> FALSE,
   outstanding |-> None, outstandingGeneration |-> 0,
   prepared |-> EmptyPrepared]

AdmitClose(t) ==
  /\ t \in ControlTargets /\ t \in Members
  /\ ~s.retired /\ s.op[t].phase = "absent"
  /\ CanCharge(PrimaryCharge) /\ s.revision + Credits + 1 <= MaxRevision
  /\ s' = [s EXCEPT !.op[t] = [EmptyOperation EXCEPT
       !.phase = "closing", !.acceptedRevision = s.revision,
       !.credit = TRUE, !.generation = 1, !.active = TRUE]]
JoinClose(t) ==
  /\ t \in Accepted /\ t \in Members /\ ~s.retired
  /\ s.alias.target = None /\ CanCharge(OtherCharge)
  /\ s' = [s EXCEPT !.alias = [target |-> t, base |-> s.revision,
                              acceptedRevision |-> s.op[t].acceptedRevision]]
ReplayClose(t) == /\ t \in Accepted /\ UNCHANGED vars
ConflictingClose(t) == /\ t \in Accepted /\ UNCHANGED vars
RefuseAdmissionCapacity ==
  /\ ~CanCharge(PrimaryCharge) /\ UNCHANGED vars
RefuseAdmissionRevision ==
  /\ s.revision + Credits + 1 > MaxRevision /\ UNCHANGED vars

OrdinaryCreate ==
  /\ Mode # "resume" /\ ~s.created /\ CanWrite /\ CanCharge(OtherCharge)
  /\ s' = [s EXCEPT !.tabs = Append(@, C), !.created = TRUE,
       !.revision = @ + 1, !.writes = @ + 1, !.ordinaryLedger = @ + 1]
OrdinaryRename ==
  /\ Mode # "resume" /\ ~s.renamed /\ CanWrite /\ B \in Members
  /\ CanCharge(OtherCharge)
  /\ s' = [s EXCEPT !.labels[B] = "renamed", !.renamed = TRUE,
       !.revision = @ + 1, !.writes = @ + 1, !.ordinaryLedger = @ + 1]
OrdinaryMove ==
  /\ Mode # "resume" /\ ~s.moved /\ CanWrite /\ Len(s.tabs) >= 2
  /\ CanCharge(OtherCharge)
  /\ s' = [s EXCEPT !.tabs = Append(Tail(@), Head(@)), !.moved = TRUE,
       !.revision = @ + 1, !.writes = @ + 1, !.ordinaryLedger = @ + 1]
OrdinaryNoop == UNCHANGED vars
FillLedger ==
  /\ CanCharge(OtherCharge)
  /\ s' = [s EXCEPT !.ordinaryLedger = @ + 1]

SubmitStep(t) ==
  /\ s.op[t].phase = "closing" /\ s.op[t].active
  /\ ~s.op[t].expired /\ ~s.op[t].submitted /\ s.outstanding = None
  /\ s' = [s EXCEPT !.outstanding = t,
       !.outstandingGeneration = s.op[t].generation, !.op[t].submitted = TRUE]
ExpireAttempt(t) ==
  /\ s.op[t].active /\ ~s.op[t].expired
  /\ s' = [s EXCEPT !.op[t].expired = TRUE]
ParkBudget(t) ==
  /\ s.op[t].active /\ s.op[t].expired /\ s.outstanding # t
  /\ s' = [s EXCEPT !.op[t].active = FALSE,
       !.op[t].phase = IF s.op[t].disposed THEN "disposed" ELSE "blocked"]
CompleteKnownBlocked(t) ==
  /\ s.outstanding = t /\ s.outstandingGeneration = s.op[t].generation
  /\ s' = [s EXCEPT !.outstanding = None, !.outstandingGeneration = 0,
       !.op[t].phase = "blocked", !.op[t].active = FALSE]
CompleteUnknown(t) ==
  /\ s.outstanding = t /\ s.outstandingGeneration = s.op[t].generation
  /\ s' = [s EXCEPT !.outstanding = None, !.outstandingGeneration = 0,
       !.op[t].phase = "unknown", !.op[t].active = FALSE]
CompleteDisposed(t) ==
  /\ s.outstanding = t /\ s.outstandingGeneration = s.op[t].generation
  /\ s' = [s EXCEPT !.outstanding = None, !.outstandingGeneration = 0,
       !.op[t].phase = "disposed", !.op[t].disposed = TRUE,
       !.op[t].active = ~s.op[t].expired]
RejectWrongResult == /\ s.outstanding # None /\ UNCHANGED vars

Resume(t, id, expected) ==
  /\ Mode = "resume" /\ ResumeDecision(t, id, expected) = "resumed"
  /\ s' = [s EXCEPT !.op[t] = [@ EXCEPT
       !.phase = IF s.op[t].disposed THEN "disposed" ELSE "closing",
       !.generation = @ + 1, !.active = TRUE, !.expired = FALSE,
       !.submitted = FALSE,
       !.receipt = [id |-> id, expected |-> expected,
                     generation |-> s.op[t].generation + 1]]]
ReplayResume(t, id, expected) ==
  /\ ResumeDecision(t, id, expected) = "replay" /\ UNCHANGED vars
ResumeConflict(t, id, expected) ==
  /\ ResumeDecision(t, id, expected) = "conflict" /\ UNCHANGED vars
ResumeStale(t, id, expected) ==
  /\ ResumeDecision(t, id, expected) = "stale" /\ UNCHANGED vars
ResumeFuture(t, id, expected) ==
  /\ ResumeDecision(t, id, expected) = "future" /\ UNCHANGED vars
ResumeNotResumable(t, id, expected) ==
  /\ ResumeDecision(t, id, expected) = "not-resumable" /\ UNCHANGED vars
ResumeExhausted(t, id, expected) ==
  /\ ResumeDecision(t, id, expected) = "exhausted" /\ UNCHANGED vars

CleanupSuccess(t) ==
  /\ s.op[t].phase = "disposed" /\ s.op[t].active /\ ~s.op[t].expired
  /\ ~s.op[t].cleanup
  /\ s' = [s EXCEPT !.op[t].cleanup = TRUE]
CleanupFailure(t) ==
  /\ s.op[t].phase = "disposed" /\ s.op[t].active /\ ~s.op[t].cleanup
  /\ s' = [s EXCEPT !.op[t].active = FALSE]
PreparePublication(t) ==
  /\ s.op[t].phase = "disposed" /\ s.op[t].cleanup
  /\ s.op[t].active /\ ~s.op[t].expired /\ ~s.retired
  /\ s.prepared.target = None /\ t \in Members
  /\ s' = [s EXCEPT !.prepared = [target |-> t, revision |-> s.revision,
       tabs |-> Remove(s.tabs, t), labels |-> s.labels]]
PreparationFailure(t) ==
  /\ s.op[t].phase = "disposed" /\ s.op[t].cleanup /\ s.op[t].active
  /\ s' = [s EXCEPT !.op[t].active = FALSE]
ReleasePreparedGate ==
  /\ s.prepared.target # None /\ UNCHANGED vars
RejectStalePreparation ==
  /\ s.prepared.target # None /\ s.prepared.revision # s.revision
  /\ s' = [s EXCEPT !.prepared = EmptyPrepared]
CommitRemoval(t) ==
  /\ s.prepared.target = t /\ s.prepared.revision = s.revision
  /\ s.op[t].phase = "disposed" /\ s.op[t].disposed /\ s.op[t].cleanup
  /\ s.op[t].active /\ ~s.op[t].expired /\ ~s.retired /\ s.op[t].credit
  /\ s' = [s EXCEPT !.tabs = s.prepared.tabs, !.labels = s.prepared.labels,
       !.revision = @ + 1, !.prepared = EmptyPrepared,
       !.op[t].phase = "closed", !.op[t].closedRevision = s.revision + 1,
       !.op[t].credit = FALSE, !.op[t].active = FALSE]
Retire == /\ ~s.retired /\ s' = [s EXCEPT !.retired = TRUE]
StatusRead == UNCHANGED vars

ObserveTwoCredits == /\ Credits = 2 /\ UNCHANGED vars
ObserveFullLedgerResume ==
  /\ Entries = MaxEntries /\ s.op[A].generation > 1 /\ UNCHANGED vars
ObserveLastReservedRevision ==
  /\ s.revision + Credits = MaxRevision /\ Credits > 0 /\ UNCHANGED vars
ObserveLateReceipt ==
  /\ s.outstanding # None /\ s.op[s.outstanding].expired /\ UNCHANGED vars
ObserveLateDisposed ==
  /\ \E t \in Targets: s.op[t].disposed /\ s.op[t].expired /\ ~s.op[t].active
  /\ UNCHANGED vars
ObserveRebasedPublication ==
  /\ \E t \in Closed: s.op[t].closedRevision > s.op[t].acceptedRevision + 1
  /\ s.writes > 0 /\ UNCHANGED vars
ObserveAliasHistoricalRevision ==
  /\ s.alias.target # None /\ s.alias.base > s.alias.acceptedRevision
  /\ UNCHANGED vars
ObserveCleanupBeforePublicationFailure ==
  /\ \E t \in Targets: s.op[t].phase = "disposed" /\ s.op[t].cleanup /\ ~s.op[t].active
  /\ UNCHANGED vars

Next ==
  \/ \E t \in Targets: AdmitClose(t)
  \/ \E t \in Targets: JoinClose(t)
  \/ \E t \in Targets: ReplayClose(t)
  \/ \E t \in Targets: ConflictingClose(t)
  \/ RefuseAdmissionCapacity \/ RefuseAdmissionRevision
  \/ OrdinaryCreate \/ OrdinaryRename \/ OrdinaryMove \/ OrdinaryNoop \/ FillLedger
  \/ \E t \in Targets: SubmitStep(t)
  \/ \E t \in Targets: ExpireAttempt(t)
  \/ \E t \in Targets: ParkBudget(t)
  \/ \E t \in Targets: CompleteKnownBlocked(t)
  \/ \E t \in Targets: CompleteUnknown(t)
  \/ \E t \in Targets: CompleteDisposed(t)
  \/ RejectWrongResult
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       Resume(t, id, expected)
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       ReplayResume(t, id, expected)
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       ResumeConflict(t, id, expected)
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       ResumeStale(t, id, expected)
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       ResumeFuture(t, id, expected)
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       ResumeNotResumable(t, id, expected)
  \/ \E t \in Targets, id \in Controls, expected \in 1..MaxGeneration:
       ResumeExhausted(t, id, expected)
  \/ \E t \in Targets: CleanupSuccess(t)
  \/ \E t \in Targets: CleanupFailure(t)
  \/ \E t \in Targets: PreparePublication(t)
  \/ \E t \in Targets: PreparationFailure(t)
  \/ ReleasePreparedGate \/ RejectStalePreparation
  \/ \E t \in Targets: CommitRemoval(t)
  \/ Retire \/ StatusRead
  \/ ObserveTwoCredits \/ ObserveFullLedgerResume \/ ObserveLastReservedRevision
  \/ ObserveLateReceipt \/ ObserveLateDisposed \/ ObserveRebasedPublication
  \/ ObserveAliasHistoricalRevision \/ ObserveCleanupBeforePublicationFailure
Spec == Init /\ [][Next]_vars
        /\ (\A t \in Targets: WF_vars(ExpireAttempt(t)) /\ WF_vars(ParkBudget(t)))

TypeOK ==
  /\ s.tabs \in Seq(Identities) /\ Len(s.tabs) <= 3
  /\ s.revision \in 0..MaxRevision /\ s.writes \in 0..3
  /\ s.labels \in [Identities -> {"created", "renamed"}]
  /\ s.moved \in BOOLEAN /\ s.renamed \in BOOLEAN /\ s.created \in BOOLEAN
  /\ s.retired \in BOOLEAN /\ s.ordinaryLedger \in 0..MaxEntries
  /\ s.outstanding \in Targets \cup {None}
  /\ s.outstandingGeneration \in 0..MaxGeneration
  /\ s.prepared.target \in Targets \cup {None}
  /\ s.prepared.revision \in -1..MaxRevision
  /\ s.alias.target \in Targets \cup {None}
  /\ \A t \in Targets:
       /\ s.op[t].phase \in {"absent", "closing", "blocked", "unknown", "disposed", "closed"}
       /\ s.op[t].generation \in 0..MaxGeneration
       /\ s.op[t].acceptedRevision \in -1..MaxRevision
       /\ s.op[t].closedRevision \in -1..MaxRevision
       /\ s.op[t].credit \in BOOLEAN /\ s.op[t].active \in BOOLEAN
       /\ s.op[t].expired \in BOOLEAN /\ s.op[t].submitted \in BOOLEAN
       /\ s.op[t].disposed \in BOOLEAN /\ s.op[t].cleanup \in BOOLEAN
       /\ s.op[t].receipt.id \in Controls \cup {"none"}
RevisionCredits == /\ s.revision + Credits <= MaxRevision
                   /\ s.revision = s.writes + Cardinality(Closed)
ReservedLedgerCapacity == /\ Entries <= MaxEntries /\ Bytes <= MaxBytes
MembershipIsExact ==
  /\ Len(s.tabs) = Cardinality(Members)
  /\ Members = (Targets \ {t \in Targets: s.op[t].phase = "closed"})
       \cup (IF s.created THEN {C} ELSE {})
ClosedRequiresEveryCommit ==
  \A t \in Targets:
    /\ (s.op[t].phase = "closed" =>
         s.op[t].disposed /\ s.op[t].cleanup /\ t \notin Members
         /\ ~s.op[t].credit /\ s.op[t].closedRevision > s.op[t].acceptedRevision)
    /\ (t \in Accepted /\ s.op[t].phase # "closed" => s.op[t].credit /\ t \in Members)
    /\ (s.op[t].cleanup => s.op[t].disposed)
    /\ (s.op[t].disposed => s.op[t].phase \in {"disposed", "closed"})
SingleExactOutstanding ==
  /\ (s.outstanding = None) = (s.outstandingGeneration = 0)
  /\ (s.outstanding # None =>
       /\ s.op[s.outstanding].phase = "closing"
       /\ s.op[s.outstanding].active /\ s.op[s.outstanding].submitted
       /\ s.outstandingGeneration = s.op[s.outstanding].generation)
CanonicalAliasAcknowledgment ==
  s.alias.target # None =>
    /\ s.alias.acceptedRevision = s.op[s.alias.target].acceptedRevision
    /\ s.alias.base >= s.alias.acceptedRevision
GenerationAndReceipt ==
  \A t \in Targets:
    /\ (t \in Accepted => s.op[t].generation >= 1)
    /\ (s.op[t].receipt.id # "none" =>
         /\ s.op[t].receipt.generation = s.op[t].generation
         /\ s.op[t].receipt.expected + 1 = s.op[t].receipt.generation)
RetainedReceiptPrecedesCurrentPhase ==
  \A t \in Accepted:
    s.op[t].receipt.id # "none" =>
      ResumeDecision(t, s.op[t].receipt.id, s.op[t].receipt.expected) = "replay"
OlderControlCannotRestart ==
  \A t \in Accepted, id \in Controls, expected \in 1..MaxGeneration:
    expected < s.op[t].generation =>
      ResumeDecision(t, id, expected) \in {"replay", "conflict", "stale"}

AcceptedAcknowledgmentIsImmutable == [][\A t \in Accepted:
  s.op'[t].acceptedRevision = s.op[t].acceptedRevision]_vars
KnownEffectsAreMonotone == [][\A t \in Targets:
  /\ (s.op[t].disposed => s.op'[t].disposed)
  /\ (s.op[t].cleanup => s.op'[t].cleanup)
  /\ (s.op[t].phase = "closed" => s.op'[t] = s.op[t])]_vars
ResumeUsesReservedStorage == [][\A t \in Accepted:
  s.op'[t].generation > s.op[t].generation =>
    /\ s.op'[t].generation = s.op[t].generation + 1
    /\ s.outstanding # t /\ Resumable(t)
    /\ UNCHANGED <<s.revision, s.tabs, s.alias, s.ordinaryLedger>>
    /\ s.op'[t].credit = s.op[t].credit]_vars
OutstandingSurvivesExpiry == [][s.outstanding # None /\ s.outstanding' = s.outstanding =>
  /\ s.outstandingGeneration' = s.outstandingGeneration
  /\ s.op'[s.outstanding].generation = s.op[s.outstanding].generation]_vars
RemovalRebasesCurrentSnapshot == [][\A t \in Targets:
  (s.op[t].phase # "closed" /\ s.op'[t].phase = "closed") =>
    /\ s.tabs' = Remove(s.tabs, t) /\ s.labels' = s.labels
    /\ s.revision' = s.revision + 1 /\ Credits' = Credits - 1
    /\ UNCHANGED <<s.alias, s.ordinaryLedger>>]_vars
AttemptsExpireOrSettle ==
  \A t \in Targets: s.op[t].active ~> (~s.op[t].active \/ s.op[t].expired)
ExpiredSettledAttemptsPark ==
  \A t \in Targets:
    (s.op[t].active /\ s.op[t].expired /\ s.outstanding # t) ~> ~s.op[t].active
=============================================================================
