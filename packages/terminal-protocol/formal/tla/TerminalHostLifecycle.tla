------------------------ MODULE TerminalHostLifecycle ------------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Epochs, Connections, Clients, CreationIds, TerminalIds, Attachments

None == "none"
HostPhases == {"absent", "starting", "serving", "draining"}
TerminalPhases == {"absent", "running", "finishing", "exited"}
Authorities == {"desktop", "attachment"}

VARIABLES
  hostPhase,
  hostEpoch,
  upgradePending,
  connections,
  clientOf,
  creationOf,
  terminalPhase,
  runtimePresent,
  childExited,
  ptyDrained,
  terminalColumns,
  attachmentOwner,
  attachmentTerminal,
  attachmentEpoch,
  resizeLease,
  networkListener

connectionVars == <<connections, clientOf>>
hostVars == <<hostPhase, hostEpoch, upgradePending, connectionVars, networkListener>>
terminalVars ==
  <<creationOf, terminalPhase, runtimePresent, childExited, ptyDrained,
    terminalColumns>>
attachmentVars ==
  <<attachmentOwner, attachmentTerminal, attachmentEpoch, resizeLease>>
vars == <<hostVars, terminalVars, attachmentVars>>

PresentTerminals == {terminal \in TerminalIds: creationOf[terminal] # None}
PresentRuntimes == {terminal \in TerminalIds: runtimePresent[terminal]}
UsedCreations == {creationOf[terminal]: terminal \in PresentTerminals}

Init ==
  /\ hostPhase = "absent"
  /\ hostEpoch = None
  /\ upgradePending = FALSE
  /\ connections = {}
  /\ clientOf = [connection \in Connections |-> None]
  /\ creationOf = [terminal \in TerminalIds |-> None]
  /\ terminalPhase = [terminal \in TerminalIds |-> "absent"]
  /\ runtimePresent = [terminal \in TerminalIds |-> FALSE]
  /\ childExited = [terminal \in TerminalIds |-> FALSE]
  /\ ptyDrained = [terminal \in TerminalIds |-> FALSE]
  /\ terminalColumns = [terminal \in TerminalIds |-> 1]
  /\ attachmentOwner = [attachment \in Attachments |-> None]
  /\ attachmentTerminal = [attachment \in Attachments |-> None]
  /\ attachmentEpoch = [attachment \in Attachments |-> None]
  /\ resizeLease = [attachment \in Attachments |-> FALSE]
  /\ networkListener = FALSE

Launch(epoch) ==
  /\ hostPhase = "absent"
  /\ epoch \in Epochs
  /\ hostPhase' = "starting"
  /\ hostEpoch' = epoch
  /\ UNCHANGED <<upgradePending, connectionVars, networkListener,
                  terminalVars, attachmentVars>>

EndpointReady ==
  /\ hostPhase = "starting"
  /\ hostPhase' = "serving"
  /\ UNCHANGED <<hostEpoch, upgradePending, connectionVars, networkListener,
                  terminalVars, attachmentVars>>

Connect(connection, client) ==
  /\ hostPhase = "serving"
  /\ connection \in Connections
  /\ client \in Clients
  /\ connection \notin connections
  /\ connections' = connections \cup {connection}
  /\ clientOf' = [clientOf EXCEPT ![connection] = client]
  /\ UNCHANGED <<hostPhase, hostEpoch, upgradePending, networkListener,
                  terminalVars, attachmentVars>>

AttemptCreate(authority, connection, creation, terminal, spawnSucceeds) ==
  /\ authority \in Authorities
  /\ connection \in connections
  /\ creation \in CreationIds
  /\ terminal \in TerminalIds
  /\ spawnSucceeds \in BOOLEAN
  /\ hostPhase = "serving"
  /\ ~upgradePending
  /\ IF authority = "desktop" /\ spawnSucceeds
        /\ creation \notin UsedCreations /\ creationOf[terminal] = None
     THEN
       \* Runtime creation and both registry insertions are one observable commit.
       /\ creationOf' = [creationOf EXCEPT ![terminal] = creation]
       /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "running"]
       /\ runtimePresent' = [runtimePresent EXCEPT ![terminal] = TRUE]
       /\ childExited' = [childExited EXCEPT ![terminal] = FALSE]
       /\ ptyDrained' = [ptyDrained EXCEPT ![terminal] = FALSE]
     ELSE
       \* Rejection, idempotent replay, allocation failure, and spawn failure
       \* leave both registries unchanged.
       /\ UNCHANGED <<creationOf, terminalPhase, runtimePresent,
                       childExited, ptyDrained>>
  /\ UNCHANGED <<hostVars, terminalColumns, attachmentVars>>

