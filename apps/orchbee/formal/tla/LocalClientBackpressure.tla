---------------------- MODULE LocalClientBackpressure ----------------------
EXTENDS Naturals, TLC

VARIABLES phase, attempts, sequence, effects, now, action
vars == <<phase, attempts, sequence, effects, now, action>>
Pending == phase \in {"ready", "submitted", "busy"}

Init == /\ phase = "ready" /\ attempts = 0 /\ sequence = 0
        /\ effects = 0 /\ now = 0 /\ action = "init"

Submit == /\ phase \in {"ready", "busy"} /\ now < 3 /\ attempts < 3
          /\ phase' = "submitted" /\ attempts' = attempts + 1
          /\ sequence' = sequence + 1 /\ action' = "submit"
          /\ UNCHANGED <<effects, now>>
Busy == /\ phase = "submitted" /\ now < 3
        /\ phase' = "busy" /\ action' = "busy"
        /\ UNCHANGED <<attempts, sequence, effects, now>>
Accepted == /\ phase = "submitted" /\ now < 3
            /\ phase' = "accepted" /\ effects' = effects + 1
            /\ action' = "accepted"
            /\ UNCHANGED <<attempts, sequence, now>>
Unknown == /\ phase = "submitted"
           /\ phase' = "unknown" /\ effects' \in {effects, effects + 1}
           /\ action' = "unknown" /\ UNCHANGED <<attempts, sequence, now>>
Rejected == /\ phase = "submitted" /\ phase' = "failed"
            /\ action' = "rejected"
            /\ UNCHANGED <<attempts, sequence, effects, now>>
Expire == /\ Pending /\ phase' = "expired" /\ now' = 3
          /\ action' = "expire" /\ UNCHANGED <<attempts, sequence, effects>>
Exhausted == /\ phase = "busy" /\ attempts = 3 /\ phase' = "failed"
             /\ action' = "exhausted" /\ UNCHANGED <<attempts, sequence, effects, now>>
Cancel == /\ Pending /\ phase' = "cancelled" /\ action' = "cancel"
          /\ UNCHANGED <<attempts, sequence, effects, now>>
Next == Submit \/ Busy \/ Accepted \/ Unknown \/ Rejected \/ Expire \/ Exhausted \/ Cancel
Spec == Init /\ [][Next]_vars /\ WF_vars(Expire)
TypeOK == /\ phase \in {"ready", "submitted", "busy", "accepted", "unknown", "failed", "expired", "cancelled"}
          /\ attempts \in 0..3 /\ sequence \in 0..3 /\ effects \in 0..1 /\ now \in 0..3
BusyIsUnchanged == phase = "busy" => effects = 0
FreshIdentity == sequence = attempts
AcceptedBeforeDeadline == phase = "accepted" => now < 3
RetryOnlyAfterBusy == [][attempts' > attempts /\ attempts > 0 => phase = "busy"]_vars
NoReplayAfterPossibleEffect == [][effects > 0 => attempts' = attempts]_vars
EventuallySettled == <>~Pending
=============================================================================
