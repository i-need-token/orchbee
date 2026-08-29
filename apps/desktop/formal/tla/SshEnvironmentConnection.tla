------------------------ MODULE SshEnvironmentConnection ------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction for one Desktop-owned, process-local SSH Environment
\* connection. OpenSSH behavior, credentials, target parsing, remote-runtime
\* implementation, Projects, and durable Environment records remain outside.

CONSTANT MaxGeneration, MaxRetryAttempt

NoGeneration == 0
Phases == {
  "stopped", "resolving", "connecting", "interaction",
  "connected", "retry-wait", "blocked"
}
Actions == {
  "init", "start", "repeat-start", "resolve-target", "resolution-fails",
  "request-interaction", "repeat-interaction", "complete-interaction",
  "cancel-interaction", "ready", "initial-transport-fails",
  "permanent-transport-fails", "lose-connected", "retry-elapsed",
  "retry-transport-fails", "stop", "repeat-stop", "stale-ready",
  "stale-failure", "stale-timer", "stale-interaction"
}

VARIABLES
  phase,
  generation,
  activeGenerations,
  retryGenerations,
  interactionGenerations,
  readyGenerations,
  connectedOnce,
  retryAttempt,
  remoteWorkRetained,
  lastAction

vars == <<phase, generation, activeGenerations, retryGenerations,
  interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
  remoteWorkRetained, lastAction>>

Init ==
  /\ phase = "stopped"
  /\ generation = NoGeneration
  /\ activeGenerations = {}
  /\ retryGenerations = {}
  /\ interactionGenerations = {}
  /\ readyGenerations = {}
  /\ connectedOnce = FALSE
  /\ retryAttempt = 0
  /\ remoteWorkRetained = TRUE
  /\ lastAction = "init"

Start ==
  /\ phase \in {"stopped", "blocked"}
  /\ generation < MaxGeneration
  /\ phase' = "resolving"
  /\ generation' = generation + 1
  /\ activeGenerations' = {}
  /\ retryGenerations' = {}
  /\ interactionGenerations' = {}
  /\ retryAttempt' = 0
  /\ UNCHANGED <<readyGenerations, connectedOnce, remoteWorkRetained>>
  /\ lastAction' = "start"

RepeatStart ==
  /\ phase \in {"resolving", "connecting", "interaction", "connected", "retry-wait"}
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "repeat-start"

ResolveTarget ==
  /\ phase = "resolving"
  /\ phase' = "connecting"
  /\ activeGenerations' = {generation}
  /\ UNCHANGED <<generation, retryGenerations, interactionGenerations,
       readyGenerations, connectedOnce, retryAttempt, remoteWorkRetained>>
  /\ lastAction' = "resolve-target"

ResolutionFails ==
  /\ phase = "resolving"
  /\ phase' = "blocked"
  /\ activeGenerations' = {}
  /\ retryGenerations' = {}
  /\ interactionGenerations' = {}
  /\ UNCHANGED <<generation, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "resolution-fails"

RequestInteraction ==
  /\ phase = "connecting"
  /\ phase' = "interaction"
  /\ interactionGenerations' = {generation}
  /\ UNCHANGED <<generation, activeGenerations, retryGenerations,
       readyGenerations, connectedOnce, retryAttempt, remoteWorkRetained>>
  /\ lastAction' = "request-interaction"

RepeatInteraction ==
  /\ phase = "interaction"
  /\ interactionGenerations = {generation}
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "repeat-interaction"

CompleteInteraction ==
  /\ phase = "interaction"
  /\ interactionGenerations = {generation}
  /\ phase' = "connecting"
  /\ interactionGenerations' = {}
  /\ UNCHANGED <<generation, activeGenerations, retryGenerations,
       readyGenerations, connectedOnce, retryAttempt, remoteWorkRetained>>
  /\ lastAction' = "complete-interaction"

CancelInteraction ==
  /\ phase = "interaction"
  /\ phase' = "blocked"
  /\ activeGenerations' = {}
  /\ retryGenerations' = {}
  /\ interactionGenerations' = {}
  /\ UNCHANGED <<generation, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "cancel-interaction"

Ready ==
  /\ phase = "connecting"
  /\ activeGenerations = {generation}
  /\ phase' = "connected"
  /\ readyGenerations' = readyGenerations \cup {generation}
  /\ connectedOnce' = TRUE
  /\ UNCHANGED <<generation, activeGenerations, retryGenerations,
       interactionGenerations, retryAttempt, remoteWorkRetained>>
  /\ lastAction' = "ready"

InitialTransportFails ==
  /\ phase = "connecting"
  /\ retryAttempt = 0
  /\ phase' = "blocked"
  /\ activeGenerations' = {}
  /\ retryGenerations' = {}
  /\ interactionGenerations' = {}
  /\ UNCHANGED <<generation, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "initial-transport-fails"

PermanentTransportFails ==
  /\ phase \in {"connecting", "interaction"}
  /\ phase' = "blocked"
  /\ activeGenerations' = {}
  /\ retryGenerations' = {}
  /\ interactionGenerations' = {}
  /\ UNCHANGED <<generation, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "permanent-transport-fails"

