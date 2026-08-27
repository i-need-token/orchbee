---------------------- MODULE TerminalMouseInputBoundary ----------------------
EXTENDS Integers, Naturals, Sequences, TLC

\* Refines runtime-model claim TH-007 at the semantic-mouse preparation,
\* client queue, parser-mode, local-prompt recovery, and PTY writer boundary.
\* A prepared mouse input captures MouseEpoch. Mode changes, full reset, and a
\* confirmed local-shell prompt advance that epoch. Stale preparation is
\* rejected. After admission, only a wholly unstarted buttonless hover can be
\* retired; ordered input is never retired, and an entry that started before a
\* later epoch change completes from its immutable bytes. The model keeps the
\* queue as an admission-order subsequence and checks bounded progress under a
\* fair writer. Epoch exhaustion fails closed instead of wrapping.

CONSTANTS InputIds, MaxMouseEpoch, MaxQueue

Kinds == {"hover", "ordered"}
Statuses == {"absent", "prepared", "rejected", "accepted", "started", "written", "retired"}
PromptObservations == {"none", "local", "active", "unknown"}

VARIABLES MouseEpoch, MouseTracking, EpochExhausted, Kind, Status,
  PreparedEpoch, StartedEpoch, Queue, AdmissionOrder, PromptObservation

vars ==
  <<MouseEpoch, MouseTracking, EpochExhausted, Kind, Status, PreparedEpoch,
    StartedEpoch, Queue, AdmissionOrder, PromptObservation>>

Init ==
  /\ MouseEpoch = 1
  /\ MouseTracking = FALSE
  /\ EpochExhausted = FALSE
  /\ Kind = [id \in InputIds |-> "hover"]
  /\ Status = [id \in InputIds |-> "absent"]
  /\ PreparedEpoch = [id \in InputIds |-> 0]
  /\ StartedEpoch = [id \in InputIds |-> 0]
  /\ Queue = <<>>
  /\ AdmissionOrder = <<>>
  /\ PromptObservation = "none"

PrepareHover(id) ==
  /\ ~EpochExhausted
  /\ Status[id] = "absent"
  /\ Kind' = [Kind EXCEPT ![id] = "hover"]
  /\ Status' = [Status EXCEPT ![id] = "prepared"]
  /\ PreparedEpoch' = [PreparedEpoch EXCEPT ![id] = MouseEpoch]
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, StartedEpoch,
    Queue, AdmissionOrder, PromptObservation>>

PrepareOrdered(id) ==
  /\ ~EpochExhausted
  /\ Status[id] = "absent"
  /\ Kind' = [Kind EXCEPT ![id] = "ordered"]
  /\ Status' = [Status EXCEPT ![id] = "prepared"]
  /\ PreparedEpoch' = [PreparedEpoch EXCEPT ![id] = MouseEpoch]
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, StartedEpoch,
    Queue, AdmissionOrder, PromptObservation>>

CommitPrepared(id) ==
  /\ ~EpochExhausted
  /\ Status[id] = "prepared"
  /\ PreparedEpoch[id] = MouseEpoch
  /\ Len(Queue) < MaxQueue
  /\ Status' = [Status EXCEPT ![id] = "accepted"]
  /\ Queue' = Append(Queue, id)
  /\ AdmissionOrder' = Append(AdmissionOrder, id)
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, Kind,
    PreparedEpoch, StartedEpoch, PromptObservation>>

RejectStalePrepared(id) ==
  /\ Status[id] = "prepared"
  /\ PreparedEpoch[id] # MouseEpoch
  /\ Status' = [Status EXCEPT ![id] = "rejected"]
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, Kind,
    PreparedEpoch, StartedEpoch, Queue, AdmissionOrder, PromptObservation>>

ChangeMouseMode ==
  /\ ~EpochExhausted
  /\ MouseEpoch < MaxMouseEpoch
  /\ MouseEpoch' = MouseEpoch + 1
  /\ MouseTracking' = ~MouseTracking
  /\ PromptObservation' = "none"
  /\ UNCHANGED <<EpochExhausted, Kind, Status, PreparedEpoch, StartedEpoch,
    Queue, AdmissionOrder>>

FullReset ==
  /\ ~EpochExhausted
  /\ MouseEpoch < MaxMouseEpoch
  /\ MouseEpoch' = MouseEpoch + 1
  /\ MouseTracking' = FALSE
  /\ PromptObservation' = "none"
  /\ UNCHANGED <<EpochExhausted, Kind, Status, PreparedEpoch, StartedEpoch,
    Queue, AdmissionOrder>>

PromptRecoverLocal ==
  /\ ~EpochExhausted
  /\ MouseTracking
  /\ MouseEpoch < MaxMouseEpoch
  /\ MouseEpoch' = MouseEpoch + 1
  /\ MouseTracking' = FALSE
  /\ PromptObservation' = "local"
  /\ UNCHANGED <<EpochExhausted, Kind, Status, PreparedEpoch, StartedEpoch,
    Queue, AdmissionOrder>>

PromptPreserveActive ==
  /\ ~EpochExhausted
  /\ PromptObservation # "active"
  /\ PromptObservation' = "active"
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, Kind, Status,
    PreparedEpoch, StartedEpoch, Queue, AdmissionOrder>>

