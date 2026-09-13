------------------ MODULE WorkbenchMultileafMembership ------------------
EXTENDS Integers, Naturals, Sequences, FiniteSets, TLC

(*
   Focused companion for the multi-leaf TabCreate/TabMove/Create-completion
   contract. Existing ledger models (WorkbenchTabCreate,
   WorkbenchTabCloseCreateReference) and WorkspaceTabMove abstract each member
   as one owner slot; the missing refinement is a Tab owning an ORDERED
   sequence of leaf members plus the exact standalone slot association.

   Two terminal contexts are modeled: the standalone context carries the
   Empty/Creating/Settled slot; the workspace context never touches it. Each
   tab owns one or two leaves; every leaf carries one distinct binding equal to
   its owning operation identity (a Create request or the context's Split).
   Members of a context flatten leaf owners in tab order, matching the tree
   membership read order.

   Gate -> predicate mapping (mirrors the implementation):
     standalone create admission ......... strong anchor composition
     standalone/workspace move admission . strong / derivable-only
     standalone create completion ........ strong on the CURRENT slot+members
     split admission/completion .......... weak composition (standalone)
     close admission / publication-before  weak composition (standalone)
     close target member admissibility ... pending -> Busy refusal;
                                           unknown/unavailable runtime ->
                                           Unavailable refusal
   A leaf with an unbound presentation makes tree membership underivable; the
   anchor binding missing from retained state or a duplicated anchor binding
   makes strong composition fail while weak may still hold. Fault injection is
   bounded to one membership corruption and one runtime loss, both scoped to
   explicit fault-tagged paths.

   Retained-binding premise: every admitted leaf's binding is retained by
   construction; corruption actions are the only source of a missing retained
   binding. No transport, executor, UI or general scheduler is modeled.
   Progress is claimed per admitted pending Create under fair dispatch and
   completion: a later fault may only degrade the outcome to honest unknown,
   never silently erase the obligation.
*)

CONSTANTS Requests, None, MaxLeaves, MaxMoves

ASSUME /\ Cardinality(Requests) = 2
       /\ None \notin Requests
       /\ MaxLeaves = 2
       /\ MaxMoves \in 1..3

Contexts == {"standalone", "workspace"}
Standalone == "standalone"
Workspace == "workspace"
Splits == {<<"split", "standalone">>, <<"split", "workspace">>}
Owners == Requests \cup Splits
MaxTabs == 2

EmptySlot == [phase |-> "empty", ref |-> None]
EmptyCreate == [phase |-> "absent", outcome |-> "none", standalone |-> FALSE]
EmptyClose == [phase |-> "idle", context |-> None, tab |-> -1,
               captured |-> <<>>, ready |-> FALSE]

VARIABLE s
vars == <<s>>

RECURSIVE Flatten(_)
Flatten(seqs) ==
  IF seqs = <<>> THEN <<>> ELSE Head(seqs) \o Flatten(Tail(seqs))

Members(context) ==
  Flatten([i \in 1..Len(s.tabs[context]) |-> s.tabs[context][i].leaves])
MemberSet(context) == {Members(context)[i] : i \in DOMAIN Members(context)}
TabIds(context) == {s.tabs[context][i].id : i \in 1..Len(s.tabs[context])}
HasTab(context, tab) == tab \in TabIds(context)
TabIndex(context, tab) ==
  CHOOSE i \in 1..Len(s.tabs[context]) : s.tabs[context][i].id = tab
TabWith(context, tab) == s.tabs[context][TabIndex(context, tab)]
Multileaf(context) ==
  \E i \in 1..Len(s.tabs[context]) : Len(s.tabs[context][i].leaves) > 1
RemainingMembers(context, removed) ==
  Flatten([i \in 1..Len(removed) |-> removed[i].leaves])

(* Leaf phase: pending while its owning operation is in flight. *)
LeafPending(owner) ==
  IF owner \in Requests THEN s.creates[owner].phase = "pending"
  ELSE s.splitPhase[owner[2]] = "pending"

(* tree_membership_context fails when a leaf lacks a bound presentation. *)
Derivable(context) ==
  ~(s.corrupted = "leaf-unbound" /\ context = Standalone)

(* composition_matches: Empty iff no members; a nonempty slot resolves its
   retained reference and counts its binding exactly once among members. *)
StrongComposition ==
  /\ (s.slot.phase = "empty") = (Len(Members(Standalone)) = 0)
  /\ (s.slot.phase # "empty" =>
       /\ s.slot.ref \in Owners
       /\ s.retained[s.slot.ref]
       /\ Cardinality({i \in 1..Len(Members(Standalone)) :
            Members(Standalone)[i] = s.slot.ref}) = 1)

(* tree_composition_matches over the standalone context: Empty iff no
   standalone members; a nonempty slot only needs nonempty members whose
   every binding is retained. The anchor is NOT resolved here - the
   predicates are deliberately not interchangeable. It is standalone-scoped
   by ownership (it reads the standalone slot), so the signature takes no
   context; every caller guards it behind `context = Workspace \/`. *)
WeakComposition ==
  /\ (s.slot.phase = "empty") = (Len(Members(Standalone)) = 0)
  /\ (s.slot.phase # "empty" =>
       /\ Len(Members(Standalone)) > 0
       /\ \A owner \in MemberSet(Standalone) : s.retained[owner])

CommittedCreate(request) ==
  s.creates[request].phase \in {"pending", "settled", "coord-unknown"}
SplitToken(context) == <<"split", context>>
SplitLeaf(context, tab) ==
  [s.tabs[context] EXCEPT ![tab] =
     [s.tabs[context][tab] EXCEPT !.leaves =
       Append(@, SplitToken(context))]]

Init == s =
  [tabs |-> [context \in Contexts |-> <<>>],
   nextTab |-> [context \in Contexts |-> 1],
   revision |-> 0,
   slot |-> EmptySlot,
   creates |-> [request \in Requests |-> EmptyCreate],
   splitPhase |-> [context \in Contexts |-> "absent"],
   effects |-> [request \in Requests |-> 0],
   retained |-> [owner \in Owners |-> FALSE],
   close |-> EmptyClose,
   corrupted |-> "none",
   moves |-> 0,
   retired |-> FALSE,
   runtimeLost |-> [owner \in Owners |-> FALSE],
   evidence |-> {}]

(* ---- Create admission -------------------------------------------------- *)

(* Strong gate at standalone admission. A valid multi-leaf context admits;
   any membership or composition failure retires the authority pre-commit. *)
AdmitCreateStandalone(request) ==
  /\ ~s.retired
  /\ s.creates[request].phase = "absent"
  /\ Len(s.tabs[Standalone]) < MaxTabs
  /\ IF Derivable(Standalone) /\ StrongComposition
     THEN
       LET tab == [id |-> s.nextTab[Standalone], leaves |-> <<request>>]
           first == Len(s.tabs[Standalone]) = 0
       IN s' = [s EXCEPT
            !.tabs = [@ EXCEPT ![Standalone] = Append(@, tab)],
            !.nextTab = [@ EXCEPT ![Standalone] = @ + 1],
            !.revision = @ + 1,
            !.slot = IF first THEN [phase |-> "creating", ref |-> request]
                              ELSE @,
            !.creates = [@ EXCEPT ![request] =
                 [phase |-> "pending", outcome |-> "none",
                  standalone |-> TRUE]],
            !.retained = [@ EXCEPT ![request] = TRUE],
            !.evidence = @ \cup
                 (IF Multileaf(Standalone) THEN {"create-on-multileaf"}
                  ELSE {}) \cup
                 (IF s.close.phase = "closing" THEN {"unfenced-during-close"}
                  ELSE {})]
     ELSE s' = [s EXCEPT !.retired = TRUE]

