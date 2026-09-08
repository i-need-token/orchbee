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
  pendingMutations,
  candidate,
  issue,
  runningTerminal,
  initialRunningTerminal,
  lastTerminal,
  lastAction,
  previousSession,
  previousHost,
  previousDurable,
  previousRunningTerminal

vars == <<phase, session, host, durable, request, pendingMutations, candidate, issue,
  runningTerminal, initialRunningTerminal, lastTerminal, lastAction, previousSession, previousHost,
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
  /\ pendingMutations = 0
  /\ candidate = 0
  /\ issue = "none"
  /\ runningTerminal \in Values
  /\ initialRunningTerminal = runningTerminal
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
  /\ UNCHANGED <<durable, request, pendingMutations, candidate, runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("initialize")

Request(value) ==
  /\ phase = "ready"
  /\ request = "absent"
  /\ value \in Values
  /\ request' = "host"
  /\ pendingMutations' = pendingMutations + 1
  /\ candidate' = value
  /\ UNCHANGED <<phase, session, host, durable, issue, runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("request")

HostAccept ==
  /\ phase = "ready"
  /\ request = "host"
  /\ request' = "persistence"
  /\ host' = candidate
  /\ UNCHANGED <<phase, session, durable, pendingMutations, candidate, issue, runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("host-accept")

HostReject ==
  /\ phase = "ready"
  /\ request = "host"
  /\ request' = "absent"
  /\ pendingMutations' = pendingMutations - 1
  /\ UNCHANGED <<phase, session, host, durable, candidate, issue,
    runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("host-reject")

PersistenceSucceeds ==
  /\ phase = "ready"
  /\ request = "persistence"
  /\ request' = "absent"
  /\ pendingMutations' = pendingMutations - 1
  /\ session' = candidate
  /\ durable' = candidate
  /\ issue' = "none"
  /\ UNCHANGED <<phase, host, candidate, runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("persistence-succeeded")

PersistenceFails ==
  /\ phase = "ready"
  /\ request = "persistence"
  /\ request' = "absent"
  /\ pendingMutations' = pendingMutations - 1
  /\ session' = candidate
  /\ issue' = "write-failed"
  /\ UNCHANGED <<phase, host, durable, candidate,
    runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("persistence-failed")

CreateTerminal ==
  /\ phase = "ready"
  /\ lastTerminal' = host
  /\ UNCHANGED <<phase, session, host, durable, request, pendingMutations, candidate, issue,
    runningTerminal, initialRunningTerminal>>
  /\ Record("create-terminal")

RendererReload ==
  /\ phase = "ready"
  /\ UNCHANGED <<phase, session, host, durable, request, pendingMutations, candidate, issue,
    runningTerminal, initialRunningTerminal, lastTerminal>>
  /\ Record("renderer-reload")

Dispose ==
  /\ phase # "disposed"
  /\ phase' = "disposed"
  /\ request' = "absent"
  /\ pendingMutations' = 0
  /\ UNCHANGED <<session, host, durable, candidate, issue,
    runningTerminal, initialRunningTerminal, lastTerminal>>
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
  /\ pendingMutations \in 0..2
  /\ candidate \in Values
  /\ issue \in Issues
  /\ runningTerminal \in Values
  /\ initialRunningTerminal \in Values
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

HostAndSessionAgree == phase = "ready" /\ request = "absent" => host = session

\* Count admitted effects independently of the single request-phase slot. A
\* missing busy guard must produce a counterexample instead of overwriting it.
AtMostOneMutation ==
  /\ pendingMutations <= 1
  /\ (request = "absent" <=> pendingMutations = 0)

HostCommitPrecedesPersistence == request = "persistence" => host = candidate

SessionPublicationWaitsForPersistence ==
  lastAction = "host-accept" => session = previousSession

HostRejectionPreservesAcceptedState ==
  lastAction = "host-reject" =>
    /\ session = previousSession
    /\ host = previousHost
    /\ durable = previousDurable

PersistenceFailureRetainsSession ==
  lastAction = "persistence-failed" =>
    /\ session = previousHost
    /\ host = previousHost
    /\ durable = previousDurable
    /\ issue = "write-failed"

DurableChangesOnlyAfterPersistence ==
  durable # previousDurable => lastAction = "persistence-succeeded"

NoLiveHistoryEviction == runningTerminal = initialRunningTerminal

FutureTerminalCapturesAcceptedDefault ==
  lastAction = "create-terminal" =>
    lastTerminal = previousHost

=============================================================================
