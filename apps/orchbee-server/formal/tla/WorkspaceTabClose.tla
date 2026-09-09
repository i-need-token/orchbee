------------------------ MODULE WorkspaceTabClose ------------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(* Context-publication composition of WorkbenchTabCloseCoordination.
   Initial Workspace simple Tabs 1 and 2 can each accept one primary Close;
   Tab 3 is one explicit fresh Create, before or after the Workspace empties.
   Another context has one complete Tab. At most two Workspace rotations and
   one other-context write interleave with Close. Revision starts at offset 0.
   MaxRevision=5 exposes shared-credit exhaustion among six possible writes.

   Disposed abstracts the exact Worker receipt and Worker cleanup established
   by WorkerTerminalLifecycleStep; server exact-target cleanup is separate.
   Host steps, Resume controls, generations, continuation budgets, ledger byte
   capacity, authentication and delivery remain in existing Close models.
   Replay/alias actions here project retained results without another primary
   operation; they do not model response flights or alias ledger allocation.

   Only this model's Workspace context publishes Close. The other context's
   one write exercises shared Site revision and context isolation. Complete
   immutable Workspace Tab records expose accidental reconstruction. The
   Workspace directory and standalone Empty/Creating/Settled bootstrap slot
   never change. All three Init states are checked. No scheduling fairness,
   progress claim, state constraint, symmetry or implementation proof applies.
*)

CONSTANT MaxRevision
ASSUME MaxRevision = 5
Targets == {1, 2}
WorkspaceIds == 1..3
SlotStates == {"Empty", "Creating", "Settled"}
Phases == {"absent", "pending", "unknown", "disposed", "cleaned", "closed"}
FullTab(id) ==
  [id |-> id, owner |-> "workspace", label |-> <<"label", id>>,
   terminal |-> [session |-> id, generation |-> 1]]
InitialOtherTab ==
  [id |-> 4, owner |-> "other", label |-> "initial",
   terminal |-> [session |-> 4, generation |-> 1]]
InitialDirectory == [id |-> 1, worker |-> "worker", path |-> "explicit-cwd"]
EmptyOperation ==
  [phase |-> "absent", credit |-> FALSE,
   acceptedRevision |-> -1, closedRevision |-> -1]

VARIABLE s
vars == <<s>>
Members(tabs) == {tabs[i].id : i \in 1..Len(tabs)}
Accepted == {t \in Targets: s.op[t].phase # "absent"}
Closed == {t \in Targets: s.op[t].phase = "closed"}
Closing == Accepted \ Closed
Credits == Cardinality({t \in Targets: s.op[t].credit})
CanWrite == s.revision + Credits + 1 <= MaxRevision
Remove(tabs, target) == SelectSeq(tabs, LAMBDA tab: tab.id # target)
Acknowledgment(target) ==
  [target |-> target, revision |-> s.op[target].acceptedRevision]
ClosedResult(target) ==
  [target |-> target, revision |-> s.op[target].closedRevision]
PublicationState(target) ==
  [s EXCEPT !.tabs = Remove(@, target), !.revision = @ + 1,
     !.op[target].phase = "closed", !.op[target].credit = FALSE,
     !.op[target].closedRevision = s.revision + 1]
Publishable(target) ==
  /\ s.op[target].phase = "cleaned"
  /\ s.op[target].credit
  /\ target \in Members(s.tabs)

Init ==
  \E slot \in SlotStates:
    s = [tabs |-> <<FullTab(1), FullTab(2)>>,
         otherTabs |-> <<InitialOtherTab>>, directory |-> InitialDirectory,
         slot |-> slot, initialSlot |-> slot, standaloneNotifications |-> 0,
         revision |-> 0, moves |-> 0, created |-> FALSE,
         otherWritten |-> FALSE, op |-> [t \in Targets |-> EmptyOperation]]

PrimaryAcceptance ==
  /\ CanWrite
  /\ \E t \in Targets:
       /\ s.op[t].phase = "absent" /\ t \in Members(s.tabs)
       /\ s' = [s EXCEPT !.op[t] = [EmptyOperation EXCEPT
            !.phase = "pending", !.credit = TRUE,
            !.acceptedRevision = s.revision]]

KnownDispose ==
  \E t \in Targets:
    /\ s.op[t].phase = "pending"
    /\ s' = [s EXCEPT !.op[t].phase = "disposed"]

OutcomeUnknown ==
  \E t \in Targets:
    /\ s.op[t].phase = "pending"
    /\ s' = [s EXCEPT !.op[t].phase = "unknown"]

ExactCleanup ==
  \E t \in Targets:
    /\ s.op[t].phase = "disposed"
    /\ s' = [s EXCEPT !.op[t].phase = "cleaned"]

