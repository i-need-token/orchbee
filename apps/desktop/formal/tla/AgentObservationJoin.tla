----------------------- MODULE AgentObservationJoin -----------------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANT CorrelationVariant

CorrelationVariants == {"singleton", "pair"}
Presentations == {"presentation-1", "presentation-2"}
AllCorrelations == {"correlation-1", "correlation-2"}
Correlations ==
  IF CorrelationVariant = "singleton"
  THEN {"correlation-1"}
  ELSE AllCorrelations
Generations == 1..2
NoGeneration == 0
NoPresentation == "no-presentation"
EventPhases == {"absent", "pending", "expired"}

AttachmentPairs == [presentation: Presentations, generation: Generations]

VARIABLES
  currentGeneration,
  eventPhase,
  applied,
  settlementScheduled,
  hintPresent,
  hintEventMatches,
  hintPresentation,
  hintGeneration,
  matchedHistory,
  matchedPresentation,
  matchedGeneration,
  liveAttachments,
  publishedAttachments,
  applicationCount,
  applicationWitnesses,
  disposed,
  barrierActive,
  applicationAtBarrier,
  lastRetiredPresentation,
  lastAction,
  previousApplicationCount,
  previousPublishedAttachments

core ==
  <<currentGeneration, eventPhase, applied, settlementScheduled,
    hintPresent, hintEventMatches, hintPresentation, hintGeneration,
    matchedHistory, matchedPresentation, matchedGeneration, liveAttachments,
    publishedAttachments, applicationCount, applicationWitnesses, disposed,
    barrierActive, applicationAtBarrier, lastRetiredPresentation>>

history ==
  <<lastAction, previousApplicationCount, previousPublishedAttachments>>

vars == <<core, history>>

Pair(presentation, generation) ==
  [presentation |-> presentation, generation |-> generation]

PairFor(correlation) ==
  Pair(hintPresentation[correlation], hintGeneration[correlation])

MatchedPairFor(correlation) ==
  Pair(matchedPresentation[correlation], matchedGeneration[correlation])

GenerationConflict(correlation) ==
  \E matched \in matchedHistory:
    /\ matchedPresentation[matched] = hintPresentation[correlation]
    /\ matchedGeneration[matched] # hintGeneration[correlation]

Joinable(correlation) ==
  /\ ~disposed
  /\ eventPhase = "pending"
  /\ hintPresent[correlation]
  /\ hintEventMatches[correlation]
  /\ correlation \notin matchedHistory
  /\ currentGeneration[hintPresentation[correlation]] =
       hintGeneration[correlation]

Record(action) ==
  /\ lastAction' = action
  /\ previousApplicationCount' = applicationCount
  /\ previousPublishedAttachments' = publishedAttachments

Init ==
  /\ currentGeneration = [presentation \in Presentations |-> NoGeneration]
  /\ eventPhase = "absent"
  /\ applied = FALSE
  /\ settlementScheduled = FALSE
  /\ hintPresent = [correlation \in Correlations |-> FALSE]
  /\ hintEventMatches = [correlation \in Correlations |-> FALSE]
  /\ hintPresentation =
       [correlation \in Correlations |-> NoPresentation]
  /\ hintGeneration = [correlation \in Correlations |-> NoGeneration]
  /\ matchedHistory = {}
  /\ matchedPresentation =
       [correlation \in Correlations |-> NoPresentation]
  /\ matchedGeneration =
       [correlation \in Correlations |-> NoGeneration]
  /\ liveAttachments = {}
  /\ publishedAttachments = {}
  /\ applicationCount = 0
  /\ applicationWitnesses = {}
  /\ disposed = FALSE
  /\ barrierActive = FALSE
  /\ applicationAtBarrier = 0
  /\ lastRetiredPresentation = NoPresentation
  /\ lastAction = "init"
  /\ previousApplicationCount = applicationCount
  /\ previousPublishedAttachments = publishedAttachments

