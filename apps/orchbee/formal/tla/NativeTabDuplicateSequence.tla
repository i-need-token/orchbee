------------------------ MODULE NativeTabDuplicateSequence ------------------------
EXTENDS Naturals, Sequences, TLC

\* Two initially captured sources. A member uses the existing CreateBarrier;
\* this model checks the boundary between consecutive members, not its internals.
VARIABLES stage, sent, committed, started, installed, revision, accepted,
          peer, marks, cancelled, oldPane, pane, witness, lease, event,
          remaining, stopped, previousCommitted, previousSent
vars == <<stage, sent, committed, started, installed, revision, accepted,
          peer, marks, cancelled, oldPane, pane, witness, lease, event,
          remaining, stopped, previousCommitted, previousSent>>
Init == /\ stage = "waiting" /\ sent = 0 /\ committed = 0 /\ started = 0
        /\ installed = 0 /\ revision = 0 /\ accepted = 0 /\ peer = FALSE
        /\ marks = TRUE /\ cancelled = FALSE /\ oldPane = 0 /\ pane = 0
        /\ witness = FALSE /\ lease = FALSE /\ event = FALSE
        /\ remaining = 4 /\ stopped = FALSE /\ previousCommitted = 0 /\ previousSent = 0

Start == /\ stage = "waiting" /\ ~stopped /\ sent < 2 /\ remaining > 1
         /\ ~lease /\ ~event /\ ~cancelled /\ marks /\ pane = oldPane
         /\ revision = accepted /\ ~peer
         /\ sent' = sent + 1 /\ stage' = "running" /\ lease' = TRUE
         /\ previousSent' = sent /\ previousCommitted' = committed
         /\ UNCHANGED <<committed, started, installed, revision, accepted, peer, marks,
                        cancelled, oldPane, pane, witness, event, remaining, stopped>>
Receipt == /\ stage = "running"
           /\ committed' = committed + 1 /\ revision' = revision + 1
           /\ stage' = "receipt" /\ event' = TRUE /\ started' \in {started, started + 1}
           /\ UNCHANGED <<sent, installed, accepted, peer, marks, cancelled, oldPane,
                          pane, witness, lease, remaining, stopped, previousCommitted, previousSent>>
Install == /\ stage = "receipt"
           /\ witness' = (~stopped /\ ~cancelled /\ marks /\ pane = oldPane /\ ~peer
                         /\ revision = accepted + 1 /\ started = committed /\ remaining > 1)
           /\ installed' = installed + 1 /\ pane' = pane + 1
           /\ stage' = "attach" /\ event' = FALSE
           /\ UNCHANGED <<sent, committed, started, revision, accepted, peer, marks, cancelled,
                          oldPane, lease, remaining, stopped, previousCommitted, previousSent>>
Ready == /\ stage = "attach" /\ witness /\ ~cancelled /\ marks /\ ~peer
         /\ revision = accepted + 1 /\ pane = oldPane + 1
         /\ stage' = "waiting" /\ accepted' = revision /\ oldPane' = pane /\ witness' = FALSE
         /\ UNCHANGED <<sent, committed, started, installed, revision, peer, marks, cancelled,
                        pane, lease, event, remaining, stopped, previousCommitted, previousSent>>
FinishLease == /\ lease /\ stage # "running" /\ ~event /\ lease' = FALSE
               /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted,
                              peer, marks, cancelled, oldPane, pane, witness, event, remaining,
                              stopped, previousCommitted, previousSent>>
Poll == /\ stage = "waiting" /\ ~lease /\ ~event /\ event' = TRUE
        /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted, peer,
                       marks, cancelled, oldPane, pane, witness, lease, remaining, stopped,
                       previousCommitted, previousSent>>
SettlePoll == /\ event /\ stage = "waiting" /\ event' = FALSE
              /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted,
                             peer, marks, cancelled, oldPane, pane, witness, lease, remaining,
                             stopped, previousCommitted, previousSent>>
LocalDrift == /\ ~cancelled /\ cancelled' = TRUE /\ marks' = FALSE
              /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted, peer,
                             oldPane, pane, witness, lease, event, remaining, stopped,
                             previousCommitted, previousSent>>
AwayBack == /\ cancelled /\ ~marks /\ marks' = TRUE
            /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted, peer,
                           cancelled, oldPane, pane, witness, lease, event, remaining, stopped,
                           previousCommitted, previousSent>>
PeerDrift == /\ ~peer /\ peer' = TRUE
             /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted, marks,
                            cancelled, oldPane, pane, witness, lease, event, remaining, stopped,
                            previousCommitted, previousSent>>
Time == /\ remaining > 0 /\ remaining' = remaining - 1
        /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted, peer, marks,
                       cancelled, oldPane, pane, witness, lease, event, stopped,
                       previousCommitted, previousSent>>
Halt == /\ ~stopped
        /\ (cancelled \/ peer \/ remaining <= 1 \/ (stage = "receipt" /\ started < committed))
        /\ stopped' = TRUE
        /\ UNCHANGED <<stage, sent, committed, started, installed, revision, accepted, peer, marks,
                       cancelled, oldPane, pane, witness, lease, event, remaining,
                       previousCommitted, previousSent>>
Failure == /\ stage = "running" /\ stage' = "failed" /\ stopped' = TRUE
           /\ UNCHANGED <<sent, committed, started, installed, revision, accepted, peer, marks,
                          cancelled, oldPane, pane, witness, lease, event, remaining,
                          previousCommitted, previousSent>>
Next == Start \/ Receipt \/ Install \/ Ready \/ FinishLease \/ Poll \/ SettlePoll
        \/ LocalDrift \/ AwayBack \/ PeerDrift \/ Time \/ Halt \/ Failure
Spec == Init /\ [][Next]_vars /\ WF_vars(Time) /\ WF_vars(Halt)
Counts == /\ sent \in 0..2 /\ started <= committed /\ installed <= committed /\ committed <= sent
StartNeedsSettledPredecessor == stage = "running" => previousCommitted = previousSent
NoRestart == [][stopped => sent' = sent]_vars
RetainCommit == [][committed' >= committed]_vars
BoundedTermination == <> (stopped \/ sent = 2)
===================================================================================
