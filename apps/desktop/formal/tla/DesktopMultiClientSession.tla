----------------------- MODULE DesktopMultiClientSession -----------------------
EXTENDS FiniteSets, Integers, Sequences, TLC

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
ProposalRevisionValidities == {"exact-next", "invalid-revision"}
EffectPhases == {"none", "durable", "dispatched", "deferred", "completed"}
PublicationKinds == {"document", "effect"}
PublicationEvents == PublicationKinds \X Revisions
MaxPublicationCount == 2 * (MaxRevision + 1)

DocumentPublication(revisionNumber) == <<"document", revisionNumber>>
EffectPublication(revisionNumber) == <<"effect", revisionNumber>>

VARIABLES
  phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
  documentIntent,
  publishedRevision, publicationLog, connected, proposalState, proposalBase,
  proposalValue, proposalIntent, proposalRevisionValidity, proposalOutcome,
  queued, activeClient, observedPublicationCount, observedRevision,
  effectPhase, effectRevision, effectIntent, effectPublished, effectObserved,
  terminalAlive, attachmentOwner

vars == <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
  documentIntent, publishedRevision, publicationLog, connected, proposalState,
  proposalBase, proposalValue, proposalIntent, proposalRevisionValidity,
  proposalOutcome, queued, activeClient, observedPublicationCount,
  observedRevision, effectPhase, effectRevision, effectIntent,
  effectPublished, effectObserved, terminalAlive, attachmentOwner>>

Init ==
  /\ phase = "accepting"
  /\ exitOwner = NoClient
  /\ failedStoreClient = NoClient
  /\ hasDocument \in BOOLEAN
  /\ revision = 0
  /\ documentValue = "document-a"
  /\ documentIntent = "none"
  /\ publishedRevision = IF hasDocument THEN 0 ELSE NoRevision
  /\ publicationLog = IF hasDocument THEN <<DocumentPublication(0)>> ELSE <<>>
  /\ connected = [client \in Clients |-> FALSE]
  /\ proposalState = [client \in Clients |-> "idle"]
  /\ proposalBase = [client \in Clients |-> NoRevision]
  /\ proposalValue = [client \in Clients |-> "document-a"]
  /\ proposalIntent = [client \in Clients |-> "none"]
  /\ proposalRevisionValidity = [client \in Clients |-> "exact-next"]
  /\ proposalOutcome = [client \in Clients |-> "none"]
  /\ queued = {}
  /\ activeClient = NoClient
  /\ observedPublicationCount = [client \in Clients |-> 0]
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
  /\ observedPublicationCount' = [observedPublicationCount EXCEPT
       ![client] = Len(publicationLog)]
  /\ observedRevision' = [observedRevision EXCEPT ![client] = publishedRevision]
  /\ effectObserved' = [effectObserved EXCEPT
       ![client] = effectPublished /\ publishedRevision >= effectRevision]
  /\ proposalState' = [proposalState EXCEPT ![client] = "idle"]
  /\ proposalOutcome' = [proposalOutcome EXCEPT ![client] = "none"]
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, proposalBase,
       proposalValue, proposalIntent, proposalRevisionValidity, queued,
       activeClient, effectPhase, effectRevision, effectIntent,
       effectPublished, terminalAlive, attachmentOwner>>

