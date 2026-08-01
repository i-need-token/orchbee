------------------------ MODULE TerminalHostLifecycle ------------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Epochs, Clients, CreationIds, TerminalIds, Attachments

None == "none"
HostPhases == {"absent", "starting", "serving", "draining"}
TerminalPhases == {"absent", "starting", "running", "finishing", "exited"}
Authorities == {"desktop", "attachment"}

VARIABLES
  hostPhase,
  hostEpoch,
  upgradePending,
  connections,
  creationOf,
  terminalPhase,
  childExited,
  ptyDrained,
  terminalColumns,
  attachmentOwner,
  attachmentTerminal,
  attachmentEpoch,
  resizeLease,
  networkListener

hostVars == <<hostPhase, hostEpoch, upgradePending, connections, networkListener>>
terminalVars ==
  <<creationOf, terminalPhase, childExited, ptyDrained, terminalColumns>>
attachmentVars ==
  <<attachmentOwner, attachmentTerminal, attachmentEpoch, resizeLease>>
vars == <<hostVars, terminalVars, attachmentVars>>

PresentTerminals == {terminal \in TerminalIds: creationOf[terminal] # None}
UsedCreations == {creationOf[terminal]: terminal \in PresentTerminals}

Init ==
  /\ hostPhase = "absent"
  /\ hostEpoch = None
  /\ upgradePending = FALSE
  /\ connections = {}
  /\ creationOf = [terminal \in TerminalIds |-> None]
  /\ terminalPhase = [terminal \in TerminalIds |-> "absent"]
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
  /\ UNCHANGED <<upgradePending, connections, networkListener, terminalVars, attachmentVars>>

EndpointReady ==
  /\ hostPhase = "starting"
  /\ hostPhase' = "serving"
  /\ UNCHANGED <<hostEpoch, upgradePending, connections, networkListener,
                  terminalVars, attachmentVars>>

Connect(client) ==
  /\ hostPhase = "serving"
  /\ client \in Clients
  /\ client \notin connections
  /\ connections' = connections \cup {client}
  /\ UNCHANGED <<hostPhase, hostEpoch, upgradePending, networkListener,
                  terminalVars, attachmentVars>>

AttemptCreate(authority, client, creation, terminal) ==
  /\ authority \in Authorities
  /\ client \in connections
  /\ creation \in CreationIds
  /\ terminal \in TerminalIds
  /\ hostPhase = "serving"
  /\ ~upgradePending
  /\ IF authority = "desktop" /\ creation \notin UsedCreations
        /\ creationOf[terminal] = None
     THEN
       /\ creationOf' = [creationOf EXCEPT ![terminal] = creation]
       /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "starting"]
       /\ childExited' = [childExited EXCEPT ![terminal] = FALSE]
       /\ ptyDrained' = [ptyDrained EXCEPT ![terminal] = FALSE]
     ELSE
       /\ UNCHANGED <<creationOf, terminalPhase, childExited, ptyDrained>>
  /\ UNCHANGED <<hostVars, terminalColumns, attachmentVars>>

SpawnSucceeded(terminal) ==
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] = "starting"
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "running"]
  /\ UNCHANGED <<hostVars, creationOf, childExited, ptyDrained,
                  terminalColumns, attachmentVars>>

SpawnFailed(terminal) ==
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] = "starting"
  /\ creationOf' = [creationOf EXCEPT ![terminal] = None]
  /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "absent"]
  /\ childExited' = [childExited EXCEPT ![terminal] = FALSE]
  /\ ptyDrained' = [ptyDrained EXCEPT ![terminal] = FALSE]
  /\ UNCHANGED <<hostVars, terminalColumns, attachmentVars>>

