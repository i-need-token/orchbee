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

   Scope layer: the Workspace-scope Close is one operation over the Terminal
   Context's captured membership. Admission snapshots the leaf member set,
   reserves one publication credit, and fences both directions: structural
   writes (admitted through a bounded pending ledger) and member-level Close
   operations refuse while the scope operation is open, and scope admission
   refuses while a member Close or a pending write is outstanding. Empty
   membership closes vacuously; publication removes exactly the captured set
   atomically. A blocked member outcome resumes under an incremented attempt
   generation; stale generations and non-resumable states never restart the
   attempt. The closed record is a retained tombstone: it keeps the operation
   identity and its closed revision after the membership is gone.
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
ScopePhases == {"absent", "open", "blocked", "unknown", "closed"}
(* "open" runs an attempt; "blocked" is the only resumable hold; "unknown" is
   terminal and non-resumable. All three keep the scope joined and fenced. *)
EmptyScope ==
  [phase |-> "absent", members |-> {}, cleaned |-> {}, credit |-> FALSE,
   acceptedRevision |-> -1, closedRevision |-> -1, generation |-> 0]

VARIABLE s
vars == <<s>>
Members(tabs) == {tabs[i].id : i \in 1..Len(tabs)}
Accepted == {t \in Targets: s.op[t].phase # "absent"}
Closed == {t \in Targets: s.op[t].phase = "closed"}
Closing == Accepted \ Closed
ScopeOpen == s.scope.phase \in {"open", "blocked", "unknown"}
ScopeClosedMembers ==
  IF s.scope.phase = "closed" THEN s.scope.members ELSE {}
Credits ==
  Cardinality({t \in Targets: s.op[t].credit})
    + (IF s.scope.credit THEN 1 ELSE 0)
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

RemoveAll(tabs, ids) == SelectSeq(tabs, LAMBDA tab: tab.id \notin ids)
ScopePublicationState ==
  [s EXCEPT !.tabs = RemoveAll(@, s.scope.members), !.revision = @ + 1,
     !.scope.phase = "closed", !.scope.credit = FALSE,
     !.scope.closedRevision = s.revision + 1]

Init ==
  \E slot \in SlotStates:
    s = [tabs |-> <<FullTab(1), FullTab(2)>>,
         otherTabs |-> <<InitialOtherTab>>, directory |-> InitialDirectory,
         slot |-> slot, initialSlot |-> slot, standaloneNotifications |-> 0,
         revision |-> 0, moves |-> 0, created |-> FALSE,
         otherWritten |-> FALSE, op |-> [t \in Targets |-> EmptyOperation],
         scope |-> EmptyScope, pendingWrites |-> 0]

PrimaryAcceptance ==
  /\ CanWrite
  (* An open scope operation owns every captured member; per-member Close
     admission is fenced until the scope record closes. *)
  /\ ~ScopeOpen
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

(* Structural writes are two-phase: a bounded pending ledger admits them
   only while no scope operation owns the context, and their commit consumes
   the pending slot. Scope admission in turn refuses pending writes, which
   closes the fence in both directions. *)
StructuralWriteAdmitted ==
  /\ ~ScopeOpen /\ s.pendingWrites < 2
  /\ s' = [s EXCEPT !.pendingWrites = @ + 1]

MoveWhileClosing ==
  /\ Closing # {} /\ s.moves < 2 /\ Len(s.tabs) >= 2 /\ CanWrite
  /\ ~ScopeOpen /\ s.pendingWrites > 0
  /\ s' = [s EXCEPT !.tabs = Append(Tail(@), Head(@)),
       !.moves = @ + 1, !.revision = @ + 1, !.pendingWrites = @ - 1]

AppendWhileClosing ==
  /\ Closing # {} /\ ~s.created /\ CanWrite
  /\ ~ScopeOpen /\ s.pendingWrites > 0
  /\ s' = [s EXCEPT !.tabs = Append(@, FullTab(3)),
       !.created = TRUE, !.revision = @ + 1, !.pendingWrites = @ - 1]

ExplicitCreateAfterEmpty ==
  /\ s.tabs = <<>> /\ ~s.created /\ CanWrite
  /\ ~ScopeOpen /\ s.pendingWrites > 0
  /\ s' = [s EXCEPT !.tabs = <<FullTab(3)>>,
       !.created = TRUE, !.revision = @ + 1, !.pendingWrites = @ - 1]

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

