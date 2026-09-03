----------------------- MODULE LocalSiteProcessSpine -----------------------
EXTENDS Naturals, TLC

\* Models the first local Server-Worker process spine. One explicit endpoint
\* admits at most one compatible same-user Worker. Candidate handshakes,
\* Worker disconnect, stale cleanup, and Server loss may be independently
\* ordered. Client sessions, activation, Terminal Host, and persistence are
\* outside this model.

CONSTANTS Workers, MaxGeneration

ServerPhases == {"absent", "listening", "stopped"}
CandidatePhases == {"none", "compatible", "wrong-user", "malformed", "incompatible"}
WorkerValues == Workers \cup {"none"}
RejectionKinds == {
  "second-server", "duplicate-worker", "wrong-user", "malformed",
  "incompatible", "timeout"
}
Actions == {
  "init", "start-server", "reject-second-server", "begin-compatible",
  "begin-wrong-user", "begin-malformed", "begin-incompatible",
  "admit-worker", "reject-duplicate", "reject-wrong-user",
  "reject-malformed", "reject-incompatible", "handshake-timeout",
  "worker-disconnect", "stale-worker-cleanup", "stop-server"
}

VARIABLES serverPhase, candidatePhase, candidateWorker, activeWorker,
  activeGeneration, nextGeneration, staleCleanupGeneration, rejections,
  priorServerPhase, priorActiveWorker, priorActiveGeneration, lastAction

vars == <<serverPhase, candidatePhase, candidateWorker, activeWorker,
  activeGeneration, nextGeneration, staleCleanupGeneration, rejections,
  priorServerPhase, priorActiveWorker, priorActiveGeneration, lastAction>>

Init ==
  /\ serverPhase = "absent"
  /\ candidatePhase = "none"
  /\ candidateWorker = "none"
  /\ activeWorker = "none"
  /\ activeGeneration = 0
  /\ nextGeneration = 1
  /\ staleCleanupGeneration = 0
  /\ rejections = {}
  /\ priorServerPhase = serverPhase
  /\ priorActiveWorker = activeWorker
  /\ priorActiveGeneration = activeGeneration
  /\ lastAction = "init"

Record(action) ==
  /\ priorServerPhase' = serverPhase
  /\ priorActiveWorker' = activeWorker
  /\ priorActiveGeneration' = activeGeneration
  /\ lastAction' = action

StartServer ==
  /\ serverPhase = "absent"
  /\ serverPhase' = "listening"
  /\ UNCHANGED <<candidatePhase, candidateWorker, activeWorker,
       activeGeneration, nextGeneration, staleCleanupGeneration, rejections>>
  /\ Record("start-server")

RejectSecondServer ==
  /\ serverPhase = "listening"
  /\ rejections' = rejections \cup {"second-server"}
  /\ UNCHANGED <<serverPhase, candidatePhase, candidateWorker, activeWorker,
       activeGeneration, nextGeneration, staleCleanupGeneration>>
  /\ Record("reject-second-server")

BeginCompatible(worker) ==
  /\ serverPhase = "listening"
  /\ candidatePhase = "none"
  /\ worker \in Workers
  /\ candidatePhase' = "compatible"
  /\ candidateWorker' = worker
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration, rejections>>
  /\ Record("begin-compatible")

BeginWrongUser ==
  /\ serverPhase = "listening"
  /\ candidatePhase = "none"
  /\ candidatePhase' = "wrong-user"
  /\ candidateWorker' = "none"
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration, rejections>>
  /\ Record("begin-wrong-user")

BeginMalformed ==
  /\ serverPhase = "listening"
  /\ candidatePhase = "none"
  /\ candidatePhase' = "malformed"
  /\ candidateWorker' = "none"
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration, rejections>>
  /\ Record("begin-malformed")

BeginIncompatible ==
  /\ serverPhase = "listening"
  /\ candidatePhase = "none"
  /\ candidatePhase' = "incompatible"
  /\ candidateWorker' = "none"
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration, rejections>>
  /\ Record("begin-incompatible")

AdmitWorker ==
  /\ candidatePhase = "compatible"
  /\ activeWorker = "none"
  /\ nextGeneration <= MaxGeneration
  /\ activeWorker' = candidateWorker
  /\ activeGeneration' = nextGeneration
  /\ nextGeneration' = nextGeneration + 1
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ UNCHANGED <<serverPhase, staleCleanupGeneration, rejections>>
  /\ Record("admit-worker")

RejectDuplicate ==
  /\ candidatePhase = "compatible"
  /\ activeWorker # "none"
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ rejections' = rejections \cup {"duplicate-worker"}
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration>>
  /\ Record("reject-duplicate")

RejectWrongUser ==
  /\ candidatePhase = "wrong-user"
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ rejections' = rejections \cup {"wrong-user"}
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration>>
  /\ Record("reject-wrong-user")

