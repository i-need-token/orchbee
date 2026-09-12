----------------------- MODULE LocalStandaloneTerminal -----------------------
EXTENDS Naturals, FiniteSets, Sequences, TLC

CONSTANTS Clients, NoClient, References, NoReference, Bindings, NoBinding
ASSUME /\ Cardinality(Clients) = 2 /\ NoClient \notin Clients
       /\ Cardinality(References) = 2 /\ NoReference \notin References
       /\ Cardinality(Bindings) = 2 /\ NoBinding \notin Bindings
       /\ Clients \cap References = {} /\ Clients \cap Bindings = {}
       /\ References \cap Bindings = {}
VARIABLES slot, creator, phase, observed, worker, creates, action,
          membership, authorization, orderHistory, revision, navigationWrites, dispatched, workbench, capacity,
          readRevision, readMember, createdAt, reference, dispatchedBinding
vars == <<slot, creator, phase, observed, worker, creates, action,
          membership, authorization, orderHistory, revision, navigationWrites, dispatched, workbench, capacity,
          readRevision, readMember, createdAt, reference, dispatchedBinding>>
member == IF reference = NoReference THEN NoBinding ELSE authorization[reference]
(* `membership` abstracts the strict ordered leaf membership of the standalone
   context: every leaf Panel member, not one member per Tab.  `DerivedComposition`
   is the legal-shape boundary Open enforces under the Site gate: the slot is
   Empty exactly when the member sequence is empty, otherwise the anchor — the
   slot reference's retained authorization converted to a binding — occurs
   exactly once.  States violating it (missing/unconvertible retained
   authorization, zero or duplicate anchor occurrences) are unreachable through
   these transitions; production retires the Workbench authority on them. *)
DerivedComposition ==
  /\ (Len(membership) = 0) = (slot = "empty")
  /\ (slot # "empty" =>
       /\ reference # NoReference
       /\ member # NoBinding
       /\ Cardinality({i \in 1..Len(membership): membership[i] = member}) = 1)
HeadBinding == IF membership = <<>> THEN NoBinding ELSE Head(membership)
Slots == {"empty", "creating", "ready", "failed", "unknown"}
Init ==
  /\ slot = "empty" /\ creator = NoClient /\ creates = 0
  /\ phase = [c \in Clients |-> "idle"]
  /\ observed = [c \in Clients |-> NoReference]
  /\ worker \in BOOLEAN /\ action = "Init"
  /\ membership = <<>> /\ authorization = [r \in References |-> NoBinding]
  /\ orderHistory = <<NoBinding>> /\ revision = 0 /\ navigationWrites = 0 /\ dispatched = 0
  /\ workbench \in BOOLEAN /\ capacity \in BOOLEAN
  /\ readRevision = [c \in Clients |-> 0]
  /\ readMember = [c \in Clients |-> NoBinding] /\ createdAt = 0
  /\ reference = NoReference /\ dispatchedBinding = NoBinding

Reserve(c, r, b) ==
  /\ phase[c] = "idle" /\ slot = "empty" /\ worker /\ workbench /\ capacity
  /\ slot' = "creating" /\ creator' = c /\ creates' = creates + 1
  /\ membership' = <<b>>
  /\ authorization' = [authorization EXCEPT ![r] = b]
  /\ orderHistory' = Append(orderHistory, b)
  /\ reference' = r /\ revision' = revision + 1 /\ createdAt' = revision + 1
  /\ phase' = [phase EXCEPT ![c] = "waiting"]
  /\ UNCHANGED <<observed, worker, navigationWrites, dispatched, workbench,
                 capacity, readRevision, readMember, dispatchedBinding>>
  /\ action' = "Reserve"
PreflightFailure(c) ==
  /\ phase[c] = "idle" /\ slot = "empty"
  \* Includes no Worker/Workbench, entropy, collision, capacity, identity and lease failures.
  /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ UNCHANGED <<slot, creator, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "PreflightFailure"
Dispatch ==
  /\ slot = "creating" /\ dispatched = 0 /\ dispatched' = 1
  /\ dispatchedBinding' = member
  /\ UNCHANGED <<slot, creator, phase, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, workbench, capacity, readRevision, readMember, createdAt, reference>>
  /\ action' = "Dispatch"
Join(c) ==
  /\ phase[c] = "idle" /\ slot # "empty"
  /\ phase' = [phase EXCEPT ![c] = "waiting"]
  /\ UNCHANGED <<slot, creator, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "Join"
