---------------------- MODULE AgentSemanticWait ----------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction for Desktop's server-owned Agent Session wait.
\* Provider observation, semantic-command receipt retention, transport,
\* and durable restart recovery remain outside this model.

CONSTANT Sessions, StatusConditions, HealthConditions, MaxRevision,
  MaxGeneration

Conditions == StatusConditions \union HealthConditions
WaitConditionSets ==
  {{condition} : condition \in Conditions} \union {Conditions}
WaitPhases == {
  "absent", "waiting", "matched", "timed-out", "cancelled",
  "missing", "revision-ahead", "coordinator-retired"
}
NoSession == "none"
NoConditionSet == {}
NoValue == "none"

VARIABLES
  revision,
  agentMutations,
  presentSessions,
  sessionStatus,
  sessionFreshness,
  coordinatorGeneration,
  waitPhase,
  waitTarget,
  waitConditions,
  requireNewerRevision,
  baselineRevision,
  subscribed,
  matchedTarget,
  matchedConditions,
  matchedStatus,
  matchedFreshness,
  matchedRevision,
  matchedGeneration,
  lastAction

vars == <<revision, agentMutations, presentSessions, sessionStatus,
  sessionFreshness, coordinatorGeneration, waitPhase, waitTarget,
  waitConditions, requireNewerRevision, baselineRevision, subscribed,
  matchedTarget, matchedConditions, matchedStatus, matchedFreshness,
  matchedRevision, matchedGeneration, lastAction>>

ConditionMatches(status, freshness, conditions) ==
  \/ freshness \in conditions
  \/ /\ freshness = "fresh"
     /\ status \in conditions

EligibleSnapshot(target, conditions, requireNewer, baseline,
    present, statuses, freshnesses, currentRevision) ==
  /\ target \in present
  /\ ~requireNewer \/ currentRevision > baseline
  /\ ConditionMatches(statuses[target], freshnesses[target], conditions)

CurrentEligible ==
  waitTarget \in Sessions /\
  EligibleSnapshot(waitTarget, waitConditions, requireNewerRevision,
    baselineRevision, presentSessions, sessionStatus, sessionFreshness,
    revision)

MatchingConditions(target, conditions, statuses, freshnesses) ==
  {condition \in conditions :
    \/ condition = freshnesses[target]
    \/ /\ freshnesses[target] = "fresh"
       /\ condition = statuses[target]}

Init ==
  /\ revision = 0
  /\ agentMutations = 0
  /\ presentSessions = Sessions
  /\ sessionStatus = [session \in Sessions |-> "working"]
  /\ sessionFreshness = [session \in Sessions |-> "fresh"]
  /\ coordinatorGeneration = 1
  /\ waitPhase = "absent"
  /\ waitTarget = NoSession
  /\ waitConditions = NoConditionSet
  /\ requireNewerRevision = FALSE
  /\ baselineRevision = 0
  /\ subscribed = FALSE
  /\ matchedTarget = NoSession
  /\ matchedConditions = NoConditionSet
  /\ matchedStatus = NoValue
  /\ matchedFreshness = NoValue
  /\ matchedRevision = 0
  /\ matchedGeneration = 0
  /\ lastAction = "init"

BeginWait(target, conditions, requireNewer, baseline) ==
  /\ waitPhase = "absent"
  /\ target \in presentSessions
  /\ conditions \in WaitConditionSets
  /\ requireNewer \in BOOLEAN
  /\ baseline \in 0..revision
  /\ ~requireNewer => baseline = 0
  /\ waitTarget' = target
  /\ waitConditions' = conditions
  /\ requireNewerRevision' = requireNewer
  /\ baselineRevision' = baseline
  /\ IF EligibleSnapshot(target, conditions, requireNewer, baseline,
         presentSessions, sessionStatus, sessionFreshness, revision)
     THEN /\ waitPhase' = "matched"
          /\ subscribed' = FALSE
          /\ matchedTarget' = target
          /\ matchedConditions' =
               MatchingConditions(target, conditions, sessionStatus,
                 sessionFreshness)
          /\ matchedStatus' = sessionStatus[target]
          /\ matchedFreshness' = sessionFreshness[target]
          /\ matchedRevision' = revision
          /\ matchedGeneration' = coordinatorGeneration
     ELSE /\ waitPhase' = "waiting"
          /\ subscribed' = TRUE
          /\ UNCHANGED <<matchedTarget, matchedConditions, matchedStatus,
               matchedFreshness, matchedRevision, matchedGeneration>>
  /\ UNCHANGED <<revision, agentMutations, presentSessions, sessionStatus,
       sessionFreshness, coordinatorGeneration>>
  /\ lastAction' = "begin-wait"

