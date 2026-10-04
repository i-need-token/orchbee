----------------------- MODULE NativeWorkspaceClose -----------------------
EXTENDS Naturals, TLC
VARIABLE s
vars == <<s>>
Phases == {"Idle", "Prompt", "Queued", "Checked", "Submitted", "Known", "Publish", "Snapshot", "Refused", "Done", "Retired"}
Init == \E members \in {1, 2}: s = [phase |-> "Idle", opens |-> 0, token |-> 0, shown |-> 0, source |-> 0, captured |-> 0, confirmed |-> FALSE, fresh |-> FALSE, sends |-> 0, members |-> members, children |-> 0, server |-> "New", known |-> "None", success |-> FALSE, installed |-> FALSE, coverage |-> "Init"]

Open == /\ s.phase = "Idle" /\ s.opens < 2
        /\ s' = [s EXCEPT !.phase = "Prompt", !.opens = s.opens + 1, !.token = s.token + 1, !.shown = 0, !.captured = s.source, !.coverage = "Open"]

Draw == /\ s.phase = "Prompt"
        /\ s' = [s EXCEPT !.shown = s.token, !.coverage = "Draw"]

LateDraw == /\ s.phase = "Prompt" /\ s.opens = 2
            /\ s' = [s EXCEPT !.coverage = "LateDraw"]

RejectKey == /\ s.phase = "Prompt"
             /\ s' = [s EXCEPT !.coverage = "RejectKey"]

Cancel == /\ s.phase = "Prompt"
          /\ s' = [s EXCEPT !.phase = "Idle", !.shown = 0, !.coverage = "Cancel"]

RefuseGeometry == /\ s.phase = "Prompt"
                  /\ s' = [s EXCEPT !.phase = "Idle", !.shown = 0, !.coverage = "RefuseGeometry"]

SourceChange == /\ s.source = 0
                /\ s' = [s EXCEPT !.source = 1, !.coverage = "SourceChange"]

Confirm == /\ s.phase = "Prompt" /\ s.shown = s.token /\ s.captured = s.source
           /\ s' = [s EXCEPT !.phase = "Queued", !.confirmed = TRUE, !.coverage = "Confirm"]

Precheck == /\ s.phase = "Queued" /\ s.captured = s.source
            /\ s' = [s EXCEPT !.phase = "Checked", !.fresh = TRUE, !.coverage = "Precheck"]

PrecheckRefused == /\ s.phase = "Queued" /\ s.captured # s.source
                   /\ s' = [s EXCEPT !.phase = "Refused", !.coverage = "PrecheckRefused"]

Submit == /\ s.phase = "Checked" /\ s.sends = 0
          /\ s' = [s EXCEPT !.phase = "Submitted", !.sends = 1, !.server = "Pending", !.coverage = "Submit"]

DeliverAccepted == /\ s.phase = "Submitted" /\ s.server = "Pending"
                   /\ s' = [s EXCEPT !.phase = "Known", !.known = "Pending", !.coverage = "DeliverAccepted"]

ServerChildClosed == /\ s.server = "Pending" /\ s.children < s.members
                     /\ s' = [s EXCEPT !.children = s.children + 1, !.server = IF s.children + 1 = s.members THEN "Closed" ELSE "Pending", !.coverage = "ServerChildClosed"]

ServerFailed == /\ s.server = "Pending"
                /\ s' = [s EXCEPT !.server = "Failed", !.coverage = "ServerFailed"]

ReadPending == /\ s.phase = "Known" /\ s.server = "Pending"
               /\ s' = [s EXCEPT !.coverage = "ReadPending"]

ReadClosed == /\ s.phase \in {"Submitted", "Known"} /\ s.server = "Closed"
              /\ s' = [s EXCEPT !.phase = "Publish", !.known = "Closed", !.coverage = "ReadClosed"]

ReadFailed == /\ s.phase \in {"Submitted", "Known"} /\ s.server = "Failed"
              /\ s' = [s EXCEPT !.phase = "Done", !.known = "Failed", !.coverage = "ReadFailed"]

ObservationUnavailable == /\ s.phase = "Known"
                          /\ s' = [s EXCEPT !.phase = "Retired", !.coverage = "ObservationUnavailable"]

WorkerExit == /\ s.phase \in {"Queued", "Checked", "Submitted", "Known", "Publish", "Snapshot"}
              /\ s' = [s EXCEPT !.phase = "Retired", !.coverage = "WorkerExit"]

PublishReceipt == /\ s.phase = "Publish"
                  /\ s' = [s EXCEPT !.phase = "Snapshot", !.coverage = "PublishReceipt"]

InstallSnapshot == /\ s.phase = "Snapshot" /\ s.known = "Closed" /\ s.server = "Closed"
                   /\ s' = [s EXCEPT !.phase = "Done", !.installed = TRUE, !.success = TRUE, !.coverage = "InstallSnapshot"]

SnapshotUnavailable == /\ s.phase = "Snapshot"
                       /\ s' = [s EXCEPT !.phase = "Retired", !.coverage = "SnapshotUnavailable"]

Next == Open \/ Draw \/ LateDraw \/ RejectKey \/ Cancel \/ RefuseGeometry \/ SourceChange \/ Confirm \/ Precheck \/ PrecheckRefused \/ Submit \/ DeliverAccepted \/ ServerChildClosed \/ ServerFailed \/ ReadPending \/ ReadClosed \/ ReadFailed \/ ObservationUnavailable \/ WorkerExit \/ PublishReceipt \/ InstallSnapshot \/ SnapshotUnavailable
Spec == Init /\ [][Next]_vars
TypeOK == /\ s.phase \in Phases /\ s.opens \in 0..2 /\ s.token \in 0..2 /\ s.shown \in 0..2
          /\ s.source \in 0..1 /\ s.captured \in 0..1 /\ s.sends \in 0..1
          /\ s.members \in {1, 2} /\ s.children \in 0..s.members
          /\ s.server \in {"New", "Pending", "Failed", "Closed"}
          /\ s.known \in {"None", "Pending", "Failed", "Closed"}
          /\ s.confirmed \in BOOLEAN /\ s.fresh \in BOOLEAN
          /\ s.success \in BOOLEAN /\ s.installed \in BOOLEAN
AtMostOnce == s.sends <= 1
ConfirmationBeforeSend == s.sends > 0 => (s.confirmed /\ s.fresh)
NoFalseSuccess == s.success => (s.known = "Closed" /\ s.installed /\ s.server = "Closed")
SourceLast == s.server = "Closed" => s.children = s.members
ReceiptsPreserved == [][(s.known # "None" => s'.known # "None")]_vars
UnknownNeverReplays == [][(s.phase = "Retired" => s'.sends = s.sends)]_vars
=============================================================================
