---------------- MODULE WorkbenchTabCloseCreateReference ----------------
EXTENDS Integers, Sequences, FiniteSets, TLC

(* Three retained Create identities, at most two live members. Before removal,
   accepted/effects/ledger/seals project to WorkbenchTabCreate; after removal
   retained history explicitly outlives membership. Started is the exact-known
   current-generation identity class; Unknown never acquires that authority.
   Lifecycle is separate from the immutable original creation outcome. *)
CONSTANTS Owners, None, Foreign, MaxMembers
ASSUME /\ Cardinality(Owners) = 3
       /\ None \notin Owners /\ Foreign \notin Owners /\ None # Foreign
       /\ MaxMembers = 2

Outcomes == {"started", "unknown"}
EmptyAccepted == [base |-> -1, revision |-> -1, first |-> FALSE,
                  tuple |-> None, label |-> "none", fingerprint |-> None]
EmptyCreate == [phase |-> "absent", accepted |-> EmptyAccepted,
                workbenchSeal |-> None, terminalSeal |-> None,
                outcome |-> "none"]
EmptyFlight == [owner |-> None, phase |-> "none", accepted |-> EmptyAccepted,
                outcome |-> "none"]
VARIABLE s
vars == <<s>>
Members == {s.tabs[i] : i \in 1..Len(s.tabs)}
Created == {o \in Owners : s.create[o].phase # "absent"}
Removed == {o \in Owners : s.lifecycle[o] = "closed"}
Remove(tabs, owner) == SelectSeq(tabs, LAMBDA member: member # owner)
SlotFor(tabs, reference) ==
  IF Len(tabs) = 0 THEN None
  ELSE IF reference \in {tabs[i] : i \in 1..Len(tabs)}
       THEN reference ELSE tabs[1]
SlotPhase(reference, creates) ==
  IF reference = None THEN "empty"
  ELSE IF creates[reference].phase = "pending" THEN "creating" ELSE "settled"
SlotOutcome(reference, creates) ==
  IF reference = None THEN "none" ELSE creates[reference].outcome

Init == s =
  [tabs |-> <<>>, revision |-> 0, reference |-> None,
   slotPhase |-> "empty", slotOutcome |-> "none",
   create |-> [o \in Owners |-> EmptyCreate],
   effects |-> [o \in Owners |-> 0],
   exactIdentity |-> [o \in Owners |-> None],
   lifecycle |-> [o \in Owners |-> "live"],
   label |-> [o \in Owners |-> "none"],
   moves |-> 0, renames |-> 0, flight |-> EmptyFlight,
   open |-> [reference |-> None, outcome |-> "none"],
   completion |-> [reference |-> None, standaloneChanged |-> FALSE]]

CommitCreate(owner) ==
  /\ owner \notin Created /\ Len(s.tabs) < MaxMembers
  /\ LET accepted == [base |-> s.revision, revision |-> s.revision + 1,
                       first |-> Len(s.tabs) = 0, tuple |-> owner,
                       label |-> "created", fingerprint |-> owner]
     IN s' = [s EXCEPT
       !.tabs = Append(@, owner), !.revision = @ + 1,
       !.reference = IF Len(s.tabs) = 0 THEN owner ELSE @,
       !.slotPhase = IF Len(s.tabs) = 0 THEN "creating" ELSE @,
       !.slotOutcome = IF Len(s.tabs) = 0 THEN "none" ELSE @,
       !.create[owner] = [phase |-> "pending", accepted |-> accepted,
                          workbenchSeal |-> owner, terminalSeal |-> owner,
                          outcome |-> "none"],
       !.label[owner] = "created"]

DispatchCreate(owner) ==
  /\ s.create[owner].phase = "pending" /\ s.effects[owner] = 0
  /\ s' = [s EXCEPT !.effects[owner] = 1]

CompleteCreate(owner, outcome) ==
  /\ outcome \in Outcomes /\ s.create[owner].phase = "pending"
  /\ s.effects[owner] = 1
  /\ s.create[owner].workbenchSeal = owner
  /\ s.create[owner].terminalSeal = owner
  /\ LET changes == s.reference = owner /\ s.slotPhase = "creating"
     IN s' = [s EXCEPT
       !.create[owner] = [@ EXCEPT !.phase = "final", !.outcome = outcome,
                          !.workbenchSeal = None, !.terminalSeal = None],
       !.exactIdentity[owner] = IF outcome = "started" THEN owner ELSE None,
       !.slotPhase = IF changes THEN "settled" ELSE @,
       !.slotOutcome = IF changes THEN outcome ELSE @,
       !.completion = [reference |-> owner, standaloneChanged |-> changes]]

