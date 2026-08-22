---------------- MODULE ProjectedTerminalBrokerAdmission ----------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Finite abstraction of one projected-terminal broker's Renderer-to-Host
\* request admission. Every modeled command is input, so ActiveLimit refines
\* the production input subwindow while the larger total request window is an
\* invariant of the implementation. The production limits are larger; the
\* model preserves their ordering relationship with two active and two pending
\* positions. Host execution and renderer transport delivery remain outside
\* this model.

CONSTANT Scenario

Scenarios == {"absorbable-burst", "overflow"}
ActiveLimit == 2
PendingLimit == 2
CommandCount == IF Scenario = "absorbable-burst" THEN 4 ELSE 5
Commands == 1..CommandCount
Statuses == {"live", "retired"}
Actions == {
  "init", "admit-direct", "admit-queued", "reject-overflow",
  "complete", "settle-retired"
}

VARIABLE s

vars == <<s>>

SequenceRange(sequence) == {sequence[index] : index \in DOMAIN sequence}

IsPrefix(prefix, sequence) ==
  /\ Len(prefix) <= Len(sequence)
  /\ \A index \in 1..Len(prefix): prefix[index] = sequence[index]

Init ==
  s = [
    status |-> "live",
    nextCommand |-> 1,
    active |-> {},
    pending |-> <<>>,
    accepted |-> <<>>,
    handedOff |-> <<>>,
    completed |-> {},
    dropped |-> <<>>,
    rejected |-> <<>>,
    lastAction |-> "init"
  ]

AdmitDirect ==
  /\ s.status = "live"
  /\ s.nextCommand \in Commands
  /\ s.pending = <<>>
  /\ Cardinality(s.active) < ActiveLimit
  /\ LET command == s.nextCommand IN
       s' = [s EXCEPT
         !.nextCommand = command + 1,
         !.active = @ \cup {command},
         !.accepted = Append(@, command),
         !.handedOff = Append(@, command),
         !.lastAction = "admit-direct"]

AdmitQueued ==
  /\ s.status = "live"
  /\ s.nextCommand \in Commands
  /\ Cardinality(s.active) = ActiveLimit
  /\ Len(s.pending) < PendingLimit
  /\ LET command == s.nextCommand IN
       s' = [s EXCEPT
         !.nextCommand = command + 1,
         !.pending = Append(@, command),
         !.accepted = Append(@, command),
         !.lastAction = "admit-queued"]

RejectOverflow ==
  /\ s.status = "live"
  /\ s.nextCommand \in Commands
  /\ Cardinality(s.active) = ActiveLimit
  /\ Len(s.pending) = PendingLimit
  /\ LET command == s.nextCommand IN
       s' = [s EXCEPT
         !.status = "retired",
         !.nextCommand = command + 1,
         !.dropped = @ \o s.pending,
         !.pending = <<>>,
         !.rejected = Append(@, command),
         !.lastAction = "reject-overflow"]

Complete(command) ==
  /\ s.status = "live"
  /\ command \in s.active
  /\ IF s.pending = <<>>
     THEN
       s' = [s EXCEPT
         !.active = @ \ {command},
         !.completed = @ \cup {command},
         !.lastAction = "complete"]
     ELSE
       LET next == Head(s.pending) IN
         s' = [s EXCEPT
           !.active = (@ \ {command}) \cup {next},
           !.pending = Tail(@),
           !.handedOff = Append(@, next),
           !.completed = @ \cup {command},
           !.lastAction = "complete"]

SettleRetired(command) ==
  /\ s.status = "retired"
  /\ command \in s.active
  /\ s' = [s EXCEPT
       !.active = @ \ {command},
       !.completed = @ \cup {command},
       !.lastAction = "settle-retired"]

CompleteAny == \E command \in s.active: Complete(command)
SettleRetiredAny == \E command \in s.active: SettleRetired(command)

Next ==
  \/ AdmitDirect
  \/ AdmitQueued
  \/ RejectOverflow
  \/ CompleteAny
  \/ SettleRetiredAny

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CompleteAny)

TypeOK ==
  /\ Scenario \in Scenarios
  /\ s.status \in Statuses
  /\ s.nextCommand \in 1..(CommandCount + 1)
  /\ s.active \subseteq Commands
  /\ s.pending \in Seq(Commands)
  /\ s.accepted \in Seq(Commands)
  /\ s.handedOff \in Seq(Commands)
  /\ s.completed \subseteq Commands
  /\ s.dropped \in Seq(Commands)
  /\ s.rejected \in Seq(Commands)
  /\ s.lastAction \in Actions

AdmissionIsBounded ==
  /\ Cardinality(s.active) <= ActiveLimit
  /\ Len(s.pending) <= PendingLimit

CommandsHaveSingleDisposition ==
  /\ Cardinality(SequenceRange(s.accepted)) = Len(s.accepted)
  /\ Cardinality(SequenceRange(s.handedOff)) = Len(s.handedOff)
  /\ Cardinality(SequenceRange(s.dropped)) = Len(s.dropped)
  /\ Cardinality(SequenceRange(s.rejected)) = Len(s.rejected)
  /\ SequenceRange(s.accepted) \cap SequenceRange(s.rejected) = {}
  /\ SequenceRange(s.handedOff) \cap SequenceRange(s.dropped) = {}

HandoffPreservesAdmissionOrder == IsPrefix(s.handedOff, s.accepted)

AcceptedSuffixRemainsOwned ==
  IF s.status = "live"
  THEN s.accepted = s.handedOff \o s.pending
  ELSE s.accepted = s.handedOff \o s.dropped

AbsorbableBurstDoesNotRetire ==
  Scenario # "absorbable-burst" \/ s.status = "live"

OverflowRetiresOnlyAtExactBoundary ==
  s.lastAction # "reject-overflow" \/
    /\ Cardinality(s.active) = ActiveLimit
    /\ Len(s.dropped) = PendingLimit
    /\ Len(s.rejected) = 1

ArrivedBurstEventuallyDrains ==
  [](s.nextCommand = CommandCount + 1 ~> s.pending = <<>>)

=============================================================================
