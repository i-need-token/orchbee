------------------------ MODULE RelayDeliveryIsolation ------------------------
EXTENDS Naturals, Sequences, FiniteSets

\* Two independently draining viewers, two frame/history records, a one-slot
\* queue and one unsettled flight per lane. An overflowing viewer closes
\* without dropping peer work. Record identities abstract immutable opaque
\* payloads. Authentication/grants belong to ConnectionViewerOwnership; byte
\* arithmetic, codecs and exact production capacities have implementation tests.
Clients == {1, 2}
VARIABLE s
vars == <<s>>
Init == s = [live |-> [c \in Clients |-> TRUE], frameSource |-> 0, historySource |-> 0,
  frames |-> [c \in Clients |-> <<>>], history |-> [c \in Clients |-> <<>>],
  frameFlight |-> [c \in Clients |-> 0], historyFlight |-> [c \in Clients |-> 0],
  frameSeen |-> [c \in Clients |-> 0], historySeen |-> [c \in Clients |-> 0],
  worker |-> 0]
Frame ==
  /\ s.frameSource < 2
  /\ LET overflow == {c \in Clients: s.live[c] /\ Len(s.frames[c]) = 1}
     IN s' = [s EXCEPT !.frameSource = @ + 1,
       !.live = [c \in Clients |-> s.live[c] /\ c \notin overflow],
       !.frames = [c \in Clients |-> IF c \in overflow THEN <<>> ELSE IF s.live[c]
         THEN Append(s.frames[c], s.frameSource + 1) ELSE s.frames[c]],
       !.history = [c \in Clients |-> IF c \in overflow THEN <<>> ELSE s.history[c]],
       !.frameFlight = [c \in Clients |-> IF c \in overflow THEN 0 ELSE s.frameFlight[c]],
       !.historyFlight = [c \in Clients |-> IF c \in overflow THEN 0 ELSE s.historyFlight[c]]]
History ==
  /\ s.historySource < 2
  /\ LET overflow == {c \in Clients: s.live[c] /\ Len(s.history[c]) = 1}
     IN s' = [s EXCEPT !.historySource = @ + 1,
       !.live = [c \in Clients |-> s.live[c] /\ c \notin overflow],
       !.history = [c \in Clients |-> IF c \in overflow THEN <<>> ELSE IF s.live[c]
         THEN Append(s.history[c], s.historySource + 1) ELSE s.history[c]],
       !.frames = [c \in Clients |-> IF c \in overflow THEN <<>> ELSE s.frames[c]],
       !.frameFlight = [c \in Clients |-> IF c \in overflow THEN 0 ELSE s.frameFlight[c]],
       !.historyFlight = [c \in Clients |-> IF c \in overflow THEN 0 ELSE s.historyFlight[c]]]
DeliverFrame(c) ==
  /\ s.live[c] /\ s.frameFlight[c] = 0 /\ s.frames[c] # <<>>
  /\ s' = [s EXCEPT !.frameFlight[c] = Head(s.frames[c]), !.frames[c] = Tail(@)]
DeliverHistory(c) ==
  /\ s.live[c] /\ s.historyFlight[c] = 0 /\ s.history[c] # <<>>
  /\ s' = [s EXCEPT !.historyFlight[c] = Head(s.history[c]), !.history[c] = Tail(@)]
FrameObserved(c, identity) ==
  /\ s.live[c] /\ s.frameFlight[c] # 0 /\ identity = s.frameFlight[c]
  /\ s' = [s EXCEPT !.frameSeen[c] = identity, !.frameFlight[c] = 0]
HistoryObserved(c, identity) ==
  /\ s.live[c] /\ s.historyFlight[c] # 0 /\ identity = s.historyFlight[c]
  /\ s' = [s EXCEPT !.historySeen[c] = identity, !.historyFlight[c] = 0]
RejectFeedback(c, identity) ==
  /\ s.live[c] /\ identity # s.frameFlight[c] /\ identity # s.historyFlight[c]
  /\ UNCHANGED vars
Close(c) ==
  /\ s.live[c]
  /\ s' = [s EXCEPT !.live[c] = FALSE, !.frames[c] = <<>>, !.history[c] = <<>>,
       !.frameFlight[c] = 0, !.historyFlight[c] = 0]
WorkerProgress ==
  /\ s.worker = 0
  /\ s' = [s EXCEPT !.worker = 1]
Next ==
  \/ Frame \/ History \/ WorkerProgress
  \/ \E c \in Clients: DeliverFrame(c) \/ DeliverHistory(c) \/ Close(c)
  \/ \E c \in Clients, identity \in 0..2:
       FrameObserved(c, identity) \/ HistoryObserved(c, identity) \/ RejectFeedback(c, identity)
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.live \in [Clients -> BOOLEAN]
  /\ s.frameSource \in 0..2 /\ s.historySource \in 0..2 /\ s.worker \in 0..1
  /\ s.frameFlight \in [Clients -> 0..2] /\ s.historyFlight \in [Clients -> 0..2]
BoundedQueues == \A c \in Clients: Len(s.frames[c]) <= 1 /\ Len(s.history[c]) <= 1
ClosedViewerHasNoWork == \A c \in Clients: ~s.live[c] =>
  s.frames[c] = <<>> /\ s.history[c] = <<>> /\ s.frameFlight[c] = 0 /\ s.historyFlight[c] = 0
FlightsFollowSource == \A c \in Clients:
  s.frameFlight[c] <= s.frameSource /\ s.historyFlight[c] <= s.historySource
FeedbackFollowsDelivery == \A c \in Clients:
  s.frameSeen[c] <= s.frameSource /\ s.historySeen[c] <= s.historySource
WorkerNeverWaitsForViewer == s.worker = 0 => ENABLED WorkerProgress
HistoryPreservesFrameFlight ==
    [][\A c \in Clients, identity \in 0..2:
      DeliverHistory(c) \/ HistoryObserved(c, identity) =>
      UNCHANGED <<s.frames, s.frameFlight, s.frameSeen>>]_vars
ViewerOperationsAreLocal ==
  [][\A c \in Clients: Close(c) \/ DeliverFrame(c) \/ DeliverHistory(c) =>
    \A other \in Clients \ {c}: UNCHANGED
      <<s.live[other], s.frames[other], s.history[other],
        s.frameFlight[other], s.historyFlight[other]>>]_vars
FeedbackNeverRegresses ==
  [][\A c \in Clients: s'.frameSeen[c] >= s.frameSeen[c] /\ s'.historySeen[c] >= s.historySeen[c]]_vars
FeedbackUsesExactFlight ==
  [][\A c \in Clients:
    /\ (s'.frameSeen[c] # s.frameSeen[c] => s'.frameSeen[c] = s.frameFlight[c] /\ s.frameFlight[c] # 0)
    /\ (s'.historySeen[c] # s.historySeen[c] => s'.historySeen[c] = s.historyFlight[c] /\ s.historyFlight[c] # 0)]_vars
OverflowIsViewerLocal ==
  [][\A c \in Clients:
    /\ (Frame /\ s.live[c] /\ Len(s.frames[c]) < 1 => s'.live[c] /\ UNCHANGED <<s.history[c], s.frameFlight[c], s.historyFlight[c]>>)
    /\ (History /\ s.live[c] /\ Len(s.history[c]) < 1 => s'.live[c] /\ UNCHANGED <<s.frames[c], s.frameFlight[c], s.historyFlight[c]>>)]_vars
=============================================================================
