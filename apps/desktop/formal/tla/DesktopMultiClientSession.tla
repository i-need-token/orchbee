----------------------- MODULE DesktopMultiClientSession -----------------------
EXTENDS FiniteSets, Integers, TLC

Clients == {"client-a", "client-b"}
Documents == {"document-a", "document-b"}
Attachments == {"attachment-a"}
Intents == {"none", "provision", "stop"}
EffectIntents == {"provision", "stop"}
NoClient == "no-client"
NoAttachmentOwner == "no-attachment-owner"
NoRevision == -1
MaxRevision == 1
Revisions == 0..MaxRevision
ObservedRevisions == Revisions \cup {NoRevision}
Phases == {"accepting", "quiescing", "unavailable"}
ProposalStates == {"idle", "queued", "active", "completed"}
ProposalOutcomes == {"none", "committed", "converged", "stale",
  "rejected", "unavailable", "suppressed"}
EffectPhases == {"none", "durable", "dispatched", "completed"}
VARIABLES
  phase, exitOwner, hasDocument, revision, documentValue, documentIntent,
  publishedRevision, connected, proposalState, proposalBase, proposalValue,
  proposalIntent, proposalOutcome, queued, activeClient, observedRevision,
  effectPhase, effectRevision, effectIntent, effectPublished, effectObserved,
  terminalAlive, attachmentOwner

vars == <<phase, exitOwner, hasDocument, revision, documentValue,
  documentIntent, publishedRevision, connected, proposalState, proposalBase,
  proposalValue, proposalIntent, proposalOutcome, queued, activeClient,
  observedRevision, effectPhase, effectRevision, effectIntent,
  effectPublished, effectObserved, terminalAlive, attachmentOwner>>

Init ==
  /\ phase = "accepting"
  /\ exitOwner = NoClient
  /\ hasDocument \in BOOLEAN
  /\ revision = 0
  /\ documentValue = "document-a"
  /\ documentIntent = "none"
  /\ publishedRevision = IF hasDocument THEN 0 ELSE NoRevision
  /\ connected = [client \in Clients |-> FALSE]
  /\ proposalState = [client \in Clients |-> "idle"]
  /\ proposalBase = [client \in Clients |-> NoRevision]
  /\ proposalValue = [client \in Clients |-> documentValue]
  /\ proposalIntent = [client \in Clients |-> "none"]
  /\ proposalOutcome = [client \in Clients |-> "none"]
  /\ queued = {}
  /\ activeClient = NoClient
  /\ observedRevision = [client \in Clients |-> NoRevision]
  /\ effectPhase = "none"
  /\ effectRevision = 0
  /\ effectIntent = "none"
  /\ effectPublished = FALSE
  /\ effectObserved = [client \in Clients |-> FALSE]
  /\ terminalAlive = FALSE
  /\ attachmentOwner = [attachment \in Attachments |-> NoAttachmentOwner]

Connect(client) ==
  /\ ~connected[client]
  /\ proposalState[client] = "idle"
  /\ connected' = [connected EXCEPT ![client] = TRUE]
  /\ observedRevision' = [observedRevision EXCEPT ![client] = publishedRevision]
  /\ effectObserved' = [effectObserved EXCEPT
       ![client] = effectPublished /\ publishedRevision >= effectRevision]
  /\ proposalState' = [proposalState EXCEPT ![client] = "idle"]
  /\ proposalOutcome' = [proposalOutcome EXCEPT ![client] = "none"]
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, proposalBase, proposalValue,
       proposalIntent, queued, activeClient, effectPhase, effectRevision,
       effectIntent, effectPublished, terminalAlive, attachmentOwner>>

