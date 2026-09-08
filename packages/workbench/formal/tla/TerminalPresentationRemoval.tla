-------------------- MODULE TerminalPresentationRemoval --------------------
\* This bounded slice admits one whole-topology explicit removal or one
\* natural singleton removal. Abstract running quotients concrete
\* provisioning, restoring, reconciling, and running admission states.
\* Disjoint concurrent removals and survivor-priority choice are test-owned.
EXTENDS FiniteSets, Naturals, TLC

CONSTANT InitialShape

Presentations == {"primary", "child"}
InitialPresentations ==
  IF InitialShape = "primary-only" THEN {"primary"} ELSE Presentations
Phases == {"provisioning", "running", "provision-failed", "closing", "close-failed", "closed"}
NaturalRoles == {"none", "primary", "child"}
MaxAttempt == 2

ASSUME InitialShape \in {"primary-only", "primary-with-child"}

VARIABLES
  phase,
  provisionAttempt,
  closeAttempt,
  members,
  primaryPresentation,
  naturalRole,
  explicitTargets,
  topologyPresent,
  lastCompletionWasStale,
  previousPhase,
  previousMembers,
  previousPrimaryPresentation,
  previousNaturalRole,
  previousExplicitTargets,
  previousTopologyPresent

vars ==
  <<phase, provisionAttempt, closeAttempt, members, primaryPresentation,
    naturalRole, explicitTargets, topologyPresent, lastCompletionWasStale,
    previousPhase, previousMembers, previousPrimaryPresentation,
    previousNaturalRole, previousExplicitTargets,
    previousTopologyPresent>>

NaturalTargets ==
  {presentation \in Presentations : naturalRole[presentation] # "none"}

PendingTargets == explicitTargets \cup NaturalTargets

RecordNormal ==
  /\ lastCompletionWasStale' = FALSE
  /\ previousPhase' = phase
  /\ previousMembers' = members
  /\ previousPrimaryPresentation' = primaryPresentation
  /\ previousNaturalRole' = naturalRole
  /\ previousExplicitTargets' = explicitTargets
  /\ previousTopologyPresent' = topologyPresent

RecordStale ==
  /\ lastCompletionWasStale' = TRUE
  /\ previousPhase' = phase
  /\ previousMembers' = members
  /\ previousPrimaryPresentation' = primaryPresentation
  /\ previousNaturalRole' = naturalRole
  /\ previousExplicitTargets' = explicitTargets
  /\ previousTopologyPresent' = topologyPresent

Init ==
  /\ phase = [presentation \in Presentations |->
       IF presentation \in InitialPresentations THEN "provisioning" ELSE "closed"]
  /\ provisionAttempt = [presentation \in Presentations |->
       IF presentation \in InitialPresentations THEN 1 ELSE 0]
  /\ closeAttempt = [presentation \in Presentations |-> 0]
  /\ members = InitialPresentations
  /\ primaryPresentation = "primary"
  /\ naturalRole = [presentation \in Presentations |-> "none"]
  /\ explicitTargets = {}
  /\ topologyPresent = TRUE
  /\ lastCompletionWasStale = FALSE
  /\ previousPhase = phase
  /\ previousMembers = members
  /\ previousPrimaryPresentation = primaryPresentation
  /\ previousNaturalRole = naturalRole
  /\ previousExplicitTargets = explicitTargets
  /\ previousTopologyPresent = topologyPresent

IsCurrentProvision(presentation, candidateAttempt) ==
  /\ presentation \in members
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
       <<provisionAttempt, closeAttempt, members, primaryPresentation,
         naturalRole, explicitTargets, topologyPresent>>

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
       <<provisionAttempt, closeAttempt, members, primaryPresentation,
         naturalRole, explicitTargets, topologyPresent>>

RetryProvision(presentation) ==
  /\ presentation \in members
  /\ phase[presentation] = "provision-failed"
  /\ provisionAttempt[presentation] < MaxAttempt
  /\ phase' = [phase EXCEPT ![presentation] = "provisioning"]
  /\ provisionAttempt' = [provisionAttempt EXCEPT ![presentation] = @ + 1]
  /\ UNCHANGED
       <<closeAttempt, members, primaryPresentation, naturalRole,
         explicitTargets, topologyPresent>>
  /\ RecordNormal

RequestRemoval ==
  /\ PendingTargets = {}
  /\ members # {}
  /\ \A presentation \in members:
       phase[presentation] \notin {"closing", "close-failed", "closed"}
  /\ explicitTargets' = members
  /\ phase' = [presentation \in Presentations |->
       IF presentation \in members THEN "closing" ELSE phase[presentation]]
  /\ closeAttempt' = [presentation \in Presentations |->
       IF presentation \in members THEN 1 ELSE closeAttempt[presentation]]
  /\ UNCHANGED
       <<provisionAttempt, members, primaryPresentation, naturalRole,
         topologyPresent>>
  /\ RecordNormal

NaturalExit(presentation) ==
  /\ explicitTargets = {}
  /\ presentation \in members
  /\ presentation \notin NaturalTargets
  /\ phase[presentation] \in {"provisioning", "running"}
  /\ phase' = [phase EXCEPT ![presentation] = "closing"]
  /\ closeAttempt' = [closeAttempt EXCEPT ![presentation] = 1]
  /\ naturalRole' = [naturalRole EXCEPT
       ![presentation] = IF presentation = primaryPresentation THEN "primary" ELSE "child"]
  /\ UNCHANGED
       <<provisionAttempt, members, primaryPresentation, explicitTargets,
         topologyPresent>>
  /\ RecordNormal