(* Workspace create admission never consults membership or the slot. *)
AdmitCreateWorkspace(request) ==
  /\ ~s.retired
  /\ s.creates[request].phase = "absent"
  /\ Len(s.tabs[Workspace]) < MaxTabs
  /\ LET tab == [id |-> s.nextTab[Workspace], leaves |-> <<request>>]
     IN s' = [s EXCEPT
        !.tabs = [@ EXCEPT ![Workspace] = Append(@, tab)],
        !.nextTab = [@ EXCEPT ![Workspace] = @ + 1],
        !.revision = @ + 1,
        !.creates = [@ EXCEPT ![request] =
             [phase |-> "pending", outcome |-> "none",
              standalone |-> FALSE]],
        !.retained = [@ EXCEPT ![request] = TRUE]]

(* Collapsed pre-commit rejection classes: stale revision, site mismatch,
   missing context, capacity. No member, no revision, no effect. *)
RejectCreatePreflight(request) ==
  /\ ~s.retired
  /\ s.creates[request].phase = "absent"
  /\ s' = [s EXCEPT !.creates = [@ EXCEPT ![request] =
       [phase |-> "rejected", outcome |-> "none", standalone |-> FALSE]]]

(* Replay reads the retained entry before any membership gate; it stays
   readable through corruption and retirement. *)