Submit(client, base, value, intent) ==
  /\ connected[client]
  /\ proposalState[client] = "idle"
  /\ base \in ObservedRevisions
  /\ value \in Documents
  /\ intent \in Intents
  /\ \/ phase = "accepting"
     \/ /\ phase = "quiescing"
        /\ client = exitOwner
        /\ intent = "stop"
  /\ proposalState' = [proposalState EXCEPT ![client] = "queued"]
  /\ proposalBase' = [proposalBase EXCEPT ![client] = base]
  /\ proposalValue' = [proposalValue EXCEPT ![client] = value]
  /\ proposalIntent' = [proposalIntent EXCEPT ![client] = intent]
  /\ proposalOutcome' = [proposalOutcome EXCEPT ![client] = "none"]
  /\ queued' = queued \cup {client}
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, activeClient,
       observedRevision, effectPhase, effectRevision, effectIntent,
       effectPublished, effectObserved, terminalAlive, attachmentOwner>>

Start(client) ==
  /\ activeClient = NoClient
  /\ client \in queued
  /\ activeClient' = client
  /\ queued' = queued \ {client}
  /\ proposalState' = [proposalState EXCEPT ![client] = "active"]
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalBase,
       proposalValue, proposalIntent, proposalOutcome, observedRevision,
       effectPhase, effectRevision, effectIntent, effectPublished,
       effectObserved, terminalAlive, attachmentOwner>>

ProposalResult(client) ==
  IF phase = "unavailable"
  THEN "unavailable"
  ELSE IF ~hasDocument
  THEN IF proposalBase[client] = NoRevision THEN "committed" ELSE "stale"
  ELSE IF proposalBase[client] = revision
  THEN IF revision < MaxRevision THEN "committed" ELSE "rejected"
  ELSE IF proposalValue[client] = documentValue /\
          proposalIntent[client] = documentIntent
  THEN "converged"
  ELSE "stale"

EffectSettled ==
  \/ effectPhase = "none"
  \/ /\ effectPhase = "completed"
     /\ effectPublished
     /\ \A observer \in Clients: connected[observer] => effectObserved[observer]

Complete(client) ==
  /\ activeClient = client
  /\ proposalState[client] = "active"
  /\ LET result == ProposalResult(client)
         newRevision == IF ~hasDocument THEN 0 ELSE revision + 1
     IN
       /\ (result # "committed" \/ EffectSettled)
       /\ hasDocument' = IF result = "committed" THEN TRUE ELSE hasDocument
       /\ revision' = IF result = "committed" THEN newRevision ELSE revision
       /\ documentValue' = IF result = "committed"
            THEN proposalValue[client] ELSE documentValue
       /\ documentIntent' = IF result = "committed"
            THEN proposalIntent[client] ELSE documentIntent
       /\ proposalState' = [proposalState EXCEPT ![client] = "completed"]
       /\ proposalOutcome' = [proposalOutcome EXCEPT ![client] =
            IF connected[client] THEN result ELSE "suppressed"]
       /\ activeClient' = NoClient
       /\ effectPhase' =
            IF result = "committed" /\ proposalIntent[client] \in EffectIntents
            THEN "durable"
            ELSE IF result = "committed" THEN "none" ELSE effectPhase
       /\ effectRevision' = IF result = "committed" THEN newRevision ELSE effectRevision
       /\ effectIntent' = IF result = "committed"
            THEN proposalIntent[client] ELSE effectIntent
       /\ effectPublished' = IF result = "committed" THEN FALSE ELSE effectPublished
       /\ effectObserved' = IF result = "committed"
            THEN [observer \in Clients |-> FALSE] ELSE effectObserved
       /\ UNCHANGED <<phase, exitOwner, publishedRevision, connected,
            proposalBase, proposalValue, proposalIntent, queued,
            observedRevision, terminalAlive, attachmentOwner>>

PublishDocument ==
  /\ hasDocument
  /\ publishedRevision < revision
  /\ publishedRevision' = revision
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, connected, proposalState, proposalBase, proposalValue,
       proposalIntent, proposalOutcome, queued, activeClient, observedRevision,
       effectPhase, effectRevision, effectIntent, effectPublished,
       effectObserved, terminalAlive, attachmentOwner>>