RegisterPresentation(presentation) ==
  /\ ~disposed
  /\ currentGeneration[presentation] = NoGeneration
  /\ currentGeneration' =
       [currentGeneration EXCEPT ![presentation] = 1]
  /\ UNCHANGED <<eventPhase, applied, settlementScheduled, hintPresent,
       hintEventMatches, hintPresentation, hintGeneration, matchedHistory,
       matchedPresentation, matchedGeneration, liveAttachments,
       publishedAttachments, applicationCount, applicationWitnesses,
       disposed, barrierActive, applicationAtBarrier,
       lastRetiredPresentation>>
  /\ Record("register-presentation")

ReceiveEvent ==
  /\ ~disposed
  /\ eventPhase = "absent"
  /\ eventPhase' = "pending"
  /\ UNCHANGED <<currentGeneration, applied, settlementScheduled,
       hintPresent, hintEventMatches, hintPresentation, hintGeneration,
       matchedHistory, matchedPresentation, matchedGeneration,
       liveAttachments, publishedAttachments, applicationCount,
       applicationWitnesses, disposed, barrierActive,
       applicationAtBarrier, lastRetiredPresentation>>
  /\ Record("receive-event")

ReceiveHint(correlation, presentation, matchesEvent) ==
  /\ ~disposed
  /\ currentGeneration[presentation] # NoGeneration
  /\ ~hintPresent[correlation]
  /\ correlation \notin matchedHistory
  /\ hintPresent' = [hintPresent EXCEPT ![correlation] = TRUE]
  /\ hintEventMatches' =
       [hintEventMatches EXCEPT ![correlation] = matchesEvent]
  /\ hintPresentation' =
       [hintPresentation EXCEPT ![correlation] = presentation]
  /\ hintGeneration' =
       [hintGeneration EXCEPT
          ![correlation] = currentGeneration[presentation]]
  /\ UNCHANGED <<currentGeneration, eventPhase, applied,
       settlementScheduled, matchedHistory, matchedPresentation,
       matchedGeneration, liveAttachments, publishedAttachments,
       applicationCount, applicationWitnesses, disposed, barrierActive,
       applicationAtBarrier, lastRetiredPresentation>>
  /\ Record("receive-hint")

JoinPartial(correlation) ==
  /\ Joinable(correlation)
  /\ ~GenerationConflict(correlation)
  /\ ~applied
  /\ matchedHistory \cup {correlation} # Correlations
  /\ hintPresent' = [hintPresent EXCEPT ![correlation] = FALSE]
  /\ matchedHistory' = matchedHistory \cup {correlation}
  /\ matchedPresentation' =
       [matchedPresentation EXCEPT
          ![correlation] = hintPresentation[correlation]]
  /\ matchedGeneration' =
       [matchedGeneration EXCEPT
          ![correlation] = hintGeneration[correlation]]
  /\ liveAttachments' = liveAttachments \cup {PairFor(correlation)}
  /\ settlementScheduled' = TRUE
  /\ UNCHANGED <<currentGeneration, eventPhase, applied,
       hintEventMatches, hintPresentation, hintGeneration,
       publishedAttachments, applicationCount, applicationWitnesses,
       disposed, barrierActive, applicationAtBarrier,
       lastRetiredPresentation>>
  /\ Record("join-partial")

CompleteJoin(correlation, action) ==
  /\ Joinable(correlation)
  /\ ~GenerationConflict(correlation)
  /\ ~applied
  /\ matchedHistory \cup {correlation} = Correlations
  /\ hintPresent' = [hintPresent EXCEPT ![correlation] = FALSE]
  /\ matchedHistory' = Correlations
  /\ matchedPresentation' =
       [matchedPresentation EXCEPT
          ![correlation] = hintPresentation[correlation]]
  /\ matchedGeneration' =
       [matchedGeneration EXCEPT
          ![correlation] = hintGeneration[correlation]]
  /\ liveAttachments' = liveAttachments \cup {PairFor(correlation)}
  /\ publishedAttachments' =
       liveAttachments \cup {PairFor(correlation)}
  /\ applicationWitnesses' =
       liveAttachments \cup {PairFor(correlation)}
  /\ applied' = TRUE
  /\ applicationCount' = 1
  /\ settlementScheduled' = FALSE
  /\ UNCHANGED <<currentGeneration, eventPhase, hintEventMatches,
       hintPresentation, hintGeneration, disposed, barrierActive,
       applicationAtBarrier, lastRetiredPresentation>>
  /\ Record(action)