AttemptAttach(authority, attachment, connection, terminal, wantsLease) ==
  /\ authority \in Authorities
  /\ attachment \in Attachments
  /\ connection \in connections
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] \in {"running", "finishing", "exited"}
  /\ wantsLease \in BOOLEAN
  /\ attachmentOwner[attachment] = None
  /\ IF authority = "desktop"
        /\ (~wantsLease \/ ~\E other \in Attachments:
              attachmentTerminal[other] = terminal /\ resizeLease[other])
     THEN
       /\ attachmentOwner' = [attachmentOwner EXCEPT ![attachment] = connection]
       /\ attachmentTerminal' = [attachmentTerminal EXCEPT ![attachment] = terminal]
       /\ attachmentEpoch' = [attachmentEpoch EXCEPT ![attachment] = hostEpoch]
       /\ resizeLease' = [resizeLease EXCEPT ![attachment] = wantsLease]
     ELSE
       /\ UNCHANGED attachmentVars
  /\ UNCHANGED <<hostVars, terminalVars>>

Detach(attachment) ==
  /\ attachment \in Attachments
  /\ attachmentOwner[attachment] # None
  /\ attachmentOwner' = [attachmentOwner EXCEPT ![attachment] = None]
  /\ attachmentTerminal' = [attachmentTerminal EXCEPT ![attachment] = None]
  /\ attachmentEpoch' = [attachmentEpoch EXCEPT ![attachment] = None]
  /\ resizeLease' = [resizeLease EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<hostVars, terminalVars>>

Disconnect(connection) ==
  /\ connection \in connections
  /\ LET remaining == connections \ {connection}
     IN
       /\ connections' = remaining
       /\ clientOf' = [clientOf EXCEPT ![connection] = None]
       /\ IF remaining = {} /\ PresentTerminals = {} /\ hostPhase = "serving"
          THEN
            /\ hostPhase' = "absent"
            /\ hostEpoch' = None
            /\ upgradePending' = FALSE
          ELSE
            /\ UNCHANGED <<hostPhase, hostEpoch, upgradePending>>
  /\ attachmentOwner' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = connection THEN None
          ELSE attachmentOwner[attachment]]
  /\ attachmentTerminal' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = connection THEN None
          ELSE attachmentTerminal[attachment]]
  /\ attachmentEpoch' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = connection THEN None
          ELSE attachmentEpoch[attachment]]
  /\ resizeLease' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = connection THEN FALSE
          ELSE resizeLease[attachment]]
  /\ UNCHANGED <<networkListener, terminalVars>>

Resize(attachment) ==
  /\ attachment \in Attachments
  /\ attachmentOwner[attachment] # None
  /\ resizeLease[attachment]
  /\ LET terminal == attachmentTerminal[attachment]
     IN
       /\ terminalPhase[terminal] = "running"
       /\ terminalColumns' = [terminalColumns EXCEPT
            ![terminal] = IF @ = 1 THEN 2 ELSE 1]
  /\ UNCHANGED <<hostVars, creationOf, terminalPhase, runtimePresent,
                  childExited, ptyDrained, attachmentVars>>

ChildExited(terminal) ==
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] \in {"running", "finishing"}
  /\ ~childExited[terminal]
  /\ childExited' = [childExited EXCEPT ![terminal] = TRUE]
  /\ terminalPhase' = [terminalPhase EXCEPT
       ![terminal] = IF ptyDrained[terminal] THEN "exited" ELSE "finishing"]
  /\ UNCHANGED <<hostVars, creationOf, runtimePresent, ptyDrained,
                  terminalColumns, attachmentVars>>

PtyDrained(terminal) ==
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] \in {"running", "finishing"}
  /\ ~ptyDrained[terminal]
  /\ ptyDrained' = [ptyDrained EXCEPT ![terminal] = TRUE]
  /\ terminalPhase' = [terminalPhase EXCEPT
       ![terminal] = IF childExited[terminal] THEN "exited" ELSE "finishing"]
  /\ UNCHANGED <<hostVars, creationOf, runtimePresent, childExited,
                  terminalColumns, attachmentVars>>