ReplayPendingCreate(request) ==
  /\ s.creates[request].phase = "pending"
  /\ s' = [s EXCEPT !.evidence = @ \cup {"replay-pending"}]

ReplayFinalCreate(request) ==
  /\ s.creates[request].phase \in {"settled", "coord-unknown", "rejected"}
  /\ s' = [s EXCEPT !.evidence = @ \cup {"replay-final"}]

DispatchCreate(request) ==
  /\ s.creates[request].phase = "pending"
  /\ s.effects[request] = 0
  /\ s' = [s EXCEPT !.effects = [@ EXCEPT ![request] = 1]]

(* ---- Create completion ------------------------------------------------- *)

(* Post-commit gates: the pending association needs its retained binding; a
   standalone-metadata create additionally re-reads tree membership and the
   strong anchor composition against the CURRENT slot. *)
CompletionGate(request) ==
  /\ s.retained[request]
  /\ (s.creates[request].standalone =>
       Derivable(Standalone) /\ StrongComposition)

(* A pending standalone member can never leave the tree - Close admission
   refuses pending targets - so a standalone request is always a member while
   it completes. The non-anchor tags therefore distinguish a still-present
   member settling ledger-only while another member anchors, from a
   workspace request which never carries standalone membership. *)
CompletionEvidence(request) ==
  (IF s.slot.phase # "empty" /\ s.slot.ref # request
   THEN IF request \in MemberSet(Standalone)
        THEN {"non-current-member-completion"}
        ELSE {"off-membership-completion"}
   ELSE {})
  \cup
  (IF s.slot.phase = "empty" THEN {"empty-slot-completion"} ELSE {})
  \cup
  (IF Multileaf(Standalone) THEN {"multileaf-completion"} ELSE {})

(* The completing request is the CURRENT slot anchor: settle the slot. *)
CompleteCreateAnchorSettled(request) ==
  /\ s.creates[request].phase = "pending"
  /\ s.effects[request] = 1
  /\ CompletionGate(request)
  /\ s.slot.phase = "creating" /\ s.slot.ref = request
  /\ s' = [s EXCEPT
     !.creates = [@ EXCEPT ![request] =
          [phase |-> "settled", outcome |-> "started",
           standalone |-> @.standalone]],
     !.slot = [phase |-> "settled", ref |-> request],
     !.evidence = @ \cup CompletionEvidence(request) \cup
          (IF Multileaf(Standalone) THEN {"anchor-multileaf-complete"}
           ELSE {})]

(* The completing request is not the current anchor (never anchored, or the
   slot legally retargeted onto a surviving member while its own leaf stays
   in the tree): settle the ledger only. *)
CompleteCreateSettled(request) ==
  /\ s.creates[request].phase = "pending"
  /\ s.effects[request] = 1
  /\ CompletionGate(request)
  /\ ~(s.slot.phase = "creating" /\ s.slot.ref = request)
  /\ s' = [s EXCEPT
     !.creates = [@ EXCEPT ![request] =
          [phase |-> "settled", outcome |-> "started",
           standalone |-> @.standalone]],
     !.evidence = @ \cup CompletionEvidence(request)]

(* Gate or association failure after dispatch is honest unknown, never a
   definite rejection and never a fabricated success; no extra retirement. *)
CompleteCreateUnknown(request) ==
  /\ s.creates[request].phase = "pending"
  /\ s.effects[request] = 1
  /\ ~CompletionGate(request)
  /\ s' = [s EXCEPT
     !.creates = [@ EXCEPT ![request] =
          [phase |-> "coord-unknown", outcome |-> "unknown",
           standalone |-> @.standalone]],
     !.evidence = @ \cup {"unknown-completion"}]

(* ---- Panel split -------------------------------------------------------- *)

(* The same-Tab aggregate Close fence: its publication re-proves the complete
   captured ordered leaf membership this commit would invalidate. *)
CloseFences(context, tab) ==
  s.close.phase = "closing" /\ s.close.context = context
  /\ s.close.tab = tab

SplitTarget(context, tab) ==
  /\ HasTab(context, tab)
  /\ Len(TabWith(context, tab).leaves) = 1
  /\ ~CloseFences(context, tab)

AdmitSplitStandalone ==
  /\ ~s.retired
  /\ s.splitPhase[Standalone] = "absent"
  /\ \E tab \in TabIds(Standalone):
       /\ SplitTarget(Standalone, tab)
       /\ IF Derivable(Standalone) /\ WeakComposition
          THEN s' = [s EXCEPT
               !.tabs = [@ EXCEPT ![Standalone] =
                    SplitLeaf(Standalone, TabIndex(Standalone, tab))],
               !.revision = @ + 1,
               !.splitPhase = [@ EXCEPT ![Standalone] = "pending"],
               !.retained = [@ EXCEPT ![SplitToken(Standalone)] = TRUE]]
          ELSE s' = [s EXCEPT !.retired = TRUE]

