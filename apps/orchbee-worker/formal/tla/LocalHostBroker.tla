-------------------------- MODULE LocalHostBroker --------------------------
EXTENDS TLC

\* Models the additive Worker-to-Terminal-Host broker connection spine.
\* Server admission must precede Host contact. The Worker sends only the
\* existing Terminal Protocol Hello, emits readiness only after both peers
\* accept, and closes both links when either peer is lost or rejects it.
\* Terminal launch, PTY effects, reconnect, and service activation are absent.

WorkerPhases == {
  "absent", "connecting-server", "connecting-host", "host-handshake",
  "host-admitted", "ready", "stopped"
}
LinkPhases == {"closed", "connecting", "active"}
StopCauses == {
  "none", "server-rejected", "server-lost", "host-peer-rejected",
  "host-wrong-role", "host-incompatible", "host-malformed",
  "host-timeout", "host-lost"
}
HostRequests == {"none", "hello"}
Actions == {
  "init", "start-worker", "accept-server", "reject-server",
  "connect-host", "reject-host-peer", "accept-host-handshake",
  "reject-host-wrong-role", "reject-host-incompatible",
  "reject-host-malformed", "host-handshake-timeout", "emit-ready",
  "lose-server", "lose-host"
}

VARIABLES workerPhase, serverLink, hostLink, stopCause, hostRequest,
  readinessEmitted, terminalCommandCount, lastAction

vars == <<workerPhase, serverLink, hostLink, stopCause, hostRequest,
  readinessEmitted, terminalCommandCount, lastAction>>

Init ==
  /\ workerPhase = "absent"
  /\ serverLink = "closed"
  /\ hostLink = "closed"
  /\ stopCause = "none"
  /\ hostRequest = "none"
  /\ readinessEmitted = FALSE
  /\ terminalCommandCount = 0
  /\ lastAction = "init"

StartWorker ==
  /\ workerPhase = "absent"
  /\ workerPhase' = "connecting-server"
  /\ serverLink' = "connecting"
  /\ lastAction' = "start-worker"
  /\ UNCHANGED <<hostLink, stopCause, hostRequest, readinessEmitted,
       terminalCommandCount>>

AcceptServer ==
  /\ workerPhase = "connecting-server"
  /\ workerPhase' = "connecting-host"
  /\ serverLink' = "active"
  /\ lastAction' = "accept-server"
  /\ UNCHANGED <<hostLink, stopCause, hostRequest, readinessEmitted,
       terminalCommandCount>>

RejectServer ==
  /\ workerPhase = "connecting-server"
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ stopCause' = "server-rejected"
  /\ lastAction' = "reject-server"
  /\ UNCHANGED <<hostLink, hostRequest, readinessEmitted,
       terminalCommandCount>>

ConnectHost ==
  /\ workerPhase = "connecting-host"
  /\ workerPhase' = "host-handshake"
  /\ hostLink' = "connecting"
  /\ hostRequest' = "hello"
  /\ lastAction' = "connect-host"
  /\ UNCHANGED <<serverLink, stopCause, readinessEmitted,
       terminalCommandCount>>

StopForHostFailure(cause, action) ==
  /\ workerPhase \in {"connecting-host", "host-handshake", "host-admitted"}
  /\ cause \in StopCauses \ {"none", "server-rejected", "server-lost"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = cause
  /\ lastAction' = action
  /\ UNCHANGED <<hostRequest, readinessEmitted, terminalCommandCount>>

RejectHostPeer ==
  /\ workerPhase = "connecting-host"
  /\ StopForHostFailure("host-peer-rejected", "reject-host-peer")

AcceptHostHandshake ==
  /\ workerPhase = "host-handshake"
  /\ workerPhase' = "host-admitted"
  /\ hostLink' = "active"
  /\ lastAction' = "accept-host-handshake"
  /\ UNCHANGED <<serverLink, stopCause, hostRequest, readinessEmitted,
       terminalCommandCount>>

RejectHostWrongRole ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostFailure("host-wrong-role", "reject-host-wrong-role")

RejectHostIncompatible ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostFailure("host-incompatible", "reject-host-incompatible")

RejectHostMalformed ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostFailure("host-malformed", "reject-host-malformed")

HostHandshakeTimeout ==
  /\ workerPhase = "host-handshake"
  /\ StopForHostFailure("host-timeout", "host-handshake-timeout")

EmitReady ==
  /\ workerPhase = "host-admitted"
  /\ workerPhase' = "ready"
  /\ readinessEmitted' = TRUE
  /\ lastAction' = "emit-ready"
  /\ UNCHANGED <<serverLink, hostLink, stopCause, hostRequest,
       terminalCommandCount>>

LoseServer ==
  /\ serverLink = "active"
  /\ workerPhase \in {"connecting-host", "host-handshake", "host-admitted", "ready"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "server-lost"
  /\ lastAction' = "lose-server"
  /\ UNCHANGED <<hostRequest, readinessEmitted, terminalCommandCount>>

LoseHost ==
  /\ hostLink \in {"connecting", "active"}
  /\ workerPhase \in {"host-handshake", "host-admitted", "ready"}
  /\ workerPhase' = "stopped"
  /\ serverLink' = "closed"
  /\ hostLink' = "closed"
  /\ stopCause' = "host-lost"
  /\ lastAction' = "lose-host"
  /\ UNCHANGED <<hostRequest, readinessEmitted, terminalCommandCount>>

Next ==
  \/ StartWorker
  \/ AcceptServer
  \/ RejectServer
  \/ ConnectHost
  \/ RejectHostPeer
  \/ AcceptHostHandshake
  \/ RejectHostWrongRole
  \/ RejectHostIncompatible
  \/ RejectHostMalformed
  \/ HostHandshakeTimeout
  \/ EmitReady
  \/ LoseServer
  \/ LoseHost

SettlePendingWorker ==
  AcceptServer \/ RejectServer \/ ConnectHost \/ RejectHostPeer \/
  AcceptHostHandshake \/ RejectHostWrongRole \/ RejectHostIncompatible \/
  RejectHostMalformed \/ HostHandshakeTimeout \/ EmitReady

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SettlePendingWorker)

TypeOK ==
  /\ workerPhase \in WorkerPhases
  /\ serverLink \in LinkPhases
  /\ hostLink \in LinkPhases
  /\ stopCause \in StopCauses
  /\ hostRequest \in HostRequests
  /\ readinessEmitted \in BOOLEAN
  /\ terminalCommandCount = 0
  /\ lastAction \in Actions

HostContactRequiresServerAdmission ==
  hostLink # "closed" => serverLink = "active"

OnlyHelloReachesHost ==
  hostRequest # "none" => hostRequest = "hello"

ReadinessRequiresBothAdmissions ==
  workerPhase = "ready" =>
    /\ serverLink = "active"
    /\ hostLink = "active"
    /\ hostRequest = "hello"
    /\ readinessEmitted

ReadinessWasNeverForged ==
  readinessEmitted =>
    /\ workerPhase \in {"ready", "stopped"}
    /\ stopCause \in {"none", "server-lost", "host-lost"}

StoppedWorkerOwnsNoLink ==
  workerPhase = "stopped" =>
    /\ serverLink = "closed"
    /\ hostLink = "closed"

PendingWorkerEventuallySettles ==
  [](workerPhase \notin {"absent", "ready", "stopped"} =>
    <>(workerPhase \in {"ready", "stopped"}))

=============================================================================
