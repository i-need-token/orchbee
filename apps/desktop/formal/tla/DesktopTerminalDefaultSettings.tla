---------------- MODULE DesktopTerminalDefaultSettings ----------------
EXTENDS Naturals, TLC

Values == {0, 1}
Phases == {"uninitialized", "ready", "disposed"}
Requests == {"absent", "host", "persistence"}
Issues == {"none", "write-failed"}

VARIABLES
  phase,
  session,
  host,
  durable,
  request,
  candidate,
  issue,
  runningTerminal,
  lastTerminal,
  lastAction,
  previousSession,
  previousHost,
  previousDurable,
  previousRunningTerminal

vars == <<phase, session, host, durable, request, candidate, issue,
  runningTerminal, lastTerminal, lastAction, previousSession, previousHost,
  previousDurable, previousRunningTerminal>>

Record(action) ==
  /\ lastAction' = action
  /\ previousSession' = session
  /\ previousHost' = host
  /\ previousDurable' = durable
  /\ previousRunningTerminal' = runningTerminal

Init ==
  /\ phase = "uninitialized"
  /\ session = 0
  /\ host = 0
  /\ durable \in Values
  /\ request = "absent"
  /\ candidate = 0
  /\ issue = "none"
  /\ runningTerminal = 0
  /\ lastTerminal = 0
  /\ lastAction = "init"
  /\ previousSession = session
  /\ previousHost = host
  /\ previousDurable = durable
  /\ previousRunningTerminal = runningTerminal

Initialize ==
  /\ phase = "uninitialized"
  /\ phase' = "ready"
  /\ session' = durable
  /\ host' = durable
  /\ issue' = "none"
  /\ UNCHANGED <<durable, request, candidate, runningTerminal, lastTerminal>>
  /\ Record("initialize")

Request(value) ==
  /\ phase = "ready"
  /\ request = "absent"
  /\ value \in Values
  /\ request' = "host"
  /\ candidate' = value
  /\ UNCHANGED <<phase, session, host, durable, issue, runningTerminal, lastTerminal>>
  /\ Record("request")

HostAccept ==
  /\ phase = "ready"
  /\ request = "host"
  /\ request' = "persistence"
  /\ session' = candidate
  /\ host' = candidate
  /\ UNCHANGED <<phase, durable, candidate, issue, runningTerminal, lastTerminal>>
  /\ Record("host-accept")

HostReject ==
  /\ phase = "ready"
  /\ request = "host"
  /\ request' = "absent"
  /\ UNCHANGED <<phase, session, host, durable, candidate, issue,
    runningTerminal, lastTerminal>>
  /\ Record("host-reject")

PersistenceSucceeds ==
  /\ phase = "ready"
  /\ request = "persistence"
  /\ request' = "absent"
  /\ durable' = candidate
  /\ issue' = "none"
  /\ UNCHANGED <<phase, session, host, candidate, runningTerminal, lastTerminal>>
  /\ Record("persistence-succeeded")

PersistenceFails ==
  /\ phase = "ready"
  /\ request = "persistence"
  /\ request' = "absent"
  /\ issue' = "write-failed"
  /\ UNCHANGED <<phase, session, host, durable, candidate,
    runningTerminal, lastTerminal>>
  /\ Record("persistence-failed")

CreateTerminal ==
  /\ phase = "ready"
  /\ lastTerminal' = host
  /\ UNCHANGED <<phase, session, host, durable, request, candidate, issue,
    runningTerminal>>
  /\ Record("create-terminal")

RendererReload ==
  /\ phase = "ready"
  /\ UNCHANGED <<phase, session, host, durable, request, candidate, issue,
    runningTerminal, lastTerminal>>
  /\ Record("renderer-reload")

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ request' = "absent"
  /\ UNCHANGED <<session, host, durable, candidate, issue,
    runningTerminal, lastTerminal>>
  /\ Record("dispose")

Next ==
  \/ Initialize
  \/ \E value \in Values: Request(value)
  \/ HostAccept
  \/ HostReject
  \/ PersistenceSucceeds
  \/ PersistenceFails
  \/ CreateTerminal
  \/ RendererReload
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in Phases
  /\ session \in Values
  /\ host \in Values
  /\ durable \in Values
  /\ request \in Requests
  /\ candidate \in Values
  /\ issue \in Issues
  /\ runningTerminal \in Values
  /\ lastTerminal \in Values
  /\ lastAction \in {
       "init", "initialize", "request", "host-accept", "host-reject",
       "persistence-succeeded", "persistence-failed", "create-terminal",
       "renderer-reload", "dispose"
     }
  /\ previousSession \in Values
  /\ previousHost \in Values
  /\ previousDurable \in Values
  /\ previousRunningTerminal \in Values

HostAndSessionAgree == phase = "ready" => host = session

AtMostOneMutation == request \in Requests

HostRejectionPreservesAcceptedState ==
  lastAction = "host-reject" =>
    /\ session = previousSession
    /\ host = previousHost
    /\ durable = previousDurable

PersistenceFailureRetainsSession ==
  lastAction = "persistence-failed" =>
    /\ session = previousSession
    /\ host = previousHost
    /\ durable = previousDurable
    /\ issue = "write-failed"

DurableChangesOnlyAfterPersistence ==
  durable # previousDurable => lastAction = "persistence-succeeded"

NoLiveHistoryEviction == runningTerminal = 0

FutureTerminalCapturesAcceptedDefault ==
  lastAction = "create-terminal" =>
    /\ lastTerminal = previousHost
    /\ previousHost = previousSession

=============================================================================