(* The Workspace-scope Close: one admission over the captured leaf
   membership, one bounded attempt window, one atomic publication, and a
   retained tombstone. Member outcomes are abstracted to three terminals —
   cleaned (publishable), blocked (resumable hold), unknown (terminal and
   non-resumable) — matching the scope record the authority keeps. *)
ScopeCloseAcceptance ==
  /\ CanWrite
  /\ s.scope.phase = "absent"
  (* Admission is fenced by anything already outstanding on this context:
     an open member Close or an admitted-but-uncommitted structural write. *)
  /\ Closing = {} /\ s.pendingWrites = 0
  /\ s' = [s EXCEPT !.scope = [EmptyScope EXCEPT
       !.phase = "open", !.members = Members(s.tabs), !.credit = TRUE,
       !.acceptedRevision = s.revision, !.generation = 1]]

ScopeMemberCleaned ==
  /\ s.scope.phase = "open"
  /\ \E m \in s.scope.members \ s.scope.cleaned:
       s' = [s EXCEPT !.scope.cleaned = @ \cup {m}]

ScopeMemberBlocked ==
  /\ s.scope.phase = "open" /\ s.scope.cleaned # s.scope.members
  /\ s' = [s EXCEPT !.scope.phase = "blocked"]

ScopeMemberUnknown ==
  /\ s.scope.phase = "open" /\ s.scope.cleaned # s.scope.members
  (* The unknown terminal can never publish, but the record stays joined,
     fenced, AND charged: the credit is retained exactly like open/blocked —
     an OutcomeUnknown-terminal record keeps its reserved revision forever,
     matching the authority ledger and the owning claims document. *)
  /\ s' = [s EXCEPT !.scope.phase = "unknown"]

ScopeResume ==
  /\ s.scope.phase = "blocked" /\ s.scope.generation < 3
  /\ s' = [s EXCEPT !.scope.phase = "open",
       !.scope.generation = @ + 1]

ScopePublication ==
  /\ s.scope.phase = "open" /\ s.scope.cleaned = s.scope.members
  /\ s.scope.credit
  /\ s' = ScopePublicationState

(* Replay, join, rerun, resume-fence and status observations: the retained
   record projects them without another primary operation. *)
ScopeCloseJoin == /\ ScopeOpen /\ UNCHANGED vars
ScopeRerunRejected == /\ s.scope.phase = "closed" /\ UNCHANGED vars
ScopeResumeStaleRejected ==
  /\ s.scope.phase = "blocked" /\ UNCHANGED vars
ScopeResumeNonResumableRejected ==
  /\ s.scope.phase \in {"open", "unknown"} /\ UNCHANGED vars
ScopeStatusNotClosing == /\ s.scope.phase = "absent" /\ UNCHANGED vars
ScopeStatusTombstone == /\ s.scope.phase = "closed" /\ UNCHANGED vars

ObserveScopeVacuousClose ==
  /\ s.scope.phase = "closed" /\ s.scope.members = {}
  /\ UNCHANGED vars
ObserveScopeMembershipCaptured ==
  /\ ScopeOpen /\ s.scope.members # {} /\ UNCHANGED vars
ObserveScopeResumed ==
  /\ s.scope.phase = "open" /\ s.scope.generation > 1 /\ UNCHANGED vars
ObserveScopeTombstoneAfterRemoval ==
  /\ s.scope.phase = "closed" /\ s.scope.members # {}
  /\ Members(s.tabs) \cap s.scope.members = {}
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
  \/ StructuralWriteAdmitted
  \/ MoveWhileClosing \/ AppendWhileClosing \/ ExplicitCreateAfterEmpty
  \/ OtherContextWrite \/ ReplayAcknowledgment \/ AliasAcknowledgment
  \/ ReplayClosedResult \/ RejectWrongContext \/ RejectUnknownPublication
  \/ RejectPendingPublication \/ RefuseWriteReservedCredit
  \/ ScopeCloseAcceptance \/ ScopeMemberCleaned \/ ScopeMemberBlocked
  \/ ScopeMemberUnknown \/ ScopeResume \/ ScopePublication
  \/ ScopeCloseJoin \/ ScopeRerunRejected \/ ScopeResumeStaleRejected
  \/ ScopeResumeNonResumableRejected \/ ScopeStatusNotClosing
  \/ ScopeStatusTombstone \/ ObserveScopeVacuousClose
  \/ ObserveScopeMembershipCaptured \/ ObserveScopeResumed
  \/ ObserveScopeTombstoneAfterRemoval
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
  /\ s.pendingWrites \in 0..2
  /\ s.op \in [Targets -> [phase: Phases, credit: BOOLEAN,
       acceptedRevision: -1..MaxRevision, closedRevision: -1..MaxRevision]]
  /\ s.scope \in [phase: ScopePhases,
       members: SUBSET WorkspaceIds, cleaned: SUBSET WorkspaceIds,
       credit: BOOLEAN, acceptedRevision: -1..MaxRevision,
       closedRevision: -1..MaxRevision, generation: 0..3]

