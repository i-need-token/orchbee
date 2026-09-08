---------------------- MODULE TerminalMouseInputBoundary ----------------------
EXTENDS Integers, Naturals, Sequences, TLC

\* Refines runtime-model claim TH-007 at the semantic-mouse preparation,
\* client queue, parser-mode, local-prompt recovery, and PTY writer boundary.
\* A prepared mouse input captures MouseEpoch. Mode changes, full reset, and a
\* confirmed local-shell prompt advance that epoch. Two representative mode
\* components expose active-to-active configuration changes while
\* MouseTracking remains their derived aggregate. Stale preparation is
\* rejected. After admission, only a wholly unstarted buttonless hover can be
\* retired; ordered input is never retired, and an entry that started before a
\* later epoch change completes from its immutable bytes. The model keeps the
\* queue as an admission-order subsequence and checks bounded progress under a
\* fair writer. The next causal advance at the maximum epoch fails closed even
\* with prepared, queued, or already-started work; no wrap is admitted.

CONSTANTS InputIds, MaxMouseEpoch, MaxQueue

Kinds == {"hover", "ordered"}
Statuses == {
  "absent", "prepared", "rejected", "accepted", "started", "written",
  "retired", "failed"
}
PromptObservations == {"none", "local", "active", "unknown"}
ModeComponents == {"event", "format"}

VARIABLES MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
  FailureFenced, Kind, Status, PreparedEpoch, StartedEpoch, Queue,
  AdmissionOrder, PromptObservation

vars ==
  <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted, FailureFenced,
    Kind, Status, PreparedEpoch, StartedEpoch, Queue, AdmissionOrder,
    PromptObservation>>

MouseTracking == EventEnabled \/ NonDefaultFormat

QueueIds == {Queue[index]: index \in 1..Len(Queue)}

Init ==
  /\ MaxMouseEpoch > 1
  /\ MaxQueue > 0
  /\ MouseEpoch = 1
  /\ EventEnabled = FALSE
  /\ NonDefaultFormat = FALSE
  /\ EpochExhausted = FALSE
  /\ FailureFenced = FALSE
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
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, StartedEpoch, Queue, AdmissionOrder, PromptObservation>>

PrepareOrdered(id) ==
  /\ ~EpochExhausted
  /\ Status[id] = "absent"
  /\ Kind' = [Kind EXCEPT ![id] = "ordered"]
  /\ Status' = [Status EXCEPT ![id] = "prepared"]
  /\ PreparedEpoch' = [PreparedEpoch EXCEPT ![id] = MouseEpoch]
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, StartedEpoch, Queue, AdmissionOrder, PromptObservation>>

CommitPrepared(id) ==
  /\ ~EpochExhausted
  /\ Status[id] = "prepared"
  /\ PreparedEpoch[id] = MouseEpoch
  /\ Len(Queue) < MaxQueue
  /\ Status' = [Status EXCEPT ![id] = "accepted"]
  /\ Queue' = Append(Queue, id)
  /\ AdmissionOrder' = Append(AdmissionOrder, id)
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, Kind, PreparedEpoch, StartedEpoch, PromptObservation>>

RejectStalePrepared(id) ==
  /\ Status[id] = "prepared"
  /\ (EpochExhausted \/ PreparedEpoch[id] # MouseEpoch)
  /\ Status' = [Status EXCEPT ![id] =
       IF EpochExhausted THEN "failed" ELSE "rejected"]
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, Kind, PreparedEpoch, StartedEpoch, Queue, AdmissionOrder,
    PromptObservation>>

AdvanceEpochOrExhaust ==
  IF MouseEpoch < MaxMouseEpoch
    THEN /\ MouseEpoch' = MouseEpoch + 1
         /\ UNCHANGED <<EpochExhausted, FailureFenced>>
    ELSE /\ MouseEpoch' = MouseEpoch
         /\ EpochExhausted' = TRUE
         /\ FailureFenced' = TRUE

ComponentValue(component) ==
  IF component = "event" THEN EventEnabled ELSE NonDefaultFormat

ChangeMouseMode(component, enabled) ==
  /\ ~EpochExhausted
  /\ component \in ModeComponents
  /\ enabled \in BOOLEAN
  /\ enabled # ComponentValue(component)
  /\ AdvanceEpochOrExhaust
  /\ EventEnabled' = IF component = "event" THEN enabled ELSE EventEnabled
  /\ NonDefaultFormat' =
       IF component = "format" THEN enabled ELSE NonDefaultFormat
  /\ PromptObservation' = "none"
  /\ UNCHANGED <<Kind, Status, PreparedEpoch, StartedEpoch, Queue,
                  AdmissionOrder>>