Submit(client, base, value, intent, revisionValidity) ==
  /\ connected[client]
  /\ proposalState[client] = "idle"
  /\ base \in ObservedRevisions
  /\ value \in Documents
  /\ intent \in Intents
  /\ revisionValidity \in ProposalRevisionValidities
  /\ phase \in {"accepting", "quiescing"}
  \* Exact successor requests beyond MaxRevision are outside this finite instance;
  \* invalid revision requests remain admitted at every represented base.
  /\ revisionValidity = "invalid-revision" \/
       base = NoRevision \/ base < MaxRevision
  /\ proposalState' = [proposalState EXCEPT ![client] = "queued"]
  /\ proposalBase' = [proposalBase EXCEPT ![client] = base]
  /\ proposalValue' = [proposalValue EXCEPT ![client] = value]
  /\ proposalIntent' = [proposalIntent EXCEPT ![client] = intent]
  /\ proposalRevisionValidity' = [proposalRevisionValidity EXCEPT
       ![client] = revisionValidity]
  /\ proposalOutcome' = [proposalOutcome EXCEPT ![client] = "none"]
  /\ queued' = queued \cup {client}
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       activeClient, observedPublicationCount, observedRevision, effectPhase,
       effectRevision, effectIntent, effectPublished, effectObserved,
       terminalAlive, attachmentOwner>>

SubmitExactRevision(client, base, value, intent) ==
  /\ base = NoRevision \/ base < MaxRevision
  /\ Submit(client, base, value, intent, "exact-next")

SubmitInvalidRevision(client, base, value, intent) ==
  /\ phase \in {"accepting", "quiescing"}
  /\ Submit(client, base, value, intent, "invalid-revision")

Start(client) ==
  /\ activeClient = NoClient
  /\ client \in queued
  /\ activeClient' = client
  /\ queued' = queued \ {client}
  /\ proposalState' = [proposalState EXCEPT ![client] = "active"]
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalBase, proposalValue, proposalIntent, proposalRevisionValidity,
       proposalOutcome, observedPublicationCount, observedRevision,
       effectPhase, effectRevision, effectIntent, effectPublished,
       effectObserved, terminalAlive, attachmentOwner>>

ProposalAllowedAtDecision(client) ==
  \/ phase = "accepting"
  \/ /\ phase = "quiescing"
     /\ client = exitOwner
     /\ proposalIntent[client] = "stop"

ProposalResult(client) ==
  IF phase = "unavailable"
  THEN IF client = failedStoreClient THEN "unavailable" ELSE "rejected"
  ELSE IF ~ProposalAllowedAtDecision(client)
  THEN "rejected"
  ELSE IF proposalRevisionValidity[client] # "exact-next"
  THEN "rejected"
  ELSE IF ~hasDocument
  THEN IF proposalBase[client] = NoRevision THEN "committed" ELSE "stale"
  ELSE IF proposalBase[client] = revision
  THEN "committed"
  ELSE IF proposalValue[client] = documentValue /\
          proposalIntent[client] = documentIntent
  THEN "converged"
  ELSE "stale"

EffectSettled ==
  \/ effectPhase = "none"
  \/ /\ effectPhase = "completed"
     /\ effectPublished

EffectReleased == EffectSettled \/ effectPhase = "deferred"

