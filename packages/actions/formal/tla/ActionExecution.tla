--------------------------- MODULE ActionExecution ---------------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

Executions == {1, 2}
Policies == {"parallel", "reject-duplicate", "replace-latest", "serialize"}
NonTerminal == {"absent", "resolving", "awaiting", "running", "cancelling"}
Terminal == {"completed", "unchanged", "blocked", "cancelled", "failed", "unsupported"}
States == NonTerminal \cup Terminal

VARIABLES
  policy,
  phase,
  committed,
  handlerStarts,
  resultCount,
  invocationOrder

vars == <<policy, phase, committed, handlerStarts, resultCount, invocationOrder>>

IsActive(execution) ==
  phase[execution] \in {"awaiting", "running", "cancelling"}

IndexOf(sequence, value) ==
  CHOOSE index \in 1..Len(sequence): sequence[index] = value

HasEarlierPendingInvocation(execution) ==
  \E other \in Executions:
    /\ other # execution
    /\ other \in {invocationOrder[index]: index \in 1..(IndexOf(invocationOrder, execution) - 1)}
    /\ phase[other] \notin Terminal

Init ==
  /\ policy \in Policies
  /\ phase = [execution \in Executions |-> "absent"]
  /\ committed = {}
  /\ handlerStarts = [execution \in Executions |-> 0]
  /\ resultCount = [execution \in Executions |-> 0]
  /\ invocationOrder = <<>>

Invoke(execution) ==
  /\ phase[execution] = "absent"
  /\ phase' = [phase EXCEPT ![execution] = "resolving"]
  /\ invocationOrder' = Append(invocationOrder, execution)
  /\ UNCHANGED <<policy, committed, handlerStarts, resultCount>>

Resolve(execution) ==
  /\ phase[execution] = "resolving"
  /\ IF policy = "reject-duplicate"
        /\ \E other \in Executions \ {execution}: IsActive(other)
     THEN
        /\ phase' = [phase EXCEPT ![execution] = "blocked"]
        /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
        /\ UNCHANGED <<committed, handlerStarts>>
     ELSE IF policy = "serialize" /\ HasEarlierPendingInvocation(execution)
     THEN
        /\ UNCHANGED <<phase, committed, handlerStarts, resultCount>>
     ELSE
        /\ phase' = [phase EXCEPT ![execution] = "awaiting"]
        /\ IF policy = "replace-latest"
              /\ \E other \in Executions \ {execution}: IsActive(other)
           THEN
              LET other == CHOOSE candidate \in Executions \ {execution}: IsActive(candidate)
              IN
                /\ phase' = [phase EXCEPT
                    ![execution] = "awaiting",
                    ![other] = IF phase[other] = "awaiting" THEN "cancelled" ELSE "cancelling"]
                /\ resultCount' = IF phase[other] = "awaiting"
                    THEN [resultCount EXCEPT ![other] = @ + 1]
                    ELSE resultCount
           ELSE
              /\ UNCHANGED resultCount
        /\ UNCHANGED <<committed, handlerStarts>>
  /\ UNCHANGED <<policy, invocationOrder>>

AcceptConfirmation(execution) ==
  /\ phase[execution] = "awaiting"
  /\ phase' = [phase EXCEPT ![execution] = "running"]
  /\ handlerStarts' = [handlerStarts EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, committed, resultCount, invocationOrder>>

RejectConfirmation(execution) ==
  /\ phase[execution] = "awaiting"
  /\ phase' = [phase EXCEPT ![execution] = "cancelled"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, committed, handlerStarts, invocationOrder>>

Commit(execution) ==
  /\ phase[execution] \in {"running", "cancelling"}
  /\ committed' = committed \cup {execution}
  /\ UNCHANGED <<policy, phase, handlerStarts, resultCount, invocationOrder>>

Cancel(execution) ==
  /\ phase[execution] \in {"resolving", "awaiting", "running"}
  /\ IF phase[execution] = "running"
     THEN
        /\ phase' = [phase EXCEPT ![execution] = "cancelling"]
        /\ UNCHANGED resultCount
     ELSE
        /\ phase' = [phase EXCEPT ![execution] = "cancelled"]
        /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, committed, handlerStarts, invocationOrder>>

Finish(execution, outcome) ==
  /\ phase[execution] \in {"running", "cancelling"}
  /\ outcome \in {"completed", "unchanged", "failed"}
  /\ phase' = [phase EXCEPT
      ![execution] = IF phase[execution] = "cancelling" /\ execution \notin committed
        THEN "cancelled"
        ELSE outcome]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, committed, handlerStarts, invocationOrder>>

Next ==
  \/ \E execution \in Executions: Invoke(execution)
  \/ \E execution \in Executions: Resolve(execution)
  \/ \E execution \in Executions: AcceptConfirmation(execution)
  \/ \E execution \in Executions: RejectConfirmation(execution)
  \/ \E execution \in Executions: Commit(execution)
  \/ \E execution \in Executions: Cancel(execution)
  \/ \E execution \in Executions, outcome \in {"completed", "unchanged", "failed"}:
       Finish(execution, outcome)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ policy \in Policies
  /\ phase \in [Executions -> States]
  /\ committed \subseteq Executions
  /\ handlerStarts \in [Executions -> 0..1]
  /\ resultCount \in [Executions -> 0..1]
  /\ invocationOrder \in Seq(Executions)

ExactlyOneTerminalResult ==
  \A execution \in Executions:
    (phase[execution] \in Terminal) = (resultCount[execution] = 1)

AtMostOneHandler ==
  \A execution \in Executions: handlerStarts[execution] <= 1

CancellationBeforeCommitDoesNotComplete ==
  \A execution \in Executions:
    phase[execution] = "completed" => execution \in committed \/ handlerStarts[execution] = 1

CancelledHandlerDidNotCommit ==
  \A execution \in Executions:
    phase[execution] = "cancelled" /\ handlerStarts[execution] = 1 =>
      execution \notin committed

NonParallelExclusion ==
  policy # "parallel" =>
    Cardinality({execution \in Executions: phase[execution] \in {"awaiting", "running"}}) <= 1

SerializedInvocationOrder ==
  policy = "serialize" /\ Len(invocationOrder) = 2 =>
    LET first == invocationOrder[1]
        second == invocationOrder[2]
    IN phase[second] \in {"awaiting", "running", "cancelling"} =>
       phase[first] \in Terminal

=============================================================================
