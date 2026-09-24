------------------------- MODULE ClientViewControl -------------------------
EXTENDS Naturals, FiniteSets, TLC

\* Connection identities are admitted, never reused generations. This bounded
\* model refines routing, UI interference and post-draw acknowledgement; Rust
\* witnesses own exact elapsed-time, byte, capacity and sequence boundaries.
CONSTANTS Clients, Operations, Owner, MaxRevision, LayoutOperations, LayoutBudget, DirectionOperations
VARIABLES connected, fresh, revision, expected, status, deliveries, effect, drawn

vars == <<connected, fresh, revision, expected, status, deliveries, effect, drawn>>
ModelOwner == [o \in Operations |-> IF o = "one" THEN "client-a" ELSE "client-b"]
Pending == {"queued", "delivered"}
Completed == {"applied", "rejected", "incomplete", "layout-read", "direction-unchanged"}
States == Pending \cup Completed \cup {"absent", "expired"}

LayoutPending == {o \in LayoutOperations : status[o] \in Pending}
LayoutCompleted == {o \in LayoutOperations : status[o] = "layout-read"}
\* One unit abstracts a bounded completed payload; two reserve its maximum.
ReservedBytes == 2 * Cardinality(LayoutPending) + Cardinality(LayoutCompleted)

ASSUME /\ IsFiniteSet(Clients) /\ IsFiniteSet(Operations)
       /\ Owner \in [Operations -> Clients] /\ MaxRevision > 0
       /\ LayoutOperations \subseteq Operations /\ LayoutBudget >= 2
       /\ DirectionOperations \subseteq Operations
       /\ DirectionOperations \cap LayoutOperations = {}

Init ==
  /\ connected = Clients
  /\ fresh = Clients
  /\ revision = [c \in Clients |-> 1]
  /\ expected = [o \in Operations |-> 0]
  /\ status = [o \in Operations |-> "absent"]
  /\ deliveries = [o \in Operations |-> 0]
  /\ effect = {}
  /\ drawn = {}

Sample(c) ==
  /\ c \in connected
  /\ revision[c] < MaxRevision
  /\ revision' = [revision EXCEPT ![c] = @ + 1]
  /\ fresh' = fresh \cup {c}
  /\ UNCHANGED <<connected, expected, status, deliveries, effect, drawn>>

Freeze(c) ==
  /\ c \in fresh
  /\ fresh' = fresh \ {c}
  /\ UNCHANGED <<connected, revision, expected, status, deliveries, effect, drawn>>

Submit(o) ==
  /\ status[o] = "absent"
  /\ (o \notin LayoutOperations \/ ReservedBytes + 2 <= LayoutBudget)
  /\ Owner[o] \in connected \cap fresh
  /\ ~\E other \in Operations : Owner[other] = Owner[o] /\ status[other] \in Pending
  /\ status' = [status EXCEPT ![o] = "queued"]
  /\ expected' = [expected EXCEPT ![o] = revision[Owner[o]]]
  /\ UNCHANGED <<connected, fresh, revision, deliveries, effect, drawn>>

Deliver(o) ==
  /\ status[o] = "queued"
  /\ Owner[o] \in connected \cap fresh
  /\ expected[o] = revision[Owner[o]]
  /\ status' = [status EXCEPT ![o] = "delivered"]
  /\ deliveries' = [deliveries EXCEPT ![o] = @ + 1]
  /\ UNCHANGED <<connected, fresh, revision, expected, effect, drawn>>