JoinSingleton(correlation) ==
  /\ CorrelationVariant = "singleton"
  /\ CompleteJoin(correlation, "join-singleton")

JoinComplete(correlation) ==
  /\ CorrelationVariant = "pair"
  /\ CompleteJoin(correlation, "join-complete")

Settle ==
  /\ ~disposed
  /\ eventPhase = "pending"
  /\ settlementScheduled
  /\ ~applied
  /\ liveAttachments # {}
  /\ applied' = TRUE
  /\ settlementScheduled' = FALSE
  /\ publishedAttachments' = liveAttachments
  /\ applicationCount' = 1
  /\ applicationWitnesses' = liveAttachments
  /\ UNCHANGED <<currentGeneration, eventPhase, hintPresent,
       hintEventMatches, hintPresentation, hintGeneration, matchedHistory,
       matchedPresentation, matchedGeneration, liveAttachments, disposed,
       barrierActive, applicationAtBarrier, lastRetiredPresentation>>
  /\ Record("settle")

JoinLate(correlation) ==
  /\ Joinable(correlation)
  /\ ~GenerationConflict(correlation)
  /\ applied
  /\ hintPresent' = [hintPresent EXCEPT ![correlation] = FALSE]
  /\ matchedHistory' = matchedHistory \cup {correlation}
  /\ matchedPresentation' =
       [matchedPresentation EXCEPT
          ![correlation] = hintPresentation[correlation]]
  /\ matchedGeneration' =
       [matchedGeneration EXCEPT
          ![correlation] = hintGeneration[correlation]]
  /\ liveAttachments' = liveAttachments \cup {PairFor(correlation)}
  /\ publishedAttachments' =
       publishedAttachments \cup {PairFor(correlation)}
  /\ UNCHANGED <<currentGeneration, eventPhase, applied,
       settlementScheduled, hintEventMatches, hintPresentation,
       hintGeneration, applicationCount, applicationWitnesses, disposed,
       barrierActive, applicationAtBarrier, lastRetiredPresentation>>
  /\ Record("join-late")

RejectGenerationRetarget(correlation) ==
  /\ Joinable(correlation)
  /\ GenerationConflict(correlation)
  /\ hintPresent' = [hintPresent EXCEPT ![correlation] = FALSE]
  /\ UNCHANGED <<currentGeneration, eventPhase, applied,
       settlementScheduled, hintEventMatches, hintPresentation,
       hintGeneration, matchedHistory, matchedPresentation,
       matchedGeneration, liveAttachments, publishedAttachments,
       applicationCount, applicationWitnesses, disposed, barrierActive,
       applicationAtBarrier, lastRetiredPresentation>>
  /\ Record("reject-generation-retarget")

RejectMismatchedIdentity(correlation) ==
  /\ ~disposed
  /\ eventPhase = "pending"
  /\ hintPresent[correlation]
  /\ ~hintEventMatches[correlation]
  /\ UNCHANGED core
  /\ Record("reject-mismatched-identity")

ExpireHint(correlation) ==
  /\ ~disposed
  /\ hintPresent[correlation]
  /\ hintPresent' = [hintPresent EXCEPT ![correlation] = FALSE]
  /\ UNCHANGED <<currentGeneration, eventPhase, applied,
       settlementScheduled, hintEventMatches, hintPresentation,
       hintGeneration, matchedHistory, matchedPresentation,
       matchedGeneration, liveAttachments, publishedAttachments,
       applicationCount, applicationWitnesses, disposed, barrierActive,
       applicationAtBarrier, lastRetiredPresentation>>
  /\ Record("expire-hint")

