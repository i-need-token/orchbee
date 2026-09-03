------------------------- MODULE LocalClientSessions -------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the additive same-user local Human Client admission boundary. Each
\* accepted transport owns one independent Client Session generation. Multiple
\* sessions, a silent handshake, malformed candidates, duplicate handshakes,
\* disconnect, stale cleanup, Worker presence, and Server loss may be ordered
\* independently. Workbench commands, terminal relay, rendering, activation,
\* persistence, and remote authentication are outside this model.

CONSTANTS Clients, MaxSessions, MaxGeneration

ServerPhases == {"absent", "listening", "stopped"}
ClientValues == Clients \cup {"none"}
RejectionKinds == {
  "wrong-user", "malformed", "incompatible-protocol",
  "incompatible-cohort", "wrong-message", "capacity", "timeout"
}
Actions == {
  "init", "start-server", "register-worker", "begin-slow",
  "reject-wrong-user", "reject-malformed", "reject-incompatible-protocol",
  "reject-incompatible-cohort", "reject-wrong-message", "reject-capacity",
  "handshake-timeout", "admit-client", "client-disconnect",
  "duplicate-hello", "stale-session-cleanup", "stop-server"
}

VARIABLES serverPhase, sessions, slowCandidates, nextGeneration,
  staleGenerations, rejection, workerRegistered, priorSessions,
  priorServerPhase, priorWorkerRegistered, lastClient, lastAction

vars == <<serverPhase, sessions, slowCandidates, nextGeneration,
  staleGenerations, rejection, workerRegistered, priorSessions,
  priorServerPhase, priorWorkerRegistered, lastClient, lastAction>>

ActiveClients == {client \in Clients : sessions[client] # 0}
ActiveGenerations == {sessions[client] : client \in ActiveClients}

Init ==
  /\ serverPhase = "absent"
  /\ sessions = [client \in Clients |-> 0]
  /\ slowCandidates = {}
  /\ nextGeneration = 1
  /\ staleGenerations = {}
  /\ rejection = "none"
  /\ workerRegistered = FALSE
  /\ priorSessions = sessions
  /\ priorServerPhase = serverPhase
  /\ priorWorkerRegistered = workerRegistered
  /\ lastClient = "none"
  /\ lastAction = "init"

Record(action, client) ==
  /\ priorSessions' = sessions
  /\ priorServerPhase' = serverPhase
  /\ priorWorkerRegistered' = workerRegistered
  /\ lastClient' = client
  /\ lastAction' = action

StartServer ==
  /\ serverPhase = "absent"
  /\ serverPhase' = "listening"
  /\ UNCHANGED <<sessions, slowCandidates, nextGeneration,
       staleGenerations, rejection, workerRegistered>>
  /\ Record("start-server", "none")

RegisterWorker ==
  /\ serverPhase = "listening"
  /\ ~workerRegistered
  /\ workerRegistered' = TRUE
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, rejection>>
  /\ Record("register-worker", "none")

BeginSlow(client) ==
  /\ serverPhase = "listening"
  /\ client \in Clients
  /\ sessions[client] = 0
  /\ client \notin slowCandidates
  /\ slowCandidates' = slowCandidates \cup {client}
  /\ UNCHANGED <<serverPhase, sessions, nextGeneration, staleGenerations,
       rejection, workerRegistered>>
  /\ Record("begin-slow", client)

RejectWrongUser ==
  /\ serverPhase = "listening"
  /\ rejection' = "wrong-user"
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-wrong-user", "none")

RejectMalformed ==
  /\ serverPhase = "listening"
  /\ rejection' = "malformed"
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-malformed", "none")

RejectIncompatibleProtocol ==
  /\ serverPhase = "listening"
  /\ rejection' = "incompatible-protocol"
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-incompatible-protocol", "none")

RejectIncompatibleCohort ==
  /\ serverPhase = "listening"
  /\ rejection' = "incompatible-cohort"
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-incompatible-cohort", "none")

RejectWrongMessage ==
  /\ serverPhase = "listening"
  /\ rejection' = "wrong-message"
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-wrong-message", "none")

RejectCapacity ==
  /\ serverPhase = "listening"
  /\ Cardinality(ActiveClients) = MaxSessions
  /\ rejection' = "capacity"
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("reject-capacity", "none")

HandshakeTimeout(client) ==
  /\ serverPhase = "listening"
  /\ client \in slowCandidates
  /\ slowCandidates' = slowCandidates \ {client}
  /\ rejection' = "timeout"
  /\ UNCHANGED <<serverPhase, sessions, nextGeneration,
       staleGenerations, workerRegistered>>
  /\ Record("handshake-timeout", client)

AdmitClient(client) ==
  /\ serverPhase = "listening"
  /\ client \in Clients
  /\ sessions[client] = 0
  /\ client \notin slowCandidates
  /\ Cardinality(ActiveClients) < MaxSessions
  /\ nextGeneration <= MaxGeneration
  /\ sessions' = [sessions EXCEPT ![client] = nextGeneration]
  /\ nextGeneration' = nextGeneration + 1
  /\ UNCHANGED <<serverPhase, slowCandidates, staleGenerations,
       rejection, workerRegistered>>
  /\ Record("admit-client", client)

ClientDisconnect(client) ==
  /\ serverPhase = "listening"
  /\ client \in ActiveClients
  /\ sessions' = [sessions EXCEPT ![client] = 0]
  /\ staleGenerations' = staleGenerations \cup {sessions[client]}
  /\ UNCHANGED <<serverPhase, slowCandidates, nextGeneration,
       rejection, workerRegistered>>
  /\ Record("client-disconnect", client)

DuplicateHello(client) ==
  /\ serverPhase = "listening"
  /\ client \in ActiveClients
  /\ sessions' = [sessions EXCEPT ![client] = 0]
  /\ staleGenerations' = staleGenerations \cup {sessions[client]}
  /\ UNCHANGED <<serverPhase, slowCandidates, nextGeneration,
       rejection, workerRegistered>>
  /\ Record("duplicate-hello", client)

CleanupStaleGeneration(generation) ==
  /\ generation \in staleGenerations
  /\ staleGenerations' = staleGenerations \ {generation}
  /\ UNCHANGED <<serverPhase, sessions, slowCandidates, nextGeneration,
       rejection, workerRegistered>>
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
  /\ UNCHANGED <<nextGeneration, rejection>>
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
  /\ rejection \in RejectionKinds \cup {"none"}
  /\ workerRegistered \in BOOLEAN
  /\ priorSessions \in [Clients -> 0..MaxGeneration]
  /\ priorServerPhase \in ServerPhases
  /\ priorWorkerRegistered \in BOOLEAN
  /\ lastClient \in ClientValues
  /\ lastAction \in Actions

SessionCapacityIsBounded == Cardinality(ActiveClients) <= MaxSessions

ActiveGenerationsAreUnique ==
  Cardinality(ActiveGenerations) = Cardinality(ActiveClients)

ActiveGenerationsNeverRepeat ==
  \A client \in ActiveClients: sessions[client] < nextGeneration

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
    "reject-incompatible-cohort", "reject-wrong-message", "reject-capacity"
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
