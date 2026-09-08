---------------------- MODULE GatewayTransportLifecycle ----------------------
EXTENDS Naturals, TLC

\* Models the Browser Gateway's listener cancellation, authentication deadline,
\* and heartbeat ownership. Timer actions represent expiry ordering rather than
\* wall-clock duration. Product page and attachment cardinality is intentionally
\* outside this one-connection transport model.

ServerPhases == {"idle", "starting", "listening", "closed"}
ListenOutcomes == {"none", "ready", "failed", "cancelled"}
ConnectionPhases == {"absent", "awaiting-authentication", "authenticated", "closed"}
HeartbeatPhases == {"idle", "awaiting-pong"}
Actions == {
  "init", "begin-listen", "listen-succeeds", "listen-fails",
  "dispose-during-listen", "accept-socket", "authenticate",
  "authentication-timeout", "send-ping", "receive-pong",
  "miss-pong", "close-socket", "runtime-server-error", "dispose-active"
}

VARIABLES processActive, serverPhase, listenOutcome, connectionPhase,
  authenticationTimer, heartbeatPhase, desktopAuthority, failureReports,
  priorConnectionPhase, lastAction

vars == <<processActive, serverPhase, listenOutcome, connectionPhase,
  authenticationTimer, heartbeatPhase, desktopAuthority, failureReports,
  priorConnectionPhase, lastAction>>

Init ==
  /\ processActive = TRUE
  /\ serverPhase = "idle"
  /\ listenOutcome = "none"
  /\ connectionPhase = "absent"
  /\ authenticationTimer = FALSE
  /\ heartbeatPhase = "idle"
  /\ desktopAuthority = FALSE
  /\ failureReports = 0
  /\ priorConnectionPhase = connectionPhase
  /\ lastAction = "init"

Record(action) ==
  /\ priorConnectionPhase' = connectionPhase
  /\ lastAction' = action

BeginListen ==
  /\ processActive
  /\ serverPhase = "idle"
  /\ serverPhase' = "starting"
  /\ UNCHANGED <<processActive, listenOutcome, connectionPhase,
       authenticationTimer, heartbeatPhase, desktopAuthority, failureReports>>
  /\ Record("begin-listen")

ListenSucceeds ==
  /\ processActive
  /\ serverPhase = "starting"
  /\ serverPhase' = "listening"
  /\ listenOutcome' = "ready"
  /\ UNCHANGED <<processActive, connectionPhase, authenticationTimer,
       heartbeatPhase, desktopAuthority, failureReports>>
  /\ Record("listen-succeeds")

ListenFails ==
  /\ processActive
  /\ serverPhase = "starting"
  /\ serverPhase' = "closed"
  /\ listenOutcome' = "failed"
  /\ failureReports' = failureReports + 1
  /\ UNCHANGED <<processActive, connectionPhase, authenticationTimer,
       heartbeatPhase, desktopAuthority>>
  /\ Record("listen-fails")

DisposeDuringListen ==
  /\ processActive
  /\ serverPhase = "starting"
  /\ processActive' = FALSE
  /\ serverPhase' = "closed"
  /\ listenOutcome' = "cancelled"
  /\ connectionPhase' = "closed"
  /\ authenticationTimer' = FALSE
  /\ heartbeatPhase' = "idle"
  /\ desktopAuthority' = FALSE
  /\ UNCHANGED failureReports
  /\ Record("dispose-during-listen")

AcceptSocket ==
  /\ processActive
  /\ serverPhase = "listening"
  /\ connectionPhase = "absent"
  /\ connectionPhase' = "awaiting-authentication"
  /\ authenticationTimer' = TRUE
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome, heartbeatPhase,
       desktopAuthority, failureReports>>
  /\ Record("accept-socket")

Authenticate ==
  /\ processActive
  /\ connectionPhase = "awaiting-authentication"
  /\ connectionPhase' = "authenticated"
  /\ authenticationTimer' = FALSE
  /\ desktopAuthority' = TRUE
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome, heartbeatPhase,
       failureReports>>
  /\ Record("authenticate")

AuthenticationTimeout ==
  /\ connectionPhase = "awaiting-authentication"
  /\ authenticationTimer
  /\ connectionPhase' = "closed"
  /\ authenticationTimer' = FALSE
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome, heartbeatPhase,
       desktopAuthority, failureReports>>
  /\ Record("authentication-timeout")

SendPing ==
  /\ processActive
  /\ connectionPhase = "authenticated"
  /\ heartbeatPhase = "idle"
  /\ heartbeatPhase' = "awaiting-pong"
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome, connectionPhase,
       authenticationTimer, desktopAuthority, failureReports>>
  /\ Record("send-ping")

