-------------------- MODULE TerminalPresentationRemoval --------------------
EXTENDS FiniteSets, Naturals, TLC

Presentations == {"primary", "child"}
Phases == {"provisioning", "running", "provision-failed", "closing", "close-failed", "closed"}
MaxAttempt == 2

VARIABLES
  phase,
  provisionAttempt,
  closeAttempt,
  removalRequested,
  topologyPresent,
  finalized,
  lastCompletionWasStale,
  previousPhase,
  previousTopologyPresent

vars ==
  <<phase, provisionAttempt, closeAttempt, removalRequested, topologyPresent,
    finalized, lastCompletionWasStale, previousPhase, previousTopologyPresent>>

RecordNormal ==
  /\ lastCompletionWasStale' = FALSE
  /\ previousPhase' = phase
  /\ previousTopologyPresent' = topologyPresent

RecordStale ==
  /\ lastCompletionWasStale' = TRUE
  /\ previousPhase' = phase
  /\ previousTopologyPresent' = topologyPresent

Init ==
  /\ phase = [presentation \in Presentations |-> "provisioning"]
  /\ provisionAttempt = [presentation \in Presentations |-> 1]
  /\ closeAttempt = [presentation \in Presentations |-> 0]
  /\ removalRequested = FALSE
  /\ topologyPresent = TRUE
  /\ finalized = FALSE
  /\ lastCompletionWasStale = FALSE
  /\ previousPhase = phase
  /\ previousTopologyPresent = topologyPresent

IsCurrentProvision(presentation, candidateAttempt) ==
  /\ phase[presentation] = "provisioning"
  /\ provisionAttempt[presentation] = candidateAttempt

ProvisionCompletes(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ IF IsCurrentProvision(presentation, candidateAttempt)
     THEN
       /\ phase' = [phase EXCEPT ![presentation] = "running"]
       /\ RecordNormal
     ELSE
       /\ UNCHANGED phase
       /\ RecordStale
  /\ UNCHANGED
       <<provisionAttempt, closeAttempt, removalRequested, topologyPresent, finalized>>

ProvisionFails(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ IF IsCurrentProvision(presentation, candidateAttempt)
     THEN
       /\ phase' = [phase EXCEPT ![presentation] = "provision-failed"]
       /\ RecordNormal
     ELSE
       /\ UNCHANGED phase
       /\ RecordStale
  /\ UNCHANGED
       <<provisionAttempt, closeAttempt, removalRequested, topologyPresent, finalized>>

RetryProvision(presentation) ==
  /\ phase[presentation] = "provision-failed"
  /\ provisionAttempt[presentation] < MaxAttempt
  /\ phase' = [phase EXCEPT ![presentation] = "provisioning"]
  /\ provisionAttempt' = [provisionAttempt EXCEPT ![presentation] = @ + 1]
  /\ UNCHANGED <<closeAttempt, removalRequested, topologyPresent, finalized>>
  /\ RecordNormal

RequestRemoval ==
  /\ ~removalRequested
  /\ topologyPresent
  /\ \A presentation \in Presentations:
       phase[presentation] \notin {"closing", "close-failed", "closed"}
  /\ removalRequested' = TRUE
  /\ phase' = [presentation \in Presentations |-> "closing"]
  /\ closeAttempt' = [presentation \in Presentations |-> 1]
  /\ UNCHANGED <<provisionAttempt, topologyPresent, finalized>>
  /\ RecordNormal

IsCurrentClose(presentation, candidateAttempt) ==
  /\ removalRequested
  /\ phase[presentation] = "closing"
  /\ closeAttempt[presentation] = candidateAttempt

CloseCompletes(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ IF IsCurrentClose(presentation, candidateAttempt)
     THEN
       /\ phase' = [phase EXCEPT ![presentation] = "closed"]
       /\ RecordNormal
     ELSE
       /\ UNCHANGED phase
       /\ RecordStale
  /\ UNCHANGED
       <<provisionAttempt, closeAttempt, removalRequested, topologyPresent, finalized>>

CloseFails(presentation, candidateAttempt) ==
  /\ candidateAttempt \in 1..MaxAttempt
  /\ IF IsCurrentClose(presentation, candidateAttempt)
     THEN
       /\ phase' = [phase EXCEPT ![presentation] = "close-failed"]
       /\ RecordNormal
     ELSE
       /\ UNCHANGED phase
       /\ RecordStale
  /\ UNCHANGED
       <<provisionAttempt, closeAttempt, removalRequested, topologyPresent, finalized>>

RetryClose(presentation) ==
  /\ phase[presentation] = "close-failed"
  /\ closeAttempt[presentation] < MaxAttempt
  /\ phase' = [phase EXCEPT ![presentation] = "closing"]
  /\ closeAttempt' = [closeAttempt EXCEPT ![presentation] = @ + 1]
  /\ UNCHANGED
       <<provisionAttempt, removalRequested, topologyPresent, finalized>>
  /\ RecordNormal

FinalizeRemoval ==
  /\ removalRequested
  /\ topologyPresent
  /\ \A presentation \in Presentations: phase[presentation] = "closed"
  /\ topologyPresent' = FALSE
  /\ finalized' = TRUE
  /\ UNCHANGED <<phase, provisionAttempt, closeAttempt, removalRequested>>
  /\ RecordNormal

Next ==
  \/ RequestRemoval
  \/ FinalizeRemoval
  \/ \E presentation \in Presentations: RetryProvision(presentation)
  \/ \E presentation \in Presentations: RetryClose(presentation)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       ProvisionCompletes(presentation, candidateAttempt)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       ProvisionFails(presentation, candidateAttempt)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       CloseCompletes(presentation, candidateAttempt)
  \/ \E presentation \in Presentations, candidateAttempt \in 1..MaxAttempt:
       CloseFails(presentation, candidateAttempt)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in [Presentations -> Phases]
  /\ provisionAttempt \in [Presentations -> 1..MaxAttempt]
  /\ closeAttempt \in [Presentations -> 0..MaxAttempt]
  /\ removalRequested \in BOOLEAN
  /\ topologyPresent \in BOOLEAN
  /\ finalized \in BOOLEAN
  /\ lastCompletionWasStale \in BOOLEAN
  /\ previousPhase \in [Presentations -> Phases]
  /\ previousTopologyPresent \in BOOLEAN

RemovalIsAtomic ==
  ~topologyPresent => \A presentation \in Presentations: phase[presentation] = "closed"

PartialFailureRetainsTopology ==
  (\E presentation \in Presentations: phase[presentation] = "close-failed") => topologyPresent

FinalizedMatchesTopology == finalized <=> ~topologyPresent

StaleCompletionDoesNotMutate ==
  lastCompletionWasStale =>
    /\ phase = previousPhase
    /\ topologyPresent = previousTopologyPresent

=============================================================================
