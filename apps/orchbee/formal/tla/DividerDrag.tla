---------------------- MODULE DividerDrag ----------------------
EXTENDS Naturals, FiniteSets, TLC
VARIABLE s
vars == <<s>>
Init == s = [current |-> 1, painted |-> 0, captured |-> FALSE,
             issued |-> 0, drag |-> 0, source |-> 0, changed |-> FALSE,
             pending |-> 0, pendingSource |-> 0, released |-> {}, attempts |-> {},
             committed |-> {}, cancelled |-> {}, coverage |-> "Init"]
Paint == /\ ~s.captured /\ s' = [s EXCEPT !.painted = s.current, !.coverage = "Paint"]
Press == /\ ~s.captured /\ s.painted = s.current /\ s.drag = 0 /\ s.pending = 0 /\ s.issued < 2
         /\ s' = [s EXCEPT !.issued = @ + 1, !.drag = s.issued + 1, !.source = s.current,
                            !.changed = FALSE, !.coverage = "Press"]
Move == /\ s.drag # 0 /\ \E changed \in BOOLEAN:
            s' = [s EXCEPT !.changed = changed, !.coverage = "Move"]
\* The release coordinate has its own final projection, independent of the last motion.
ReleaseNoChange == /\ s.drag # 0
                   /\ s' = [s EXCEPT !.drag = 0, !.changed = FALSE, !.coverage = "ReleaseNoChange"]
Release == /\ s.drag # 0 /\ s.pending = 0
           /\ s.source = s.current /\ s.painted = s.current /\ ~s.captured
           /\ s' = [s EXCEPT !.released = @ \cup {s.drag}, !.attempts = @ \cup {s.drag},
                              !.pending = s.drag, !.pendingSource = s.source,
                              !.drag = 0, !.changed = FALSE, !.coverage = "Release"]
Esc == /\ s.drag # 0
       /\ s' = [s EXCEPT !.cancelled = @ \cup {s.drag}, !.drag = 0,
                          !.changed = FALSE, !.coverage = "Esc"]
ShiftCancel == /\ s.drag # 0
               /\ s' = [s EXCEPT !.cancelled = @ \cup {s.drag}, !.drag = 0,
                                  !.changed = FALSE, !.coverage = "ShiftCancel"]
ShiftPress == /\ s.drag = 0 /\ s' = [s EXCEPT !.coverage = "ShiftPress"]
BusyPress == /\ s.pending # 0 /\ s' = [s EXCEPT !.coverage = "BusyPress"]
DuplicateRelease == /\ s.drag = 0 /\ s' = [s EXCEPT !.coverage = "DuplicateRelease"]
DeadPress == /\ s.painted # s.current /\ s' = [s EXCEPT !.coverage = "DeadPress"]
Invalidate == s' = [s EXCEPT !.painted = 0, !.cancelled = IF s.drag = 0 THEN @ ELSE @ \cup {s.drag},
                              !.drag = 0, !.changed = FALSE, !.coverage = "Invalidate"]
SurfaceChange == /\ s.current = 1
                 /\ s' = [s EXCEPT !.current = 2, !.painted = 0,
                       !.cancelled = IF s.drag = 0 THEN @ ELSE @ \cup {s.drag},
                       !.drag = 0, !.changed = FALSE, !.coverage = "SurfaceChange"]
OpenCapture == s' = [s EXCEPT !.captured = TRUE, !.painted = 0,
                       !.cancelled = IF s.drag = 0 THEN @ ELSE @ \cup {s.drag},
                       !.drag = 0, !.changed = FALSE, !.coverage = "OpenCapture"]
CloseCapture == /\ s.captured /\ s' = [s EXCEPT !.captured = FALSE, !.coverage = "CloseCapture"]
SettleKnown == /\ s.pending # 0 /\ s.pendingSource = s.current
               /\ s' = [s EXCEPT !.committed = @ \cup {s.pending}, !.pending = 0, !.coverage = "SettleKnown"]
SettleRejected == /\ s.pending # 0 /\ s' = [s EXCEPT !.pending = 0, !.coverage = "SettleRejected"]
SettleUnknown == /\ s.pending # 0 /\ s' = [s EXCEPT !.pending = 0, !.coverage = "SettleUnknown"]
Next == Paint \/ Press \/ Move \/ ReleaseNoChange \/ Release \/ Esc \/ ShiftCancel \/ ShiftPress
        \/ BusyPress \/ DuplicateRelease \/ DeadPress \/ Invalidate \/ SurfaceChange \/ OpenCapture
        \/ CloseCapture \/ SettleKnown \/ SettleRejected \/ SettleUnknown
Spec == Init /\ [][Next]_vars
TypeOK == /\ s.current \in 1..2 /\ s.painted \in 0..2 /\ s.issued \in 0..2
          /\ s.drag \in 0..s.issued /\ s.pending \in 0..s.issued /\ s.changed \in BOOLEAN
          /\ s.attempts \subseteq 1..s.issued /\ s.cancelled \subseteq 1..s.issued
ReleaseOnly == s.attempts = s.released
NoCancelledCommit == s.cancelled \cap s.attempts = {}
NoBacklog == s.pending # 0 => s.drag = 0
PreviewOnly == [][(s.coverage' \in {"Press", "Move", "Esc", "ShiftCancel", "Invalidate", "SurfaceChange", "OpenCapture"}
                  => s'.attempts = s.attempts /\ s'.committed = s.committed)]_vars
ExactHandoff == [][(s'.attempts # s.attempts => s.coverage' = "Release" /\ s.drag # 0
                  /\ s.source = s.current /\ s.painted = s.current /\ ~s.captured /\ s.pending = 0)]_vars
=============================================================================
