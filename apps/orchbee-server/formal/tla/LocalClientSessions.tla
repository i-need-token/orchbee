------------------------- MODULE LocalClientSessions -------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the additive same-user local Human Client admission boundary. Each
\* accepted transport owns one independent Client Session generation and one
\* bounded handler slot. Slow candidates consume handler capacity without
\* consuming Session capacity. Post-admission OBCS stream behavior refines an
\* active generation in LocalClientObcsSessions; it is intentionally separate
\* so carrier state cannot multiply this admission model's state space.

CONSTANTS Clients, MaxSessions, MaxHandlers, MaxGeneration

ServerPhases == {"absent", "listening", "stopped"}
ClientValues == Clients \cup {"none"}
\* Named rejection actions retain the abstract classification boundary and
\* authority-preservation checks. Exact typed or silent wire output is owned by
\* deterministic protocol and service tests rather than additional model state.
Actions == {
  "init", "start-server", "register-worker", "begin-slow",
  "reject-wrong-user", "reject-malformed", "reject-incompatible-protocol",
  "reject-incompatible-cohort", "reject-wrong-message", "reject-capacity",
  "reject-handler-saturation", "handshake-timeout", "admit-client",
  "client-disconnect", "duplicate-hello", "stale-session-cleanup",
  "stop-server"
}

ASSUME
  /\ IsFiniteSet(Clients)
  /\ Cardinality(Clients) >= MaxHandlers
  /\ MaxSessions \in Nat \ {0}
  /\ MaxHandlers \in Nat \ {0}
  /\ MaxSessions < MaxHandlers
  /\ MaxGeneration \in Nat
  /\ MaxGeneration > MaxHandlers

VARIABLES serverPhase, sessions, slowCandidates, nextGeneration,
  staleGenerations, workerRegistered, priorSessions, priorNextGeneration,
  priorServerPhase, priorWorkerRegistered, lastClient, lastAction

vars == <<serverPhase, sessions, slowCandidates, nextGeneration,
  staleGenerations, workerRegistered, priorSessions, priorNextGeneration,
  priorServerPhase, priorWorkerRegistered, lastClient, lastAction>>