AttemptAttach(authority, attachment, client, terminal, wantsLease) ==
  /\ authority \in Authorities
  /\ attachment \in Attachments
  /\ client \in connections
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] \in {"running", "finishing", "exited"}
  /\ wantsLease \in BOOLEAN
  /\ attachmentOwner[attachment] = None
  /\ IF authority = "desktop"
        /\ (~wantsLease \/ ~\E other \in Attachments:
              attachmentTerminal[other] = terminal /\ resizeLease[other])
     THEN
       /\ attachmentOwner' = [attachmentOwner EXCEPT ![attachment] = client]
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

Disconnect(client) ==
  /\ client \in connections
  /\ connections' = connections \ {client}
  /\ attachmentOwner' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = client THEN None
          ELSE attachmentOwner[attachment]]
  /\ attachmentTerminal' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = client THEN None
          ELSE attachmentTerminal[attachment]]
  /\ attachmentEpoch' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = client THEN None
          ELSE attachmentEpoch[attachment]]
  /\ resizeLease' =
       [attachment \in Attachments |->
          IF attachmentOwner[attachment] = client THEN FALSE
          ELSE resizeLease[attachment]]
  /\ UNCHANGED <<hostPhase, hostEpoch, upgradePending, networkListener, terminalVars>>

Resize(attachment) ==
  /\ attachment \in Attachments
  /\ attachmentOwner[attachment] # None
  /\ resizeLease[attachment]
  /\ LET terminal == attachmentTerminal[attachment]
     IN
       /\ terminalPhase[terminal] = "running"
       /\ terminalColumns' = [terminalColumns EXCEPT
            ![terminal] = IF @ = 1 THEN 2 ELSE 1]
  /\ UNCHANGED <<hostVars, creationOf, terminalPhase, childExited,
                  ptyDrained, attachmentVars>>

ChildExited(terminal) ==
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] \in {"running", "finishing"}
  /\ ~childExited[terminal]
  /\ childExited' = [childExited EXCEPT ![terminal] = TRUE]
  /\ terminalPhase' = [terminalPhase EXCEPT
       ![terminal] = IF ptyDrained[terminal] THEN "exited" ELSE "finishing"]
  /\ UNCHANGED <<hostVars, creationOf, ptyDrained, terminalColumns, attachmentVars>>

PtyDrained(terminal) ==
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] \in {"running", "finishing"}
  /\ ~ptyDrained[terminal]
  /\ ptyDrained' = [ptyDrained EXCEPT ![terminal] = TRUE]
  /\ terminalPhase' = [terminalPhase EXCEPT
       ![terminal] = IF childExited[terminal] THEN "exited" ELSE "finishing"]
  /\ UNCHANGED <<hostVars, creationOf, childExited, terminalColumns, attachmentVars>>

AttemptDispose(authority, client, terminal) ==
  /\ authority \in Authorities
  /\ client \in connections
  /\ terminal \in PresentTerminals
  /\ terminalPhase[terminal] = "exited"
  /\ IF authority = "desktop"
     THEN
       LET remaining == PresentTerminals \ {terminal}
       IN
         /\ creationOf' = [creationOf EXCEPT ![terminal] = None]
         /\ terminalPhase' = [terminalPhase EXCEPT ![terminal] = "absent"]
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
            ELSE
              /\ UNCHANGED <<hostPhase, connections>>
     ELSE
       /\ UNCHANGED <<hostPhase, connections, creationOf, terminalPhase,
                       childExited, ptyDrained, attachmentVars>>
  /\ UNCHANGED <<hostEpoch, upgradePending, networkListener, terminalColumns>>

RequestUpgrade ==
  /\ hostPhase = "serving"
  /\ ~upgradePending
  /\ upgradePending' = TRUE
  /\ IF PresentTerminals = {}
     THEN
       /\ hostPhase' = "draining"
       /\ connections' = {}
       /\ attachmentOwner' = [attachment \in Attachments |-> None]
       /\ attachmentTerminal' = [attachment \in Attachments |-> None]
       /\ attachmentEpoch' = [attachment \in Attachments |-> None]
       /\ resizeLease' = [attachment \in Attachments |-> FALSE]
     ELSE
       /\ UNCHANGED <<hostPhase, connections, attachmentVars>>
  /\ UNCHANGED <<hostEpoch, networkListener, terminalVars>>