RejectStale(o) ==
  /\ status[o] = "queued"
  /\ (Owner[o] \notin fresh \/ expected[o] # revision[Owner[o]])
  /\ status' = [status EXCEPT ![o] = "rejected"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

UiStep(o) ==
  /\ o \notin LayoutOperations
  /\ status[o] = "delivered"
  /\ o \notin effect
  /\ Owner[o] \in connected \cap fresh
  /\ expected[o] = revision[Owner[o]]
  /\ effect' = effect \cup {o}
  /\ UNCHANGED <<connected, fresh, revision, expected, status, deliveries, drawn>>

Draw(o) ==
  /\ status[o] = "delivered"
  /\ o \in effect
  /\ expected[o] = revision[Owner[o]]
  /\ drawn' = drawn \cup {o}
  /\ UNCHANGED <<connected, fresh, revision, expected, status, deliveries, effect>>

Acknowledge(o) ==
  /\ status[o] = "delivered"
  /\ o \in drawn
  /\ status' = [status EXCEPT ![o] = "applied"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

ObserveLayout(o) ==
  /\ o \in LayoutOperations
  /\ status[o] = "delivered"
  /\ Owner[o] \in connected \cap fresh
  /\ expected[o] = revision[Owner[o]]
  /\ status' = [status EXCEPT ![o] = "layout-read"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

NoNeighbor(o) ==
  /\ o \in DirectionOperations
  /\ status[o] = "delivered"
  /\ o \notin effect
  /\ Owner[o] \in connected \cap fresh
  /\ expected[o] = revision[Owner[o]]
  /\ status' = [status EXCEPT ![o] = "direction-unchanged"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

Interrupt(o) ==
  /\ status[o] = "delivered"
  /\ status' = [status EXCEPT ![o] = IF o \in effect THEN "incomplete" ELSE "rejected"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

Disconnect(c) ==
  /\ c \in connected
  /\ connected' = connected \ {c}
  /\ fresh' = fresh \ {c}
  /\ status' = [o \in Operations |->
       IF Owner[o] # c THEN status[o]
       ELSE IF status[o] = "queued" THEN "rejected"
       ELSE IF status[o] = "delivered" THEN "incomplete" ELSE status[o]]
  /\ UNCHANGED <<revision, expected, deliveries, effect, drawn>>

ExpireQueue(o) ==
  /\ status[o] = "queued"
  /\ status' = [status EXCEPT ![o] = "rejected"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

ExpireReceipt(o) ==
  /\ status[o] \in Completed
  /\ status' = [status EXCEPT ![o] = "expired"]
  /\ UNCHANGED <<connected, fresh, revision, expected, deliveries, effect, drawn>>

ReadOrReplay == UNCHANGED vars
Next == (\E c \in Clients : Sample(c) \/ Freeze(c) \/ Disconnect(c))
     \/ (\E o \in Operations : Submit(o) \/ Deliver(o) \/ RejectStale(o)
          \/ UiStep(o) \/ Draw(o) \/ Acknowledge(o) \/ ObserveLayout(o) \/ NoNeighbor(o) \/ Interrupt(o)
          \/ ExpireQueue(o) \/ ExpireReceipt(o)) \/ ReadOrReplay
Spec == Init /\ [][Next]_vars

TypeOK == /\ connected \subseteq Clients /\ fresh \subseteq connected
          /\ revision \in [Clients -> 1..MaxRevision]
          /\ expected \in [Operations -> 0..MaxRevision]
          /\ status \in [Operations -> States]
          /\ deliveries \in [Operations -> Nat]
          /\ effect \subseteq Operations /\ drawn \subseteq effect
AtMostOneDelivery == \A o \in Operations : deliveries[o] <= 1
AppliedRequiresDraw == \A o \in Operations : status[o] = "applied" => o \in drawn
EffectsRequireDelivery == \A o \in effect : deliveries[o] = 1
PendingRequiresConnection == \A o \in Operations : status[o] \in Pending => Owner[o] \in connected
OneActiveCommand == \A c \in Clients : Cardinality({o \in Operations : Owner[o] = c /\ status[o] \in Pending}) <= 1
LayoutReadsArePure == /\ effect \cap LayoutOperations = {}
                      /\ drawn \cap LayoutOperations = {}
LayoutReadRequiresDelivery == \A o \in Operations : status[o] = "layout-read" => o \in LayoutOperations /\ deliveries[o] = 1
NoNeighborIsPure == \A o \in Operations : status[o] = "direction-unchanged" =>
                     o \in DirectionOperations /\ deliveries[o] = 1 /\ o \notin effect /\ o \notin drawn
LayoutBudgetBounded == ReservedBytes <= LayoutBudget
=============================================================================