ActiveClients == {client \in Clients : sessions[client] # 0}
ActiveGenerations == {sessions[client] : client \in ActiveClients}
HandlerCount == Cardinality(ActiveClients) + Cardinality(slowCandidates)

Init ==
  /\ serverPhase = "absent"
  /\ sessions = [client \in Clients |-> 0]
  /\ slowCandidates = {}
  /\ nextGeneration = 1
  /\ staleGenerations = {}
  /\ workerRegistered = FALSE
  /\ priorSessions = sessions
  /\ priorNextGeneration = nextGeneration
  /\ priorServerPhase = serverPhase
  /\ priorWorkerRegistered = workerRegistered
  /\ lastClient = "none"
  /\ lastAction = "init"

Record(action, client) ==
  /\ priorSessions' = sessions
  /\ priorNextGeneration' = nextGeneration
  /\ priorServerPhase' = serverPhase
  /\ priorWorkerRegistered' = workerRegistered
  /\ lastClient' = client
  /\ lastAction' = action

StartServer ==
  /\ serverPhase = "absent"
  /\ serverPhase' = "listening"
  /\ UNCHANGED <<sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("start-server", "none")

RegisterWorker ==
  /\ serverPhase = "listening"
  /\ ~workerRegistered
  /\ workerRegistered' = TRUE
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations>>
  /\ Record("register-worker", "none")

BeginSlow(client) ==
  /\ serverPhase = "listening"
  /\ client \in Clients
  /\ sessions[client] = 0
  /\ client \notin slowCandidates
  /\ HandlerCount < MaxHandlers
  /\ slowCandidates' = slowCandidates \cup {client}
  /\ UNCHANGED <<serverPhase, sessions, nextGeneration, staleGenerations,
       workerRegistered>>
  /\ Record("begin-slow", client)

RejectWrongUser ==
  /\ serverPhase = "listening"
  /\ HandlerCount < MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-wrong-user", "none")

RejectMalformed ==
  /\ serverPhase = "listening"
  /\ HandlerCount < MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-malformed", "none")

RejectIncompatibleProtocol ==
  /\ serverPhase = "listening"
  /\ HandlerCount < MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-incompatible-protocol", "none")

RejectIncompatibleCohort ==
  /\ serverPhase = "listening"
  /\ HandlerCount < MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-incompatible-cohort", "none")

RejectWrongMessage ==
  /\ serverPhase = "listening"
  /\ HandlerCount < MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-wrong-message", "none")

RejectCapacity ==
  /\ serverPhase = "listening"
  /\ Cardinality(ActiveClients) = MaxSessions
  /\ HandlerCount < MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-capacity", "none")

RejectHandlerSaturation ==
  /\ serverPhase = "listening"
  /\ HandlerCount = MaxHandlers
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-handler-saturation", "none")

HandshakeTimeout(client) ==
  /\ serverPhase = "listening"
  /\ client \in slowCandidates
  /\ slowCandidates' = slowCandidates \ {client}
  /\ UNCHANGED <<serverPhase, sessions, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("handshake-timeout", client)

AdmitClient(client) ==
  /\ serverPhase = "listening"
  /\ client \in Clients
  /\ sessions[client] = 0
  /\ client \notin slowCandidates
  /\ Cardinality(ActiveClients) < MaxSessions
  /\ HandlerCount < MaxHandlers
  /\ nextGeneration <= MaxGeneration
  /\ sessions' = [sessions EXCEPT ![client] = nextGeneration]
  /\ nextGeneration' = nextGeneration + 1
  /\ UNCHANGED <<serverPhase, slowCandidates, staleGenerations,
       workerRegistered>>
  /\ Record("admit-client", client)

ClientDisconnect(client) ==
  /\ serverPhase = "listening"
  /\ client \in ActiveClients
  /\ sessions' = [sessions EXCEPT ![client] = 0]
  /\ staleGenerations' = staleGenerations \cup {sessions[client]}
  /\ UNCHANGED <<serverPhase, slowCandidates, nextGeneration,
       workerRegistered>>
  /\ Record("client-disconnect", client)

DuplicateHello(client) ==
  /\ serverPhase = "listening"
  /\ client \in ActiveClients
  /\ sessions' = [sessions EXCEPT ![client] = 0]
  /\ staleGenerations' = staleGenerations \cup {sessions[client]}
  /\ UNCHANGED <<serverPhase, slowCandidates, nextGeneration,
       workerRegistered>>
  /\ Record("duplicate-hello", client)

CleanupStaleGeneration(generation) ==
  /\ generation \in staleGenerations
  /\ staleGenerations' = staleGenerations \ {generation}
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       workerRegistered>>
  /\ Record("stale-session-cleanup", "none")

StaleSessionCleanup ==
  \E generation \in staleGenerations: CleanupStaleGeneration(generation)

StopServer ==
  /\ serverPhase = "listening"
  /\ serverPhase' = "stopped"
  /\ sessions' = [client \in Clients |-> 0]
  /\ slowCandidates' = {}
  /\ staleGenerations' = {}
  /\ workerRegistered' = FALSE
  /\ UNCHANGED nextGeneration
  /\ Record("stop-server", "none")

Next ==
  \/ StartServer
  \/ RegisterWorker
  \/ \E client \in Clients: BeginSlow(client)
  \/ RejectWrongUser
  \/ RejectMalformed
  \/ RejectIncompatibleProtocol
  \/ RejectIncompatibleCohort
  \/ RejectWrongMessage
  \/ RejectCapacity
  \/ RejectHandlerSaturation
  \/ \E client \in Clients: HandshakeTimeout(client)
  \/ \E client \in Clients: AdmitClient(client)
  \/ \E client \in Clients: ClientDisconnect(client)
  \/ \E client \in Clients: DuplicateHello(client)
  \/ StaleSessionCleanup
  \/ StopServer

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ serverPhase \in ServerPhases
  /\ sessions \in [Clients -> 0..MaxGeneration]
  /\ slowCandidates \subseteq Clients
  /\ nextGeneration \in 1..(MaxGeneration + 1)
  /\ staleGenerations \subseteq 1..MaxGeneration
  /\ workerRegistered \in BOOLEAN
  /\ priorSessions \in [Clients -> 0..MaxGeneration]
  /\ priorNextGeneration \in 1..(MaxGeneration + 1)
  /\ priorServerPhase \in ServerPhases
  /\ priorWorkerRegistered \in BOOLEAN
  /\ lastClient \in ClientValues
  /\ lastAction \in Actions

SessionCapacityIsBounded == Cardinality(ActiveClients) <= MaxSessions

HandlerCapacityIsBounded == HandlerCount <= MaxHandlers

ActiveGenerationsAreUnique ==
  Cardinality(ActiveGenerations) = Cardinality(ActiveClients)

ActiveGenerationsPrecedeCursor ==
  \A client \in ActiveClients: sessions[client] < nextGeneration

AdmissionUsesPreviousCursor ==
  lastAction = "admit-client" =>
    /\ lastClient \in Clients
    /\ priorSessions[lastClient] = 0
    /\ sessions[lastClient] = priorNextGeneration
    /\ nextGeneration = priorNextGeneration + 1

GenerationCursorChangesOnlyOnAdmission ==
  nextGeneration # priorNextGeneration => lastAction = "admit-client"

SlowCandidatesOwnNoSession ==
  \A client \in slowCandidates: sessions[client] = 0

StaleGenerationsAreNotActive ==
  staleGenerations \intersect ActiveGenerations = {}

ActiveSessionRequiresListeningServer ==
  ActiveClients # {} => serverPhase = "listening"

WorkerRequiresListeningServer == workerRegistered => serverPhase = "listening"

RejectionPreservesAuthority ==
  lastAction \in {
    "reject-wrong-user", "reject-malformed", "reject-incompatible-protocol",
    "reject-incompatible-cohort", "reject-wrong-message", "reject-capacity",
    "reject-handler-saturation"
  } =>
    /\ sessions = priorSessions
    /\ serverPhase = priorServerPhase
    /\ workerRegistered = priorWorkerRegistered

ClientLossIsSessionScoped ==
  lastAction \in {"client-disconnect", "duplicate-hello"} =>
    /\ lastClient \in Clients
    /\ sessions[lastClient] = 0
    /\ serverPhase = "listening"
    /\ workerRegistered = priorWorkerRegistered
    /\ \A client \in Clients \ {lastClient}:
         sessions[client] = priorSessions[client]

StaleCleanupPreservesEveryCurrentSession ==
  lastAction = "stale-session-cleanup" => sessions = priorSessions

StoppedServerOwnsNoConnection ==
  serverPhase = "stopped" =>
    /\ ActiveClients = {}
    /\ slowCandidates = {}
    /\ ~workerRegistered

=============================================================================