ReplayPendingCreate(owner) ==
  /\ s.create[owner].phase = "pending" /\ s.flight.phase = "none"
  /\ s' = [s EXCEPT !.flight =
       [owner |-> owner, phase |-> "pending",
        accepted |-> s.create[owner].accepted, outcome |-> "none"]]

WritePendingReplay ==
  /\ s.flight.phase = "pending"
  /\ s' = [s EXCEPT !.flight.phase = "pending-written"]

ReplayFinalCreate(owner) ==
  /\ s.create[owner].phase = "final"
  /\ s.flight.phase = "none" \/
       (s.flight.owner = owner /\ s.flight.phase = "pending-written")
  /\ s' = [s EXCEPT !.flight =
       [owner |-> owner, phase |-> "final",
        accepted |-> s.create[owner].accepted,
        outcome |-> s.create[owner].outcome]]

WriteFinalReplay ==
  /\ s.flight.phase = "final"
  /\ s' = [s EXCEPT !.flight.phase = "final-written"]

RejectForeignWorkbenchSeal(owner) ==
  /\ s.create[owner].phase = "pending"
  /\ Foreign # s.create[owner].workbenchSeal /\ UNCHANGED vars
RejectForeignTerminalSeal(owner) ==
  /\ s.create[owner].phase = "pending"
  /\ Foreign # s.create[owner].terminalSeal /\ UNCHANGED vars
RejectStaleCompletion(owner) ==
  /\ s.create[owner].phase = "final" /\ UNCHANGED vars

BeginClose(owner) ==
  /\ owner \in Members /\ s.lifecycle[owner] = "live"
  /\ s.create[owner].phase = "final"
  /\ s.create[owner].outcome = "started"
  /\ s.exactIdentity[owner] = owner
  /\ s' = [s EXCEPT !.lifecycle[owner] = "closing"]
RefuseUnknownClose(owner) ==
  /\ owner \in Members /\ s.create[owner].outcome = "unknown"
  /\ s.exactIdentity[owner] = None /\ UNCHANGED vars
RefusePendingClose(owner) ==
  /\ owner \in Members /\ s.create[owner].phase = "pending"
  /\ UNCHANGED vars
ObserveDisposed(owner) ==
  /\ s.lifecycle[owner] = "closing"
  /\ s' = [s EXCEPT !.lifecycle[owner] = "disposed"]

(* The companion coordination model supplies exact cleanup, revision credit
   and successful current-snapshot preparation at this atomic removal cut. *)
CommitCloseRemoval(owner) ==
  /\ owner \in Members /\ s.lifecycle[owner] = "disposed"
  /\ LET tabs == Remove(s.tabs, owner)
         reference == SlotFor(tabs, s.reference)
     IN s' = [s EXCEPT !.tabs = tabs, !.revision = @ + 1,
       !.lifecycle[owner] = "closed", !.reference = reference,
       !.slotPhase = SlotPhase(reference, s.create),
       !.slotOutcome = SlotOutcome(reference, s.create)]

MoveMembers ==
  /\ Len(s.tabs) = 2 /\ s.moves = 0
  /\ s' = [s EXCEPT !.tabs = <<s.tabs[2], s.tabs[1]>>,
                      !.moves = 1, !.revision = @ + 1]
RenameMember(owner) ==
  /\ owner \in Members /\ s.renames = 0
  /\ s' = [s EXCEPT !.label[owner] = "renamed", !.renames = 1,
                      !.revision = @ + 1]
OpenObserveCurrent ==
  /\ s.reference # None /\ s.slotPhase = "settled"
  /\ s' = [s EXCEPT !.open =
       [reference |-> s.reference, outcome |-> s.slotOutcome]]
OpenWaitCurrent ==
  /\ s.reference # None /\ s.slotPhase = "creating"
  /\ s' = [s EXCEPT !.open =
       [reference |-> s.reference, outcome |-> "waiting"]]