AdmitSplitWorkspace ==
  /\ ~s.retired
  /\ s.splitPhase[Workspace] = "absent"
  /\ \E tab \in TabIds(Workspace):
       /\ SplitTarget(Workspace, tab)
       /\ s' = [s EXCEPT
          !.tabs = [@ EXCEPT ![Workspace] =
               SplitLeaf(Workspace, TabIndex(Workspace, tab))],
          !.revision = @ + 1,
          !.splitPhase = [@ EXCEPT ![Workspace] = "pending"],
          !.retained = [@ EXCEPT ![SplitToken(Workspace)] = TRUE]]

(* A same-Tab structural mutation attempted while its aggregate Close is
   pending is transiently refused; nothing is retained or dispatched. *)
RejectSplitFenced(context) ==
  /\ ~s.retired
  /\ s.splitPhase[context] = "absent"
  /\ \E tab \in TabIds(context):
       /\ CloseFences(context, tab)
       /\ Len(TabWith(context, tab).leaves) = 1
  /\ s' = [s EXCEPT !.evidence = @ \cup {"split-fenced"}]

(* Split completion re-checks the weak composition for standalone metadata. *)
CompleteSplitStarted(context) ==
  /\ s.splitPhase[context] = "pending"
  /\ Derivable(context)
  /\ (context = Workspace \/ WeakComposition)
  /\ s' = [s EXCEPT !.splitPhase = [@ EXCEPT ![context] = "started"]]

CompleteSplitUnknown(context) ==
  /\ s.splitPhase[context] = "pending"
  /\ ~(Derivable(context)
       /\ (context = Workspace \/ WeakComposition))
  /\ s' = [s EXCEPT !.splitPhase = [@ EXCEPT ![context] = "unknown"]]

(* ---- TabMove ------------------------------------------------------------ *)

(* Whole-Tab reorder: leaf membership contents untouched, slot untouched,
   revision +1. Standalone admission uses the strong gate; Workspace admission
   only needs derivable membership and never consults the slot. Reordering a
   pending-Close target is legal because the captured leaf tuple is
   preserved; only same-Tab structural mutations are fenced. *)
MoveTab(context) ==
  /\ ~s.retired
  /\ s.moves < MaxMoves
  /\ \E tab \in TabIds(context):
       /\ s.tabs[context][1].id # tab
       /\ IF Derivable(context)
            /\ (context = Workspace \/ StrongComposition)
          THEN
            LET order == s.tabs[context]
                index == TabIndex(context, tab)
                rest == SelectSeq(order, LAMBDA entry : entry.id # tab)
            IN s' = [s EXCEPT
               !.tabs = [@ EXCEPT ![context] = <<order[index]>> \o rest],
               !.revision = @ + 1,
               !.moves = @ + 1,
               !.evidence = @ \cup
                    (IF Multileaf(context)
                     THEN IF context = Standalone
                          THEN {"multileaf-move-standalone"}
                          ELSE {"multileaf-move-workspace"}
                     ELSE {}) \cup
                    (IF s.close.phase = "closing"
                     THEN {"unfenced-during-close"}
                     ELSE {})]
          ELSE s' = [s EXCEPT !.retired = TRUE]

(* ---- Aggregate Close ---------------------------------------------------- *)

(* The single Close record models one aggregate Close at a time. That is a
   bounded-checking abstraction boundary, not a production concurrency
   restriction: in-source, independent close lifecycles may coexist. *)

(* Runtime admissibility of one Close target member: the owning operation
   must have reached its usable runtime phase - Settled for a Create leaf,
   Started for a Split leaf - and the runtime must still be available. A
   pending owner means Busy; anything else non-usable means Unavailable. A
   mere `not pending` test never makes an unknown or no-runtime binding
   closable. *)
MemberRuntimeOk(context, owner) ==
  /\ ~s.runtimeLost[owner]
  /\ IF owner \in Requests THEN s.creates[owner].phase = "settled"
     ELSE s.splitPhase[owner[2]] = "started"

(* Admission freezes the target Tab's complete ordered leaf membership. A
   corrupt membership or composition fails the admission gate and retires the
   authority, matching the in-source retirement behavior. Every captured leaf
   must own a known usable runtime, so a pending Create member can never be
   legally removed by Close; a pending member on a different surviving Tab
   does not block the target. *)
