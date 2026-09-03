------------------ MODULE LocalTerminalClientAuthority ------------------
EXTENDS FiniteSets, Naturals, TLC

\* Models the accepted product-level authority for one retained terminal on
\* the first Server-relayed path. Each Client slot has an independent session
\* generation. Several current sessions may attach as viewers, while one
\* generation-scoped controller grant combines input and resize authority.
\* Server and Worker loss may release temporary grants or freeze their registry
\* snapshot, but cannot end the retained terminal. Host loss is the only
\* modeled terminal-loss event. The model does not select takeover,
\* collaborative input, direct Worker attachment, or
\* durable recovery policy.

CONSTANTS Clients, NoClient, NoController, MaxSessionGeneration,
  MaxInputSequence, MaxGeometryRevision

SessionGenerations == 1..MaxSessionGeneration
SessionRefs == [client : Clients, generation : SessionGenerations]
ControllerValues == SessionRefs \cup {NoController}
TerminalStates == {"retained", "lost"}
Actions == {
  "init", "open-initial-session", "reconnect-session",
  "attach-first-viewer", "attach-additional-viewer",
  "acquire-unowned-controller", "accept-controller-input",
  "reject-non-controller-input", "accept-controller-resize",
  "reject-non-controller-resize", "disconnect-controller",
  "disconnect-viewer", "disconnect-unattached-session",
  "cleanup-disconnected-session",
  "cleanup-stale-session", "cleanup-stale-after-regrant",
  "lose-server", "lose-worker", "lose-host"
}

VARIABLES
  serverUp,
  workerUp,
  hostUp,
  terminalState,
  \* Last Server-owned session-registry facts. They are not transport
  \* liveness: ClientTransportActive also requires serverUp.
  sessionOpen,
  sessionGeneration,
  viewers,
  controller,
  pendingCleanup,
  inputSequence,
  geometryRevision,
  priorController,
  priorInputSequence,
  priorGeometryRevision,
  priorTerminalState,
  lastAction,
  lastSession

DomainVars == <<
  serverUp,
  workerUp,
  hostUp,
  terminalState,
  sessionOpen,
  sessionGeneration,
  viewers,
  controller,
  pendingCleanup,
  inputSequence,
  geometryRevision
>>

AuditVars == <<
  priorController,
  priorInputSequence,
  priorGeometryRevision,
  priorTerminalState,
  lastAction,
  lastSession
>>

vars == <<DomainVars, AuditVars>>

CurrentSession(client) ==
  [client |-> client, generation |-> sessionGeneration[client]]

SessionRecordIsCurrent(session) ==
  /\ session \in SessionRefs
  /\ sessionOpen[session.client]
  /\ sessionGeneration[session.client] = session.generation

ClientTransportActive(session) ==
  /\ serverUp
  /\ SessionRecordIsCurrent(session)

TerminalPathAvailable ==
  /\ serverUp
  /\ workerUp
  /\ hostUp
  /\ terminalState = "retained"

ControllerClient ==
  IF controller = NoController THEN NoClient ELSE controller.client

ControllerGeneration ==
  IF controller = NoController THEN 0 ELSE controller.generation

EffectiveInputControllers ==
  IF TerminalPathAvailable /\ controller # NoController
  THEN {controller}
  ELSE {}

EffectiveResizeControllers ==
  IF TerminalPathAvailable /\ controller # NoController
  THEN {controller}
  ELSE {}

Record(action, session) ==
  /\ priorController' = controller
  /\ priorInputSequence' = inputSequence
  /\ priorGeometryRevision' = geometryRevision
  /\ priorTerminalState' = terminalState
  /\ lastAction' = action
  /\ lastSession' = session

Init ==
  /\ serverUp = TRUE
  /\ workerUp = TRUE
  /\ hostUp = TRUE
  /\ terminalState = "retained"
  /\ sessionOpen = [client \in Clients |-> FALSE]
  /\ sessionGeneration = [client \in Clients |-> 0]
  /\ viewers = {}
  /\ controller = NoController
  /\ pendingCleanup = {}
  /\ inputSequence = 0
  /\ geometryRevision = 0
  /\ priorController = controller
  /\ priorInputSequence = inputSequence
  /\ priorGeometryRevision = geometryRevision
  /\ priorTerminalState = terminalState
  /\ lastAction = "init"
  /\ lastSession = NoController