Complete(client) ==
  /\ activeClient = client
  /\ proposalState[client] = "active"
  /\ LET result == ProposalResult(client)
         newRevision == IF ~hasDocument THEN 0 ELSE revision + 1
     IN
       /\ (result # "committed" \/ EffectReleased)
       /\ hasDocument' = IF result = "committed" THEN TRUE ELSE hasDocument
       /\ revision' = IF result = "committed" THEN newRevision ELSE revision
       /\ documentValue' = IF result = "committed"
            THEN proposalValue[client] ELSE documentValue
       /\ documentIntent' = IF result = "committed"
            THEN proposalIntent[client] ELSE documentIntent
       \* A committed completion includes publication of its exact revision.
       /\ publishedRevision' = IF result = "committed"
            THEN newRevision ELSE publishedRevision
       /\ publicationLog' = IF result = "committed"
            THEN Append(publicationLog, DocumentPublication(newRevision))
            ELSE publicationLog
       /\ proposalState' = [proposalState EXCEPT ![client] = "completed"]
       /\ proposalOutcome' = [proposalOutcome EXCEPT ![client] =
            IF connected[client] THEN result ELSE "suppressed"]
       \* Completed payload is dead. Keep outcome and client delivery state.
       /\ proposalBase' = [proposalBase EXCEPT ![client] = NoRevision]
       /\ proposalValue' = [proposalValue EXCEPT ![client] = "document-a"]
       /\ proposalIntent' = [proposalIntent EXCEPT ![client] = "none"]
       /\ proposalRevisionValidity' = [proposalRevisionValidity EXCEPT
            ![client] = "exact-next"]
       /\ activeClient' = NoClient
       /\ effectPhase' =
            IF result = "committed" /\ proposalIntent[client] \in EffectIntents
            THEN "durable"
            ELSE IF result = "committed" THEN "none" ELSE effectPhase
       /\ effectRevision' = IF result = "committed"
            THEN newRevision ELSE effectRevision
       /\ effectIntent' = IF result = "committed"
            THEN proposalIntent[client] ELSE effectIntent
       /\ effectPublished' = IF result = "committed" THEN FALSE ELSE effectPublished
       /\ effectObserved' = IF result = "committed"
            THEN [observer \in Clients |-> FALSE] ELSE effectObserved
       /\ UNCHANGED <<phase, exitOwner, connected, queued,
            failedStoreClient, observedPublicationCount, observedRevision,
            terminalAlive, attachmentOwner>>

CompleteCommittedAndPublishExact(client) ==
  /\ ProposalResult(client) = "committed"
  /\ Complete(client)

CompleteQuiescingRejection(client) ==
  /\ phase = "quiescing"
  /\ ~ProposalAllowedAtDecision(client)
  /\ Complete(client)

CompleteOtherProposal(client) ==
  /\ ProposalResult(client) # "committed"
  /\ ~(phase = "quiescing" /\ ~ProposalAllowedAtDecision(client))
  /\ ~(phase = "unavailable" /\ client # failedStoreClient)
  /\ Complete(client)

CompleteQueuedAfterStoreFailure(client) ==
  /\ phase = "unavailable"
  /\ client # failedStoreClient
  /\ Complete(client)

NextPublication(client) ==
  publicationLog[observedPublicationCount[client] + 1]

ObserveDocument(client) ==
  /\ connected[client]
  /\ observedPublicationCount[client] < Len(publicationLog)
  /\ NextPublication(client)[1] = "document"
  /\ observedPublicationCount' = [observedPublicationCount EXCEPT
       ![client] = @ + 1]
  /\ observedRevision' = [observedRevision EXCEPT
       ![client] = NextPublication(client)[2]]
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       effectPhase, effectRevision, effectIntent, effectPublished,
       effectObserved, terminalAlive, attachmentOwner>>

DispatchEffect ==
  /\ effectPhase = "durable"
  /\ effectIntent \in EffectIntents
  /\ publishedRevision >= effectRevision
  /\ effectPhase' = "dispatched"
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedPublicationCount, observedRevision, effectRevision,
       effectIntent, effectPublished, effectObserved, terminalAlive,
       attachmentOwner>>

CompleteEffect ==
  /\ effectPhase = "dispatched"
  /\ effectPhase' = "completed"
  /\ terminalAlive' = IF effectIntent = "provision" THEN TRUE ELSE FALSE
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedPublicationCount, observedRevision, effectRevision,
       effectIntent, effectPublished, effectObserved, attachmentOwner>>

DeferEffect ==
  /\ effectPhase = "dispatched"
  /\ effectPhase' = "deferred"
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedPublicationCount, observedRevision, effectRevision,
       effectIntent, effectPublished, effectObserved, terminalAlive,
       attachmentOwner>>

RetryDeferredEffect ==
  /\ effectPhase = "deferred"
  \* Explicit reconciliation uses the same serialized tail. A retry accepted
  \* later cannot overtake an already queued or active proposal.
  /\ activeClient = NoClient
  /\ queued = {}
  /\ effectPhase' = "dispatched"
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedPublicationCount, observedRevision, effectRevision,
       effectIntent, effectPublished, effectObserved, terminalAlive,
       attachmentOwner>>