PromptPreserveUnknown ==
  /\ ~EpochExhausted
  /\ PromptObservation # "unknown"
  /\ PromptObservation' = "unknown"
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, Kind, Status,
    PreparedEpoch, StartedEpoch, Queue, AdmissionOrder>>

EnableMouseAfterPrompt ==
  /\ ~EpochExhausted
  /\ PromptObservation = "local"
  /\ MouseEpoch < MaxMouseEpoch
  /\ MouseEpoch' = MouseEpoch + 1
  /\ MouseTracking' = TRUE
  /\ PromptObservation' = "none"
  /\ UNCHANGED <<EpochExhausted, Kind, Status, PreparedEpoch, StartedEpoch,
    Queue, AdmissionOrder>>

WriterStartOrRetire ==
  /\ Len(Queue) > 0
  /\ LET id == Head(Queue)
     IN /\ Status[id] = "accepted"
        /\ IF Kind[id] = "hover" /\ PreparedEpoch[id] # MouseEpoch
              THEN /\ Status' = [Status EXCEPT ![id] = "retired"]
                   /\ Queue' = Tail(Queue)
                   /\ UNCHANGED StartedEpoch
              ELSE /\ Status' = [Status EXCEPT ![id] = "started"]
                   /\ StartedEpoch' = [StartedEpoch EXCEPT ![id] = MouseEpoch]
                   /\ UNCHANGED Queue
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, Kind,
    PreparedEpoch, AdmissionOrder, PromptObservation>>

WriterFinish ==
  /\ Len(Queue) > 0
  /\ LET id == Head(Queue)
     IN /\ Status[id] = "started"
        /\ Status' = [Status EXCEPT ![id] = "written"]
  /\ Queue' = Tail(Queue)
  /\ UNCHANGED <<MouseEpoch, MouseTracking, EpochExhausted, Kind,
    PreparedEpoch, StartedEpoch, AdmissionOrder, PromptObservation>>

ExhaustMouseEpoch ==
  /\ ~EpochExhausted
  /\ MouseEpoch = MaxMouseEpoch
  /\ Queue = <<>>
  /\ \A id \in InputIds : Status[id] # "prepared"
  /\ EpochExhausted' = TRUE
  /\ UNCHANGED <<MouseEpoch, MouseTracking, Kind, Status, PreparedEpoch,
    StartedEpoch, Queue, AdmissionOrder, PromptObservation>>

Next ==
  \/ \E id \in InputIds : PrepareHover(id)
  \/ \E id \in InputIds : PrepareOrdered(id)
  \/ \E id \in InputIds : CommitPrepared(id)
  \/ \E id \in InputIds : RejectStalePrepared(id)
  \/ ChangeMouseMode
  \/ FullReset
  \/ PromptRecoverLocal
  \/ PromptPreserveActive
  \/ PromptPreserveUnknown
  \/ EnableMouseAfterPrompt
  \/ WriterStartOrRetire
  \/ WriterFinish
  \/ ExhaustMouseEpoch

Spec ==
  Init /\ [][Next]_vars /\ WF_vars(WriterStartOrRetire) /\ WF_vars(WriterFinish)

TypeOK ==
  /\ MouseEpoch \in 1..MaxMouseEpoch
  /\ MouseTracking \in BOOLEAN
  /\ EpochExhausted \in BOOLEAN
  /\ Kind \in [InputIds -> Kinds]
  /\ Status \in [InputIds -> Statuses]
  /\ PreparedEpoch \in [InputIds -> 0..MaxMouseEpoch]
  /\ StartedEpoch \in [InputIds -> 0..MaxMouseEpoch]
  /\ Queue \in Seq(InputIds)
  /\ Len(Queue) <= MaxQueue
  /\ AdmissionOrder \in Seq(InputIds)
  /\ PromptObservation \in PromptObservations

QueueMembersAreLive ==
  /\ \A index \in 1..Len(Queue) : Status[Queue[index]] \in {"accepted", "started"}
  /\ \A id \in InputIds :
       (Status[id] \in {"accepted", "started"}) = (id \in {Queue[index] : index \in 1..Len(Queue)})

QueuePreservesAdmissionOrder ==
  \A left, right \in 1..Len(Queue) :
    left < right =>
      (CHOOSE i \in 1..Len(AdmissionOrder) : AdmissionOrder[i] = Queue[left]) <
      (CHOOSE i \in 1..Len(AdmissionOrder) : AdmissionOrder[i] = Queue[right])

OnlyHoverRetires ==
  \A id \in InputIds : Status[id] = "retired" => Kind[id] = "hover"

HoverNeverStartsStale ==
  \A id \in InputIds :
    (Kind[id] = "hover" /\ Status[id] \in {"started", "written"}) =>
      StartedEpoch[id] = PreparedEpoch[id]

StartedInputNeverRetires ==
  \A id \in InputIds : StartedEpoch[id] # 0 => Status[id] # "retired"

EpochNeverWraps == EpochExhausted => MouseEpoch = MaxMouseEpoch

AcceptedInputEventuallySettles ==
  \A id \in InputIds :
    (Status[id] = "accepted") ~> (Status[id] \in {"written", "retired"})

=============================================================================