(* Witnesses identify independently ordered state classes, not extra guards. *)
ObserveRetargetedPending ==
  /\ Removed # {} /\ s.slotPhase = "creating"
  /\ ~s.create[s.reference].accepted.first /\ UNCHANGED vars
CompleteRetargetedPending ==
  /\ Removed # {} /\ s.slotPhase = "creating"
  /\ ~s.create[s.reference].accepted.first
  /\ \E outcome \in Outcomes: CompleteCreate(s.reference, outcome)
RemoveReferenceAfterSurvivorSettled ==
  /\ s.reference # None /\ Len(s.tabs) = 2
  /\ \E survivor \in Members \ {s.reference}:
       /\ s.create[survivor].phase = "final"
       /\ ~s.create[survivor].accepted.first
  /\ CommitCloseRemoval(s.reference)
ObserveRetargetedSettled ==
  /\ Removed # {} /\ s.slotPhase = "settled"
  /\ ~s.create[s.reference].accepted.first /\ UNCHANGED vars
ObserveRetargetedUnknown ==
  /\ Removed # {} /\ s.slotOutcome = "unknown"
  /\ ~s.create[s.reference].accepted.first /\ UNCHANGED vars
ObserveClosingReference ==
  /\ Removed # {} /\ s.reference # None
  /\ s.lifecycle[s.reference] = "closing" /\ UNCHANGED vars
ObserveEmptyAfterClose ==
  /\ Removed # {} /\ Len(s.tabs) = 0 /\ UNCHANGED vars
ObserveFreshAfterEmpty ==
  /\ Removed # {} /\ s.reference # None
  /\ s.create[s.reference].accepted.first /\ UNCHANGED vars
ObserveOldCompletionAfterFresh ==
  /\ ObserveFreshAfterEmpty
  /\ \E owner \in Removed: RejectStaleCompletion(owner)
ObservePendingFlightAfterSettlement ==
  /\ s.flight.phase = "pending-written"
  /\ s.create[s.flight.owner].phase = "final" /\ UNCHANGED vars
ObserveReplayAfterRemoval ==
  /\ s.flight.phase \in {"final", "final-written"}
  /\ s.flight.owner \in Removed /\ UNCHANGED vars

Next ==
  \/ \E owner \in Owners: CommitCreate(owner)
  \/ \E owner \in Owners: DispatchCreate(owner)
  \/ \E owner \in Owners, outcome \in Outcomes: CompleteCreate(owner, outcome)
  \/ \E owner \in Owners: ReplayPendingCreate(owner)
  \/ WritePendingReplay
  \/ \E owner \in Owners: ReplayFinalCreate(owner)
  \/ WriteFinalReplay
  \/ \E owner \in Owners: RejectForeignWorkbenchSeal(owner)
  \/ \E owner \in Owners: RejectForeignTerminalSeal(owner)
  \/ \E owner \in Owners: RejectStaleCompletion(owner)
  \/ \E owner \in Owners: BeginClose(owner)
  \/ \E owner \in Owners: RefuseUnknownClose(owner)
  \/ \E owner \in Owners: RefusePendingClose(owner)
  \/ \E owner \in Owners: ObserveDisposed(owner)
  \/ \E owner \in Owners: CommitCloseRemoval(owner)
  \/ MoveMembers
  \/ \E owner \in Owners: RenameMember(owner)
  \/ OpenObserveCurrent \/ OpenWaitCurrent
  \/ ObserveRetargetedPending \/ ObserveRetargetedSettled
  \/ CompleteRetargetedPending \/ RemoveReferenceAfterSurvivorSettled
  \/ ObserveRetargetedUnknown \/ ObserveClosingReference
  \/ ObserveEmptyAfterClose \/ ObserveFreshAfterEmpty
  \/ ObserveOldCompletionAfterFresh
  \/ ObservePendingFlightAfterSettlement \/ ObserveReplayAfterRemoval
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.tabs \in Seq(Owners) /\ Len(s.tabs) <= MaxMembers
  /\ s.revision \in 0..8 /\ s.reference \in Owners \cup {None}
  /\ s.slotPhase \in {"empty", "creating", "settled"}
  /\ s.slotOutcome \in Outcomes \cup {"none"}
  /\ s.effects \in [Owners -> 0..1]
  /\ s.exactIdentity \in [Owners -> Owners \cup {None}]
  /\ s.lifecycle \in [Owners -> {"live", "closing", "disposed", "closed"}]
  /\ s.label \in [Owners -> {"none", "created", "renamed"}]
  /\ s.moves \in 0..1 /\ s.renames \in 0..1
  /\ \A o \in Owners:
       /\ s.create[o].phase \in {"absent", "pending", "final"}
       /\ s.create[o].outcome \in Outcomes \cup {"none"}
       /\ s.create[o].workbenchSeal \in Owners \cup {None}
       /\ s.create[o].terminalSeal \in Owners \cup {None}
  /\ s.flight.owner \in Owners \cup {None}
  /\ s.flight.phase \in {"none", "pending", "pending-written", "final", "final-written"}
  /\ s.completion.reference \in Owners \cup {None}
  /\ s.completion.standaloneChanged \in BOOLEAN