AttemptDispose(authority, connection, terminal) ==
  /\ authority \in Authorities
  /\ connection \in connections
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] = "exited"
  /\ IF authority = "desktop"
     THEN
       LET remaining == PresentTerminals \ {terminal}
       IN
         /\ creationOf' = [creationOf EXCEPT ![terminal] = None]
         /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "absent"]
         /\ runtimePresent' = [runtimePresent EXCEPT ![terminal] = FALSE]
         /\ childExited' = [childExited EXCEPT ![terminal] = FALSE]
         /\ ptyDrained' = [ptyDrained EXCEPT ![terminal] = FALSE]
         /\ attachmentOwner' =
              [attachment \in Attachments |->
                 IF attachmentTerminal[attachment] = terminal THEN None
                 ELSE attachmentOwner[attachment]]
         /\ attachmentTerminal' =
              [attachment \in Attachments |->
                 IF attachmentTerminal[attachment] = terminal THEN None
                 ELSE attachmentTerminal[attachment]]
         /\ attachmentEpoch' =
              [attachment \in Attachments |->
                 IF attachmentTerminal[attachment] = terminal THEN None
                 ELSE attachmentEpoch[attachment]]
         /\ resizeLease' =
              [attachment \in Attachments |->
                 IF attachmentTerminal[attachment] = terminal THEN FALSE
                 ELSE resizeLease[attachment]]
         /\ IF upgradePending /\ remaining = {}
            THEN
              /\ hostPhase' = "draining"
              /\ connections' = {}
              /\ clientOf' = [candidate \in Connections |-> None]
            ELSE
              /\ UNCHANGED <<hostPhase, connectionVars>>
     ELSE
       /\ UNCHANGED <<hostPhase, connectionVars, creationOf, terminalPhase,
                       runtimePresent, childExited, ptyDrained, attachmentVars>>
  /\ UNCHANGED <<hostEpoch, upgradePending, networkListener, terminalColumns>>

RequestUpgrade ==
  /\ hostPhase = "serving"
  /\ ~upgradePending
  /\ upgradePending' = TRUE
  /\ IF PresentTerminals = {}
     THEN
       /\ hostPhase' = "draining"
       /\ connections' = {}
       /\ clientOf' = [connection \in Connections |-> None]
       /\ attachmentOwner' = [attachment \in Attachments |-> None]
       /\ attachmentTerminal' = [attachment \in Attachments |-> None]
       /\ attachmentEpoch' = [attachment \in Attachments |-> None]
       /\ resizeLease' = [attachment \in Attachments |-> FALSE]
     ELSE
       /\ UNCHANGED <<hostPhase, connectionVars, attachmentVars>>
  /\ UNCHANGED <<hostEpoch, networkListener, terminalVars>>

StopHost ==
  /\ hostPhase = "draining"
  /\ PresentTerminals = {}
  /\ connections = {}
  /\ hostPhase' = "absent"
  /\ hostEpoch' = None
  /\ upgradePending' = FALSE
  /\ UNCHANGED <<connectionVars, networkListener, terminalVars, attachmentVars>>

Next ==
  \/ \E epoch \in Epochs: Launch(epoch)
  \/ EndpointReady
  \/ \E connection \in Connections, client \in Clients:
       Connect(connection, client)
  \/ \E authority \in Authorities, connection \in Connections,
       creation \in CreationIds, terminal \in TerminalIds,
       spawnSucceeds \in BOOLEAN:
       AttemptCreate(authority, connection, creation, terminal, spawnSucceeds)
  \/ \E authority \in Authorities, attachment \in Attachments,
       connection \in Connections, terminal \in TerminalIds, wantsLease \in BOOLEAN:
       AttemptAttach(authority, attachment, connection, terminal, wantsLease)
  \/ \E attachment \in Attachments: Detach(attachment)
  \/ \E connection \in Connections: Disconnect(connection)
  \/ \E attachment \in Attachments: Resize(attachment)
  \/ \E terminal \in TerminalIds: ChildExited(terminal)
  \/ \E terminal \in TerminalIds: PtyDrained(terminal)
  \/ \E authority \in Authorities, connection \in Connections,
       terminal \in TerminalIds:
       AttemptDispose(authority, connection, terminal)
  \/ RequestUpgrade
  \/ StopHost

Spec == Init /\ [][Next]_vars /\ WF_vars(StopHost)