FullReset ==
  /\ ~EpochExhausted
  /\ AdvanceEpochOrExhaust
  /\ EventEnabled' = FALSE
  /\ NonDefaultFormat' = FALSE
  /\ PromptObservation' = "none"
  /\ UNCHANGED <<Kind, Status, PreparedEpoch, StartedEpoch, Queue,
                  AdmissionOrder>>

PromptRecoverLocal ==
  /\ ~EpochExhausted
  /\ MouseTracking
  /\ AdvanceEpochOrExhaust
  \* The runtime clears the complete configuration only after the epoch
  \* advance succeeds. Exhaustion therefore preserves the active modes.
  /\ EventEnabled' =
       IF MouseEpoch < MaxMouseEpoch THEN FALSE ELSE EventEnabled
  /\ NonDefaultFormat' =
       IF MouseEpoch < MaxMouseEpoch THEN FALSE ELSE NonDefaultFormat
  /\ PromptObservation' = "local"
  /\ UNCHANGED <<Kind, Status, PreparedEpoch, StartedEpoch, Queue,
                  AdmissionOrder>>

PromptPreserveActive ==
  /\ ~EpochExhausted
  /\ PromptObservation # "active"
  /\ PromptObservation' = "active"
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, Kind, Status, PreparedEpoch, StartedEpoch, Queue,
    AdmissionOrder>>

PromptPreserveUnknown ==
  /\ ~EpochExhausted
  /\ PromptObservation # "unknown"
  /\ PromptObservation' = "unknown"
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, Kind, Status, PreparedEpoch, StartedEpoch, Queue,
    AdmissionOrder>>

EnableMouseAfterPrompt ==
  /\ ~EpochExhausted
  /\ PromptObservation = "local"
  /\ AdvanceEpochOrExhaust
  /\ EventEnabled' = TRUE
  /\ UNCHANGED NonDefaultFormat
  /\ PromptObservation' = "none"
  /\ UNCHANGED <<Kind, Status, PreparedEpoch, StartedEpoch, Queue,
                  AdmissionOrder>>

FailQueuedInputs ==
  [id \in InputIds |-> IF id \in QueueIds THEN "failed" ELSE Status[id]]

WriterStartOrRetire ==
  /\ Len(Queue) > 0
  /\ LET id == Head(Queue)
     IN /\ Status[id] = "accepted"
        /\ IF EpochExhausted
             THEN /\ Status' = FailQueuedInputs
                  /\ Queue' = <<>>
                  /\ UNCHANGED StartedEpoch
             ELSE IF Kind[id] = "hover" /\ PreparedEpoch[id] # MouseEpoch
               THEN /\ Status' = [Status EXCEPT ![id] = "retired"]
                    /\ Queue' = Tail(Queue)
                    /\ UNCHANGED StartedEpoch
               ELSE /\ Status' = [Status EXCEPT ![id] = "started"]
                    /\ StartedEpoch' =
                         [StartedEpoch EXCEPT ![id] = MouseEpoch]
                    /\ UNCHANGED Queue
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, Kind, PreparedEpoch, AdmissionOrder, PromptObservation>>

WriterFinish ==
  /\ Len(Queue) > 0
  /\ LET id == Head(Queue)
     IN /\ Status[id] = "started"
        /\ Status' = [Status EXCEPT ![id] = "written"]
  /\ Queue' = Tail(Queue)
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, EpochExhausted,
    FailureFenced, Kind, PreparedEpoch, StartedEpoch, AdmissionOrder,
    PromptObservation>>

\* Represents an otherwise abstracted accepted event-mode or format-mode action
\* that attempts the next epoch advance at the maximum. Prepared and queued
\* work are deliberately not guards: they are the concurrency being checked.
ExhaustMouseEpoch ==
  /\ ~EpochExhausted
  /\ MouseEpoch = MaxMouseEpoch
  /\ EpochExhausted' = TRUE
  /\ FailureFenced' = TRUE
  /\ UNCHANGED <<MouseEpoch, EventEnabled, NonDefaultFormat, Kind, Status,
    PreparedEpoch, StartedEpoch, Queue, AdmissionOrder, PromptObservation>>

