--------------- MODULE DesktopTerminalIntentReconciliation ---------------
EXTENDS FiniteSets, Naturals, Sequences

Presentations == {"presentation-a", "presentation-b"}
OperationIds == {"operation-a", "operation-b"}
NoOperation == "no-operation"
Phases == {"none", "durable", "dispatched", "deferred", "published"}
DispatchResults == {"settled", "deferred"}
NoDispatchResult == "none"
Revisions == 0..2

OperationFor(presentation) ==
  IF presentation = "presentation-a" THEN "operation-a" ELSE "operation-b"

OperationEvents ==
  {<<presentation, OperationFor(presentation)>> : presentation \in Presentations}

VARIABLES revision, phase, operationIdentity, intentCurrent, dispatchResult,
  outcomeCached, publicationOrder

vars == <<revision, phase, operationIdentity, intentCurrent, dispatchResult,
  outcomeCached, publicationOrder>>

AllReleased ==
  \A presentation \in Presentations:
    phase[presentation] \in {"published", "deferred"}

Init ==
  /\ revision = 0
  /\ phase = [presentation \in Presentations |-> "none"]
  /\ operationIdentity = [presentation \in Presentations |-> NoOperation]
  /\ intentCurrent = [presentation \in Presentations |-> FALSE]
  /\ dispatchResult = [presentation \in Presentations |-> NoDispatchResult]
  /\ outcomeCached = [presentation \in Presentations |-> FALSE]
  /\ publicationOrder = <<>>

\* This is an external document proposal. The spec deliberately does not make
\* its admission fair.
CommitBatch ==
  /\ revision = 0
  /\ \A presentation \in Presentations: phase[presentation] = "none"
  /\ revision' = 1
  /\ phase' = [presentation \in Presentations |-> "durable"]
  /\ operationIdentity' =
       [presentation \in Presentations |-> OperationFor(presentation)]
  /\ intentCurrent' = [presentation \in Presentations |-> TRUE]
  /\ dispatchResult' =
       [presentation \in Presentations |-> NoDispatchResult]
  /\ outcomeCached' = [presentation \in Presentations |-> FALSE]
  /\ UNCHANGED publicationOrder

\* Main dispatches the whole committed batch before awaiting either result.
DispatchBatch ==
  /\ revision = 1
  /\ \A presentation \in Presentations: phase[presentation] = "durable"
  /\ phase' = [presentation \in Presentations |-> "dispatched"]
  /\ dispatchResult' \in [Presentations -> DispatchResults]
  /\ UNCHANGED <<revision, operationIdentity, intentCurrent, outcomeCached,
                  publicationOrder>>

CompleteAndPublish(presentation) ==
  /\ phase[presentation] = "dispatched"
  /\ dispatchResult[presentation] = "settled"
  /\ phase' = [phase EXCEPT ![presentation] = "published"]
  /\ outcomeCached' = [outcomeCached EXCEPT ![presentation] = TRUE]
  /\ publicationOrder' = Append(publicationOrder,
       <<presentation, operationIdentity[presentation]>>)
  /\ UNCHANGED <<revision, operationIdentity, intentCurrent, dispatchResult>>

\* Deferred is a released-pending result: the exact durable operation remains,
\* while no terminal outcome or cache entry is manufactured.
ReleaseDeferred(presentation) ==
  /\ phase[presentation] = "dispatched"
  /\ dispatchResult[presentation] = "deferred"
  /\ phase' = [phase EXCEPT ![presentation] = "deferred"]
  /\ UNCHANGED <<revision, operationIdentity, intentCurrent, dispatchResult,
                  outcomeCached, publicationOrder>>

\* Retry is an explicit external request. It is enabled only while the same
\* operation remains current and has no fairness assumption.
RetryDeferred(presentation, result) ==
  /\ phase[presentation] = "deferred"
  /\ intentCurrent[presentation]
  /\ result \in DispatchResults
  /\ phase' = [phase EXCEPT ![presentation] = "dispatched"]
  /\ dispatchResult' = [dispatchResult EXCEPT ![presentation] = result]
  /\ UNCHANGED <<revision, operationIdentity, intentCurrent, outcomeCached,
                  publicationOrder>>

\* A later external proposal may commit after every member is either published
\* or released-pending. It also decides which exact intents remain current.
CommitNext(retainedIntents) ==
  /\ revision = 1
  /\ AllReleased
  /\ retainedIntents \subseteq Presentations
  /\ revision' = 2
  /\ intentCurrent' =
       [presentation \in Presentations |-> presentation \in retainedIntents]
  /\ outcomeCached' =
       [presentation \in Presentations |->
         presentation \in retainedIntents /\ outcomeCached[presentation]]
  /\ UNCHANGED <<phase, operationIdentity, dispatchResult, publicationOrder>>

