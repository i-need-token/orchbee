---------------------- MODULE TerminalPresentationStop ----------------------
EXTENDS FiniteSets, Naturals, TLC

Presentations == {"visible", "hidden"}
Intents == {"retain", "stop", "stopped"}
Phases == {"running", "stopping", "stop-failed", "stopped"}
BindingStates == {"present", "absent"}
WorkflowPhases == {"idle", "persisting", "closing", "failed", "complete"}
MaxAttempt == 2

VARIABLES
  intent,
  phase,
  binding,
  attempt,
  closeIssued,
  sealed,
  workflow,
  writable,
  layoutPresent,
  identityGeneration,
  lastAction,
  previousIntent,
  previousBinding,
  previousIdentityGeneration

vars == <<intent, phase, binding, attempt, closeIssued, sealed, workflow,
  writable, layoutPresent, identityGeneration, lastAction, previousIntent,
  previousBinding, previousIdentityGeneration>>

Record(action) ==
  /\ lastAction' = action
  /\ previousIntent' = intent
  /\ previousBinding' = binding
  /\ previousIdentityGeneration' = identityGeneration

Init ==
  /\ intent = [presentation \in Presentations |-> "retain"]
  /\ phase = [presentation \in Presentations |-> "running"]
  /\ binding = [presentation \in Presentations |-> "present"]
  /\ attempt = [presentation \in Presentations |-> 0]
  /\ closeIssued = [presentation \in Presentations |-> FALSE]
  /\ sealed = FALSE
  /\ workflow = "idle"
  /\ writable \in BOOLEAN
  /\ layoutPresent = [presentation \in Presentations |-> TRUE]
  /\ identityGeneration = [presentation \in Presentations |-> 0]
  /\ lastAction = "init"
  /\ previousIntent = intent
  /\ previousBinding = binding
  /\ previousIdentityGeneration = identityGeneration

RequestStop ==
  /\ workflow = "idle"
  /\ sealed' = TRUE
  /\ phase' = [presentation \in Presentations |-> "stopping"]
  /\ attempt' = [presentation \in Presentations |-> 1]
  /\ workflow' = "persisting"
  /\ UNCHANGED <<intent, binding, closeIssued, writable, layoutPresent,
       identityGeneration>>
  /\ Record("request-stop")

CommitStop ==
  /\ workflow = "persisting"
  /\ writable
  /\ intent' = [presentation \in Presentations |-> "stop"]
  /\ workflow' = "closing"
  /\ UNCHANGED <<phase, binding, attempt, closeIssued, sealed, writable,
       layoutPresent, identityGeneration>>
  /\ Record("commit-stop")

RejectStopCommit ==
  /\ workflow = "persisting"
  /\ ~writable
  /\ phase' = [presentation \in Presentations |-> "running"]
  /\ attempt' = [presentation \in Presentations |-> 0]
  /\ sealed' = FALSE
  /\ workflow' = "idle"
  /\ UNCHANGED <<intent, binding, closeIssued, writable, layoutPresent,
       identityGeneration>>
  /\ Record("reject-stop")

DispatchClose(presentation) ==
  /\ workflow \in {"closing", "failed"}
  /\ phase[presentation] = "stopping"
  /\ intent[presentation] = "stop"
  /\ ~closeIssued[presentation]
  /\ closeIssued' = [closeIssued EXCEPT ![presentation] = TRUE]
  /\ UNCHANGED <<intent, phase, binding, attempt, sealed, workflow, writable,
       layoutPresent, identityGeneration>>
  /\ Record("dispatch-close")