(* Each slot-specific branch performs actual publication, not an Init-only
   observation. Pure PublicationState shares the update without hiding these
   direct Next action names behind action aliases in TLC coverage. *)
ClosePublicationWithEmptySlot ==
  /\ s.slot = "Empty"
  /\ \E t \in Targets: Publishable(t) /\ s' = PublicationState(t)
ClosePublicationWithCreatingSlot ==
  /\ s.slot = "Creating"
  /\ \E t \in Targets: Publishable(t) /\ s' = PublicationState(t)
ClosePublicationWithSettledSlot ==
  /\ s.slot = "Settled"
  /\ \E t \in Targets: Publishable(t) /\ s' = PublicationState(t)

MoveWhileClosing ==
  /\ Closing # {} /\ s.moves < 2 /\ Len(s.tabs) >= 2 /\ CanWrite
  /\ s' = [s EXCEPT !.tabs = Append(Tail(@), Head(@)),
       !.moves = @ + 1, !.revision = @ + 1]

AppendWhileClosing ==
  /\ Closing # {} /\ ~s.created /\ CanWrite
  /\ s' = [s EXCEPT !.tabs = Append(@, FullTab(3)),
       !.created = TRUE, !.revision = @ + 1]

ExplicitCreateAfterEmpty ==
  /\ s.tabs = <<>> /\ ~s.created /\ CanWrite
  /\ s' = [s EXCEPT !.tabs = <<FullTab(3)>>,
       !.created = TRUE, !.revision = @ + 1]

OtherContextWrite ==
  /\ ~s.otherWritten /\ CanWrite
  /\ s' = [s EXCEPT !.otherTabs[1].label = "renamed",
       !.otherWritten = TRUE, !.revision = @ + 1]

ReplayAcknowledgment == /\ Accepted # {} /\ UNCHANGED vars
AliasAcknowledgment == /\ Closing # {} /\ UNCHANGED vars
ReplayClosedResult == /\ Closed # {} /\ UNCHANGED vars
RejectWrongContext == /\ Accepted # {} /\ UNCHANGED vars
RejectUnknownPublication ==
  /\ \E t \in Targets: s.op[t].phase = "unknown"
  /\ UNCHANGED vars
RejectPendingPublication ==
  /\ \E t \in Targets: s.op[t].phase \in {"pending", "disposed"}
  /\ UNCHANGED vars
RefuseWriteReservedCredit ==
  /\ Credits > 0 /\ s.revision < MaxRevision /\ ~CanWrite
  /\ UNCHANGED vars

ObserveTwoPrimaryCredits == /\ Credits = 2 /\ UNCHANGED vars
ObserveCurrentSnapshotPublication ==
  /\ \E t \in Closed: s.op[t].closedRevision > s.op[t].acceptedRevision + 1
  /\ UNCHANGED vars
ObserveAppendSurvivesClose ==
  /\ s.created /\ Closed # {} /\ 3 \in Members(s.tabs)
  /\ UNCHANGED vars
ObserveOtherContextWriteSurvivesClose ==
  /\ s.otherWritten /\ Closed # {} /\ s.otherTabs[1].label = "renamed"
  /\ UNCHANGED vars
ObserveLastTabRemoved ==
  /\ Closed = Targets /\ s.tabs = <<>> /\ ~s.created
  /\ UNCHANGED vars
ObserveFreshIdentityAfterEmpty ==
  /\ Closed = Targets /\ s.tabs = <<FullTab(3)>>
  /\ UNCHANGED vars
ObserveHistoricalAcknowledgment ==
  /\ \E t \in Accepted: Acknowledgment(t).revision < s.revision
  /\ UNCHANGED vars
ObserveHistoricalClosedResult ==
  /\ \E t \in Closed: ClosedResult(t).revision < s.revision
  /\ UNCHANGED vars

Next ==
  \/ PrimaryAcceptance \/ KnownDispose \/ OutcomeUnknown \/ ExactCleanup
  \/ ClosePublicationWithEmptySlot \/ ClosePublicationWithCreatingSlot
  \/ ClosePublicationWithSettledSlot
  \/ MoveWhileClosing \/ AppendWhileClosing \/ ExplicitCreateAfterEmpty
  \/ OtherContextWrite \/ ReplayAcknowledgment \/ AliasAcknowledgment
  \/ ReplayClosedResult \/ RejectWrongContext \/ RejectUnknownPublication
  \/ RejectPendingPublication \/ RefuseWriteReservedCredit
  \/ ObserveTwoPrimaryCredits \/ ObserveCurrentSnapshotPublication
  \/ ObserveAppendSurvivesClose \/ ObserveOtherContextWriteSurvivesClose
  \/ ObserveLastTabRemoved \/ ObserveFreshIdentityAfterEmpty
  \/ ObserveHistoricalAcknowledgment \/ ObserveHistoricalClosedResult
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.tabs \in Seq({FullTab(t): t \in WorkspaceIds}) /\ Len(s.tabs) <= 3
  /\ s.otherTabs \in {<<InitialOtherTab>>,
                      <<[InitialOtherTab EXCEPT !.label = "renamed"]>>}
  /\ s.slot \in SlotStates /\ s.initialSlot \in SlotStates
  /\ s.standaloneNotifications = 0
  /\ s.revision \in 0..MaxRevision /\ s.moves \in 0..2
  /\ s.created \in BOOLEAN /\ s.otherWritten \in BOOLEAN
  /\ s.op \in [Targets -> [phase: Phases, credit: BOOLEAN,
       acceptedRevision: -1..MaxRevision, closedRevision: -1..MaxRevision]]

