---------------------- MODULE KeyboardResize ----------------------
EXTENDS Naturals, FiniteSets, TLC
VARIABLE s
vars == <<s>>
Owners == {"local", "remote"}
Init == s = [mode |-> FALSE, issued |-> 0, worker |-> 0, workerOwner |-> "none",
             submitted |-> FALSE, result |-> "none", pending |-> 0, owner |-> "none",
             reply |-> "none", published |-> FALSE, drawn |-> FALSE, interrupted |-> FALSE,
             replaced |-> FALSE, successes |-> {}, committedRequests |-> {}, drawnRequests |-> {}, remoteReceipts |-> {}, localRequests |-> {},
             feedback |-> "idle", forwarded |-> FALSE, coverage |-> "Init"]
EnterMode == s' = [s EXCEPT !.mode = TRUE, !.coverage = "EnterMode"]
ExitMode == s' = [s EXCEPT !.mode = FALSE, !.coverage = "ExitMode"]
ModeKey == /\ s.mode /\ s' = [s EXCEPT !.forwarded = FALSE, !.coverage = "ModeKey"]
OrdinaryKey == /\ ~s.mode /\ s' = [s EXCEPT !.forwarded = TRUE, !.coverage = "OrdinaryKey"]
StartLocal == /\ s.issued < 2 /\ s.worker = 0 /\ s.pending = 0
                /\ s' = [s EXCEPT !.issued = @ + 1, !.worker = s.issued + 1, !.workerOwner = "local",
                    !.submitted = FALSE, !.result = "none", !.pending = s.issued + 1, !.owner = "local",
                    !.reply = "none", !.published = FALSE, !.drawn = FALSE,
                    !.interrupted = FALSE, !.replaced = FALSE, !.feedback = "pending",
                    !.localRequests = IF "local" = "local" THEN @ \cup {s.issued + 1} ELSE @,
                    !.coverage = IF "local" = "local" THEN "StartLocal" ELSE "StartRemote"]
StartRemote == /\ s.issued < 2 /\ s.worker = 0 /\ s.pending = 0
                /\ s' = [s EXCEPT !.issued = @ + 1, !.worker = s.issued + 1, !.workerOwner = "remote",
                    !.submitted = FALSE, !.result = "none", !.pending = s.issued + 1, !.owner = "remote",
                    !.reply = "none", !.published = FALSE, !.drawn = FALSE,
                    !.interrupted = FALSE, !.replaced = FALSE, !.feedback = "pending",
                    !.localRequests = IF "remote" = "local" THEN @ \cup {s.issued + 1} ELSE @,
                    !.coverage = IF "remote" = "local" THEN "StartLocal" ELSE "StartRemote"]
BusyKey == /\ (s.worker # 0 \/ s.pending # 0)
           /\ s' = [s EXCEPT !.feedback = "busy", !.coverage = "BusyKey"]
Submit == /\ s.worker # 0 /\ ~s.submitted /\ s.result = "none"
          /\ s' = [s EXCEPT !.submitted = TRUE, !.coverage = "Submit"]
Resolve == /\ s.worker # 0 /\ s.submitted /\ s.result = "none"
           /\ \E result \in {"committed", "rejected", "unknown"}:
               s' = [s EXCEPT !.result = result, !.committedRequests = IF result = "committed" THEN @ \cup {s.worker} ELSE @, !.coverage = "Resolve"]
NotSubmitted == /\ s.worker # 0 /\ ~s.submitted /\ s.result = "none"
                /\ s' = [s EXCEPT !.result = "rejected", !.coverage = "NotSubmitted"]
Deliver == /\ s.worker # 0 /\ s.result # "none"
           /\ s' = [s EXCEPT !.reply = IF s.pending = s.worker /\ s.owner = s.workerOwner THEN s.result ELSE @,
                      !.worker = 0, !.workerOwner = "none", !.submitted = FALSE, !.result = "none",
                      !.coverage = "Deliver"]
Publish == /\ s.pending # 0 /\ s.reply = "committed"
           /\ s' = [s EXCEPT !.published = TRUE, !.coverage = "Publish"]
Draw == /\ s.pending # 0 /\ s.reply = "committed" /\ s.published /\ ~s.interrupted /\ ~s.replaced
        /\ s' = [s EXCEPT !.drawn = TRUE, !.drawnRequests = @ \cup {s.pending}, !.coverage = "Draw"]
Finish == /\ s.pending # 0 /\ s.reply = "committed" /\ s.drawn /\ ~s.interrupted /\ ~s.replaced
          /\ s' = [s EXCEPT !.successes = @ \cup {s.pending},
                     !.remoteReceipts = IF s.owner = "remote" THEN @ \cup {s.pending} ELSE @,
                     !.pending = 0, !.owner = "none", !.feedback = "success", !.coverage = "Finish"]
Failure == /\ s.pending # 0 /\ (s.reply \in {"rejected", "unknown"} \/ s.interrupted \/ s.replaced)
           /\ s' = [s EXCEPT !.pending = 0, !.owner = "none", !.feedback = "failure", !.coverage = "Failure"]
Interrupt == /\ s.pending # 0 /\ s' = [s EXCEPT !.interrupted = TRUE, !.coverage = "Interrupt"]
Replace == /\ s.pending # 0 /\ s' = [s EXCEPT !.replaced = TRUE, !.drawn = FALSE, !.coverage = "Replace"]
LateReply == /\ s.issued = 2 /\ s' = [s EXCEPT !.coverage = "LateReply"]
Next == EnterMode \/ ExitMode \/ ModeKey \/ OrdinaryKey \/ StartLocal \/ StartRemote \/ BusyKey \/ Submit
        \/ Resolve \/ NotSubmitted \/ Deliver \/ Publish \/ Draw \/ Finish \/ Failure \/ Interrupt \/ Replace \/ LateReply
Spec == Init /\ [][Next]_vars /\ WF_vars(Resolve)
TypeOK == /\ s.issued \in 0..2 /\ s.worker \in 0..s.issued /\ s.pending \in 0..s.issued
          /\ s.owner \in Owners \cup {"none"} /\ s.workerOwner \in Owners \cup {"none"}
          /\ s.successes \subseteq 1..s.issued /\ s.localRequests \subseteq 1..s.issued
NoBacklog == /\ (s.worker # 0 /\ s.pending # 0 => s.worker = s.pending)
             /\ (s.pending = 0 <=> s.owner = "none") /\ (s.worker = 0 <=> s.workerOwner = "none")
CompletionOwner == s.remoteReceipts \cap s.localRequests = {}
LocalDraw == s.successes \subseteq (s.committedRequests \cap s.drawnRequests)
ModeOwnsInput == s.coverage = "ModeKey" => ~s.forwarded
NoQueuedKeys == [][(s.coverage' = "BusyKey" => s'.issued = s.issued)]_vars
SuccessFenced == [][(s'.successes # s.successes => s.reply = "committed" /\ s.published /\ s.drawn /\ ~s.interrupted /\ ~s.replaced)]_vars
SubmittedResolves == (s.worker # 0 /\ s.submitted /\ s.result = "none") ~> (s.result # "none" \/ s.worker = 0)
=============================================================================