ExactCurrentReference ==
  /\ (s.reference = None) = (Len(s.tabs) = 0)
  /\ (s.reference # None =>
       Cardinality({i \in 1..Len(s.tabs):
         s.create[s.tabs[i]].accepted.tuple = s.reference}) = 1)
  /\ s.slotPhase = SlotPhase(s.reference, s.create)
  /\ s.slotOutcome = SlotOutcome(s.reference, s.create)
MembershipAndHistory ==
  /\ Len(s.tabs) = Cardinality(Members)
  /\ Created = Members \cup Removed /\ Members \cap Removed = {}
  /\ s.revision = Cardinality(Created) + Cardinality(Removed) + s.moves + s.renames
  /\ \A o \in Created: s.create[o].accepted.tuple = o
ExactCreateSeals ==
  \A o \in Owners:
    /\ (s.create[o].phase = "pending") = (s.create[o].workbenchSeal = o)
    /\ (s.create[o].phase = "pending") = (s.create[o].terminalSeal = o)
    /\ (s.create[o].phase = "pending" => s.create[o].outcome = "none")
    /\ (s.create[o].phase = "final" => s.create[o].outcome \in Outcomes)
KnownIdentityBeforeClose ==
  \A o \in Owners:
    /\ (s.lifecycle[o] # "live" =>
         s.create[o].outcome = "started" /\ s.exactIdentity[o] = o)
    /\ (s.create[o].outcome = "unknown" => s.exactIdentity[o] = None)
CapturedReplyIsExact ==
  s.flight.owner # None =>
    /\ s.flight.accepted = s.create[s.flight.owner].accepted
    /\ (s.flight.phase \in {"pending", "pending-written"} => s.flight.outcome = "none")
    /\ (s.flight.phase \in {"final", "final-written"} =>
         s.flight.outcome = s.create[s.flight.owner].outcome)
NoRedispatch == \A o \in Owners: s.effects[o] = 1 => o \in Created

AcceptedFactsAreImmutable == [][\A o \in Created:
  s.create'[o].accepted = s.create[o].accepted]_vars
FinalCreateIsImmutable == [][\A o \in Owners:
  s.create[o].phase = "final" => s.create'[o] = s.create[o]]_vars
CapturedRepliesAreImmutable == [][
  s.flight.phase \in {"pending", "final", "final-written"} =>
    /\ s.flight'.owner = s.flight.owner
    /\ s.flight'.accepted = s.flight.accepted
    /\ s.flight'.outcome = s.flight.outcome]_vars
CompletionUsesCurrentReference == [][\A o \in Owners:
  (s.create[o].phase = "pending" /\ s.create'[o].phase = "final") =>
    /\ UNCHANGED <<s.tabs, s.revision, s.reference, s.label>>
    /\ s.completion'.standaloneChanged =
         (s.reference = o /\ s.slotPhase = "creating")
    /\ (s.reference # o => UNCHANGED <<s.slotPhase, s.slotOutcome>>)]_vars
RemovalPreservesCurrentSurvivors == [][\A o \in Owners:
  (s.lifecycle[o] # "closed" /\ s.lifecycle'[o] = "closed") =>
    /\ s.tabs' = Remove(s.tabs, o) /\ s.revision' = s.revision + 1
    /\ UNCHANGED <<s.create, s.label, s.effects, s.exactIdentity, s.flight>>]_vars
=============================================================================
