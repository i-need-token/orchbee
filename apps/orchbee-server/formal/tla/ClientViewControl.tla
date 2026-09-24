------------------------- MODULE ClientViewControl -------------------------
EXTENDS Naturals, FiniteSets, TLC

\* Connection identities are admitted, never reused generations. This bounded
\* model refines routing, UI interference and post-draw acknowledgement; Rust
\* witnesses own exact elapsed-time, byte, capacity and sequence boundaries.
CONSTANTS Clients, Operations, Owner, MaxRevision
VARIABLES connected, fresh, revision, expected, status, deliveries, effect, drawn

vars == <<connected, fresh, revision, expected, status, deliveries, effect, drawn>>
ModelOwner == [o \in Operations |-> IF o = "one" THEN "client-a" ELSE "client-b"]
Pending == {"queued", "delivered"}
Completed == {"applied", "rejected", "incomplete"}
States == Pending \cup Completed \cup {"absent", "expired"}

ASSUME /\ IsFiniteSet(Clients) /\ IsFiniteSet(Operations)
       /\ Owner \in [Operations -> Clients] /\ MaxRevision > 0

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
          \/ UiStep(o) \/ Draw(o) \/ Acknowledge(o) \/ Interrupt(o)
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
=============================================================================