AdmitClose(context) ==
  /\ ~s.retired
  /\ s.close.phase = "idle"
  /\ TabIds(context) # {}
  /\ IF Derivable(context)
       /\ (context = Workspace \/ WeakComposition)
     THEN \E tab \in TabIds(context):
            /\ \A i \in DOMAIN TabWith(context, tab).leaves :
                 MemberRuntimeOk(context, TabWith(context, tab).leaves[i])
            /\ s' = [s EXCEPT !.close = [EmptyClose EXCEPT
                 !.phase = "closing", !.context = context, !.tab = tab,
                 !.captured = TabWith(context, tab).leaves]]
     ELSE s' = [s EXCEPT !.retired = TRUE]

(* A Close attempted on a target containing a pending member is refused with
   Busy: no capture, no retirement, no ledger entry or effect. Modeled as a
   pure enabledness witness - a refusal changes no state, so the action is
   UNCHANGED and reachability is proved by required action coverage. *)
RefuseCloseBusy(context) ==
  /\ ~s.retired
  /\ s.close.phase = "idle"
  /\ Derivable(context)
  /\ (context = Workspace \/ WeakComposition)
  /\ \E tab \in TabIds(context):
       \E i \in DOMAIN TabWith(context, tab).leaves :
         LeafPending(TabWith(context, tab).leaves[i])
  /\ UNCHANGED vars

(* A non-pending member is still not closable while its runtime is unknown
   or unavailable; such a target refuses the Close with Unavailable. *)
RefuseCloseUnavailable(context) ==
  /\ ~s.retired
  /\ s.close.phase = "idle"
  /\ Derivable(context)
  /\ (context = Workspace \/ WeakComposition)
  /\ \E tab \in TabIds(context):
       \E i \in DOMAIN TabWith(context, tab).leaves :
         LET owner == TabWith(context, tab).leaves[i]
         IN ~LeafPending(owner) /\ ~MemberRuntimeOk(context, owner)
  /\ UNCHANGED vars

CloseCleanupReady ==
  /\ s.close.phase = "closing"
  /\ ~s.close.ready
  /\ s' = [s EXCEPT !.close = [@ EXCEPT !.ready = TRUE]]

(* Publication re-proves the frozen leaf tuple against the CURRENT membership
   and commits the removal under the weak before-composition. A mismatch or
   corrupt composition retires the authority. On success the standalone slot
   keeps its anchor when the anchor survives, otherwise retargets to the first
   surviving member (Creating while that operation is pending, Settled
   otherwise), or Empty when none survive. *)
PublishClose ==
  /\ s.close.phase = "closing"
  /\ s.close.ready
  /\ LET context == s.close.context
         current == IF HasTab(context, s.close.tab)
                    THEN TabWith(context, s.close.tab).leaves ELSE <<>>
         beforeOk == Derivable(context)
                     /\ (context = Workspace \/ WeakComposition)
     IN IF ~beforeOk \/ current # s.close.captured
        THEN s' = [s EXCEPT !.retired = TRUE]
        ELSE
          LET remainingTabs ==
                SelectSeq(s.tabs[context],
                          LAMBDA tab : tab.id # s.close.tab)
              remaining == RemainingMembers(context, remainingTabs)
              remainingSet == {remaining[i] : i \in 1..Len(remaining)}
              newSlot ==
                IF context # Standalone THEN s.slot
                ELSE IF s.slot.phase # "empty" /\ s.slot.ref \in remainingSet
                     THEN s.slot
                ELSE IF Len(remaining) = 0 THEN EmptySlot
                ELSE [phase |-> IF LeafPending(remaining[1])
                                THEN "creating" ELSE "settled",
                      ref |-> remaining[1]]
          IN s' = [s EXCEPT
             !.tabs = [@ EXCEPT ![context] = remainingTabs],
             !.revision = @ + 1,
             !.slot = newSlot,
             !.close = [@ EXCEPT !.phase = "closed"],
             !.evidence = @ \cup
                  (IF Len(remaining) = 0 THEN {"empty-after-close"} ELSE {})
                  \cup
                  (IF context = Standalone /\ s.slot.phase # "empty"
                      /\ s.slot.ref \notin remainingSet /\ Len(remaining) > 0
                   THEN IF LeafPending(remaining[1])
                        THEN {"retarget-to-pending"}
                        ELSE {"retarget-to-settled"}
                   ELSE {})]

(* ---- Fault injection ---------------------------------------------------- *)