BeginMissing(target) ==
  /\ waitPhase = "absent"
  /\ target \in Sessions \ presentSessions
  /\ waitPhase' = "missing"
  /\ waitTarget' = target
  /\ waitConditions' = Conditions
  /\ requireNewerRevision' = FALSE
  /\ baselineRevision' = 0
  /\ subscribed' = FALSE
  /\ UNCHANGED <<revision, agentMutations, presentSessions, sessionStatus,
       sessionFreshness, coordinatorGeneration, matchedTarget,
       matchedConditions, matchedStatus, matchedFreshness, matchedRevision,
       matchedGeneration>>
  /\ lastAction' = "begin-missing"

BeginAhead(target, baseline) ==
  /\ waitPhase = "absent"
  /\ target \in presentSessions
  /\ baseline \in (revision + 1)..MaxRevision
  /\ waitPhase' = "revision-ahead"
  /\ waitTarget' = target
  /\ waitConditions' = Conditions
  /\ requireNewerRevision' = TRUE
  /\ baselineRevision' = baseline
  /\ subscribed' = FALSE
  /\ UNCHANGED <<revision, agentMutations, presentSessions, sessionStatus,
       sessionFreshness, coordinatorGeneration, matchedTarget,
       matchedConditions, matchedStatus, matchedFreshness, matchedRevision,
       matchedGeneration>>
  /\ lastAction' = "begin-ahead"

AgentUpdate(session, status, freshness) ==
  /\ revision < MaxRevision
  /\ session \in presentSessions
  /\ status \in StatusConditions
  /\ freshness \in HealthConditions
  /\ LET nextRevision == revision + 1
         nextStatus == [sessionStatus EXCEPT ![session] = status]
         nextFreshness == [sessionFreshness EXCEPT ![session] = freshness]
         matches == waitPhase = "waiting" /\
           EligibleSnapshot(waitTarget, waitConditions,
             requireNewerRevision, baselineRevision, presentSessions,
             nextStatus, nextFreshness, nextRevision)
     IN
       /\ revision' = nextRevision
       /\ agentMutations' = agentMutations + 1
       /\ sessionStatus' = nextStatus
       /\ sessionFreshness' = nextFreshness
       /\ IF matches
          THEN /\ waitPhase' = "matched"
               /\ subscribed' = FALSE
               /\ matchedTarget' = waitTarget
               /\ matchedConditions' = MatchingConditions(waitTarget,
                    waitConditions, nextStatus, nextFreshness)
               /\ matchedStatus' = nextStatus[waitTarget]
               /\ matchedFreshness' = nextFreshness[waitTarget]
               /\ matchedRevision' = nextRevision
               /\ matchedGeneration' = coordinatorGeneration
          ELSE /\ UNCHANGED <<waitPhase, subscribed, matchedTarget,
                    matchedConditions, matchedStatus, matchedFreshness,
                    matchedRevision, matchedGeneration>>
  /\ UNCHANGED <<presentSessions, coordinatorGeneration, waitTarget,
       waitConditions, requireNewerRevision, baselineRevision>>
  /\ lastAction' = "agent-update"

RemoveSession(session) ==
  /\ revision < MaxRevision
  /\ session \in presentSessions
  /\ LET nextRevision == revision + 1
         nextPresent == presentSessions \ {session}
         missing == waitPhase = "waiting" /\ waitTarget = session
         matches == waitPhase = "waiting" /\ ~missing /\
           EligibleSnapshot(waitTarget, waitConditions,
             requireNewerRevision, baselineRevision, nextPresent,
             sessionStatus, sessionFreshness, nextRevision)
     IN
       /\ revision' = nextRevision
       /\ agentMutations' = agentMutations + 1
       /\ presentSessions' = nextPresent
       /\ IF missing
          THEN /\ waitPhase' = "missing"
               /\ subscribed' = FALSE
               /\ UNCHANGED <<matchedTarget, matchedConditions,
                    matchedStatus, matchedFreshness, matchedRevision,
                    matchedGeneration>>
          ELSE IF matches
          THEN /\ waitPhase' = "matched"
               /\ subscribed' = FALSE
               /\ matchedTarget' = waitTarget
               /\ matchedConditions' = MatchingConditions(waitTarget,
                    waitConditions, sessionStatus, sessionFreshness)
               /\ matchedStatus' = sessionStatus[waitTarget]
               /\ matchedFreshness' = sessionFreshness[waitTarget]
               /\ matchedRevision' = nextRevision
               /\ matchedGeneration' = coordinatorGeneration
          ELSE /\ UNCHANGED <<waitPhase, subscribed, matchedTarget,
                    matchedConditions, matchedStatus, matchedFreshness,
                    matchedRevision, matchedGeneration>>
  /\ UNCHANGED <<sessionStatus, sessionFreshness, coordinatorGeneration,
       waitTarget, waitConditions, requireNewerRevision, baselineRevision>>
  /\ lastAction' = "remove-session"