PublishEffect ==
  /\ effectPhase = "completed"
  /\ ~effectPublished
  /\ effectPublished' = TRUE
  /\ publicationLog' = Append(publicationLog, EffectPublication(effectRevision))
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalRevisionValidity,
       proposalOutcome, queued, activeClient, observedPublicationCount,
       observedRevision, effectPhase, effectRevision, effectIntent,
       effectObserved, terminalAlive, attachmentOwner>>

ObserveEffect(client) ==
  /\ connected[client]
  /\ observedPublicationCount[client] < Len(publicationLog)
  /\ NextPublication(client)[1] = "effect"
  /\ observedPublicationCount' = [observedPublicationCount EXCEPT
       ![client] = @ + 1]
  /\ effectObserved' =
       IF NextPublication(client)[2] = effectRevision /\ effectPublished
       THEN [effectObserved EXCEPT ![client] = TRUE]
       ELSE effectObserved
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedRevision, effectPhase, effectRevision, effectIntent,
       effectPublished, terminalAlive, attachmentOwner>>

OpenAttachment(client, attachment) ==
  /\ connected[client]
  /\ attachmentOwner[attachment] = NoAttachmentOwner
  /\ attachmentOwner' = [attachmentOwner EXCEPT ![attachment] = client]
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedPublicationCount, observedRevision, effectPhase,
       effectRevision, effectIntent, effectPublished, effectObserved,
       terminalAlive>>

Disconnect(client) ==
  /\ connected[client]
  /\ connected' = [connected EXCEPT ![client] = FALSE]
  /\ queued' = queued \ {client}
  /\ proposalState' = IF proposalState[client] = "active"
       THEN proposalState ELSE [proposalState EXCEPT ![client] = "completed"]
  /\ proposalOutcome' = IF proposalState[client] \in {"idle", "queued"}
       THEN [proposalOutcome EXCEPT ![client] = "suppressed"] ELSE proposalOutcome
  \* An active disconnected proposal still needs its exact payload at Complete.
  /\ proposalBase' = IF proposalState[client] = "active"
       THEN proposalBase ELSE [proposalBase EXCEPT ![client] = NoRevision]
  /\ proposalValue' = IF proposalState[client] = "active"
       THEN proposalValue ELSE [proposalValue EXCEPT ![client] = "document-a"]
  /\ proposalIntent' = IF proposalState[client] = "active"
       THEN proposalIntent ELSE [proposalIntent EXCEPT ![client] = "none"]
  /\ proposalRevisionValidity' = IF proposalState[client] = "active"
       THEN proposalRevisionValidity
       ELSE [proposalRevisionValidity EXCEPT ![client] = "exact-next"]
  /\ attachmentOwner' = [attachment \in Attachments |->
       IF attachmentOwner[attachment] = client
       THEN NoAttachmentOwner ELSE attachmentOwner[attachment]]
  /\ UNCHANGED <<phase, exitOwner, failedStoreClient, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, activeClient,
       observedPublicationCount, observedRevision, effectPhase,
       effectRevision, effectIntent, effectPublished, effectObserved,
       terminalAlive>>

BeginQuiesce(client) ==
  /\ \/ phase = "accepting"
     \/ /\ phase = "quiescing"
        /\ ~connected[exitOwner]
  /\ connected[client]
  /\ phase' = "quiescing"
  /\ exitOwner' = client
  /\ UNCHANGED <<failedStoreClient, hasDocument, revision, documentValue,
       documentIntent,
       publishedRevision, publicationLog, connected, proposalState,
       proposalBase, proposalValue, proposalIntent, proposalRevisionValidity,
       proposalOutcome, queued, activeClient, observedPublicationCount,
       observedRevision, effectPhase, effectRevision, effectIntent,
       effectPublished, effectObserved, terminalAlive, attachmentOwner>>