(* Bounded corruption examples, standalone domain only. Each is reachable and
   must surface as admission retirement or post-commit unknown, never as a
   silently accepted state. *)
CorruptAnchorDuplicate ==
  /\ ~s.retired
  /\ s.corrupted = "none"
  /\ s.slot.phase # "empty"
  /\ s.slot.ref \in Requests
  /\ \E tab \in TabIds(Standalone):
       /\ Len(TabWith(Standalone, tab).leaves) < MaxLeaves
       /\ s' = [s EXCEPT
          !.tabs = [@ EXCEPT ![Standalone] =
               [@ EXCEPT ![TabIndex(Standalone, tab)] =
                    [@ EXCEPT !.leaves = Append(@, s.slot.ref)]]],
          !.corrupted = "anchor-dup"]

CorruptAnchorLost ==
  /\ ~s.retired
  /\ s.corrupted = "none"
  /\ s.slot.phase # "empty"
  /\ s' = [s EXCEPT
     !.retained = [@ EXCEPT ![s.slot.ref] = FALSE],
     !.corrupted = "anchor-missing"]

CorruptLeafUnbound ==
  /\ ~s.retired
  /\ s.corrupted = "none"
  /\ Len(Members(Standalone)) > 0
  /\ s' = [s EXCEPT !.corrupted = "leaf-unbound"]

(* Bounded runtime-loss fault: a member's runtime may become unavailable
   while its retained binding stays valid. Fault-tagged only; it is not a
   membership corruption and never retires the authority. It exists to make
   the Unavailable close refusal reachable - every membership corruption
   already fails the close gate before runtime admissibility is evaluated.
   Bounded to one loss at a time on a non-pending standalone member: a
   pending member already refuses with Busy regardless of runtime state, and
   the Unavailable refusal uses the same predicate in both contexts. *)
LoseRuntime(owner) ==
  /\ ~s.retired
  /\ s.corrupted = "none"
  /\ s.close.phase = "idle"
  /\ ~s.runtimeLost[owner]
  /\ ~LeafPending(owner)
  /\ \A other \in Owners : other = owner \/ ~s.runtimeLost[other]
  /\ owner \in MemberSet(Standalone)
  /\ s' = [s EXCEPT !.runtimeLost = [@ EXCEPT ![owner] = TRUE]]

(* ---- Reachable witnesses required for coverage --------------------------- *)

ObserveMultileafStandalone ==
  /\ Multileaf(Standalone) /\ UNCHANGED vars
ObserveMultileafWorkspace ==
  /\ Multileaf(Workspace) /\ UNCHANGED vars
ObservePendingAnchorOnMultileaf ==
  /\ s.slot.phase = "creating" /\ Multileaf(Standalone)
  /\ \E i \in 1..Len(s.tabs[Standalone]) :
       /\ Len(s.tabs[Standalone][i].leaves) > 1
       /\ s.slot.ref \in {s.tabs[Standalone][i].leaves[j] :
            j \in 1..Len(s.tabs[Standalone][i].leaves)}
  /\ UNCHANGED vars
ObserveRetireAfterCorruption ==
  /\ s.retired /\ s.corrupted # "none" /\ UNCHANGED vars
ObserveUnknownCompletion ==
  /\ \E request \in Requests :
       s.creates[request].phase = "coord-unknown"
  /\ UNCHANGED vars
ObserveUnfencedCreateDuringClose ==
  /\ "unfenced-during-close" \in s.evidence /\ UNCHANGED vars
ObserveFencedSplit ==
  /\ "split-fenced" \in s.evidence /\ UNCHANGED vars

(* ---- Specification ------------------------------------------------------ *)

Next ==
  \/ \E request \in Requests:
       \/ AdmitCreateStandalone(request)
       \/ AdmitCreateWorkspace(request)
       \/ RejectCreatePreflight(request)
       \/ ReplayPendingCreate(request)
       \/ ReplayFinalCreate(request)
       \/ DispatchCreate(request)
       \/ CompleteCreateAnchorSettled(request)
       \/ CompleteCreateSettled(request)
       \/ CompleteCreateUnknown(request)
  \/ AdmitSplitStandalone
  \/ AdmitSplitWorkspace
  \/ RejectSplitFenced(Standalone)
  \/ RejectSplitFenced(Workspace)
  \/ AdmitClose(Standalone)
  \/ AdmitClose(Workspace)
  \/ RefuseCloseBusy(Standalone)
  \/ RefuseCloseBusy(Workspace)
  \/ RefuseCloseUnavailable(Standalone)
  \/ RefuseCloseUnavailable(Workspace)
  \/ MoveTab(Standalone)
  \/ MoveTab(Workspace)
  \/ \E context \in Contexts:
       \/ CompleteSplitStarted(context)
       \/ CompleteSplitUnknown(context)
  \/ CloseCleanupReady
  \/ PublishClose
  \/ CorruptAnchorLost
  \/ CorruptLeafUnbound
  \/ CorruptAnchorDuplicate
  \/ \E owner \in Owners : LoseRuntime(owner)
  \/ ObserveMultileafStandalone \/ ObserveMultileafWorkspace
  \/ ObservePendingAnchorOnMultileaf \/ ObserveRetireAfterCorruption
  \/ ObserveUnknownCompletion \/ ObserveUnfencedCreateDuringClose
  \/ ObserveFencedSplit

