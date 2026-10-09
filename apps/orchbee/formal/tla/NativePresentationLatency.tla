---------------- MODULE NativePresentationLatency ----------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANT Mode
VARIABLE s
vars == <<s>>

Init == s = [ui |-> "inspect", pending |-> FALSE, token |-> FALSE,
             work |-> FALSE, published |-> 0, consumed |-> 0,
             pane |-> 1, binding |-> 1, generation |-> 0,
             phase |-> "connecting", cache |-> [p \in 1..2 |-> 0],
             display |-> [pane |-> 0, binding |-> 0, fresh |-> FALSE],
             feedback |-> FALSE, input |-> [accepted |-> FALSE, fresh |-> FALSE]]

Publish == /\ Mode = "wake" /\ ~s.work /\ s.published < 2
           /\ s' = [s EXCEPT !.work = TRUE, !.pending = TRUE,
                     !.token = TRUE, !.published = @ + 1]
Inspect == /\ Mode = "wake" /\ s.ui = "inspect"
           /\ s' = [s EXCEPT !.pending = FALSE,
                     !.ui = IF s.pending THEN "service" ELSE "wait"]
Park == /\ Mode = "wake" /\ s.ui = "wait"
        /\ s' = [s EXCEPT !.ui = IF s.token THEN "service" ELSE "parked",
                  !.token = FALSE]
Wake == /\ Mode = "wake" /\ s.ui = "parked" /\ s.token
        /\ s' = [s EXCEPT !.ui = "service", !.token = FALSE]
Service == /\ Mode = "wake" /\ s.ui = "service"
           /\ s' = [s EXCEPT !.ui = "inspect", !.work = FALSE,
                     !.consumed = IF s.work THEN @ + 1 ELSE @]

Switch == /\ Mode = "pixels" /\ s.generation < 2 /\ s.phase # "retired"
          /\ \E p, b \in 1..2:
             s' = [s EXCEPT !.pane = p, !.binding = b, !.generation = @ + 1,
                   !.phase = "connecting", !.feedback = FALSE,
                   !.display = [pane |-> 0, binding |-> 0, fresh |-> FALSE]]
DrawRetained == /\ Mode = "pixels" /\ s.phase = "connecting"
                /\ s.cache[s.pane] = s.binding
                /\ s' = [s EXCEPT !.display = [pane |-> s.pane, binding |-> s.binding, fresh |-> FALSE]]
ReceiveFresh == /\ Mode = "pixels" /\ s.phase = "connecting"
                /\ s' = [s EXCEPT !.phase = "frame"]
DrawFresh == /\ Mode = "pixels" /\ s.phase = "frame"
             /\ s' = [s EXCEPT !.phase = "drawn", !.feedback = TRUE,
                       !.display = [pane |-> s.pane, binding |-> s.binding, fresh |-> TRUE]]
AcceptFeedback == /\ Mode = "pixels" /\ s.phase = "drawn" /\ s.feedback
                  /\ s' = [s EXCEPT !.phase = "ready"]
Remember == /\ Mode = "pixels" /\ s.phase = "ready"
            /\ s' = [s EXCEPT !.cache[s.pane] = s.binding]
Evict == /\ Mode = "pixels"
         /\ \E p \in 1..2: s.cache[p] # 0 /\ s' = [s EXCEPT !.cache[p] = 0]
Block == /\ Mode = "pixels" /\ s.phase \notin {"blocked", "retired"}
         /\ s' = [s EXCEPT !.phase = "blocked", !.cache[s.pane] = 0, !.feedback = FALSE,
                   !.display = [pane |-> 0, binding |-> 0, fresh |-> FALSE]]
Retire == /\ Mode = "pixels" /\ s.phase # "retired"
          /\ s' = [s EXCEPT !.phase = "retired", !.cache = [p \in 1..2 |-> 0], !.feedback = FALSE,
                    !.display = [pane |-> 0, binding |-> 0, fresh |-> FALSE]]
Input == /\ Mode = "pixels"
         /\ s' = [s EXCEPT !.input = [accepted |-> s.phase = "ready", fresh |-> s.display.fresh]]

Next == Publish \/ Inspect \/ Park \/ Wake \/ Service \/ Switch \/ DrawRetained
        \/ ReceiveFresh \/ DrawFresh \/ AcceptFeedback \/ Remember \/ Evict \/ Block \/ Retire \/ Input
Spec == Init /\ [][Next]_vars /\ WF_vars(Inspect) /\ WF_vars(Park)
        /\ WF_vars(Wake) /\ WF_vars(Service)

TypeOK == /\ s.ui \in {"inspect", "wait", "parked", "service"}
          /\ s.pending \in BOOLEAN /\ s.token \in BOOLEAN /\ s.work \in BOOLEAN
          /\ s.consumed \in 0..2 /\ s.published \in 0..2
          /\ s.pane \in 1..2 /\ s.binding \in 1..2 /\ s.generation \in 0..2
          /\ s.cache \in [1..2 -> 0..2]
          /\ s.phase \in {"connecting", "frame", "drawn", "ready", "blocked", "retired"}
NoLostWake == s.work /\ s.ui = "parked" => s.token
NoLostPayload == s.published - s.consumed = IF s.work THEN 1 ELSE 0
ExactPixels == s.display.pane # 0 => (s.display.pane = s.pane /\ s.display.binding = s.binding)
RetainedCannotAcknowledge == s.display.pane # 0 /\ ~s.display.fresh => ~s.feedback
InputRequiresFresh == s.input.accepted => s.input.fresh
RetiredReleasesPixels == s.phase = "retired" => (s.cache = [p \in 1..2 |-> 0] /\ ~s.feedback)
PublishedEventuallyConsumed == s.work ~> ~s.work
==================================================================
