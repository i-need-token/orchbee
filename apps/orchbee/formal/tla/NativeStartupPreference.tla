------------------------ MODULE NativeStartupPreference ------------------------
EXTENDS Naturals, FiniteSets, TLC

Clients == {1, 2}
Members == {1, 2}
None == 0
VARIABLES focus, epoch, receipt, pending, saved, published, alive, coverage
vars == <<focus, epoch, receipt, pending, saved, published, alive, coverage>>

Init == /\ focus = [c \in Clients |-> 1]
        /\ epoch = [c \in Clients |-> 0]
        /\ receipt = [c \in Clients |-> <<None, 0>>]
        /\ pending = [c \in Clients |-> None]
        /\ saved = None
        /\ published = {}
        /\ alive = Members
        /\ coverage = "Init"

Select(c, m) == /\ m \in alive /\ epoch[c] < 2 /\ focus[c] # m
                /\ focus' = [focus EXCEPT ![c] = m]
                /\ epoch' = [epoch EXCEPT ![c] = @ + 1]
                /\ coverage' = "Select"
                /\ UNCHANGED <<receipt, pending, saved, published, alive>>
Stage(c) == /\ receipt' = [receipt EXCEPT ![c] = <<focus[c], epoch[c]>>]
            /\ coverage' = "Stage"
            /\ UNCHANGED <<focus, epoch, pending, saved, published, alive>>
PublishDraw(c) == /\ receipt[c] = <<focus[c], epoch[c]>> /\ focus[c] \in alive
                  /\ pending' = [pending EXCEPT ![c] = focus[c]]
                  /\ published' = published \cup {focus[c]}
                  /\ coverage' = "PublishDraw"
                  /\ UNCHANGED <<focus, epoch, receipt, saved, alive>>
RejectStale(c) == /\ receipt[c] # <<focus[c], epoch[c]>>
                  /\ coverage' = "RejectStale"
                  /\ UNCHANGED <<focus, epoch, receipt, pending, saved, published, alive>>
RejectCovered == /\ coverage' = "RejectCovered"
                 /\ UNCHANGED <<focus, epoch, receipt, pending, saved, published, alive>>
RejectFailed == /\ coverage' = "RejectFailed"
                /\ UNCHANGED <<focus, epoch, receipt, pending, saved, published, alive>>
Write(c) == /\ pending[c] # None
            /\ saved' = pending[c]
            /\ pending' = [pending EXCEPT ![c] = None]
            /\ coverage' = "Write"
            /\ UNCHANGED <<focus, epoch, receipt, published, alive>>
WriteFailed(c) == /\ pending[c] # None
                  /\ pending' = [pending EXCEPT ![c] = None]
                  /\ coverage' = "WriteFailed"
                  /\ UNCHANGED <<focus, epoch, receipt, saved, published, alive>>
Remove == /\ alive = Members /\ alive' = {1}
          /\ coverage' = "Remove"
          /\ UNCHANGED <<focus, epoch, receipt, pending, saved, published>>
Restart(c) == /\ saved # None /\ epoch[c] < 2
              /\ focus' = [focus EXCEPT ![c] = IF saved \in alive THEN saved ELSE 1]
              /\ epoch' = [epoch EXCEPT ![c] = @ + 1]
              /\ coverage' = IF saved \in alive THEN "Restore" ELSE "Fallback"
              /\ UNCHANGED <<receipt, pending, saved, published, alive>>

Next == \/ \E c \in Clients, m \in Members: Select(c, m)
        \/ \E c \in Clients: Stage(c) \/ PublishDraw(c) \/ RejectStale(c)
                              \/ Write(c) \/ WriteFailed(c) \/ Restart(c)
        \/ RejectCovered \/ RejectFailed \/ Remove
Spec == Init /\ [][Next]_vars
TypeOK == /\ focus \in [Clients -> Members] /\ epoch \in [Clients -> 0..2]
          /\ pending \in [Clients -> Members \cup {None}]
          /\ saved \in Members \cup {None} /\ published \subseteq Members
SavedWasDisplayed == saved = None \/ saved \in published
WritesPreserveLiveFocus == [][saved' # saved => focus' = focus]_vars
OneClientChangesAtATime == [][Cardinality({c \in Clients: focus'[c] # focus[c]}) <= 1]_vars
=============================================================================