ExactWorkspaceMembership ==
  /\ Len(s.tabs) = Cardinality(Members(s.tabs))
  /\ Members(s.tabs) =
       (Targets \ (Closed \cup ScopeClosedMembers))
         \cup (IF s.created /\ 3 \notin ScopeClosedMembers THEN {3} ELSE {})

SharedRevisionCredits ==
  /\ s.revision + Credits <= MaxRevision
  /\ s.revision = Cardinality(Closed) + s.moves
       + (IF s.created THEN 1 ELSE 0) + (IF s.otherWritten THEN 1 ELSE 0)
       + (IF s.scope.phase = "closed" THEN 1 ELSE 0)

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

(* Scope claims: the captured set is frozen at admission, publication is the
   only remover, the fence holds in both directions while the operation is
   open, resume is generation-fenced and resumable-only, and the closed
   record is a stable tombstone. *)
ScopeStateOwnsRecord ==
  /\ (ScopeOpen => /\ s.scope.members \subseteq Members(s.tabs)
                   /\ s.scope.cleaned \subseteq s.scope.members
                   /\ s.scope.generation \in 1..3
                   /\ s.scope.acceptedRevision \in 0..s.revision)
  /\ (s.scope.phase = "blocked" => s.scope.cleaned # s.scope.members)
  /\ (s.scope.phase = "closed" =>
       /\ s.scope.closedRevision \in
            (s.scope.acceptedRevision + 1)..s.revision
       /\ Members(s.tabs) \cap s.scope.members = {})
  /\ (s.scope.phase = "absent" => s.scope = EmptyScope)
  (* Credit is held through every non-terminal scope phase — including
     unknown, whose record stays charged and fenced forever. Only absent
     (never admitted) and closed (published) release it. *)
  /\ (s.scope.credit <=> (s.scope.phase # "absent" /\ s.scope.phase # "closed"))

ScopeFencesStructuralWrites ==
  ScopeOpen => s.pendingWrites = 0

ScopeClosesOnlyFromOpen == [][
  s.scope.phase # "closed" /\ s.scope'.phase = "closed" =>
    /\ s.scope.phase = "open" /\ s.scope.cleaned = s.scope.members
    /\ s.scope.credit
    /\ s.tabs' = RemoveAll(s.tabs, s.scope.members)
    /\ s.revision' = s.revision + 1
    /\ s.scope'.closedRevision = s.revision + 1
    /\ UNCHANGED <<s.otherTabs, s.directory, s.slot,
                   s.standaloneNotifications, s.pendingWrites, s.op>>
    /\ s.scope'.members = s.scope.members
    /\ s.scope'.acceptedRevision = s.scope.acceptedRevision]_vars

ScopeMembershipFrozenWhileOpen == [][
  ScopeOpen /\ s.scope'.phase # "closed" =>
    s.scope'.members = s.scope.members]_vars

ScopeResumeGenerationFenced == [][
  s.scope'.generation # s.scope.generation =>
    /\ s.scope.phase = "blocked" /\ s.scope'.phase = "open"
    /\ s.scope'.generation = s.scope.generation + 1
    /\ s.scope'.members = s.scope.members]_vars

ScopeClosedIsTombstone == [][
  s.scope.phase = "closed" =>
    /\ s.scope'.phase = "closed"
    /\ s.scope'.members = s.scope.members
    /\ s.scope'.acceptedRevision = s.scope.acceptedRevision
    /\ s.scope'.closedRevision = s.scope.closedRevision
    /\ s.scope'.generation = s.scope.generation]_vars

ScopeAdmissionNeverReopens == [][
  s.scope.phase # "absent" => s.scope'.phase # "absent"]_vars

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
    \/ s.op[t].phase = "cleaned" /\ s.op'[t].phase = "closed"
    \/ /\ s.scope.phase = "open" /\ s.scope'.phase = "closed"
       /\ t \in s.scope.members]_vars

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

ScopeNeverTouchesOtherContext == [][
  s.scope' # s.scope =>
    UNCHANGED <<s.otherTabs, s.directory, s.slot, s.standaloneNotifications>>
]_vars
=============================================================================