ObserveDocument(client) ==
  /\ connected[client]
  /\ observedRevision[client] < publishedRevision
  /\ observedRevision' = [observedRevision EXCEPT ![client] = publishedRevision]
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, effectPhase, effectRevision, effectIntent,
       effectPublished, effectObserved, terminalAlive, attachmentOwner>>

DispatchEffect ==
  /\ effectPhase = "durable"
  /\ effectIntent \in EffectIntents
  /\ publishedRevision >= effectRevision
  /\ effectPhase' = "dispatched"
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, observedRevision, effectRevision, effectIntent,
       effectPublished, effectObserved, terminalAlive, attachmentOwner>>

CompleteEffect ==
  /\ effectPhase = "dispatched"
  /\ effectPhase' = "completed"
  /\ terminalAlive' = IF effectIntent = "provision" THEN TRUE ELSE FALSE
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, observedRevision, effectRevision, effectIntent,
       effectPublished, effectObserved, attachmentOwner>>

PublishEffect ==
  /\ effectPhase = "completed"
  /\ ~effectPublished
  /\ effectPublished' = TRUE
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, observedRevision, effectPhase, effectRevision,
       effectIntent, effectObserved, terminalAlive, attachmentOwner>>

ObserveEffect(client) ==
  /\ connected[client]
  /\ effectPublished
  /\ ~effectObserved[client]
  /\ observedRevision[client] >= effectRevision
  /\ effectObserved' = [effectObserved EXCEPT ![client] = TRUE]
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, observedRevision, effectPhase, effectRevision,
       effectIntent, effectPublished, terminalAlive, attachmentOwner>>

OpenAttachment(client, attachment) ==
  /\ connected[client]
  /\ attachmentOwner[attachment] = NoAttachmentOwner
  /\ attachmentOwner' = [attachmentOwner EXCEPT ![attachment] = client]
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, observedRevision, effectPhase, effectRevision,
       effectIntent, effectPublished, effectObserved, terminalAlive>>

Disconnect(client) ==
  /\ connected[client]
  /\ connected' = [connected EXCEPT ![client] = FALSE]
  /\ queued' = queued \ {client}
  /\ proposalState' = IF proposalState[client] = "active"
       THEN proposalState ELSE [proposalState EXCEPT ![client] = "completed"]
  /\ proposalOutcome' = IF proposalState[client] \in {"idle", "queued"}
       THEN [proposalOutcome EXCEPT ![client] = "suppressed"] ELSE proposalOutcome
  /\ attachmentOwner' = [attachment \in Attachments |->
       IF attachmentOwner[attachment] = client
       THEN NoAttachmentOwner ELSE attachmentOwner[attachment]]
  /\ UNCHANGED <<phase, exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, proposalBase, proposalValue,
       proposalIntent, activeClient, observedRevision, effectPhase,
       effectRevision, effectIntent, effectPublished, effectObserved,
       terminalAlive>>

BeginQuiesce(client) ==
  /\ \/ phase = "accepting"
     \/ /\ phase = "quiescing"
        /\ ~connected[exitOwner]
  /\ connected[client]
  /\ phase' = "quiescing"
  /\ exitOwner' = client
  /\ UNCHANGED <<hasDocument, revision, documentValue, documentIntent,
       publishedRevision, connected, proposalState, proposalBase,
       proposalValue, proposalIntent, proposalOutcome, queued, activeClient,
       observedRevision, effectPhase, effectRevision, effectIntent,
       effectPublished, effectObserved, terminalAlive, attachmentOwner>>

StoreFailed ==
  /\ phase # "unavailable"
  /\ phase' = "unavailable"
  /\ UNCHANGED <<exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalOutcome, queued,
       activeClient, observedRevision, effectPhase, effectRevision,
       effectIntent, effectPublished, effectObserved, terminalAlive,
       attachmentOwner>>

