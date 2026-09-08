---------------- MODULE WindowsSemanticEndpointAccept ----------------
EXTENDS Naturals, TLC

\* Two successive peers and three ticks expose poll/completion, expiry,
\* publication and cancellation/reuse orderings. OS completion is fair;
\* peer arrival and byte delivery deliberately have no fairness assumption.
VARIABLES phase, operation, completed, now, deadline, generation,
          prefix, delivered, publishedAt, authenticated, poll
vars == <<phase, operation, completed, now, deadline, generation,
          prefix, delivered, publishedAt, authenticated, poll>>

Init == /\ phase = "idle" /\ operation = "none" /\ completed = FALSE
        /\ now = 0 /\ deadline = 0 /\ generation = 1 /\ prefix = FALSE
        /\ delivered = 0 /\ publishedAt = 0 /\ authenticated = FALSE /\ poll = FALSE
StartListen == /\ phase = "idle" /\ phase' = "listen" /\ operation' = "connect"
               /\ completed' = FALSE
               /\ UNCHANGED <<now, deadline, generation, prefix, delivered, publishedAt, authenticated, poll>>
CompleteOperation == /\ operation # "none" /\ ~completed /\ completed' = TRUE
                     /\ UNCHANGED <<phase, operation, now, deadline, generation, prefix, delivered, publishedAt, authenticated, poll>>
ObserveConnection == /\ phase = "listen" /\ completed
                     /\ phase' = "identity" /\ operation' = "read" /\ completed' = FALSE
                     /\ deadline' = now + 2
                     /\ UNCHANGED <<now, generation, prefix, delivered, publishedAt, authenticated, poll>>
PollExpires == /\ phase \in {"listen", "identity"}
               /\ poll' = ~poll
               /\ UNCHANGED <<phase, operation, completed, now, deadline, generation, prefix, delivered, publishedAt, authenticated>>
Tick == /\ now < 3 /\ now' = now + 1
        /\ UNCHANGED <<phase, operation, completed, deadline, generation, prefix, delivered, publishedAt, authenticated, poll>>
Authenticate == /\ phase = "identity" /\ completed /\ now < deadline
                /\ phase' = "authenticated" /\ operation' = "none" /\ completed' = FALSE
                /\ authenticated' = TRUE /\ prefix' = TRUE /\ publishedAt' = now
                /\ UNCHANGED <<now, deadline, generation, delivered, poll>>
PublishPrefix == /\ phase = "authenticated" /\ prefix /\ now < deadline
                 /\ prefix' = FALSE /\ delivered' = delivered + 1 /\ publishedAt' = now
                 /\ UNCHANGED <<phase, operation, completed, now, deadline, generation, authenticated, poll>>
StartBodyRead == /\ phase = "authenticated" /\ ~prefix /\ operation = "none" /\ delivered = 1 /\ now < deadline
                 /\ operation' = "body" /\ completed' = FALSE
                 /\ UNCHANGED <<phase, now, deadline, generation, prefix, delivered, publishedAt, authenticated, poll>>
PublishBody == /\ phase = "authenticated" /\ operation = "body" /\ completed /\ now < deadline
               /\ operation' = "none" /\ completed' = FALSE /\ delivered' = 2 /\ publishedAt' = now
               /\ UNCHANGED <<phase, now, deadline, generation, prefix, authenticated, poll>>
Expire == /\ phase \in {"identity", "authenticated"} /\ now >= deadline
          /\ phase' = "closing" /\ prefix' = FALSE
          /\ UNCHANGED <<operation, completed, now, deadline, generation, delivered, publishedAt, authenticated, poll>>
RefuseOrStop == /\ phase \in {"idle", "listen", "identity", "authenticated"}
                /\ phase' = "closing" /\ prefix' = FALSE
                /\ UNCHANGED <<operation, completed, now, deadline, generation, delivered, publishedAt, authenticated, poll>>
Quiesce == /\ phase = "closing" /\ (operation = "none" \/ completed)
           /\ phase' = "closed" /\ operation' = "none" /\ completed' = FALSE
           /\ UNCHANGED <<now, deadline, generation, prefix, delivered, publishedAt, authenticated, poll>>
ExplicitDisconnect == /\ phase = "closed" /\ generation < 2
                      /\ phase' = "idle" /\ generation' = generation + 1 /\ now' = 0 /\ deadline' = 0
                      /\ delivered' = 0 /\ publishedAt' = 0 /\ authenticated' = FALSE
                      /\ UNCHANGED <<operation, completed, prefix, poll>>
Retire == /\ phase = "closing" /\ (operation = "none" \/ completed)
          /\ phase' = "retired" /\ operation' = "none" /\ completed' = FALSE
          /\ UNCHANGED <<now, deadline, generation, prefix, delivered, publishedAt, authenticated, poll>>
Next == StartListen \/ CompleteOperation \/ ObserveConnection \/ PollExpires \/ Tick
        \/ Authenticate \/ PublishPrefix \/ StartBodyRead \/ PublishBody \/ Expire
        \/ RefuseOrStop \/ Quiesce \/ ExplicitDisconnect \/ Retire
CancelCompletes == /\ phase = "closing" /\ CompleteOperation
Spec == Init /\ [][Next]_vars /\ WF_vars(CancelCompletes) /\ WF_vars(Quiesce)
NoUnauthenticatedBytes == delivered > 0 => authenticated
NoExpiredPublication == authenticated => publishedAt < deadline
PrefixExactlyOnce == delivered \in 0..2 /\ (prefix => delivered = 0)
ClosedOwnsNoOperation == phase \in {"closed", "retired"} => operation = "none" /\ ~prefix
NoReusablePendingStorage == phase = "idle" => operation = "none"
CleanupSettles == phase = "closing" ~> phase \in {"closed", "retired"}
PollPreservesOwner == [] [poll' # poll => UNCHANGED <<phase, operation, completed, deadline, generation, prefix>>]_vars
BudgetNeverRenews == [] [(phase \in {"identity", "authenticated"} /\ phase' # "idle") => deadline' = deadline]_vars
=============================================================================
