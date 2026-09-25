----------------------- MODULE NativePanelInputOwnership -----------------------
EXTENDS Integers, Sequences

(* One bounded right-button gesture. Publication and terminal permission are
   assumptions supplied by their existing owners; no transport liveness claim. *)
VARIABLES policy, phase, target, focus, trace, cancelled, foreignIgnored
vars == <<policy, phase, target, focus, trace, cancelled, foreignIgnored>>
Panels == {"a", "b"}

Init ==
  /\ policy \in {"orchbee", "panel"}
  /\ phase = "idle" /\ target \in Panels /\ focus \in Panels
  /\ trace = <<>> /\ cancelled = FALSE /\ foreignIgnored = FALSE

SetOrchbee ==
  /\ policy = "panel" /\ policy' = "orchbee"
  /\ UNCHANGED <<phase, target, focus, trace, cancelled, foreignIgnored>>
SetPanel ==
  /\ policy = "orchbee" /\ policy' = "panel"
  /\ UNCHANGED <<phase, target, focus, trace, cancelled, foreignIgnored>>
QueueRight ==
  /\ phase = "idle" /\ policy = "panel" /\ focus # target
  /\ phase' = "pending" /\ focus' = target
  /\ UNCHANGED <<policy, target, trace, cancelled, foreignIgnored>>
AdmitRight ==
  /\ phase = "idle" /\ policy = "panel" /\ focus = target
  /\ phase' = "pane" /\ trace' = <<[button |-> "right", kind |-> "press", pane |-> target]>>
  /\ UNCHANGED <<policy, target, focus, cancelled, foreignIgnored>>
FlushRight ==
  /\ phase = "pending" /\ policy = "panel" /\ focus = target
  /\ phase' = "pane" /\ trace' = <<[button |-> "right", kind |-> "press", pane |-> target]>>
  /\ UNCHANGED <<policy, target, focus, cancelled, foreignIgnored>>
DropQueued ==
  /\ phase = "pending" /\ policy = "orchbee"
  /\ phase' = "done" /\ cancelled' = TRUE
  /\ UNCHANGED <<policy, target, focus, trace, foreignIgnored>>
ReleaseRight ==
  /\ phase = "pane" /\ focus = target
  /\ phase' = "done"
  /\ trace' = Append(trace, [button |-> "right", kind |-> "release", pane |-> target])
  /\ UNCHANGED <<policy, target, focus, cancelled, foreignIgnored>>
RejectForeignButton ==
  /\ phase = "pane" /\ ~foreignIgnored /\ foreignIgnored' = TRUE
  /\ UNCHANGED <<policy, phase, target, focus, trace, cancelled>>
CancelGesture ==
  /\ phase \in {"pending", "pane"}
  /\ phase' = "done" /\ cancelled' = TRUE
  /\ focus' \in Panels
  /\ UNCHANGED <<policy, target, trace, foreignIgnored>>
IdleOrDone == phase \in {"idle", "done"} /\ UNCHANGED vars
Next == SetOrchbee \/ SetPanel \/ QueueRight \/ AdmitRight \/ FlushRight \/ DropQueued \/ ReleaseRight \/ RejectForeignButton \/ CancelGesture \/ IdleOrDone

TypeOK ==
  /\ policy \in {"orchbee", "panel"}
  /\ phase \in {"idle", "pending", "pane", "done"}
  /\ target \in Panels /\ focus \in Panels
  /\ cancelled \in BOOLEAN /\ foreignIgnored \in BOOLEAN
  /\ Len(trace) \in 0..2
CapturedTarget == \A i \in 1..Len(trace): trace[i].pane = target /\ trace[i].button = "right"
ReleaseFollowsPress == Len(trace) = 2 => trace[1].kind = "press" /\ trace[2].kind = "release"
PendingHasNoBytes == phase \in {"idle", "pending"} => trace = <<>>
CancellationNeverReleases == cancelled => Len(trace) < 2
Spec == Init /\ [][Next]_vars
=============================================================================