Complete(result) ==
  /\ slot = "creating" /\ dispatched = 1 /\ DerivedComposition /\ result \in {"ready", "failed", "unknown"}
  /\ (result = "ready" => worker)
  /\ slot' = result /\ action' = "Complete"
  /\ UNCHANGED <<creator, phase, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
Observe(c) ==
  /\ phase[c] = "waiting" /\ slot \in {"ready", "failed", "unknown"}
  /\ phase' = [phase EXCEPT ![c] = IF slot = "ready" /\ worker THEN "opened" ELSE "failed"]
  /\ observed' = [observed EXCEPT ![c] = IF slot = "ready" /\ worker THEN reference ELSE NoReference]
  /\ UNCHANGED <<slot, creator, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "Observe"
Timeout(c) ==
  /\ phase[c] = "waiting"
  /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ UNCHANGED <<slot, creator, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "Timeout"
Disconnect(c) ==
  /\ phase[c] \in {"waiting", "opened", "failed"}
  /\ phase' = [phase EXCEPT ![c] = "exited"]
  /\ UNCHANGED <<slot, creator, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "Disconnect"
Reopen(c) ==
  /\ phase[c] = "exited"
  /\ phase' = [phase EXCEPT ![c] = "idle"]
  /\ observed' = [observed EXCEPT ![c] = NoReference]
  /\ UNCHANGED <<slot, creator, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "Reopen"
WorkerLoss ==
  /\ worker /\ worker' = FALSE /\ action' = "WorkerLoss"
  /\ UNCHANGED <<slot, creator, phase, observed, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
WorkerAdmission ==
  /\ slot = "empty" /\ ~worker /\ worker' = TRUE /\ action' = "WorkerAdmission"
  /\ UNCHANGED <<slot, creator, phase, observed, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity,
                 readRevision, readMember, createdAt, reference, dispatchedBinding>>
Navigate ==
  /\ workbench /\ navigationWrites < 2
  /\ revision' = revision + 1 /\ navigationWrites' = navigationWrites + 1
  /\ capacity' \in BOOLEAN
  /\ orderHistory' = Append(orderHistory, HeadBinding)
  /\ UNCHANGED <<slot, creator, phase, observed, worker, creates, membership, authorization,
                 dispatched, workbench, readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "Navigate"
ReadWorkbench(c) ==
  /\ workbench /\ phase[c] \in {"idle", "opened", "failed"}
  /\ readRevision' = [readRevision EXCEPT ![c] = revision]
  /\ readMember' = [readMember EXCEPT ![c] = HeadBinding]
  /\ UNCHANGED <<slot, creator, phase, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, workbench, capacity, createdAt, reference, dispatchedBinding>>
  /\ action' = "ReadWorkbench"
RetireWorkbench ==
  /\ workbench /\ workbench' = FALSE
  /\ UNCHANGED <<slot, creator, phase, observed, worker, creates, membership, authorization, orderHistory, revision,
                 navigationWrites, dispatched, capacity, readRevision, readMember, createdAt, reference, dispatchedBinding>>
  /\ action' = "RetireWorkbench"

(* A later leaf member has passed its joint owner preflight: its authorization
and membership are committed together.  The added member abstracts either a
later tab.create or a panel.split leaf; the split's orientation, dispatch and
per-leaf settlement detail stay in WorkbenchTabCreate and WorkbenchPanelSplit.
A pending split leaf is already a member here, so Open replays the settled
slot reference while it exists; the Site stores no second binding or
positional anchor. *)
CreateLaterMember(r, b) ==
  /\ workbench /\ slot # "empty" /\ DerivedComposition
  /\ Len(membership) = 1 /\ navigationWrites < 2
  /\ r # reference /\ authorization[r] = NoBinding /\ b # member
  /\ membership' = Append(membership, b)
  /\ authorization' = [authorization EXCEPT ![r] = b]
  /\ revision' = revision + 1 /\ navigationWrites' = navigationWrites + 1
  /\ orderHistory' = Append(orderHistory, HeadBinding)
  /\ action' = "CreateLaterMember"
  /\ UNCHANGED <<slot, creator, phase, observed, worker, creates,
       dispatched, workbench, capacity, readRevision, readMember, createdAt,
       reference, dispatchedBinding>>

(* A legal leaf reorder: membership shape changes without touching the slot,
   so the anchor reference and every Open replay are preserved. *)