ExpireEvent ==
  /\ ~disposed
  /\ eventPhase = "pending"
  /\ eventPhase' = "expired"
  /\ settlementScheduled' = FALSE
  /\ barrierActive' = TRUE
  /\ applicationAtBarrier' = applicationCount
  /\ UNCHANGED <<currentGeneration, applied, hintPresent,
       hintEventMatches, hintPresentation, hintGeneration, matchedHistory,
       matchedPresentation, matchedGeneration, liveAttachments,
       publishedAttachments, applicationCount, applicationWitnesses,
       disposed, lastRetiredPresentation>>
  /\ Record("expire-event")

AdvanceGeneration(presentation) ==
  /\ ~disposed
  /\ currentGeneration[presentation] = 1
  /\ currentGeneration' =
       [currentGeneration EXCEPT ![presentation] = 2]
  /\ hintPresent' =
       [correlation \in Correlations |->
          IF hintPresent[correlation]
             /\ hintPresentation[correlation] = presentation
          THEN FALSE
          ELSE hintPresent[correlation]]
  /\ liveAttachments' =
       {attachment \in liveAttachments:
          attachment.presentation # presentation}
  /\ publishedAttachments' =
       {attachment \in publishedAttachments:
          attachment.presentation # presentation}
  /\ lastRetiredPresentation' = presentation
  /\ UNCHANGED <<eventPhase, applied, settlementScheduled,
       hintEventMatches, hintPresentation, hintGeneration, matchedHistory,
       matchedPresentation, matchedGeneration, applicationCount,
       applicationWitnesses, disposed, barrierActive,
       applicationAtBarrier>>
  /\ Record("advance-generation")

RetirePresentation(presentation) ==
  /\ ~disposed
  /\ currentGeneration[presentation] # NoGeneration
  /\ currentGeneration' =
       [currentGeneration EXCEPT ![presentation] = NoGeneration]
  /\ hintPresent' =
       [correlation \in Correlations |->
          IF hintPresent[correlation]
             /\ hintPresentation[correlation] = presentation
          THEN FALSE
          ELSE hintPresent[correlation]]
  /\ liveAttachments' =
       {attachment \in liveAttachments:
          attachment.presentation # presentation}
  /\ publishedAttachments' =
       {attachment \in publishedAttachments:
          attachment.presentation # presentation}
  /\ lastRetiredPresentation' = presentation
  /\ UNCHANGED <<eventPhase, applied, settlementScheduled,
       hintEventMatches, hintPresentation, hintGeneration, matchedHistory,
       matchedPresentation, matchedGeneration, applicationCount,
       applicationWitnesses, disposed, barrierActive,
       applicationAtBarrier>>
  /\ Record("retire-presentation")

FireStaleDeadline ==
  /\ ~settlementScheduled
  /\ eventPhase = "expired" \/ disposed
  /\ UNCHANGED core
  /\ Record("fire-stale-deadline")

Dispose ==
  /\ ~disposed
  /\ disposed' = TRUE
  /\ eventPhase' = "absent"
  /\ settlementScheduled' = FALSE
  /\ hintPresent' = [correlation \in Correlations |-> FALSE]
  /\ liveAttachments' = {}
  /\ barrierActive' = TRUE
  /\ applicationAtBarrier' = applicationCount
  /\ UNCHANGED <<currentGeneration, applied, hintEventMatches,
       hintPresentation, hintGeneration, matchedHistory,
       matchedPresentation, matchedGeneration, publishedAttachments,
       applicationCount, applicationWitnesses,
       lastRetiredPresentation>>
  /\ Record("dispose")

Next ==
  \/ \E presentation \in Presentations:
       RegisterPresentation(presentation)
  \/ ReceiveEvent
  \/ \E correlation \in Correlations,
       presentation \in Presentations,
       matchesEvent \in BOOLEAN:
       ReceiveHint(correlation, presentation, matchesEvent)
  \/ \E correlation \in Correlations: JoinPartial(correlation)
  \/ \E correlation \in Correlations: JoinSingleton(correlation)
  \/ \E correlation \in Correlations: JoinComplete(correlation)
  \/ Settle
  \/ \E correlation \in Correlations: JoinLate(correlation)
  \/ \E correlation \in Correlations:
       RejectGenerationRetarget(correlation)
  \/ \E correlation \in Correlations:
       RejectMismatchedIdentity(correlation)
  \/ \E correlation \in Correlations: ExpireHint(correlation)
  \/ ExpireEvent
  \/ \E presentation \in Presentations:
       AdvanceGeneration(presentation)
  \/ \E presentation \in Presentations:
       RetirePresentation(presentation)
  \/ FireStaleDeadline
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ CorrelationVariant \in CorrelationVariants
  /\ currentGeneration \in [Presentations -> 0..2]
  /\ eventPhase \in EventPhases
  /\ applied \in BOOLEAN
  /\ settlementScheduled \in BOOLEAN
  /\ hintPresent \in [Correlations -> BOOLEAN]
  /\ hintEventMatches \in [Correlations -> BOOLEAN]
  /\ hintPresentation \in
       [Correlations -> Presentations \cup {NoPresentation}]
  /\ hintGeneration \in [Correlations -> 0..2]
  /\ matchedHistory \subseteq Correlations
  /\ matchedPresentation \in
       [Correlations -> Presentations \cup {NoPresentation}]
  /\ matchedGeneration \in [Correlations -> 0..2]
  /\ liveAttachments \subseteq AttachmentPairs
  /\ publishedAttachments \subseteq AttachmentPairs
  /\ applicationCount \in 0..1
  /\ applicationWitnesses \subseteq AttachmentPairs
  /\ disposed \in BOOLEAN
  /\ barrierActive \in BOOLEAN
  /\ applicationAtBarrier \in 0..1
  /\ lastRetiredPresentation \in Presentations \cup {NoPresentation}
  /\ previousApplicationCount \in 0..1
  /\ previousPublishedAttachments \subseteq AttachmentPairs

SingletonNeverSchedulesSettlement ==
  Cardinality(Correlations) = 1 => ~settlementScheduled

ApplicationRequiresExactLiveJoin ==
  applicationCount = 1 =>
    /\ applicationWitnesses # {}
    /\ \A attachment \in applicationWitnesses:
         \E correlation \in matchedHistory:
           attachment = MatchedPairFor(correlation)

ApplicationIsAtMostOnce == applicationCount <= 1

LateJoinNeverReapplies ==
  lastAction = "join-late" =>
    /\ applicationCount = previousApplicationCount
    /\ previousPublishedAttachments \subseteq publishedAttachments

MatchedPresentationNeverChangesGeneration ==
  \A left, right \in matchedHistory:
    matchedPresentation[left] = matchedPresentation[right] =>
      matchedGeneration[left] = matchedGeneration[right]

RetirementIsPresentationLocal ==
  lastAction \in {"advance-generation", "retire-presentation"} =>
    publishedAttachments =
      {attachment \in previousPublishedAttachments:
        attachment.presentation # lastRetiredPresentation}

ExpiredOrDisposedCannotApply ==
  barrierActive => applicationCount = applicationAtBarrier

StaleDeadlineCannotApply ==
  lastAction = "fire-stale-deadline" =>
    /\ applicationCount = previousApplicationCount
    /\ publishedAttachments = previousPublishedAttachments

UnmatchedHalvesCannotApply ==
  lastAction \in {
    "receive-event", "receive-hint", "reject-mismatched-identity"
  } => applicationCount = previousApplicationCount

LiveAttachmentsUseCurrentMatchedGeneration ==
  \A attachment \in liveAttachments:
    /\ currentGeneration[attachment.presentation] = attachment.generation
    /\ \E correlation \in matchedHistory:
         attachment = MatchedPairFor(correlation)

PublishedAttachmentsRequireApplication ==
  publishedAttachments # {} => applied

=============================================================================