LoseConnected ==
  /\ phase = "connected"
  /\ generation < MaxGeneration
  /\ phase' = "retry-wait"
  /\ generation' = generation + 1
  /\ activeGenerations' = {}
  /\ retryGenerations' = {generation + 1}
  /\ interactionGenerations' = {}
  /\ retryAttempt' = 1
  /\ UNCHANGED <<readyGenerations, connectedOnce, remoteWorkRetained>>
  /\ lastAction' = "lose-connected"

RetryElapsed ==
  /\ phase = "retry-wait"
  /\ retryGenerations = {generation}
  /\ phase' = "resolving"
  /\ retryGenerations' = {}
  /\ UNCHANGED <<generation, activeGenerations, interactionGenerations,
       readyGenerations, connectedOnce, retryAttempt, remoteWorkRetained>>
  /\ lastAction' = "retry-elapsed"

RetryTransportFails ==
  /\ phase = "connecting"
  /\ connectedOnce
  /\ retryAttempt > 0
  /\ generation < MaxGeneration
  /\ phase' = "retry-wait"
  /\ generation' = generation + 1
  /\ activeGenerations' = {}
  /\ retryGenerations' = {generation + 1}
  /\ interactionGenerations' = {}
  /\ retryAttempt' = IF retryAttempt < MaxRetryAttempt THEN retryAttempt + 1 ELSE retryAttempt
  /\ UNCHANGED <<readyGenerations, connectedOnce, remoteWorkRetained>>
  /\ lastAction' = "retry-transport-fails"

Stop ==
  /\ phase # "stopped"
  /\ phase' = "stopped"
  /\ generation' = IF generation < MaxGeneration THEN generation + 1 ELSE generation
  /\ activeGenerations' = {}
  /\ retryGenerations' = {}
  /\ interactionGenerations' = {}
  /\ retryAttempt' = 0
  /\ UNCHANGED <<readyGenerations, connectedOnce, remoteWorkRetained>>
  /\ lastAction' = "stop"

RepeatStop ==
  /\ phase = "stopped"
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "repeat-stop"

StaleReady(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "stale-ready"

StaleFailure(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "stale-failure"

StaleTimer(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "stale-timer"

StaleInteraction(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, activeGenerations, retryGenerations,
       interactionGenerations, readyGenerations, connectedOnce, retryAttempt,
       remoteWorkRetained>>
  /\ lastAction' = "stale-interaction"

Next ==
  \/ Start
  \/ RepeatStart
  \/ ResolveTarget
  \/ ResolutionFails
  \/ RequestInteraction
  \/ RepeatInteraction
  \/ CompleteInteraction
  \/ CancelInteraction
  \/ Ready
  \/ InitialTransportFails
  \/ PermanentTransportFails
  \/ LoseConnected
  \/ RetryElapsed
  \/ RetryTransportFails
  \/ Stop
  \/ RepeatStop
  \/ \E staleGeneration \in 0..MaxGeneration : StaleReady(staleGeneration)
  \/ \E staleGeneration \in 0..MaxGeneration : StaleFailure(staleGeneration)
  \/ \E staleGeneration \in 0..MaxGeneration : StaleTimer(staleGeneration)
  \/ \E staleGeneration \in 0..MaxGeneration : StaleInteraction(staleGeneration)

Spec == Init /\ [][Next]_vars /\ WF_vars(RetryElapsed)

TypeOK ==
  /\ phase \in Phases
  /\ generation \in 0..MaxGeneration
  /\ activeGenerations \subseteq 1..MaxGeneration
  /\ retryGenerations \subseteq 1..MaxGeneration
  /\ interactionGenerations \subseteq 1..MaxGeneration
  /\ readyGenerations \subseteq 1..MaxGeneration
  /\ connectedOnce \in BOOLEAN
  /\ retryAttempt \in 0..MaxRetryAttempt
  /\ remoteWorkRetained \in BOOLEAN
  /\ lastAction \in Actions

SingleActiveGeneration == Cardinality(activeGenerations) <= 1
CurrentGenerationOwnsTransport == activeGenerations \subseteq {generation}
ConnectedRequiresReadyHandshake == phase # "connected" \/ generation \in readyGenerations
RetryRequiresPriorReady == phase # "retry-wait" \/ connectedOnce
SingleRetryTimer == Cardinality(retryGenerations) <= 1
CurrentGenerationOwnsRetry == retryGenerations \subseteq {generation}
CurrentGenerationOwnsInteraction == interactionGenerations \subseteq {generation}
BlockedIsQuiescent ==
  phase # "blocked" \/
    (activeGenerations = {} /\ retryGenerations = {} /\ interactionGenerations = {})
StoppedIsQuiescent ==
  phase # "stopped" \/
    (activeGenerations = {} /\ retryGenerations = {} /\ interactionGenerations = {})
RemoteWorkRetained == remoteWorkRetained
ReadyHistoryAgrees == connectedOnce = (readyGenerations # {})

RetryWaitEventuallyLeaves == [](phase = "retry-wait" ~> phase # "retry-wait")

=============================================================================