Next ==
  \/ \E id \in InputIds : PrepareHover(id)
  \/ \E id \in InputIds : PrepareOrdered(id)
  \/ \E id \in InputIds : CommitPrepared(id)
  \/ \E id \in InputIds : RejectStalePrepared(id)
  \/ \E component \in ModeComponents, enabled \in BOOLEAN:
       ChangeMouseMode(component, enabled)
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
  /\ EventEnabled \in BOOLEAN
  /\ NonDefaultFormat \in BOOLEAN
  /\ EpochExhausted \in BOOLEAN
  /\ FailureFenced \in BOOLEAN
  /\ Kind \in [InputIds -> Kinds]
  /\ Status \in [InputIds -> Statuses]
  /\ PreparedEpoch \in [InputIds -> 0..MaxMouseEpoch]
  /\ StartedEpoch \in [InputIds -> 0..MaxMouseEpoch]
  /\ Queue \in Seq(InputIds)
  /\ Len(Queue) <= MaxQueue
  /\ AdmissionOrder \in Seq(InputIds)
  /\ PromptObservation \in PromptObservations

QueueMembersAreLive ==
  /\ \A index \in 1..Len(Queue) :
       Status[Queue[index]] \in {"accepted", "started"}
  /\ \A id \in InputIds :
       (Status[id] \in {"accepted", "started"}) = (id \in QueueIds)

QueuePreservesAdmissionOrder ==
  \A left, right \in 1..Len(Queue) :
    left < right =>
      (CHOOSE index \in 1..Len(AdmissionOrder) :
         AdmissionOrder[index] = Queue[left]) <
      (CHOOSE index \in 1..Len(AdmissionOrder) :
         AdmissionOrder[index] = Queue[right])

OnlyHoverRetires ==
  \A id \in InputIds : Status[id] = "retired" => Kind[id] = "hover"

HoverNeverStartsStale ==
  \A id \in InputIds :
    (Kind[id] = "hover" /\ Status[id] \in {"started", "written"}) =>
      StartedEpoch[id] = PreparedEpoch[id]

StartedInputNeverRetires ==
  \A id \in InputIds : StartedEpoch[id] # 0 => Status[id] # "retired"

EpochNeverWraps == EpochExhausted => MouseEpoch = MaxMouseEpoch

FailureFenceMatchesExhaustion == FailureFenced = EpochExhausted

PromptRecoveryUsesAggregate ==
  (~EpochExhausted /\ (EventEnabled \/ NonDefaultFormat)) =>
    ENABLED PromptRecoverLocal

WorkDoesNotBlockCausalExhaustion ==
  (MouseEpoch = MaxMouseEpoch /\ ~EpochExhausted /\
   (Queue # <<>> \/ \E id \in InputIds : Status[id] = "prepared")) =>
    \E component \in ModeComponents, enabled \in BOOLEAN:
      ENABLED ChangeMouseMode(component, enabled)

ExhaustedInputCannotCommit ==
  EpochExhausted => (\A id \in InputIds : ~ENABLED CommitPrepared(id))

ExhaustedInputCannotPrepare ==
  EpochExhausted =>
    (\A id \in InputIds:
       /\ ~ENABLED PrepareHover(id)
       /\ ~ENABLED PrepareOrdered(id))

ExhaustedPreparedCanReject ==
  EpochExhausted =>
    (\A id \in InputIds:
       Status[id] = "prepared" => ENABLED RejectStalePrepared(id))

EpochAdvanceAtMaximumFailsClosed ==
  [][MouseEpoch = MaxMouseEpoch /\
       (\/ \E component \in ModeComponents, enabled \in BOOLEAN:
             ChangeMouseMode(component, enabled)
        \/ FullReset
        \/ PromptRecoverLocal
        \/ EnableMouseAfterPrompt
        \/ ExhaustMouseEpoch) =>
       /\ MouseEpoch' = MouseEpoch
       /\ EpochExhausted'
       /\ FailureFenced']_vars

ExhaustedQueueFailsClosed ==
  [][EpochExhausted /\ WriterStartOrRetire =>
       /\ Queue' = <<>>
       /\ \A id \in QueueIds : Status'[id] = "failed"]_vars

FailureFenceIsPermanent ==
  [] [FailureFenced => FailureFenced']_vars

AcceptedInputEventuallySettles ==
  \A id \in InputIds :
    (Status[id] = "accepted") ~>
      (Status[id] \in {"written", "retired", "failed"})

=============================================================================