StoreFailed ==
  /\ phase # "unavailable"
  /\ activeClient # NoClient
  /\ ProposalResult(activeClient) = "committed"
  /\ EffectReleased
  /\ phase' = "unavailable"
  /\ failedStoreClient' = activeClient
  /\ UNCHANGED <<exitOwner, hasDocument, revision, documentValue,
       documentIntent, publishedRevision, publicationLog, connected,
       proposalState, proposalBase, proposalValue, proposalIntent,
       proposalRevisionValidity, proposalOutcome, queued, activeClient,
       observedPublicationCount, observedRevision, effectPhase,
       effectRevision, effectIntent, effectPublished, effectObserved,
       terminalAlive, attachmentOwner>>

Next ==
  \/ \E client \in Clients: Connect(client)
  \/ \E client \in Clients, base \in ObservedRevisions,
       value \in Documents, intent \in Intents:
       SubmitExactRevision(client, base, value, intent)
  \/ \E client \in Clients, base \in ObservedRevisions,
       value \in Documents, intent \in Intents:
       SubmitInvalidRevision(client, base, value, intent)
  \/ \E client \in Clients: Start(client)
  \/ \E client \in Clients: CompleteCommittedAndPublishExact(client)
  \/ \E client \in Clients: CompleteQuiescingRejection(client)
  \/ \E client \in Clients: CompleteOtherProposal(client)
  \/ \E client \in Clients: CompleteQueuedAfterStoreFailure(client)
  \/ \E client \in Clients: ObserveDocument(client)
  \/ DispatchEffect
  \/ CompleteEffect
  \/ DeferEffect
  \/ RetryDeferredEffect
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
  /\ WF_vars(DispatchEffect)
  /\ WF_vars(CompleteEffect)
  /\ WF_vars(DeferEffect)
  /\ WF_vars(PublishEffect)
  /\ \A client \in Clients:
       /\ WF_vars(Start(client))
       /\ WF_vars(Complete(client))
       /\ WF_vars(ObserveDocument(client))
       /\ WF_vars(ObserveEffect(client))

TypeOK ==
  /\ phase \in Phases
  /\ exitOwner \in Clients \cup {NoClient}
  /\ failedStoreClient \in Clients \cup {NoClient}
  /\ hasDocument \in BOOLEAN
  /\ revision \in Revisions
  /\ documentValue \in Documents
  /\ documentIntent \in Intents
  /\ publishedRevision \in ObservedRevisions
  /\ publicationLog \in Seq(PublicationEvents)
  /\ Len(publicationLog) <= MaxPublicationCount
  /\ connected \in [Clients -> BOOLEAN]
  /\ proposalState \in [Clients -> ProposalStates]
  /\ proposalBase \in [Clients -> ObservedRevisions]
  /\ proposalValue \in [Clients -> Documents]
  /\ proposalIntent \in [Clients -> Intents]
  /\ proposalRevisionValidity \in [Clients -> ProposalRevisionValidities]
  /\ proposalOutcome \in [Clients -> ProposalOutcomes]
  /\ queued \subseteq Clients
  /\ activeClient \in Clients \cup {NoClient}
  /\ observedPublicationCount \in [Clients -> 0..MaxPublicationCount]
  /\ \A client \in Clients:
       observedPublicationCount[client] <= Len(publicationLog)
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

StoreFailureIdentityIsExact ==
  /\ (phase = "unavailable") <=> (failedStoreClient \in Clients)
  /\ \A client \in Clients:
       proposalOutcome[client] = "unavailable" => client = failedStoreClient

ExactCurrentProposalIsRepresentable ==
  \A client \in Clients:
    /\ proposalState[client] \in {"queued", "active"}
    /\ proposalRevisionValidity[client] = "exact-next"
    /\ hasDocument
    /\ proposalBase[client] = revision
    => revision < MaxRevision