Next ==
  \/ CommitBatch
  \/ DispatchBatch
  \/ \E presentation \in Presentations: CompleteAndPublish(presentation)
  \/ \E presentation \in Presentations: ReleaseDeferred(presentation)
  \/ \E presentation \in Presentations, result \in DispatchResults:
       RetryDeferred(presentation, result)
  \/ \E retainedIntents \in SUBSET Presentations: CommitNext(retainedIntents)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(DispatchBatch)
  /\ \A presentation \in Presentations:
       /\ WF_vars(CompleteAndPublish(presentation))
       /\ WF_vars(ReleaseDeferred(presentation))

TypeOK ==
  /\ revision \in Revisions
  /\ phase \in [Presentations -> Phases]
  /\ operationIdentity \in
       [Presentations -> OperationIds \cup {NoOperation}]
  /\ intentCurrent \in [Presentations -> BOOLEAN]
  /\ dispatchResult \in
       [Presentations -> DispatchResults \cup {NoDispatchResult}]
  /\ outcomeCached \in [Presentations -> BOOLEAN]
  /\ publicationOrder \in Seq(OperationEvents)
  /\ Len(publicationOrder) <= Cardinality(Presentations)

BatchDispatchIsAtomic ==
  (\E presentation \in Presentations: phase[presentation] = "durable") =>
    \A presentation \in Presentations: phase[presentation] = "durable"

DurableOperationIdentityIsExact ==
  \A presentation \in Presentations:
    phase[presentation] # "none" =>
      operationIdentity[presentation] = OperationFor(presentation)

PublishedPresentationHasExactOutcome ==
  \A presentation \in Presentations:
    phase[presentation] = "published" =>
      /\ dispatchResult[presentation] = "settled"
      /\ <<presentation, operationIdentity[presentation]>> \in
           {publicationOrder[index] : index \in DOMAIN publicationOrder}

DeferredPresentationHasNoSyntheticOutcome ==
  \A presentation \in Presentations:
    phase[presentation] = "deferred" =>
      /\ dispatchResult[presentation] = "deferred"
      /\ ~outcomeCached[presentation]
      /\ <<presentation, operationIdentity[presentation]>> \notin
           {publicationOrder[index] : index \in DOMAIN publicationOrder}

CurrentOutcomeCacheIsExact ==
  \A presentation \in Presentations:
    outcomeCached[presentation] <=>
      /\ intentCurrent[presentation]
      /\ phase[presentation] = "published"

PublicationIdentityIsUnique ==
  /\ \A index \in DOMAIN publicationOrder:
       LET event == publicationOrder[index]
       IN /\ phase[event[1]] = "published"
          /\ event[2] = operationIdentity[event[1]]
  /\ \A left, right \in DOMAIN publicationOrder:
       left # right => publicationOrder[left][1] # publicationOrder[right][1]

SupersededDeferredIntentCannotRetry ==
  \A presentation \in Presentations:
    phase[presentation] = "deferred" /\ ~intentCurrent[presentation] =>
      ~ENABLED (\E result \in DispatchResults:
        RetryDeferred(presentation, result))

LaterCommitRequiresReleasedBatch ==
  [][revision = 1 /\ revision' = 2 => AllReleased]_vars

DeferredRetryRequiresCurrentExactIntent ==
  [][\A presentation \in Presentations:
    phase[presentation] = "deferred" /\
    phase'[presentation] = "dispatched" =>
      /\ intentCurrent[presentation]
      /\ operationIdentity' = operationIdentity
      /\ UNCHANGED <<revision, outcomeCached, publicationOrder>>
  ]_vars

DurableBatchEventuallyDispatches ==
  (revision = 1 /\
   \A presentation \in Presentations: phase[presentation] = "durable") ~>
    (\A presentation \in Presentations:
       phase[presentation] \in {"dispatched", "deferred", "published"})

SettledDispatchEventuallyPublishes ==
  \A presentation \in Presentations:
    (phase[presentation] = "dispatched" /\
     dispatchResult[presentation] = "settled") ~>
      phase[presentation] = "published"

DeferredDispatchEventuallyReleases ==
  \A presentation \in Presentations:
    (phase[presentation] = "dispatched" /\
     dispatchResult[presentation] = "deferred") ~>
      phase[presentation] = "deferred"

=============================================================================