StopHost ==
  /\ hostPhase = "draining"
  /\ PresentTerminals = {}
  /\ connections = {}
  /\ hostPhase' = "absent"
  /\ hostEpoch' = None
  /\ upgradePending' = FALSE
  /\ UNCHANGED <<connections, networkListener, terminalVars, attachmentVars>>

Next ==
  \/ \E epoch \in Epochs: Launch(epoch)
  \/ EndpointReady
  \/ \E client \in Clients: Connect(client)
  \/ \E authority \in Authorities, client \in Clients,
       creation \in CreationIds, terminal \in TerminalIds:
       AttemptCreate(authority, client, creation, terminal)
  \/ \E terminal \in TerminalIds: SpawnSucceeded(terminal)
  \/ \E terminal \in TerminalIds: SpawnFailed(terminal)
  \/ \E authority \in Authorities, attachment \in Attachments,
       client \in Clients, terminal \in TerminalIds, wantsLease \in BOOLEAN:
       AttemptAttach(authority, attachment, client, terminal, wantsLease)
  \/ \E attachment \in Attachments: Detach(attachment)
  \/ \E client \in Clients: Disconnect(client)
  \/ \E attachment \in Attachments: Resize(attachment)
  \/ \E terminal \in TerminalIds: ChildExited(terminal)
  \/ \E terminal \in TerminalIds: PtyDrained(terminal)
  \/ \E authority \in Authorities, client \in Clients, terminal \in TerminalIds:
       AttemptDispose(authority, client, terminal)
  \/ RequestUpgrade
  \/ StopHost

Spec == Init /\ [][Next]_vars /\ WF_vars(StopHost)

TypeOK ==
  /\ hostPhase \in HostPhases
  /\ hostEpoch \in Epochs \cup {None}
  /\ upgradePending \in BOOLEAN
  /\ connections \subseteq Clients
  /\ creationOf \in [TerminalIds -> CreationIds \cup {None}]
  /\ terminalPhase \in [TerminalIds -> TerminalPhases]
  /\ childExited \in [TerminalIds -> BOOLEAN]
  /\ ptyDrained \in [TerminalIds -> BOOLEAN]
  /\ terminalColumns \in [TerminalIds -> {1, 2}]
  /\ attachmentOwner \in [Attachments -> Clients \cup {None}]
  /\ attachmentTerminal \in [Attachments -> TerminalIds \cup {None}]
  /\ attachmentEpoch \in [Attachments -> Epochs \cup {None}]
  /\ resizeLease \in [Attachments -> BOOLEAN]
  /\ networkListener \in BOOLEAN

HostEpochIntegrity ==
  (hostPhase = "absent") = (hostEpoch = None)

TerminalIdentityIntegrity ==
  /\ \A terminal \in TerminalIds:
       (creationOf[terminal] = None) = (terminalPhase[terminal] = "absent")
  /\ Cardinality(UsedCreations) = Cardinality(PresentTerminals)

CompletionPhaseIntegrity ==
  \A terminal \in PresentTerminals:
    /\ terminalPhase[terminal] \in {"starting", "running"} =>
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
  [][\A client \in Clients:
       Disconnect(client) => UNCHANGED terminalVars]_vars

DetachPreservesTerminalState ==
  [][\A attachment \in Attachments:
       Detach(attachment) => UNCHANGED terminalVars]_vars

ScopedAuthorityCannotCreate ==
  [][\A client \in Clients, creation \in CreationIds, terminal \in TerminalIds:
       AttemptCreate("attachment", client, creation, terminal) =>
         UNCHANGED <<creationOf, terminalPhase, childExited, ptyDrained>>]_vars

DrainingEventuallyStops == hostPhase = "draining" ~> hostPhase = "absent"

=============================================================================
