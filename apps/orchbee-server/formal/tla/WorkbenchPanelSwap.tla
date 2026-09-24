-------------------------- MODULE WorkbenchPanelSwap --------------------------
EXTENDS Integers, Sequences, FiniteSets

(* Fixed slots stand for leaf positions in one validated nested tree. Whole
   Panel identity carries its Presentation/binding metadata; concrete permutation
   tests refine that footprint. This model owns canonical/focus phase ordering,
   ambiguous reply cuts, no replay and preservation of known commit evidence.
   It does not model transport liveness, arbitrary tree validation, selection ABA,
   or retained Server ledger internals (covered by the placement authority). *)

Original == <<"a", "b", "c">>
Exchanged == <<"c", "b", "a">>
InitialFocus == [one |-> "b", two |-> "c"]
SourceFocus == [one |-> "a", two |-> "c"]
Topology == <<"split-1", "slot-1", "slot-2", "split-2", "slot-3", "slot-4">>

VARIABLES positions, topology, revision, identityCursor, focus, phase,
          canonical, acknowledgment, submissions, focusSubmissions
vars == <<positions, topology, revision, identityCursor, focus, phase,
          canonical, acknowledgment, submissions, focusSubmissions>>

Init ==
  /\ positions = Original
  /\ topology = Topology
  /\ revision = 0
  /\ identityCursor = 17
  /\ focus = InitialFocus
  /\ phase = "resolve"
  /\ canonical = "not-submitted"
  /\ acknowledgment = "not-submitted"
  /\ submissions = 0
  /\ focusSubmissions = 0

ResolvePair ==
  /\ phase = "resolve"
  /\ phase' = "swap"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, focus, canonical,
                 acknowledgment, submissions, focusSubmissions>>

ResolveNoop ==
  /\ phase = "resolve"
  /\ phase' = "done"
  /\ canonical' = "unchanged"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, focus,
                 acknowledgment, submissions, focusSubmissions>>

RefuseSwap ==
  /\ phase = "swap"
  /\ phase' = "done"
  /\ canonical' = "refused"
  /\ submissions' = 1
  /\ UNCHANGED <<positions, topology, revision, identityCursor, focus,
                 acknowledgment, focusSubmissions>>

CommitSwap ==
  /\ phase = "swap"
  /\ positions' = Exchanged
  /\ revision' = 1
  /\ canonical' = "committed"
  /\ submissions' = 1
  /\ phase' = "observe"
  /\ UNCHANGED <<topology, identityCursor, focus, acknowledgment, focusSubmissions>>

LoseSwapReply ==
  /\ phase = "swap"
  /\ revision' \in 0..1
  /\ positions' = IF revision' = 1 THEN Exchanged ELSE Original
  /\ canonical' = "unknown"
  /\ submissions' = 1
  /\ phase' = "done"
  /\ UNCHANGED <<topology, identityCursor, focus, acknowledgment, focusSubmissions>>

ObserveReady ==
  /\ phase = "observe"
  /\ phase' = "focus"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, focus, canonical,
                 acknowledgment, submissions, focusSubmissions>>

StopWaiting ==
  /\ phase \in {"resolve", "swap", "observe", "focus"}
  /\ phase' = "done"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, focus, canonical,
                 acknowledgment, submissions, focusSubmissions>>

ApplyFocus ==
  /\ phase = "focus"
  /\ focus' = SourceFocus
  /\ acknowledgment' = "applied"
  /\ focusSubmissions' = 1
  /\ phase' = "done"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, canonical, submissions>>

LoseFocusReply ==
  /\ phase = "focus"
  /\ focus' \in {InitialFocus, SourceFocus}
  /\ acknowledgment' = "unknown"
  /\ focusSubmissions' = 1
  /\ phase' = "done"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, canonical, submissions>>

RefuseFocus ==
  /\ phase = "focus"
  /\ acknowledgment' = "refused"
  /\ focusSubmissions' = 1
  /\ phase' = "done"
  /\ UNCHANGED <<positions, topology, revision, identityCursor, focus, canonical, submissions>>

Done == phase = "done" /\ UNCHANGED vars

Next == ResolvePair \/ ResolveNoop \/ RefuseSwap \/ CommitSwap \/ LoseSwapReply
        \/ ObserveReady \/ StopWaiting \/ ApplyFocus \/ LoseFocusReply \/ RefuseFocus \/ Done

TypeOK ==
  /\ positions \in {Original, Exchanged}
  /\ topology = Topology
  /\ revision \in 0..1
  /\ identityCursor = 17
  /\ focus \in {InitialFocus, SourceFocus}
  /\ phase \in {"resolve", "swap", "observe", "focus", "done"}
  /\ canonical \in {"not-submitted", "refused", "unchanged", "committed", "unknown"}
  /\ acknowledgment \in {"not-submitted", "applied", "refused", "unknown"}
  /\ submissions \in 0..1
  /\ focusSubmissions \in 0..1

AtomicPositions == positions = IF revision = 0 THEN Original ELSE Exchanged
NoReplay == submissions <= 1 /\ focusSubmissions <= 1
KnownCommit == canonical = "committed" => revision = 1
FocusRequiresCommit == focusSubmissions = 1 => canonical = "committed"
OtherClientUnchanged == focus.two = InitialFocus.two
NoopHasNoEffects == canonical = "unchanged" =>
  /\ positions = Original
  /\ focus = InitialFocus
  /\ submissions = 0
  /\ focusSubmissions = 0
CompleteIsTruthful == acknowledgment = "applied" =>
  /\ canonical = "committed"
  /\ positions = Exchanged
  /\ focus.one = "a"

Spec == Init /\ [][Next]_vars
=============================================================================