MoveMembership ==
  /\ workbench /\ DerivedComposition /\ Len(membership) = 2
  /\ navigationWrites < 2
  /\ membership' = <<membership[2], membership[1]>>
  /\ revision' = revision + 1 /\ navigationWrites' = navigationWrites + 1
  /\ orderHistory' = Append(orderHistory, membership[2])
  /\ action' = "MoveMembership"
  /\ UNCHANGED <<slot, creator, phase, observed, worker, creates,
       authorization, dispatched, workbench, capacity, readRevision,
       readMember, createdAt, reference, dispatchedBinding>>

CompleteAfterMovedRetirement ==
  /\ ~workbench /\ HeadBinding # member
  /\ \E result \in {"ready", "failed", "unknown"}: Complete(result)

Next ==
  \/ \E c \in Clients, r \in References, b \in Bindings: Reserve(c, r, b)
  \/ \E c \in Clients: PreflightFailure(c) \/ Join(c)
       \/ Observe(c) \/ Timeout(c) \/ Disconnect(c) \/ Reopen(c) \/ ReadWorkbench(c)
  \/ \E result \in {"ready", "failed", "unknown"}: Complete(result)
  \/ WorkerLoss \/ WorkerAdmission \/ Dispatch \/ Navigate \/ RetireWorkbench
  \/ \E r \in References, b \in Bindings: CreateLaterMember(r, b)
  \/ MoveMembership \/ CompleteAfterMovedRetirement
Spec == Init /\ [][Next]_vars /\ \A c \in Clients: WF_vars(Timeout(c))
TypeOK ==
  /\ slot \in Slots /\ creator \in Clients \cup {NoClient} /\ creates \in 0..1
  /\ phase \in [Clients -> {"idle", "waiting", "opened", "failed", "exited"}]
  /\ observed \in [Clients -> References \cup {NoReference}] /\ worker \in BOOLEAN
  /\ membership \in Seq(Bindings) /\ Len(membership) <= 2
  /\ authorization \in [References -> Bindings \cup {NoBinding}]
  /\ orderHistory \in Seq(Bindings \cup {NoBinding})
  /\ Len(orderHistory) = revision + 1
  /\ revision \in 0..3
  /\ navigationWrites \in 0..2 /\ dispatched \in 0..1
  /\ workbench \in BOOLEAN /\ capacity \in BOOLEAN
  /\ readRevision \in [Clients -> 0..3]
  /\ readMember \in [Clients -> Bindings \cup {NoBinding}] /\ createdAt \in 0..3
  /\ reference \in References \cup {NoReference}
  /\ dispatchedBinding \in Bindings \cup {NoBinding}
Singleton == /\ (slot = "empty" <=> creates = 0) /\ (creates = 0 <=> creator = NoClient)
SameReference == \A c \in Clients: observed[c] # NoReference => observed[c] = reference
OpenedWasCommitted == \A c \in Clients: phase[c] = "opened" => slot = "ready"
MembershipCommitted ==
  /\ DerivedComposition
  /\ (member = NoBinding <=> slot = "empty")
  /\ (reference = NoReference <=> slot = "empty")
  /\ revision = navigationWrites + creates
  /\ (creates = 0 <=> createdAt = 0)
DispatchHasMembership == /\ dispatched <= creates
  /\ (dispatched = 1 => dispatchedBinding = member /\ member # NoBinding)
ReadIsCommitted == \A c \in Clients:
  /\ readRevision[c] <= revision
  /\ readMember[c] = orderHistory[readRevision[c] + 1]
NoReset == [][(slot # "empty" => slot' # "empty")]_vars
SettledSlotIsFinal == [][(slot \in {"ready", "failed", "unknown"} => slot' = slot)]_vars
DisconnectPreservesSlot ==
  [][(action' = "Disconnect" => UNCHANGED <<slot, creator, creates, membership, authorization, orderHistory, revision, dispatched>>)]_vars
PreflightPreservesState ==
  [][(action' = "PreflightFailure" => UNCHANGED <<slot, creator, creates, membership, authorization, orderHistory, revision, dispatched>>)]_vars
(* Scope: this model has no leaf removal; member contraction, slot retarget
   after anchor removal and the final-Empty transition are owned by
   WorkbenchTabCloseCreateReference and WorkbenchPanelSplit. *)
MembershipIsRetained == [][(member # NoBinding =>
  /\ member' = member /\ reference' = reference)]_vars
MovePreservesBootstrap == [][action' = "MoveMembership" =>
  UNCHANGED <<slot, reference, authorization, creator, creates, dispatched,
    dispatchedBinding, phase, observed>>]_vars
EveryWaitSettles == \A c \in Clients: phase[c] = "waiting" ~> phase[c] # "waiting"
=============================================================================
