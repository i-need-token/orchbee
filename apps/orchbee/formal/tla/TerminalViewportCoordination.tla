-------------------- MODULE TerminalViewportCoordination --------------------
EXTENDS Naturals, Sequences

\* One pane, two entry/read attempts, two anonymous input commands and a
\* two-slot FIFO. The abstraction owns command/outcome serialization and
\* incarnation fencing. Rows, geometry and navigation arithmetic are tested
\* against the Rust viewport directly. Recovery retains already-admitted work.
None == [kind |-> "none", sequence |-> 0, epoch |-> 0]
VARIABLE s
vars == <<s>>
Command(kind, sequence) == [kind |-> kind, sequence |-> sequence, epoch |-> s.epoch]
Init == s = [mode |-> "live", epoch |-> 1, entries |-> 0, inputs |-> 0,
  issued |-> 0, flight |-> 0, pin |-> 0, window |-> None,
  stage |-> None, queue |-> <<>>, outcome |-> None]
Enter ==
  /\ s.mode = "live" /\ s.entries < 2 /\ s.stage = None
  /\ s' = [s EXCEPT !.mode = "entering", !.entries = @ + 1,
       !.stage = Command("enter", 0)]
Input ==
  /\ s.mode \in {"live", "viewing"} /\ s.stage = None
  /\ s.inputs < 2 /\ Len(s.queue) < 2
  /\ s' = [s EXCEPT !.inputs = @ + 1,
       !.queue = Append(@, Command("input", 0))]
Read ==
  /\ s.mode = "viewing" /\ s.flight = 0 /\ s.stage = None
  /\ s.issued < 2
  /\ s' = [s EXCEPT !.issued = @ + 1, !.flight = s.issued + 1,
       !.stage = Command("read", s.issued + 1)]
Recover ==
  /\ s.mode \in {"entering", "viewing"}
  /\ s' = [s EXCEPT !.mode = "recovering", !.stage = Command("recover", 0),
       !.window = None]
Admit ==
  /\ s.stage # None /\ Len(s.queue) < 2
  /\ s' = [s EXCEPT !.queue = Append(@, s.stage), !.stage = None]
Execute ==
  /\ s.queue # <<>> /\ s.outcome = None
  /\ LET cmd == Head(s.queue)
         current == cmd.epoch = s.epoch /\ s.mode # "lost"
         result == CASE ~current \/ cmd.kind = "input" -> None
           [] cmd.kind = "enter" -> [cmd EXCEPT !.kind = "entered"]
           [] cmd.kind = "recover" -> [cmd EXCEPT !.kind = "recovered"]
           [] cmd.kind = "read" /\ s.pin = 0 -> [cmd EXCEPT !.kind = "window"]
           [] cmd.kind = "ack" /\ s.pin = cmd.sequence -> [cmd EXCEPT !.kind = "acked"]
           [] OTHER -> [cmd EXCEPT !.kind = "rejected"]
     IN s' = [s EXCEPT !.queue = Tail(@), !.outcome = result,
          !.pin = CASE ~current -> @
            [] result.kind = "window" -> cmd.sequence
            [] result.kind \in {"acked", "recovered"} -> 0
            [] OTHER -> @]
Consume ==
  /\ s.outcome # None
  /\ LET out == s.outcome
         current == out.epoch = s.epoch /\ s.mode # "lost"
         install == current /\ s.mode = "viewing" /\
           out.kind = "window" /\ out.sequence = s.flight /\ out.sequence # 0
         fault == current /\ s.mode \in {"entering", "viewing"} /\
           (out.kind = "rejected" \/ (out.kind = "window" /\ out.sequence = 0))
         recovered == current /\ out.kind = "recovered"
     IN s' = [s EXCEPT !.outcome = None,
          !.mode = CASE recovered -> "live"
            [] fault -> "recovering"
            [] current /\ out.kind = "entered" /\ s.mode = "entering" -> "viewing"
            [] OTHER -> @,
          !.stage = CASE fault -> Command("recover", 0)
            [] install -> Command("ack", out.sequence)
            [] OTHER -> @,
          !.flight = IF recovered \/ (current /\ out.kind = "acked") THEN 0 ELSE @,
          !.window = IF fault \/ recovered THEN None ELSE IF install THEN out ELSE @]
CorruptWindow ==
  /\ s.outcome.kind = "window" /\ s.outcome.sequence # 0
  /\ s' = [s EXCEPT !.outcome.sequence = 0]
Detach ==
  /\ s.epoch = 1
  /\ s' = [s EXCEPT !.epoch = 2, !.mode = "lost", !.stage = None,
       !.flight = 0, !.pin = 0, !.window = None]
Next == Enter \/ Input \/ Read \/ Recover \/ Admit \/ Execute \/ Consume \/ CorruptWindow \/ Detach
Spec == Init /\ [][Next]_vars /\ WF_vars(Admit) /\ WF_vars(Execute) /\ WF_vars(Consume)

TypeOK ==
  /\ s.mode \in {"live", "entering", "viewing", "recovering", "lost"}
  /\ s.epoch \in 1..2 /\ s.entries \in 0..2 /\ s.inputs \in 0..2
  /\ s.issued \in 0..2 /\ s.flight \in 0..2 /\ s.pin \in 0..2
BoundedQueue == Len(s.queue) <= 2
HostPinHasClientFlight == s.pin # 0 => s.flight = s.pin
SurfaceIsCurrent == s.window # None =>
  s.mode = "viewing" /\ s.window.epoch = s.epoch
StagedReadHasOneFlight == s.stage.kind = "read" => s.stage.sequence = s.flight /\ s.pin = 0
OutcomeProductionOwned ==
  [][s.outcome = None /\ s'.outcome # None => Execute]_vars
RecoveryKeepsAdmittedCommands ==
  [][Recover => UNCHANGED s.queue]_vars
InputDoesNotOvertakeStaging ==
  [][Input => s.stage = None]_vars
EnteredEventuallySettles ==
  s.mode = "entering" ~> s.mode \in {"viewing", "live", "lost"}
RecoveryEventuallySettles ==
  s.mode = "recovering" ~> s.mode \in {"live", "lost"}
=============================================================================
