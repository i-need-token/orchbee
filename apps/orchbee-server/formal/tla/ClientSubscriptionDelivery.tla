-------------------- MODULE ClientSubscriptionDelivery --------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC
CONSTANTS Clients, Capacity, MaxRevision, FenceGeneration, FenceCancellation
VARIABLES revision, generation, phase, queue, flight, captured, delivered,
          snapshot, gapIssued, gapSeen, cancelled, unsafeDelivery
vars == <<revision, generation, phase, queue, flight, captured, delivered,
          snapshot, gapIssued, gapSeen, cancelled, unsafeDelivery>>
Empty == [kind |-> "empty", rev |-> 0, gen |-> 0, gap |-> 0]
Frame(c, kind) == [kind |-> kind, rev |-> revision, gen |-> generation[c], gap |-> gapIssued[c]]

Init == /\ revision = 0
        /\ generation = [c \in Clients |-> 1]
        /\ phase = [c \in Clients |-> "snapshot"]
        /\ queue = [c \in Clients |-> <<>>]
        /\ flight = [c \in Clients |-> Empty]
        /\ captured = [c \in Clients |-> 0]
        /\ delivered = [c \in Clients |-> 0]
        /\ snapshot = [c \in Clients |-> FALSE]
        /\ gapIssued = [c \in Clients |-> 0]
        /\ gapSeen = [c \in Clients |-> 0]
        /\ cancelled = [c \in Clients |-> FALSE]
        /\ unsafeDelivery = [generation |-> FALSE, cancellation |-> FALSE,
                              snapshot |-> FALSE, gap |-> FALSE]
Publish == /\ revision < MaxRevision
           /\ revision' = revision + 1
           /\ UNCHANGED <<generation, phase, queue, flight, captured, delivered,
                          snapshot, gapIssued, gapSeen, cancelled, unsafeDelivery>>
Capture(c) == /\ phase[c] = "snapshot" /\ ~cancelled[c]
              /\ Len(queue[c]) < Capacity
              /\ queue' = [queue EXCEPT ![c] = Append(@, Frame(c, "snapshot"))]
              /\ captured' = [captured EXCEPT ![c] = revision]
              /\ phase' = [phase EXCEPT ![c] = "live"]
              /\ UNCHANGED <<revision, generation, flight, delivered, snapshot,
                             gapIssued, gapSeen, cancelled, unsafeDelivery>>
Enqueue(c) == /\ phase[c] = "live" /\ ~cancelled[c]
              /\ Len(queue[c]) < Capacity /\ revision > captured[c]
              /\ queue' = [queue EXCEPT ![c] = Append(@, Frame(c, "update"))]
              /\ captured' = [captured EXCEPT ![c] = revision]
              /\ UNCHANGED <<revision, generation, phase, flight, delivered, snapshot,
                             gapIssued, gapSeen, cancelled, unsafeDelivery>>
Overflow(c) == /\ phase[c] = "live" /\ ~cancelled[c]
               /\ Len(queue[c]) = Capacity /\ revision > captured[c]
               /\ gapIssued' = [gapIssued EXCEPT ![c] = @ + 1]
               /\ queue' = [queue EXCEPT ![c] = <<[kind |-> "gap", rev |-> revision,
                    gen |-> generation[c], gap |-> gapIssued[c] + 1]>>]
               /\ phase' = [phase EXCEPT ![c] = "gap"]
               /\ captured' = [captured EXCEPT ![c] = revision]
               /\ UNCHANGED <<revision, generation, flight, delivered, snapshot,
                              gapSeen, cancelled, unsafeDelivery>>
\* Resnapshot may race the consumer; the explicit gap remains before its snapshot.
Resnapshot(c) == /\ phase[c] = "gap" /\ ~cancelled[c]
                 /\ phase' = [phase EXCEPT ![c] = "snapshot"]
                 /\ UNCHANGED <<revision, generation, queue, flight, captured, delivered,
                                snapshot, gapIssued, gapSeen, cancelled, unsafeDelivery>>