FairComplete(request) ==
  \/ DispatchCreate(request)
  \/ CompleteCreateAnchorSettled(request)
  \/ CompleteCreateSettled(request)
  \/ CompleteCreateUnknown(request)

Spec ==
  Init /\ [][Next]_vars
  /\ \A request \in Requests : WF_vars(FairComplete(request))

(* Diagnostic-only specification for the withheld-completion negative
   control: identical Init/Next with no fairness assumption. TLC must report
   PendingCompletionProgress violated under SpecNoFairness; that trace is
   produced by removing the accepted fair-completion premise, so it is
   evidence that the property detects withheld completion - not a
   counterexample to the production Spec. *)
SpecNoFairness == Init /\ [][Next]_vars

(* ---- Invariants --------------------------------------------------------- *)

LeafSeqs == UNION {[1..k -> Owners] : k \in 0..MaxLeaves}
TabRecords == [id : 1..MaxTabs, leaves : LeafSeqs]
TabSeqs == UNION {[1..k -> TabRecords] : k \in 0..MaxTabs}

TypeOK ==
  /\ s.tabs \in [Contexts -> TabSeqs]
  /\ s.nextTab \in [Contexts -> 1..(MaxTabs + 1)]
  /\ s.revision \in 0..12
  /\ s.slot \in [phase : {"empty", "creating", "settled"},
                 ref : Owners \cup {None}]
  /\ s.creates \in [Requests ->
       [phase : {"absent", "pending", "settled", "coord-unknown", "rejected"},
        outcome : {"none", "started", "unknown"},
        standalone : BOOLEAN]]
  /\ s.splitPhase \in [Contexts -> {"absent", "pending", "started", "unknown"}]
  /\ s.effects \in [Requests -> 0..1]
  /\ s.retained \in [Owners -> BOOLEAN]
  /\ s.close \in [phase : {"idle", "closing", "closed"},
                  context : Contexts \cup {None},
                  tab : -1..MaxTabs,
                  captured : LeafSeqs,
                  ready : BOOLEAN]
  /\ s.corrupted \in {"none", "anchor-dup", "anchor-missing", "leaf-unbound"}
  /\ s.moves \in 0..MaxMoves
  /\ s.retired \in BOOLEAN
  /\ s.runtimeLost \in [Owners -> BOOLEAN]
  /\ s.evidence \subseteq {"create-on-multileaf", "unfenced-during-close",
     "replay-pending", "replay-final", "non-current-member-completion",
     "off-membership-completion", "empty-slot-completion",
     "multileaf-completion", "anchor-multileaf-complete",
     "unknown-completion", "split-fenced", "multileaf-move-standalone",
     "multileaf-move-workspace", "empty-after-close", "retarget-to-pending",
     "retarget-to-settled"}

(* Core claim: a legal supported multi-leaf shape never retires the shared
   authority; retirement requires an actual corruption or composition proof
   failure. *)
NoShapeRetirement == s.retired => s.corrupted # "none"

(* Valid-state construction premise: without corruption every projected leaf
   binding is retained. *)
MembersRetained ==
  s.corrupted = "none" =>
    \A context \in Contexts :
      \A owner \in MemberSet(context) : s.retained[owner]

(* Without corruption the anchor relation holds: Empty iff no standalone
   members; otherwise the slot reference resolves exactly once. *)
AnchorExactInValidStates ==
  s.corrupted = "none" =>
    /\ (s.slot.phase = "empty") = (Len(Members(Standalone)) = 0)
    /\ (s.slot.phase # "empty" =>
         Cardinality({i \in 1..Len(Members(Standalone)) :
            Members(Standalone)[i] = s.slot.ref}) = 1)