OpenInitialSession(client) ==
  /\ serverUp
  /\ ~sessionOpen[client]
  /\ sessionGeneration[client] = 0
  /\ sessionOpen' = [sessionOpen EXCEPT ![client] = TRUE]
  /\ sessionGeneration' = [sessionGeneration EXCEPT ![client] = 1]
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, viewers,
       controller, pendingCleanup, inputSequence, geometryRevision>>
  /\ Record(
       "open-initial-session",
       [client |-> client, generation |-> 1]
     )

ReconnectSession(client) ==
  /\ serverUp
  /\ ~sessionOpen[client]
  /\ sessionGeneration[client] \in 1..(MaxSessionGeneration - 1)
  /\ LET nextGeneration == sessionGeneration[client] + 1
     IN
       /\ sessionOpen' = [sessionOpen EXCEPT ![client] = TRUE]
       /\ sessionGeneration' =
            [sessionGeneration EXCEPT ![client] = nextGeneration]
       /\ Record(
            "reconnect-session",
            [client |-> client, generation |-> nextGeneration]
          )
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, viewers,
       controller, pendingCleanup, inputSequence, geometryRevision>>

AttachFirstViewer(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ viewers = {}
  /\ LET session == CurrentSession(client)
     IN
       /\ session \notin viewers
       /\ viewers' = viewers \cup {session}
       /\ Record("attach-first-viewer", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, controller, pendingCleanup, inputSequence,
       geometryRevision>>

AttachAdditionalViewer(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ viewers # {}
  /\ LET session == CurrentSession(client)
     IN
       /\ session \notin viewers
       /\ viewers' = viewers \cup {session}
       /\ Record("attach-additional-viewer", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, controller, pendingCleanup, inputSequence,
       geometryRevision>>

AcquireUnownedController(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ controller = NoController
  /\ LET session == CurrentSession(client)
     IN
       /\ session \in viewers
       /\ controller' = session
       /\ Record("acquire-unowned-controller", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, viewers, pendingCleanup, inputSequence,
       geometryRevision>>

AcceptControllerInput(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ inputSequence < MaxInputSequence
  /\ LET session == CurrentSession(client)
     IN
       /\ session = controller
       /\ inputSequence' = inputSequence + 1
       /\ Record("accept-controller-input", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, viewers, controller, pendingCleanup,
       geometryRevision>>

RejectNonControllerInput(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session \in viewers
       /\ session # controller
       /\ Record("reject-non-controller-input", session)
  /\ UNCHANGED DomainVars

AcceptControllerResize(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ geometryRevision < MaxGeometryRevision
  /\ LET session == CurrentSession(client)
     IN
       /\ session = controller
       /\ geometryRevision' = geometryRevision + 1
       /\ Record("accept-controller-resize", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, viewers, controller, pendingCleanup,
       inputSequence>>

RejectNonControllerResize(client) ==
  /\ TerminalPathAvailable
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session \in viewers
       /\ session # controller
       /\ Record("reject-non-controller-resize", session)
  /\ UNCHANGED DomainVars

DisconnectController(client) ==
  /\ serverUp
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session = controller
       /\ sessionOpen' = [sessionOpen EXCEPT ![client] = FALSE]
       /\ viewers' = viewers \ {session}
       /\ controller' = NoController
       /\ pendingCleanup' = pendingCleanup \cup {session}
       /\ Record("disconnect-controller", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState,
       sessionGeneration, inputSequence, geometryRevision>>

DisconnectViewer(client) ==
  /\ serverUp
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session \in viewers
       /\ session # controller
       /\ sessionOpen' = [sessionOpen EXCEPT ![client] = FALSE]
       /\ viewers' = viewers \ {session}
       /\ pendingCleanup' = pendingCleanup \cup {session}
       /\ Record("disconnect-viewer", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState,
       sessionGeneration, controller, inputSequence, geometryRevision>>

DisconnectUnattachedSession(client) ==
  /\ serverUp
  /\ sessionOpen[client]
  /\ LET session == CurrentSession(client)
     IN
       /\ session \notin viewers
       /\ sessionOpen' = [sessionOpen EXCEPT ![client] = FALSE]
       /\ pendingCleanup' = pendingCleanup \cup {session}
       /\ Record("disconnect-unattached-session", session)
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState,
       sessionGeneration, viewers, controller, inputSequence,
       geometryRevision>>

CleanupDisconnectedSession(session) ==
  /\ serverUp
  /\ session \in pendingCleanup
  /\ sessionGeneration[session.client] = session.generation
  /\ ~sessionOpen[session.client]
  /\ pendingCleanup' = pendingCleanup \ {session}
  /\ viewers' = viewers \ {session}
  /\ controller' =
       IF controller = session THEN NoController ELSE controller
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, inputSequence, geometryRevision>>
  /\ Record("cleanup-disconnected-session", session)

HasNewControllerFor(session) ==
  /\ controller \in SessionRefs
  /\ ControllerClient = session.client
  /\ ControllerGeneration > session.generation

CleanupStaleSession(session) ==
  /\ serverUp
  /\ session \in pendingCleanup
  /\ sessionGeneration[session.client] > session.generation
  /\ ~HasNewControllerFor(session)
  /\ pendingCleanup' = pendingCleanup \ {session}
  /\ viewers' = viewers \ {session}
  /\ controller' =
       IF controller = session THEN NoController ELSE controller
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, inputSequence, geometryRevision>>
  /\ Record("cleanup-stale-session", session)

CleanupStaleAfterRegrant(session) ==
  /\ serverUp
  /\ session \in pendingCleanup
  /\ sessionGeneration[session.client] > session.generation
  /\ HasNewControllerFor(session)
  /\ pendingCleanup' = pendingCleanup \ {session}
  /\ viewers' = viewers \ {session}
  /\ controller' =
       IF controller = session THEN NoController ELSE controller
  /\ UNCHANGED <<serverUp, workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, inputSequence, geometryRevision>>
  /\ Record("cleanup-stale-after-regrant", session)

LoseServer ==
  /\ serverUp
  /\ serverUp' = FALSE
  /\ controller' \in {controller, NoController}
  /\ UNCHANGED <<workerUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, viewers, pendingCleanup,
       inputSequence, geometryRevision>>
  /\ Record("lose-server", controller)

LoseWorker ==
  /\ workerUp
  /\ workerUp' = FALSE
  /\ controller' \in {controller, NoController}
  /\ UNCHANGED <<serverUp, hostUp, terminalState, sessionOpen,
       sessionGeneration, viewers, pendingCleanup,
       inputSequence, geometryRevision>>
  /\ Record("lose-worker", controller)

LoseHost ==
  /\ hostUp
  /\ terminalState = "retained"
  /\ hostUp' = FALSE
  /\ terminalState' = "lost"
  /\ UNCHANGED <<serverUp, workerUp, sessionOpen, sessionGeneration,
       viewers, controller, pendingCleanup, inputSequence,
       geometryRevision>>
  /\ Record("lose-host", controller)

CleanupAny ==
  \/ \E session \in SessionRefs: CleanupDisconnectedSession(session)
  \/ \E session \in SessionRefs: CleanupStaleSession(session)
  \/ \E session \in SessionRefs: CleanupStaleAfterRegrant(session)

Next ==
  \/ \E client \in Clients: OpenInitialSession(client)
  \/ \E client \in Clients: ReconnectSession(client)
  \/ \E client \in Clients: AttachFirstViewer(client)
  \/ \E client \in Clients: AttachAdditionalViewer(client)
  \/ \E client \in Clients: AcquireUnownedController(client)
  \/ \E client \in Clients: AcceptControllerInput(client)
  \/ \E client \in Clients: RejectNonControllerInput(client)
  \/ \E client \in Clients: AcceptControllerResize(client)
  \/ \E client \in Clients: RejectNonControllerResize(client)
  \/ \E client \in Clients: DisconnectController(client)
  \/ \E client \in Clients: DisconnectViewer(client)
  \/ \E client \in Clients: DisconnectUnattachedSession(client)
  \/ CleanupAny
  \/ LoseServer
  \/ LoseWorker
  \/ LoseHost

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CleanupAny)

TypeOK ==
  /\ NoClient \notin Clients
  /\ NoController \notin SessionRefs
  /\ MaxSessionGeneration >= 2
  /\ serverUp \in BOOLEAN
  /\ workerUp \in BOOLEAN
  /\ hostUp \in BOOLEAN
  /\ terminalState \in TerminalStates
  /\ sessionOpen \in [Clients -> BOOLEAN]
  /\ sessionGeneration \in [Clients -> 0..MaxSessionGeneration]
  /\ viewers \subseteq SessionRefs
  /\ controller \in ControllerValues
  /\ pendingCleanup \subseteq SessionRefs
  /\ inputSequence \in 0..MaxInputSequence
  /\ geometryRevision \in 0..MaxGeometryRevision
  /\ priorController \in ControllerValues
  /\ priorInputSequence \in 0..MaxInputSequence
  /\ priorGeometryRevision \in 0..MaxGeometryRevision
  /\ priorTerminalState \in TerminalStates
  /\ lastAction \in Actions
  /\ lastSession \in ControllerValues

ViewerBelongsToCurrentSession ==
  \A session \in viewers: SessionRecordIsCurrent(session)

ControllerGrantBelongsToViewer ==
  controller = NoController \/ controller \in viewers

ServerLossEndsEffectiveClientTransport ==
  ~serverUp =>
    \A session \in SessionRefs: ~ClientTransportActive(session)

InputAndResizeHaveOneController ==
  /\ EffectiveInputControllers = EffectiveResizeControllers
  /\ Cardinality(EffectiveInputControllers) <= 1

AcquisitionNeverTakesOver ==
  lastAction = "acquire-unowned-controller" =>
    /\ priorController = NoController
    /\ controller = lastSession

NoAutomaticControllerTransfer ==
  controller # priorController =>
    \/ /\ lastAction = "acquire-unowned-controller"
       /\ priorController = NoController
       /\ controller = lastSession
    \/ /\ lastAction = "disconnect-controller"
       /\ priorController = lastSession
       /\ controller = NoController
    \/ /\ lastAction \in {"lose-server", "lose-worker"}
       /\ controller = NoController

ControllerInputIsExact ==
  lastAction = "accept-controller-input" =>
    /\ priorController = lastSession
    /\ priorController # NoController
    /\ inputSequence = priorInputSequence + 1

ControllerResizeIsExact ==
  lastAction = "accept-controller-resize" =>
    /\ priorController = lastSession
    /\ priorController # NoController
    /\ geometryRevision = priorGeometryRevision + 1

NonControllerInputPreservesTerminalInput ==
  lastAction = "reject-non-controller-input" =>
    /\ lastSession # priorController
    /\ inputSequence = priorInputSequence

NonControllerResizePreservesGeometry ==
  lastAction = "reject-non-controller-resize" =>
    /\ lastSession # priorController
    /\ geometryRevision = priorGeometryRevision

OnlyControllerInputAdvances ==
  inputSequence # priorInputSequence =>
    lastAction = "accept-controller-input"

OnlyControllerResizeAdvances ==
  geometryRevision # priorGeometryRevision =>
    lastAction = "accept-controller-resize"

ControllerDisconnectReleasesAuthority ==
  lastAction = "disconnect-controller" =>
    /\ priorController = lastSession
    /\ controller = NoController
    /\ lastSession \notin viewers

StaleCleanupPreservesNewAuthority ==
  lastAction = "cleanup-stale-after-regrant" =>
    /\ priorController # NoController
    /\ priorController # lastSession
    /\ controller = priorController

ServerAndWorkerLossNeverTransferAuthority ==
  lastAction \in {"lose-server", "lose-worker"} =>
    controller \in {priorController, NoController}

NonHostLossRetainsTerminal ==
  lastAction \in {
    "disconnect-controller", "disconnect-viewer",
    "disconnect-unattached-session",
    "lose-server", "lose-worker"
  } => terminalState = priorTerminalState

OnlyHostLossChangesTerminalState ==
  terminalState # priorTerminalState => lastAction = "lose-host"

HostLossReportsTerminalLost ==
  lastAction = "lose-host" => terminalState = "lost"

LostTerminalAcceptsNoFurtherControl ==
  terminalState = "lost" => ~TerminalPathAvailable

PendingCleanupEventuallySettles ==
  [](serverUp /\ pendingCleanup # {} =>
      <>(~serverUp \/ pendingCleanup = {}))

=============================================================================