Take(c) == /\ flight[c] = Empty /\ Len(queue[c]) > 0
           /\ flight' = [flight EXCEPT ![c] = Head(queue[c])]
           /\ queue' = [queue EXCEPT ![c] = Tail(@)]
           /\ UNCHANGED <<revision, generation, phase, captured, delivered, snapshot,
                          gapIssued, gapSeen, cancelled, unsafeDelivery>>
\* Independent completion survives cancellation/reconnect, unlike the owned queue.
Deliver(c) == /\ flight[c] # Empty
              /\ LET item == flight[c]
                     accepted == (~FenceCancellation \/ ~cancelled[c])
                                 /\ (~FenceGeneration \/ item.gen = generation[c])
                 IN /\ delivered' = IF accepted /\ item.kind # "gap"
                                      THEN [delivered EXCEPT ![c] = item.rev] ELSE delivered
                    /\ snapshot' = IF accepted THEN [snapshot EXCEPT ![c] =
                          IF item.kind = "gap" THEN FALSE
                          ELSE IF item.kind = "snapshot" THEN TRUE ELSE @] ELSE snapshot
                    /\ gapSeen' = IF accepted /\ item.kind = "gap"
                                  THEN [gapSeen EXCEPT ![c] = item.gap] ELSE gapSeen
                    /\ unsafeDelivery' = [unsafeDelivery EXCEPT
                         !.generation = @ \/ (accepted /\ item.gen # generation[c]),
                         !.cancellation = @ \/ (accepted /\ cancelled[c]),
                         !.snapshot = @ \/ (accepted /\ item.kind = "update" /\ ~snapshot[c]),
                         !.gap = @ \/ (accepted /\ item.kind = "snapshot" /\ item.gap > gapSeen[c])]
              /\ flight' = [flight EXCEPT ![c] = Empty]
              /\ UNCHANGED <<revision, generation, phase, queue, captured, gapIssued, cancelled>>
Cancel(c) == /\ ~cancelled[c]
             /\ cancelled' = [cancelled EXCEPT ![c] = TRUE]
             /\ queue' = [queue EXCEPT ![c] = <<>>]
             /\ UNCHANGED <<revision, generation, phase, flight, captured, delivered,
                            snapshot, gapIssued, gapSeen, unsafeDelivery>>
Reconnect(c) == /\ generation[c] = 1 /\ ~cancelled[c]
                /\ generation' = [generation EXCEPT ![c] = 2]
                /\ queue' = [queue EXCEPT ![c] = <<>>]
                /\ phase' = [phase EXCEPT ![c] = "snapshot"]
                /\ snapshot' = [snapshot EXCEPT ![c] = FALSE]
                /\ gapIssued' = [gapIssued EXCEPT ![c] = 0]
                /\ gapSeen' = [gapSeen EXCEPT ![c] = 0]
                /\ UNCHANGED <<revision, flight, captured, delivered, cancelled, unsafeDelivery>>
Next == Publish \/ \E c \in Clients : Capture(c) \/ Enqueue(c) \/ Overflow(c)
        \/ Resnapshot(c) \/ Take(c) \/ Deliver(c) \/ Cancel(c) \/ Reconnect(c)
Spec == Init /\ [][Next]_vars
Bounded == \A c \in Clients : Len(queue[c]) <= Capacity
CancelledEmpty == \A c \in Clients : cancelled[c] => Len(queue[c]) = 0
CurrentGeneration == ~unsafeDelivery.generation
CancellationFence == ~unsafeDelivery.cancellation
SnapshotFirst == ~unsafeDelivery.snapshot
GapBeforeSnapshot == ~unsafeDelivery.gap
NoFutureDelivery == \A c \in Clients : delivered[c] <= revision
PublisherIndependent == revision < MaxRevision => ENABLED Publish
=============================================================================