(* An open aggregate Close fences the frozen Tab tuple: while pending and
   uncorrupted, the target Tab's leaves remain exactly the captured order. *)
AggregateFenceStable ==
  (s.corrupted = "none" /\ s.close.phase = "closing") =>
    s.close.captured = TabWith(s.close.context, s.close.tab).leaves

(* Each committed Create dispatched at most once. *)
NoRedispatch == \A request \in Requests : s.effects[request] <= 1

(* Shared global revision counts exactly the structural commits. *)
SharedRevision ==
  s.revision =
    Cardinality({request \in Requests : CommittedCreate(request)})
    + Cardinality({context \in Contexts : s.splitPhase[context] # "absent"})
    + s.moves
    + (IF s.close.phase = "closed" THEN 1 ELSE 0)

(* Retained ledger results are immutable: settled/rejected/unknown entries
   never change again. *)
LedgerResultStable ==
  [][\A request \in Requests :
       s.creates[request].phase \in {"settled", "coord-unknown", "rejected"}
         => s'.creates[request] = s.creates[request]]_vars

(* Completion never mutates the projection: membership and revision are
   unchanged by any pending -> final ledger transition. *)
CompletionPreservesProjection ==
  [][(\E request \in Requests :
        s.creates[request].phase = "pending"
        /\ s'.creates[request].phase # "pending") =>
      s.tabs' = s.tabs /\ s.revision' = s.revision]_vars

(* The standalone slot changes only through (a) a first Create commit, (b) a
   completing Create that is the current Creating anchor, or (c) a successful
   standalone Close publication retarget. No other step may write the slot. *)
SlotTransitionsExact ==
  [][s.slot' # s.slot =>
      \/ /\ s'.close.phase = "closed"
         /\ s.close.phase = "closing"
         /\ s.close.context = Standalone
      \/ \E request \in Requests :
           /\ s.creates[request].phase = "absent"
           /\ s'.creates[request].phase = "pending"
           /\ s.slot.phase = "empty"
           /\ s'.slot = [phase |-> "creating", ref |-> request]
      \/ \E request \in Requests :
           /\ s.creates[request].phase = "pending"
           /\ s'.creates[request].phase = "settled"
           /\ s.slot = [phase |-> "creating", ref |-> request]
           /\ s'.slot = [phase |-> "settled", ref |-> request]]_vars

(* Retarget legality: a successful standalone Close publication leaves the
   slot anchored on the surviving anchor, else on the first surviving member
   with the phase that member's operation actually has, else Empty. *)
RetargetLegal ==
  [][/\ s.close.phase = "closing"
      /\ s'.close.phase = "closed"
      /\ s.close.context = Standalone
      /\ ~s'.retired
     =>
      LET remainingTabs == s'.tabs[Standalone]
          remaining == Flatten([i \in 1..Len(remainingTabs) |->
                        remainingTabs[i].leaves])
          remainingSet == {remaining[i] : i \in 1..Len(remaining)}
      IN IF s.slot.phase # "empty" /\ s.slot.ref \in remainingSet
         THEN s'.slot = s.slot
         ELSE IF Len(remaining) = 0 THEN s'.slot = EmptySlot
         ELSE s'.slot.ref = remaining[1]
              /\ s'.slot.phase =
                   IF LeafPending(remaining[1]) THEN "creating" ELSE "settled"
  ]_vars

(* Workspace changes never write the standalone slot. *)
WorkspaceIsolation ==
  [][s'.tabs[Workspace] # s.tabs[Workspace] => s.slot' = s.slot]_vars

(* A dispatched Create never ends in a definite pre-commit rejection, and a
   coordination failure is never reported as a successful settlement. *)
NoRejectionAfterDispatch ==
  \A request \in Requests :
    s.effects[request] = 1 => s.creates[request].phase # "rejected"
UnknownIsNotSuccess ==
  \A request \in Requests :
    s.creates[request].phase = "coord-unknown" =>
      s.creates[request].outcome = "unknown"

(* The final ledger outcomes reachable from a pending Create: a retained
   settle or an honest coordination-unknown. *)
RequestFinal(request) ==
  s.creates[request].phase \in {"settled", "coord-unknown"}

(* Real per-request progress obligation: once a Create commits as pending it
   must eventually reach a final ledger outcome. The obligation attaches at
   admission and is not erased by later corruption, runtime loss or
   retirement - a fault can only turn the outcome into honest unknown, never
   into silence, because the completion actions stay enabled. The premise is
   WF_vars(FairComplete(request)): fair dispatch plus fair completion under
   the retained association. *)
PendingCompletionProgress ==
  \A request \in Requests :
    (s.creates[request].phase = "pending") ~> RequestFinal(request)

=============================================================================