ReceivePong ==
  /\ connectionPhase = "authenticated"
  /\ heartbeatPhase = "awaiting-pong"
  /\ heartbeatPhase' = "idle"
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome, connectionPhase,
       authenticationTimer, desktopAuthority, failureReports>>
  /\ Record("receive-pong")

MissPong ==
  /\ connectionPhase = "authenticated"
  /\ heartbeatPhase = "awaiting-pong"
  /\ connectionPhase' = "closed"
  /\ heartbeatPhase' = "idle"
  /\ desktopAuthority' = FALSE
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome,
       authenticationTimer, failureReports>>
  /\ Record("miss-pong")

CloseSocket ==
  /\ connectionPhase \in {"awaiting-authentication", "authenticated"}
  /\ connectionPhase' = "closed"
  /\ authenticationTimer' = FALSE
  /\ heartbeatPhase' = "idle"
  /\ desktopAuthority' = FALSE
  /\ UNCHANGED <<processActive, serverPhase, listenOutcome, failureReports>>
  /\ Record("close-socket")

RuntimeServerError ==
  /\ processActive
  /\ serverPhase = "listening"
  /\ processActive' = FALSE
  /\ serverPhase' = "closed"
  /\ connectionPhase' = "closed"
  /\ authenticationTimer' = FALSE
  /\ heartbeatPhase' = "idle"
  /\ desktopAuthority' = FALSE
  /\ failureReports' = failureReports + 1
  /\ UNCHANGED listenOutcome
  /\ Record("runtime-server-error")

DisposeActive ==
  /\ processActive
  /\ serverPhase \in {"idle", "listening", "closed"}
  /\ processActive' = FALSE
  /\ serverPhase' = "closed"
  /\ listenOutcome' = IF listenOutcome = "none" THEN "cancelled" ELSE listenOutcome
  /\ connectionPhase' = "closed"
  /\ authenticationTimer' = FALSE
  /\ heartbeatPhase' = "idle"
  /\ desktopAuthority' = FALSE
  /\ UNCHANGED failureReports
  /\ Record("dispose-active")

Next ==
  \/ BeginListen
  \/ ListenSucceeds
  \/ ListenFails
  \/ DisposeDuringListen
  \/ AcceptSocket
  \/ Authenticate
  \/ AuthenticationTimeout
  \/ SendPing
  \/ ReceivePong
  \/ MissPong
  \/ CloseSocket
  \/ RuntimeServerError
  \/ DisposeActive

SettleListen == ListenSucceeds \/ ListenFails \/ DisposeDuringListen
SettleAuthentication == Authenticate \/ AuthenticationTimeout \/ CloseSocket
HeartbeatTimerFires == SendPing \/ MissPong

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SettleListen)
  /\ WF_vars(SettleAuthentication)
  /\ WF_vars(HeartbeatTimerFires)

TypeOK ==
  /\ processActive \in BOOLEAN
  /\ serverPhase \in ServerPhases
  /\ listenOutcome \in ListenOutcomes
  /\ connectionPhase \in ConnectionPhases
  /\ authenticationTimer \in BOOLEAN
  /\ heartbeatPhase \in HeartbeatPhases
  /\ desktopAuthority \in BOOLEAN
  /\ failureReports \in Nat
  /\ priorConnectionPhase \in ConnectionPhases
  /\ lastAction \in Actions

AuthenticationPrecedesDesktopAuthority ==
  desktopAuthority => connectionPhase = "authenticated"

AuthenticationTimerHasExactOwner ==
  authenticationTimer <=> connectionPhase = "awaiting-authentication"

HeartbeatHasExactOwner ==
  heartbeatPhase = "awaiting-pong" => connectionPhase = "authenticated"

ClosedConnectionOwnsNoTransportState ==
  connectionPhase = "closed" =>
    /\ ~authenticationTimer
    /\ heartbeatPhase = "idle"
    /\ ~desktopAuthority

DisposedProcessOwnsNoListener ==
  ~processActive => serverPhase = "closed"

ListenPromiseSettlesWithServerStart ==
  serverPhase \in {"listening", "closed"} => listenOutcome # "none"

OneFailureReport == failureReports <= 1

MissedPongClosesConnection ==
  lastAction = "miss-pong" => connectionPhase = "closed"

PendingListenEventuallySettles ==
  [](serverPhase = "starting" => <>(listenOutcome # "none"))

AwaitingAuthenticationEventuallySettles ==
  [](connectionPhase = "awaiting-authentication" =>
    <>(connectionPhase # "awaiting-authentication"))

AwaitingPongEventuallySettles ==
  [](heartbeatPhase = "awaiting-pong" => <>(heartbeatPhase = "idle"))

AuthenticatedHeartbeatRecurs ==
  [](connectionPhase = "authenticated" /\ heartbeatPhase = "idle" =>
    <>(connectionPhase # "authenticated" \/ heartbeatPhase = "awaiting-pong"))

=============================================================================