PublicationLogIsOrdered ==
  /\ \A left, right \in DOMAIN publicationLog:
       left # right => publicationLog[left] # publicationLog[right]
  /\ \A index \in DOMAIN publicationLog:
       publicationLog[index][1] = "effect" =>
         \E prior \in 1..(index - 1):
           publicationLog[prior] = DocumentPublication(publicationLog[index][2])
  /\ \A left, right \in DOMAIN publicationLog:
       /\ left < right
       /\ publicationLog[left][1] = "document"
       /\ publicationLog[right][1] = "document"
       => publicationLog[left][2] < publicationLog[right][2]

CanonicalPublicationOrder ==
  /\ publishedRevision <= revision
  /\ hasDocument => publishedRevision = revision
  /\ \A client \in Clients: observedRevision[client] <= publishedRevision
  /\ effectPublished =>
       /\ effectPhase = "completed"
       /\ EffectPublication(effectRevision) \in
            {publicationLog[index] : index \in DOMAIN publicationLog}
  /\ \A client \in Clients:
       effectObserved[client] =>
         /\ effectPublished
         /\ \E index \in 1..observedPublicationCount[client]:
              publicationLog[index] = EffectPublication(effectRevision)

EffectRequiresPublishedIntent ==
  effectPhase \in {"durable", "dispatched", "deferred", "completed"} =>
    /\ hasDocument
    /\ effectIntent \in EffectIntents
    /\ publishedRevision >= effectRevision

DeferredEffectRetainsDurableIdentity ==
  effectPhase = "deferred" =>
    /\ hasDocument
    /\ effectIntent \in EffectIntents
    /\ publishedRevision >= effectRevision
    /\ ~effectPublished
    /\ EffectPublication(effectRevision) \notin
         {publicationLog[index] : index \in DOMAIN publicationLog}

CanonicalDocumentShape ==
  ~hasDocument => revision = 0

AttachmentBelongsToConnectedSession ==
  \A attachment \in Attachments:
    attachmentOwner[attachment] # NoAttachmentOwner =>
      connected[attachmentOwner[attachment]]

QuiescingHasOwner ==
  phase = "quiescing" => exitOwner \in Clients

\* Session and document actions must never manufacture terminal lifecycle intent.
TerminalLifetimeSemantics ==
  [][IF effectPhase = "dispatched" /\ effectPhase' = "completed"
     THEN terminalAlive' = (effectIntent = "provision")
     ELSE UNCHANGED terminalAlive]_vars

\* A raw request may queue while quiescing, but authority is rechecked when
\* that request reaches the serialized decision point.
ProposalAdmissionSemantics ==
  [][\A client \in Clients:
    /\ proposalState[client] = "active"
    /\ proposalState'[client] = "completed"
    /\ phase = "quiescing"
    /\ ~(client = exitOwner /\ proposalIntent[client] = "stop")
    => /\ IF connected[client]
          THEN proposalOutcome'[client] = "rejected"
          ELSE proposalOutcome'[client] = "suppressed"
       /\ UNCHANGED <<hasDocument, revision, documentValue, documentIntent>>
  ]_vars

StoreFailureSemantics ==
  [][phase # "unavailable" /\ phase' = "unavailable" =>
       LET client == activeClient
           admissionAllowed ==
             \/ phase = "accepting"
             \/ /\ phase = "quiescing"
                /\ client = exitOwner
                /\ proposalIntent[client] = "stop"
           candidateValid ==
             proposalRevisionValidity[client] = "exact-next"
           currentBase ==
             IF hasDocument THEN proposalBase[client] = revision
             ELSE proposalBase[client] = NoRevision
       IN /\ client # NoClient
          /\ proposalState[client] = "active"
          /\ admissionAllowed
          /\ candidateValid
          /\ currentBase
          /\ EffectReleased
          /\ failedStoreClient' = client
          /\ UNCHANGED <<hasDocument, revision, documentValue, documentIntent,
                          publishedRevision, publicationLog>>]_vars