RejectMalformed ==
  /\ candidatePhase = "malformed"
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ rejections' = rejections \cup {"malformed"}
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration>>
  /\ Record("reject-malformed")

RejectIncompatible ==
  /\ candidatePhase = "incompatible"
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ rejections' = rejections \cup {"incompatible"}
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration>>
  /\ Record("reject-incompatible")

HandshakeTimeout ==
  /\ candidatePhase # "none"
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ rejections' = rejections \cup {"timeout"}
  /\ UNCHANGED <<serverPhase, activeWorker, activeGeneration, nextGeneration,
       staleCleanupGeneration>>
  /\ Record("handshake-timeout")

WorkerDisconnect ==
  /\ activeWorker # "none"
  /\ staleCleanupGeneration' = activeGeneration
  /\ activeWorker' = "none"
  /\ activeGeneration' = 0
  /\ UNCHANGED <<serverPhase, candidatePhase, candidateWorker, nextGeneration,
       rejections>>
  /\ Record("worker-disconnect")

StaleWorkerCleanup ==
  /\ staleCleanupGeneration # 0
  /\ staleCleanupGeneration' = 0
  /\ IF staleCleanupGeneration = activeGeneration
       THEN /\ activeWorker' = "none"
            /\ activeGeneration' = 0
       ELSE /\ UNCHANGED <<activeWorker, activeGeneration>>
  /\ UNCHANGED <<serverPhase, candidatePhase, candidateWorker,
       nextGeneration, rejections>>
  /\ Record("stale-worker-cleanup")

StopServer ==
  /\ serverPhase = "listening"
  /\ serverPhase' = "stopped"
  /\ candidatePhase' = "none"
  /\ candidateWorker' = "none"
  /\ activeWorker' = "none"
  /\ activeGeneration' = 0
  /\ staleCleanupGeneration' = 0
  /\ UNCHANGED <<nextGeneration, rejections>>
  /\ Record("stop-server")

Next ==
  \/ StartServer
  \/ RejectSecondServer
  \/ \E worker \in Workers: BeginCompatible(worker)
  \/ BeginWrongUser
  \/ BeginMalformed
  \/ BeginIncompatible
  \/ AdmitWorker
  \/ RejectDuplicate
  \/ RejectWrongUser
  \/ RejectMalformed
  \/ RejectIncompatible
  \/ HandshakeTimeout
  \/ WorkerDisconnect
  \/ StaleWorkerCleanup
  \/ StopServer

SettleCandidate ==
  AdmitWorker \/ RejectDuplicate \/ RejectWrongUser \/ RejectMalformed \/
  RejectIncompatible \/ HandshakeTimeout \/ StopServer

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SettleCandidate)
  /\ WF_vars(StaleWorkerCleanup)

TypeOK ==
  /\ serverPhase \in ServerPhases
  /\ candidatePhase \in CandidatePhases
  /\ candidateWorker \in WorkerValues
  /\ activeWorker \in WorkerValues
  /\ activeGeneration \in 0..MaxGeneration
  /\ nextGeneration \in 1..(MaxGeneration + 1)
  /\ staleCleanupGeneration \in 0..MaxGeneration
  /\ rejections \subseteq RejectionKinds
  /\ priorServerPhase \in ServerPhases
  /\ priorActiveWorker \in WorkerValues
  /\ priorActiveGeneration \in 0..MaxGeneration
  /\ lastAction \in Actions

CandidateShapeIsExact ==
  (candidatePhase = "compatible") <=> (candidateWorker \in Workers)

ActiveWorkerRequiresListeningServer ==
  activeWorker # "none" => serverPhase = "listening"

ActiveWorkerHasGeneration ==
  (activeWorker = "none") <=> (activeGeneration = 0)

RejectionPreservesAuthority ==
  lastAction \in {
    "reject-second-server", "reject-duplicate", "reject-wrong-user",
    "reject-malformed", "reject-incompatible", "handshake-timeout"
  } =>
    /\ serverPhase = priorServerPhase
    /\ activeWorker = priorActiveWorker
    /\ activeGeneration = priorActiveGeneration

WorkerDisconnectPreservesServer ==
  lastAction = "worker-disconnect" => serverPhase = "listening"

StaleCleanupPreservesReplacement ==
  lastAction = "stale-worker-cleanup" /\
  priorActiveGeneration # 0 /\
  priorActiveGeneration # staleCleanupGeneration =>
    /\ activeWorker = priorActiveWorker
    /\ activeGeneration = priorActiveGeneration

StoppedServerOwnsNoConnection ==
  serverPhase = "stopped" =>
    /\ candidatePhase = "none"
    /\ activeWorker = "none"
    /\ activeGeneration = 0

PendingHandshakeEventuallySettles ==
  [](candidatePhase # "none" => <>(candidatePhase = "none"))

PendingCleanupEventuallySettles ==
  [](staleCleanupGeneration # 0 => <>(staleCleanupGeneration = 0))

=============================================================================