CloseCompletes(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ IF /\ phase[presentation] = "stopping"
        /\ attempt[presentation] = candidateAttempt
        /\ closeIssued[presentation]
     THEN
       /\ phase' = [phase EXCEPT ![presentation] = "stopped"]
       /\ binding' = [binding EXCEPT ![presentation] = "absent"]
       /\ closeIssued' = [closeIssued EXCEPT ![presentation] = FALSE]
     ELSE
       /\ UNCHANGED <<phase, binding, closeIssued>>
  /\ UNCHANGED <<intent, attempt, sealed, workflow, writable, layoutPresent,
       identityGeneration>>
  /\ Record("close-completed")

CloseFails(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ phase[presentation] = "stopping"
  /\ attempt[presentation] = candidateAttempt
  /\ closeIssued[presentation]
  /\ phase' = [phase EXCEPT ![presentation] = "stop-failed"]
  /\ closeIssued' = [closeIssued EXCEPT ![presentation] = FALSE]
  /\ workflow' = "failed"
  /\ UNCHANGED <<intent, binding, attempt, sealed, writable, layoutPresent,
       identityGeneration>>
  /\ Record("close-failed")

RetryClose(presentation) ==
  /\ workflow = "failed"
  /\ phase[presentation] = "stop-failed"
  /\ attempt[presentation] < MaxAttempt
  /\ phase' = [phase EXCEPT ![presentation] = "stopping"]
  /\ attempt' = [attempt EXCEPT ![presentation] = @ + 1]
  /\ workflow' = "closing"
  /\ UNCHANGED <<intent, binding, closeIssued, sealed, writable,
       layoutPresent, identityGeneration>>
  /\ Record("retry-close")

FinishStop ==
  /\ workflow \in {"closing", "failed"}
  /\ \A presentation \in Presentations:
       phase[presentation] = "stopped" /\ binding[presentation] = "absent"
  /\ intent' = [presentation \in Presentations |-> "stopped"]
  /\ workflow' = "complete"
  /\ UNCHANGED <<phase, binding, attempt, closeIssued, sealed, writable,
       layoutPresent, identityGeneration>>
  /\ Record("finish-stop")

StartFresh(presentation) ==
  /\ workflow = "complete"
  /\ phase[presentation] = "stopped"
  /\ intent[presentation] = "stopped"
  /\ phase' = [phase EXCEPT ![presentation] = "running"]
  /\ intent' = [intent EXCEPT ![presentation] = "retain"]
  /\ binding' = [binding EXCEPT ![presentation] = "present"]
  /\ identityGeneration' = [identityGeneration EXCEPT ![presentation] = @ + 1]
  /\ UNCHANGED <<attempt, closeIssued, sealed, workflow, writable,
       layoutPresent>>
  /\ Record("start-fresh")

Next ==
  \/ RequestStop
  \/ CommitStop
  \/ RejectStopCommit
  \/ FinishStop
  \/ \E presentation \in Presentations: DispatchClose(presentation)
  \/ \E presentation \in Presentations: RetryClose(presentation)
  \/ \E presentation \in Presentations: StartFresh(presentation)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       CloseCompletes(presentation, candidateAttempt)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       CloseFails(presentation, candidateAttempt)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ intent \in [Presentations -> Intents]
  /\ phase \in [Presentations -> Phases]
  /\ binding \in [Presentations -> BindingStates]
  /\ attempt \in [Presentations -> 0..MaxAttempt]
  /\ closeIssued \in [Presentations -> BOOLEAN]
  /\ sealed \in BOOLEAN
  /\ workflow \in WorkflowPhases
  /\ writable \in BOOLEAN
  /\ layoutPresent \in [Presentations -> BOOLEAN]
  /\ identityGeneration \in [Presentations -> Nat]

LayoutIsPreserved == \A presentation \in Presentations: layoutPresent[presentation]

CloseRequiresDurableStop ==
  lastAction = "dispatch-close" =>
    \E presentation \in Presentations:
      previousIntent[presentation] = "stop" /\
      previousBinding[presentation] = "present"

StoppedHasNoBinding ==
  \A presentation \in Presentations:
    phase[presentation] = "stopped" => binding[presentation] = "absent"

FreshStartChangesIdentity ==
  lastAction = "start-fresh" =>
    \E presentation \in Presentations:
      identityGeneration[presentation] > previousIdentityGeneration[presentation]

FailedCommitHasNoCloseEffect ==
  lastAction = "reject-stop" => binding = previousBinding

=============================================================================