\* Each event already enqueued to a live session is eventually consumed in
\* sequence. Retry of a deferred remote effect is deliberately not fair.
CanonicalPublicationProgress ==
  \A client \in Clients, index \in 1..MaxPublicationCount:
    (connected[client] /\
     Len(publicationLog) >= index /\
     observedPublicationCount[client] < index) ~>
      (~connected[client] \/ observedPublicationCount[client] >= index)

ClientPublicationCursorSemantics ==
  [][\A client \in Clients:
    observedPublicationCount'[client] # observedPublicationCount[client] =>
      \/ /\ ~connected[client]
         /\ connected'[client]
         /\ observedPublicationCount'[client] = Len(publicationLog)
      \/ /\ connected[client]
         /\ connected'[client]
         /\ publicationLog' = publicationLog
         /\ observedPublicationCount'[client] =
              observedPublicationCount[client] + 1
  ]_vars

\* Check the decision against the pre-commit document, not a later client's write.
\* Action properties avoid adding history variables to the explored state space.
ProposalCompletionSemantics ==
  [][\A client \in Clients:
    /\ proposalState[client] = "active"
    /\ proposalState'[client] = "completed"
    => LET available == phase # "unavailable"
           failedStoreOperation ==
             phase = "unavailable" /\ client = failedStoreClient
           admissionAllowed == ProposalAllowedAtDecision(client)
           candidateValid == proposalRevisionValidity[client] = "exact-next"
           eligible == available /\ admissionAllowed /\ candidateValid
           currentBase == IF hasDocument THEN proposalBase[client] = revision
                          ELSE proposalBase[client] = NoRevision
           sameContent == proposalValue[client] = documentValue /\
                          proposalIntent[client] = documentIntent
           canCommit == eligible /\ currentBase
           newRevision == IF hasDocument THEN revision + 1 ELSE 0
       IN
         /\ IF connected[client]
            THEN
              /\ (proposalOutcome'[client] = "committed") <=> canCommit
              /\ (proposalOutcome'[client] = "converged") <=>
                   (eligible /\ hasDocument /\ ~currentBase /\ sameContent)
              /\ (proposalOutcome'[client] = "stale") <=>
                   (eligible /\ ~currentBase /\ (~hasDocument \/ ~sameContent))
              /\ (proposalOutcome'[client] = "rejected") <=>
                   \/ /\ ~available
                         /\ client # failedStoreClient
                      \/ /\ available
                         /\ (~admissionAllowed \/ ~candidateValid)
              /\ (proposalOutcome'[client] = "unavailable") <=>
                   failedStoreOperation
            ELSE proposalOutcome'[client] = "suppressed"
         /\ IF canCommit
            THEN /\ hasDocument'
                 /\ revision' = newRevision
                 /\ documentValue' = proposalValue[client]
                 /\ documentIntent' = proposalIntent[client]
                 /\ publishedRevision' = newRevision
                 /\ publicationLog' =
                      Append(publicationLog, DocumentPublication(newRevision))
            ELSE UNCHANGED <<hasDocument, revision, documentValue,
                              documentIntent, publishedRevision, publicationLog>>
  ]_vars

DeferredRetryPreservesIdentity ==
  [][effectPhase = "deferred" /\ effectPhase' = "dispatched" =>
       /\ UNCHANGED <<effectRevision, effectIntent, terminalAlive,
                       effectPublished, publicationLog>>]_vars

QueuedProposalEventuallySettles ==
  \A client \in Clients:
    proposalState[client] = "queued" ~>
      proposalState[client] = "completed" \/ ~connected[client]

DurableEffectEventuallyReleases ==
  effectPhase = "durable" ~> EffectReleased

CompletedEffectEventuallyPublishes ==
  effectPhase = "completed" ~> effectPublished

=============================================================================