ExactWorkspaceMembership ==
  /\ Len(s.tabs) = Cardinality(Members(s.tabs))
  /\ Members(s.tabs) = (Targets \ Closed) \cup (IF s.created THEN {3} ELSE {})

SharedRevisionCredits ==
  /\ s.revision + Credits <= MaxRevision
  /\ s.revision = Cardinality(Closed) + s.moves
       + (IF s.created THEN 1 ELSE 0) + (IF s.otherWritten THEN 1 ELSE 0)

CloseStateOwnsCredit ==
  \A t \in Targets:
    /\ (s.op[t].credit <=> t \in Closing)
    /\ (t \in Closing => t \in Members(s.tabs))
    /\ (t \in Closed =>
         /\ t \notin Members(s.tabs)
         /\ s.op[t].closedRevision > s.op[t].acceptedRevision
         /\ s.op[t].closedRevision <= s.revision)
    /\ (t \notin Closed => s.op[t].closedRevision = -1)
    /\ (t \in Accepted => s.op[t].acceptedRevision \in 0..s.revision)
    /\ (t \notin Accepted => s.op[t] = EmptyOperation)

IndependentStatePreserved ==
  /\ s.directory = InitialDirectory /\ s.slot = s.initialSlot
  /\ s.standaloneNotifications = 0
  /\ s.otherTabs = <<IF s.otherWritten
       THEN [InitialOtherTab EXCEPT !.label = "renamed"] ELSE InitialOtherTab>>

AcceptanceReservesWithoutPublication == [][\A t \in Targets:
  s.op[t].phase = "absent" /\ s.op'[t].phase # "absent" =>
    /\ s.op'[t].phase = "pending" /\ Credits' = Credits + 1
    /\ s.op'[t].acceptedRevision = s.revision
    /\ UNCHANGED <<s.tabs, s.otherTabs, s.revision>>]_vars

PublicationRemovesCurrentExactTarget == [][\A t \in Targets:
  s.op[t].phase # "closed" /\ s.op'[t].phase = "closed" =>
    /\ s.op[t].phase = "cleaned" /\ s.op[t].credit
    /\ s.tabs' = Remove(s.tabs, t)
    /\ s.revision' = s.revision + 1 /\ Credits' = Credits - 1
    /\ s.op'[t].closedRevision = s.revision + 1
    /\ UNCHANGED <<s.otherTabs, s.directory, s.slot, s.standaloneNotifications>>
    /\ \A other \in Targets \ {t}: s.op'[other] = s.op[other]]_vars

CleanupRequiresKnownDispose == [][\A t \in Targets:
  s.op[t].phase # "cleaned" /\ s.op'[t].phase = "cleaned" =>
    s.op[t].phase = "disposed"]_vars

AcknowledgmentAndClosedRevisionFixed == [][\A t \in Accepted:
  /\ Acknowledgment(t)' = Acknowledgment(t)
  /\ (t \in Closed => s.op'[t] = s.op[t])]_vars

RemovalOnlyByPublication == [][\A t \in Targets:
  t \in Members(s.tabs) /\ t \notin Members(s.tabs') =>
    s.op[t].phase = "cleaned" /\ s.op'[t].phase = "closed"]_vars

OnlyExplicitCreateAddsFreshIdentity == [][
  3 \notin Members(s.tabs) /\ 3 \in Members(s.tabs') =>
    /\ ~s.created /\ s.created' /\ s.revision' = s.revision + 1
    /\ s.tabs' = Append(s.tabs, FullTab(3))
    /\ UNCHANGED <<s.op, s.otherTabs, s.slot, s.standaloneNotifications>>]_vars

ContextWritesAreIsolated == [][
  /\ (s.tabs' # s.tabs => s.otherTabs' = s.otherTabs)
  /\ (s.otherTabs' # s.otherTabs =>
       UNCHANGED <<s.tabs, s.op, s.directory, s.slot, s.standaloneNotifications>>)
]_vars
=============================================================================