Timeout ==
  /\ waitPhase = "waiting"
  /\ ~CurrentEligible
  /\ waitPhase' = "timed-out"
  /\ subscribed' = FALSE
  /\ UNCHANGED <<revision, agentMutations, presentSessions, sessionStatus,
       sessionFreshness, coordinatorGeneration, waitTarget, waitConditions,
       requireNewerRevision, baselineRevision, matchedTarget,
       matchedConditions, matchedStatus, matchedFreshness, matchedRevision,
       matchedGeneration>>
  /\ lastAction' = "timeout"

Cancel ==
  /\ waitPhase = "waiting"
  /\ waitPhase' = "cancelled"
  /\ subscribed' = FALSE
  /\ UNCHANGED <<revision, agentMutations, presentSessions, sessionStatus,
       sessionFreshness, coordinatorGeneration, waitTarget, waitConditions,
       requireNewerRevision, baselineRevision, matchedTarget,
       matchedConditions, matchedStatus, matchedFreshness, matchedRevision,
       matchedGeneration>>
  /\ lastAction' = "cancel"

CoordinatorRetire ==
  /\ coordinatorGeneration < MaxGeneration
  /\ coordinatorGeneration' = coordinatorGeneration + 1
  /\ IF waitPhase = "waiting"
     THEN /\ waitPhase' = "coordinator-retired"
          /\ subscribed' = FALSE
     ELSE /\ UNCHANGED <<waitPhase, subscribed>>
  /\ UNCHANGED <<revision, agentMutations, presentSessions, sessionStatus,
       sessionFreshness, waitTarget, waitConditions, requireNewerRevision,
       baselineRevision, matchedTarget, matchedConditions, matchedStatus,
       matchedFreshness, matchedRevision, matchedGeneration>>
  /\ lastAction' = "coordinator-retire"

Next ==
  \/ \E target \in Sessions, conditions \in WaitConditionSets,
       requireNewer \in BOOLEAN, baseline \in 0..MaxRevision:
       BeginWait(target, conditions, requireNewer, baseline)
  \/ \E target \in Sessions: BeginMissing(target)
  \/ \E target \in Sessions, baseline \in 0..MaxRevision:
       BeginAhead(target, baseline)
  \/ \E session \in Sessions, status \in StatusConditions,
       freshness \in HealthConditions: AgentUpdate(session, status, freshness)
  \/ \E session \in Sessions: RemoveSession(session)
  \/ Timeout
  \/ Cancel
  \/ CoordinatorRetire

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ Sessions # {}
  /\ StatusConditions # {}
  /\ HealthConditions = {"fresh", "stale", "disconnected"}
  /\ revision \in 0..MaxRevision
  /\ agentMutations \in 0..MaxRevision
  /\ presentSessions \subseteq Sessions
  /\ sessionStatus \in [Sessions -> StatusConditions]
  /\ sessionFreshness \in [Sessions -> HealthConditions]
  /\ coordinatorGeneration \in 1..MaxGeneration
  /\ waitPhase \in WaitPhases
  /\ waitTarget \in Sessions \union {NoSession}
  /\ waitConditions \subseteq Conditions
  /\ requireNewerRevision \in BOOLEAN
  /\ baselineRevision \in 0..MaxRevision
  /\ subscribed \in BOOLEAN
  /\ matchedTarget \in Sessions \union {NoSession}
  /\ matchedConditions \subseteq Conditions
  /\ matchedStatus \in StatusConditions \union {NoValue}
  /\ matchedFreshness \in HealthConditions \union {NoValue}
  /\ matchedRevision \in 0..MaxRevision
  /\ matchedGeneration \in 0..MaxGeneration

RuntimeRevisionHasOneOwner == revision = agentMutations

WaitingHasExactSubscription ==
  subscribed = (waitPhase = "waiting")

WaitingSnapshotIsNotAlreadyEligible ==
  waitPhase = "waiting" => ~CurrentEligible

MatchedSnapshotIsEligible ==
  waitPhase = "matched" =>
    /\ matchedTarget = waitTarget
    /\ matchedConditions # {}
    /\ matchedConditions = MatchingConditions(matchedTarget,
         waitConditions, [session \in Sessions |->
           IF session = matchedTarget THEN matchedStatus ELSE sessionStatus[session]],
         [session \in Sessions |->
           IF session = matchedTarget THEN matchedFreshness ELSE sessionFreshness[session]])
    /\ ~requireNewerRevision \/ matchedRevision > baselineRevision
    /\ matchedGeneration >= 1

SemanticMatchRequiresFresh ==
  waitPhase = "matched" =>
    \A condition \in matchedConditions:
      condition \in StatusConditions => matchedFreshness = "fresh"

UnmatchedWaitHasNoInventedOutput ==
  waitPhase # "matched" =>
    /\ matchedTarget = NoSession
    /\ matchedConditions = {}
    /\ matchedStatus = NoValue
    /\ matchedFreshness = NoValue
    /\ matchedRevision = 0
    /\ matchedGeneration = 0

======================================================================