TypeOK ==
  /\ hostPhase \in HostPhases
  /\ hostEpoch \in Epochs \cup {None}
  /\ upgradePending \in BOOLEAN
  /\ connections \subseteq Connections
  /\ clientOf \in [Connections -> Clients \cup {None}]
  /\ creationOf \in [TerminalIds -> CreationIds \cup {None}]
  /\ terminalPhase \in [TerminalIds -> TerminalPhases]
  /\ runtimePresent \in [TerminalIds -> BOOLEAN]
  /\ childExited \in [TerminalIds -> BOOLEAN]
  /\ ptyDrained \in [TerminalIds -> BOOLEAN]
  /\ terminalColumns \in [TerminalIds -> {1, 2}]
  /\ attachmentOwner \in [Attachments -> Connections \cup {None}]
  /\ attachmentTerminal \in [Attachments -> TerminalIds \cup {None}]
  /\ attachmentEpoch \in [Attachments -> Epochs \cup {None}]
  /\ resizeLease \in [Attachments -> BOOLEAN]
  /\ networkListener \in BOOLEAN

HostEpochIntegrity ==
  (hostPhase = "absent") = (hostEpoch = None)

ConnectionIdentityIntegrity ==
  \A connection \in Connections:
    (connection \in connections) = (clientOf[connection] # None)

TerminalIdentityIntegrity ==
  /\ \A terminal \in TerminalIds:
       (creationOf[terminal] = None) = (terminalPhase[terminal] = "absent")
  /\ Cardinality(UsedCreations) = Cardinality(PresentTerminals)

RuntimeRegistryIntegrity ==
  PresentRuntimes = PresentTerminals

CompletionPhaseIntegrity ==
  \A terminal \in PresentTerminals:
    /\ terminalPhase[terminal] = "running" =>
         ~childExited[terminal] /\ ~ptyDrained[terminal]
    /\ terminalPhase[terminal] = "finishing" =>
         childExited[terminal] # ptyDrained[terminal]
    /\ terminalPhase[terminal] = "exited" =>
         childExited[terminal] /\ ptyDrained[terminal]

AttachmentIntegrity ==
  \A attachment \in Attachments:
    /\ (attachmentOwner[attachment] = None) =
         (attachmentTerminal[attachment] = None)
    /\ attachmentOwner[attachment] # None =>
         /\ attachmentOwner[attachment] \in connections
         /\ attachmentTerminal[attachment] \in PresentTerminals
         /\ attachmentEpoch[attachment] = hostEpoch

ExclusiveResizeLease ==
  \A terminal \in TerminalIds:
    Cardinality({attachment \in Attachments:
      attachmentTerminal[attachment] = terminal /\ resizeLease[attachment]}) <= 1

GracefulStopRequiresNoTerminal ==
  hostPhase \in {"absent", "draining"} => PresentTerminals = {}

NoNetworkListener == ~networkListener

DisconnectPreservesTerminalState ==
  [][\A connection \in Connections:
       Disconnect(connection) => UNCHANGED terminalVars]_vars

EmptyFinalDisconnectStopsHost ==
  [][\A connection \in Connections:
       Disconnect(connection) /\ connections \ {connection} = {}
         /\ PresentTerminals = {} /\ hostPhase = "serving" =>
           hostPhase' = "absent"]_vars

DisconnectPreservesOtherAttachments ==
  [][\A connection \in Connections:
       Disconnect(connection) =>
         \A attachment \in Attachments:
           attachmentOwner[attachment] # connection =>
             UNCHANGED <<attachmentOwner[attachment],
                         attachmentTerminal[attachment],
                         attachmentEpoch[attachment],
                         resizeLease[attachment]>>]_vars

FailedCreateIsAtomic ==
  [][\A authority \in Authorities, connection \in Connections,
       creation \in CreationIds, terminal \in TerminalIds:
       AttemptCreate(authority, connection, creation, terminal, FALSE) =>
         UNCHANGED terminalVars]_vars

TargetedDisposePreservesOtherTerminals ==
  [][\A connection \in Connections, terminal \in TerminalIds:
       AttemptDispose("desktop", connection, terminal) =>
         \A other \in TerminalIds \ {terminal}:
           UNCHANGED <<creationOf[other], terminalPhase[other],
                       runtimePresent[other], childExited[other],
                       ptyDrained[other], terminalColumns[other]>>]_vars

DetachPreservesTerminalState ==
  [][\A attachment \in Attachments:
       Detach(attachment) => UNCHANGED terminalVars]_vars

ScopedAuthorityCannotCreate ==
  [][\A connection \in Connections, creation \in CreationIds,
       terminal \in TerminalIds, spawnSucceeds \in BOOLEAN:
       AttemptCreate("attachment", connection, creation, terminal, spawnSucceeds) =>
         UNCHANGED terminalVars]_vars

DrainingEventuallyStops == hostPhase = "draining" ~> hostPhase = "absent"

=============================================================================
