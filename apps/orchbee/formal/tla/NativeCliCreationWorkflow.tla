----------------------- MODULE NativeCliCreationWorkflow -----------------------
EXTENDS Integers

(* Finite CLI phase/evidence abstraction. Canonical allocation and Client draw
   refine existing owner models; this model covers their ordering and fault cuts,
   not transport liveness, arbitrary topology or selection ABA. *)
VARIABLES phase, wantsRatio, wantsFocus, created, canonical, ratio, focus,
          ratioAck, focusAck, calls, targetCurrent, complete
vars == <<phase, wantsRatio, wantsFocus, created, canonical, ratio, focus,
          ratioAck, focusAck, calls, targetCurrent, complete>>

Init ==
  /\ phase = "preflight"
  /\ wantsRatio \in BOOLEAN /\ wantsFocus \in BOOLEAN
  /\ created = FALSE /\ canonical = "none"
  /\ ratio = FALSE /\ focus = FALSE
  /\ ratioAck = "none" /\ focusAck = "none"
  /\ calls = [create |-> 0, ratio |-> 0, focus |-> 0]
  /\ targetCurrent = TRUE /\ complete = FALSE

Preflight ==
  /\ phase = "preflight" /\ phase' = "create"
  /\ UNCHANGED <<wantsRatio, wantsFocus, created, canonical, ratio, focus,
                 ratioAck, focusAck, calls, targetCurrent, complete>>

Create ==
  /\ phase = "create"
  /\ created' = TRUE /\ canonical' = "known"
  /\ calls' = [calls EXCEPT !.create = 1]
  /\ phase' = IF wantsRatio THEN "ratio" ELSE IF wantsFocus THEN "focus" ELSE "done"
  /\ complete' = (~wantsRatio /\ ~wantsFocus)
  /\ UNCHANGED <<wantsRatio, wantsFocus, ratio, focus, ratioAck, focusAck, targetCurrent>>

LoseCreate ==
  /\ phase = "create" /\ created' \in BOOLEAN /\ canonical' = "unknown"
  /\ calls' = [calls EXCEPT !.create = 1] /\ phase' = "done"
  /\ UNCHANGED <<wantsRatio, wantsFocus, ratio, focus, ratioAck, focusAck, targetCurrent, complete>>

ReplaceTarget ==
  /\ phase = "ratio" /\ targetCurrent /\ targetCurrent' = FALSE
  /\ UNCHANGED <<phase, wantsRatio, wantsFocus, created, canonical, ratio, focus,
                 ratioAck, focusAck, calls, complete>>

AdjustRatio ==
  /\ phase = "ratio" /\ targetCurrent
  /\ ratio' = TRUE /\ ratioAck' = "known"
  /\ calls' = [calls EXCEPT !.ratio = 1]
  /\ phase' = IF wantsFocus THEN "focus" ELSE "done"
  /\ complete' = ~wantsFocus
  /\ UNCHANGED <<wantsRatio, wantsFocus, created, canonical, focus, focusAck, targetCurrent>>

LoseRatio ==
  /\ phase = "ratio" /\ targetCurrent
  /\ ratio' \in BOOLEAN /\ ratioAck' = "unknown"
  /\ calls' = [calls EXCEPT !.ratio = 1] /\ phase' = "done"
  /\ UNCHANGED <<wantsRatio, wantsFocus, created, canonical, focus, focusAck, targetCurrent, complete>>

Focus ==
  /\ phase = "focus" /\ focus' = TRUE /\ focusAck' = "known"
  /\ calls' = [calls EXCEPT !.focus = 1] /\ phase' = "done" /\ complete' = TRUE
  /\ UNCHANGED <<wantsRatio, wantsFocus, created, canonical, ratio, ratioAck, targetCurrent>>

LoseFocus ==
  /\ phase = "focus" /\ focus' \in BOOLEAN /\ focusAck' = "unknown"
  /\ calls' = [calls EXCEPT !.focus = 1] /\ phase' = "done"
  /\ UNCHANGED <<wantsRatio, wantsFocus, created, canonical, ratio, ratioAck, targetCurrent, complete>>

Stop ==
  /\ phase # "done" /\ phase' = "done"
  /\ UNCHANGED <<wantsRatio, wantsFocus, created, canonical, ratio, focus,
                 ratioAck, focusAck, calls, targetCurrent, complete>>
Done == phase = "done" /\ UNCHANGED vars
Next == Preflight \/ Create \/ LoseCreate \/ ReplaceTarget \/ AdjustRatio \/ LoseRatio \/ Focus \/ LoseFocus \/ Stop \/ Done

TypeOK ==
  /\ phase \in {"preflight", "create", "ratio", "focus", "done"}
  /\ wantsRatio \in BOOLEAN /\ wantsFocus \in BOOLEAN /\ created \in BOOLEAN
  /\ ratio \in BOOLEAN /\ focus \in BOOLEAN /\ targetCurrent \in BOOLEAN /\ complete \in BOOLEAN
  /\ canonical \in {"none", "known", "unknown"}
  /\ ratioAck \in {"none", "known", "unknown"} /\ focusAck \in {"none", "known", "unknown"}
  /\ calls \in [{"create", "ratio", "focus"} -> 0..1]
KnownCreationPersists == canonical = "known" => created
PostCreateRequiresKnown == calls.ratio + calls.focus > 0 => canonical = "known"
RatioIdentityFence == ratio => targetCurrent
NoUnrequestedEffect == (calls.ratio > 0 => wantsRatio) /\ (calls.focus > 0 => wantsFocus)
PhaseOrder == calls.focus > 0 /\ wantsRatio => ratioAck = "known"
CompleteTruth == complete => canonical = "known" /\ (wantsRatio => ratioAck = "known") /\ (wantsFocus => focusAck = "known")
Spec == Init /\ [][Next]_vars
=============================================================================