RejectDuplicateRemoval ==
  /\ PendingTargets # {}
  /\ UNCHANGED
       <<phase, provisionAttempt, closeAttempt, members, primaryPresentation,
         naturalRole, explicitTargets, topologyPresent>>
  /\ RecordStale

IsCurrentClose(presentation, candidateAttempt) ==
  /\ presentation \in PendingTargets
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
       <<provisionAttempt, closeAttempt, members, primaryPresentation,
         naturalRole, explicitTargets, topologyPresent>>

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
       <<provisionAttempt, closeAttempt, members, primaryPresentation,
         naturalRole, explicitTargets, topologyPresent>>

RetryClose(presentation) ==
  /\ presentation \in PendingTargets
  /\ phase[presentation] = "close-failed"
  /\ closeAttempt[presentation] < MaxAttempt
  /\ phase' = [phase EXCEPT ![presentation] = "closing"]
  /\ closeAttempt' = [closeAttempt EXCEPT ![presentation] = @ + 1]
  /\ UNCHANGED
       <<provisionAttempt, members, primaryPresentation, naturalRole,
         explicitTargets, topologyPresent>>
  /\ RecordNormal

FinalizeExplicitRemoval ==
  /\ explicitTargets # {}
  /\ \A presentation \in explicitTargets: phase[presentation] = "closed"
  /\ members' = members \ explicitTargets
  /\ explicitTargets' = {}
  /\ topologyPresent' = FALSE
  /\ UNCHANGED
       <<phase, provisionAttempt, closeAttempt, primaryPresentation, naturalRole>>
  /\ RecordNormal

FinalizeNaturalChild(presentation) ==
  /\ naturalRole[presentation] = "child"
  /\ phase[presentation] = "closed"
  /\ members' = members \ {presentation}
  /\ naturalRole' = [naturalRole EXCEPT ![presentation] = "none"]
  /\ UNCHANGED
       <<phase, provisionAttempt, closeAttempt, primaryPresentation,
         explicitTargets, topologyPresent>>
  /\ RecordNormal

NaturalPrimarySurvivors(presentation) == members \ {presentation}

FinalizeNaturalPrimary(presentation) ==
  /\ naturalRole[presentation] = "primary"
  /\ phase[presentation] = "closed"
  /\ NaturalPrimarySurvivors(presentation) \cap PendingTargets = {}
  /\ members' = NaturalPrimarySurvivors(presentation)
  /\ primaryPresentation' =
       IF NaturalPrimarySurvivors(presentation) = {}
       THEN primaryPresentation
       ELSE CHOOSE survivor \in NaturalPrimarySurvivors(presentation): TRUE
  /\ naturalRole' = [naturalRole EXCEPT ![presentation] = "none"]
  /\ topologyPresent' = (NaturalPrimarySurvivors(presentation) # {})
  /\ UNCHANGED
       <<phase, provisionAttempt, closeAttempt, explicitTargets>>
  /\ RecordNormal

Next ==
  \/ RequestRemoval
  \/ RejectDuplicateRemoval
  \/ FinalizeExplicitRemoval
  \/ \E presentation \in Presentations: NaturalExit(presentation)
  \/ \E presentation \in Presentations: FinalizeNaturalChild(presentation)
  \/ \E presentation \in Presentations: FinalizeNaturalPrimary(presentation)
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
  /\ provisionAttempt \in [Presentations -> 0..MaxAttempt]
  /\ closeAttempt \in [Presentations -> 0..MaxAttempt]
  /\ members \subseteq Presentations
  /\ primaryPresentation \in Presentations
  /\ naturalRole \in [Presentations -> NaturalRoles]
  /\ explicitTargets \subseteq Presentations
  /\ topologyPresent \in BOOLEAN
  /\ lastCompletionWasStale \in BOOLEAN
  /\ previousPhase \in [Presentations -> Phases]
  /\ previousMembers \subseteq Presentations
  /\ previousPrimaryPresentation \in Presentations
  /\ previousNaturalRole \in [Presentations -> NaturalRoles]
  /\ previousExplicitTargets \subseteq Presentations
  /\ previousTopologyPresent \in BOOLEAN

TopologyMatchesMembership == topologyPresent <=> (members # {})

PrimaryResolves == topologyPresent => primaryPresentation \in members

PendingTargetsRemainMembers == PendingTargets \subseteq members

PendingTargetsHaveCloseLifecycle ==
  \A presentation \in PendingTargets:
    phase[presentation] \in {"closing", "close-failed", "closed"}

CloseLifecycleRequiresPendingRemoval ==
  \A presentation \in members:
    phase[presentation] \in {"closing", "close-failed", "closed"} =>
      presentation \in PendingTargets

ExplicitRemovalIsExclusive ==
  explicitTargets # {} =>
    /\ NaturalTargets = {}
    /\ explicitTargets = members

NaturalRolesMatchOwnership ==
  /\ \A presentation \in NaturalTargets:
       naturalRole[presentation] = "primary" => presentation = primaryPresentation
  /\ \A presentation \in NaturalTargets:
       naturalRole[presentation] = "child" => presentation # primaryPresentation

PartialFailureRetainsTarget ==
  \A presentation \in Presentations:
    phase[presentation] = "close-failed" =>
      /\ presentation \in members
      /\ presentation \in PendingTargets

StaleCompletionDoesNotMutate ==
  lastCompletionWasStale =>
    /\ phase = previousPhase
    /\ members = previousMembers
    /\ primaryPresentation = previousPrimaryPresentation
    /\ naturalRole = previousNaturalRole
    /\ explicitTargets = previousExplicitTargets
    /\ topologyPresent = previousTopologyPresent

=============================================================================