Next ==
  \/ \E client \in Clients: Connect(client)
  \/ \E client \in Clients, base \in ObservedRevisions,
       value \in Documents, intent \in Intents: Submit(client, base, value, intent)
  \/ \E client \in Clients: Start(client)
  \/ \E client \in Clients: Complete(client)
  \/ PublishDocument
  \/ \E client \in Clients: ObserveDocument(client)
  \/ DispatchEffect
  \/ CompleteEffect
  \/ PublishEffect
  \/ \E client \in Clients: ObserveEffect(client)
  \/ \E client \in Clients, attachment \in Attachments:
       OpenAttachment(client, attachment)
  \/ \E client \in Clients: Disconnect(client)
  \/ \E client \in Clients: BeginQuiesce(client)
  \/ StoreFailed

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(PublishDocument)
  /\ WF_vars(DispatchEffect)
  /\ WF_vars(CompleteEffect)
  /\ WF_vars(PublishEffect)
  /\ \A client \in Clients:
       /\ WF_vars(Start(client))
       /\ WF_vars(Complete(client))
       /\ WF_vars(ObserveDocument(client))
       /\ WF_vars(ObserveEffect(client))

TypeOK ==
  /\ phase \in Phases
  /\ exitOwner \in Clients \cup {NoClient}
  /\ hasDocument \in BOOLEAN
  /\ revision \in Revisions
  /\ documentValue \in Documents
  /\ documentIntent \in Intents
  /\ publishedRevision \in ObservedRevisions
  /\ connected \in [Clients -> BOOLEAN]
  /\ proposalState \in [Clients -> ProposalStates]
  /\ proposalBase \in [Clients -> ObservedRevisions]
  /\ proposalValue \in [Clients -> Documents]
  /\ proposalIntent \in [Clients -> Intents]
  /\ proposalOutcome \in [Clients -> ProposalOutcomes]
  /\ queued \subseteq Clients
  /\ activeClient \in Clients \cup {NoClient}
  /\ observedRevision \in [Clients -> ObservedRevisions]
  /\ effectPhase \in EffectPhases
  /\ effectRevision \in Revisions
  /\ effectIntent \in Intents
  /\ effectPublished \in BOOLEAN
  /\ effectObserved \in [Clients -> BOOLEAN]
  /\ terminalAlive \in BOOLEAN
  /\ attachmentOwner \in [Attachments -> Clients \cup {NoAttachmentOwner}]

ProposalIntegrity ==
  /\ \A client \in Clients:
       (client \in queued) <=> (proposalState[client] = "queued")
  /\ (activeClient = NoClient) <=>
       (\A client \in Clients: proposalState[client] # "active")
  /\ activeClient # NoClient => proposalState[activeClient] = "active"

CanonicalPublicationOrder ==
  /\ publishedRevision <= revision
  /\ \A client \in Clients: observedRevision[client] <= publishedRevision
  /\ effectPublished => publishedRevision >= effectRevision
  /\ \A client \in Clients:
       effectObserved[client] => observedRevision[client] >= effectRevision

EffectRequiresPublishedIntent ==
  effectPhase \in {"dispatched", "completed"} =>
    /\ hasDocument
    /\ effectIntent \in EffectIntents
    /\ publishedRevision >= effectRevision

CanonicalDocumentShape ==
  ~hasDocument => revision = 0

AttachmentBelongsToConnectedSession ==
  \A attachment \in Attachments:
    attachmentOwner[attachment] # NoAttachmentOwner =>
      connected[attachmentOwner[attachment]]

QuiescingHasOwner ==
  phase = "quiescing" => exitOwner \in Clients

QueuedProposalEventuallySettles ==
  \A client \in Clients:
    proposalState[client] = "queued" ~>
      proposalState[client] = "completed" \/ ~connected[client]

DurableEffectEventuallyCompletes ==
  effectPhase = "durable" ~> effectPhase = "completed"

=============================================================================
