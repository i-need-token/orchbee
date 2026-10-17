---------------------- MODULE NativeControlTakeover ----------------------
EXTENDS Naturals, FiniteSets, TLC
VARIABLE s
vars == <<s>>
Clients == {1, 2}
Phases == {"Idle", "Queued", "Submitted", "Reserved", "Detached", "Attached", "Granted", "Sized", "Paused", "Rearmed", "Ready", "Refused", "Retired"}
Init == s = [phase |-> "Idle", client |-> 0, token |-> 0, gestures |-> 0,
             sent |-> {}, server |-> 1, host |-> {1}, grantOwner |-> [i \in 1..4 |-> IF i = 1 THEN 1 ELSE 0],
             reported |-> [c \in Clients |-> IF c = 1 THEN 1 ELSE 0],
             frameReady |-> [c \in Clients |-> c = 1],
             receipts |-> {}, geometry |-> 1, resizeOwner |-> 1,
             lastInput |-> [supplied |-> 0, authority |-> 0, accepted |-> FALSE, drawn |-> FALSE], coverage |-> "Init"]

Explicit == /\ s.phase \in {"Idle", "Ready", "Refused"} /\ s.gestures < 3
            /\ \E c \in Clients: s' = [s EXCEPT !.phase = "Queued", !.client = c,
                !.token = s.gestures + 2, !.gestures = s.gestures + 1, !.coverage = "Explicit"]
Submit == /\ s.phase = "Queued"
          /\ s' = [s EXCEPT !.phase = "Submitted", !.sent = @ \cup {s.token}, !.coverage = "Submit"]
NotSubmitted == /\ s.phase = "Queued"
                /\ s' = [s EXCEPT !.phase = "Refused", !.coverage = "NotSubmitted"]
Reserve == /\ s.phase = "Submitted"
           /\ s' = [s EXCEPT !.phase = "Reserved", !.server = s.token,
                     !.grantOwner[s.token] = s.client, !.coverage = "Reserve"]
Detach == /\ s.phase = "Reserved"
          /\ s' = [s EXCEPT !.phase = "Detached", !.host = {}, !.coverage = "Detach"]
Attach == /\ s.phase = "Detached" /\ s.host = {}
          /\ s' = [s EXCEPT !.phase = "Attached", !.host = @ \cup {s.token}, !.coverage = "Attach"]
Refuse == /\ s.phase \in {"Submitted", "Reserved", "Detached"}
          /\ s' = [s EXCEPT !.phase = "Refused", !.host = {}, !.server = 0, !.coverage = "Refuse"]
ReceiveGrant == /\ s.phase = "Attached"
                /\ s' = [s EXCEPT !.phase = "Granted", !.receipts = @ \cup {s.token}, !.coverage = "ReceiveGrant"]
Resize == /\ s.phase = "Granted" /\ s.host = {s.token} /\ s.server = s.token
          /\ s' = [s EXCEPT !.phase = "Sized", !.geometry = s.client, !.resizeOwner = s.client, !.coverage = "Resize"]
PauseDisplay == /\ s.phase = "Sized"
                /\ s' = [s EXCEPT !.phase = "Paused", !.coverage = "PauseDisplay"]
ResumeDisplay == /\ s.phase = "Paused"
                 /\ s' = [s EXCEPT !.phase = "Rearmed", !.coverage = "ResumeDisplay"]
Deliver == /\ s.phase = "Rearmed"
           /\ s' = [s EXCEPT !.phase = "Ready", !.reported[s.client] = s.token, !.frameReady[s.client] = FALSE, !.coverage = "Deliver"]
DrawFeedback == /\ s.phase = "Ready" /\ s.reported[s.client] = s.server /\ s.host = {s.server}
                /\ s' = [s EXCEPT !.frameReady[s.client] = TRUE, !.coverage = "DrawFeedback"]
WorkerExit == /\ s.phase \in {"Queued", "Submitted", "Reserved", "Detached", "Attached", "Granted", "Sized", "Paused", "Rearmed"}
              /\ s' = [s EXCEPT !.phase = "Retired", !.coverage = "WorkerExit"]
StaleCleanup == /\ s.phase \in {"Attached", "Granted", "Sized", "Paused", "Rearmed", "Ready"}
                /\ s' = [s EXCEPT !.coverage = "StaleCleanup"]
ConcurrentRefusal == /\ s.phase \in {"Reserved", "Detached", "Attached", "Granted", "Sized", "Paused", "Rearmed"}
                     /\ s' = [s EXCEPT !.coverage = "ConcurrentRefusal"]
Interact == \E c \in Clients:
    LET token == s.reported[c]
        authority == IF s.phase \in {"Reserved", "Detached", "Attached"} THEN 0 ELSE s.server
        accepted == token # 0 /\ token = authority /\ token \in s.host /\ s.frameReady[c]
    IN /\ s' = [s EXCEPT !.lastInput = [supplied |-> token, authority |-> authority, accepted |-> accepted, drawn |-> s.frameReady[c]],
                 !.reported[c] = IF token = authority /\ token \in s.host THEN token ELSE 0,
                 !.coverage = IF accepted THEN "Input" ELSE "OldInputRefused"]
OrdinaryKey == s' = [s EXCEPT !.coverage = "OrdinaryKey"]
Next == Explicit \/ Submit \/ NotSubmitted \/ Reserve \/ Detach \/ Attach \/ Refuse \/ ReceiveGrant \/ Resize \/ PauseDisplay \/ ResumeDisplay \/ Deliver \/ DrawFeedback \/ WorkerExit \/ StaleCleanup \/ ConcurrentRefusal \/ Interact \/ OrdinaryKey
Spec == Init /\ [][Next]_vars
TypeOK == /\ s.phase \in Phases /\ s.client \in 0..2 /\ s.token \in 0..4 /\ s.gestures \in 0..3
          /\ s.sent \subseteq 2..4 /\ s.host \subseteq 1..4 /\ s.receipts \subseteq s.sent
          /\ s.server \in 0..4 /\ s.reported \in [Clients -> 0..4]
          /\ s.frameReady \in [Clients -> BOOLEAN]
          /\ s.grantOwner \in [1..4 -> 0..2] /\ s.geometry \in Clients /\ s.resizeOwner \in Clients
ExclusiveAuthority == Cardinality(s.host) <= 1
ExplicitOnly == Cardinality(s.sent) <= s.gestures
OldInputFenced == s.lastInput.accepted => (s.lastInput.supplied # 0 /\ s.lastInput.supplied = s.lastInput.authority)
FrameBeforeInput == s.lastInput.accepted => s.lastInput.drawn
ControllerGeometry == s.phase = "Ready" => (s.geometry = s.client /\ s.resizeOwner = s.grantOwner[s.token])
ReceiptsPreserved == [][(s.receipts \subseteq s'.receipts)]_vars
NoReplay == [][(s.phase = "Retired" => s'.sent = s.sent)]_vars
=============================================================================
