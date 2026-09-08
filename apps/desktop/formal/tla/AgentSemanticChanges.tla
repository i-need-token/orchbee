-------------------- MODULE AgentSemanticChanges --------------------
EXTENDS Naturals, Sequences, TLC

\* Bounded abstraction for the volatile, loss-aware Agent change feed.
\* HMAC implementation, provider ingestion, semantic-command receipt
\* retention, transport, and restart persistence remain outside this model.

CONSTANT MaxRevision, Retention, MaxBatch, MaxGeneration

FeedPhases == {
  "absent", "waiting", "baseline", "changes", "current",
  "reset-required", "invalid", "cancelled", "coordinator-retired"
}

VARIABLES
  revision,
  journal,
  coordinatorGeneration,
  feedPhase,
  afterRevision,
  batchLimit,
  subscribed,
  outputItems,
  outputThroughRevision,
  outputRuntimeRevision,
  outputGeneration,
  outputHasMore,
  lastAction

vars == <<revision, journal, coordinatorGeneration, feedPhase,
  afterRevision, batchLimit, subscribed, outputItems,
  outputThroughRevision, outputRuntimeRevision, outputGeneration,
  outputHasMore, lastAction>>

Min(left, right) == IF left <= right THEN left ELSE right

ExpectedJournal(currentRevision) ==
  LET retained == Min(currentRevision, Retention)
  IN [index \in 1..retained |-> currentRevision - retained + index]

AppendRetained(records, nextRevision) ==
  IF Len(records) < Retention
  THEN Append(records, nextRevision)
  ELSE Append(Tail(records), nextRevision)

GapAt(baseline, currentRevision, records) ==
  baseline < currentRevision /\
    IF Len(records) = 0
    THEN TRUE
    ELSE baseline < Head(records) - 1

BatchAfter(baseline, limit, currentRevision) ==
  [index \in 1..Min(limit, currentRevision - baseline) |-> baseline + index]

Init ==
  /\ revision = 0
  /\ journal = <<>>
  /\ coordinatorGeneration = 1
  /\ feedPhase = "absent"
  /\ afterRevision = 0
  /\ batchLimit = 1
  /\ subscribed = FALSE
  /\ outputItems = <<>>
  /\ outputThroughRevision = 0
  /\ outputRuntimeRevision = 0
  /\ outputGeneration = 0
  /\ outputHasMore = FALSE
  /\ lastAction = "init"

BeginBaseline(limit) ==
  /\ feedPhase = "absent"
  /\ limit \in 1..MaxBatch
  /\ feedPhase' = "baseline"
  /\ afterRevision' = revision
  /\ batchLimit' = limit
  /\ subscribed' = FALSE
  /\ outputItems' = <<>>
  /\ outputThroughRevision' = revision
  /\ outputRuntimeRevision' = revision
  /\ outputGeneration' = coordinatorGeneration
  /\ outputHasMore' = FALSE
  /\ UNCHANGED <<revision, journal, coordinatorGeneration>>
  /\ lastAction' = "begin-baseline"

BeginFollowCurrent(baseline, limit) ==
  /\ feedPhase = "absent"
  /\ baseline = revision
  /\ limit \in 1..MaxBatch
  /\ feedPhase' = "waiting"
  /\ afterRevision' = baseline
  /\ batchLimit' = limit
  /\ subscribed' = TRUE
  /\ outputItems' = <<>>
  /\ outputThroughRevision' = 0
  /\ outputRuntimeRevision' = 0
  /\ outputGeneration' = 0
  /\ outputHasMore' = FALSE
  /\ UNCHANGED <<revision, journal, coordinatorGeneration>>
  /\ lastAction' = "begin-follow-current"

BeginFollowChanges(baseline, limit) ==
  /\ feedPhase = "absent"
  /\ baseline \in 0..revision
  /\ baseline < revision
  /\ ~GapAt(baseline, revision, journal)
  /\ limit \in 1..MaxBatch
  /\ LET items == BatchAfter(baseline, limit, revision)
     IN /\ feedPhase' = "changes"
        /\ afterRevision' = baseline
        /\ batchLimit' = limit
        /\ subscribed' = FALSE
        /\ outputItems' = items
        /\ outputThroughRevision' = items[Len(items)]
        /\ outputRuntimeRevision' = revision
        /\ outputGeneration' = coordinatorGeneration
        /\ outputHasMore' = (items[Len(items)] < revision)
  /\ UNCHANGED <<revision, journal, coordinatorGeneration>>
  /\ lastAction' = "begin-follow-changes"

BeginFollowReset(baseline, limit) ==
  /\ feedPhase = "absent"
  /\ baseline \in 0..revision
  /\ GapAt(baseline, revision, journal)
  /\ limit \in 1..MaxBatch
  /\ feedPhase' = "reset-required"
  /\ afterRevision' = baseline
  /\ batchLimit' = limit
  /\ subscribed' = FALSE
  /\ outputItems' = <<>>
  /\ outputThroughRevision' = revision
  /\ outputRuntimeRevision' = revision
  /\ outputGeneration' = coordinatorGeneration
  /\ outputHasMore' = FALSE
  /\ UNCHANGED <<revision, journal, coordinatorGeneration>>
  /\ lastAction' = "begin-follow-reset"

BeginInvalidCursor ==
  /\ feedPhase = "absent"
  /\ feedPhase' = "invalid"
  /\ subscribed' = FALSE
  /\ outputItems' = <<>>
  /\ outputThroughRevision' = 0
  /\ outputRuntimeRevision' = 0
  /\ outputGeneration' = 0
  /\ outputHasMore' = FALSE
  /\ UNCHANGED <<revision, journal, coordinatorGeneration,
       afterRevision, batchLimit>>
  /\ lastAction' = "begin-invalid-cursor"

AgentUpdate ==
  /\ revision < MaxRevision
  /\ LET nextRevision == revision + 1
         nextJournal == AppendRetained(journal, nextRevision)
     IN /\ revision' = nextRevision
        /\ journal' = nextJournal
        /\ IF feedPhase = "waiting"
           THEN LET items == BatchAfter(afterRevision, batchLimit, nextRevision)
                IN /\ feedPhase' = "changes"
                   /\ subscribed' = FALSE
                   /\ outputItems' = items
                   /\ outputThroughRevision' = items[Len(items)]
                   /\ outputRuntimeRevision' = nextRevision
                   /\ outputGeneration' = coordinatorGeneration
                   /\ outputHasMore' = (items[Len(items)] < nextRevision)
           ELSE /\ UNCHANGED <<feedPhase, subscribed, outputItems,
                       outputThroughRevision, outputRuntimeRevision,
                       outputGeneration, outputHasMore>>
  /\ UNCHANGED <<coordinatorGeneration, afterRevision, batchLimit>>
  /\ lastAction' = "agent-update"

Timeout ==
  /\ feedPhase = "waiting"
  /\ feedPhase' = "current"
  /\ subscribed' = FALSE
  /\ outputItems' = <<>>
  /\ outputThroughRevision' = afterRevision
  /\ outputRuntimeRevision' = revision
  /\ outputGeneration' = coordinatorGeneration
  /\ outputHasMore' = FALSE
  /\ UNCHANGED <<revision, journal, coordinatorGeneration,
       afterRevision, batchLimit>>
  /\ lastAction' = "timeout"

Cancel ==
  /\ feedPhase = "waiting"
  /\ feedPhase' = "cancelled"
  /\ subscribed' = FALSE
  /\ UNCHANGED <<revision, journal, coordinatorGeneration,
       afterRevision, batchLimit, outputItems, outputThroughRevision,
       outputRuntimeRevision, outputGeneration, outputHasMore>>
  /\ lastAction' = "cancel"

CoordinatorRetire ==
  /\ coordinatorGeneration < MaxGeneration
  /\ coordinatorGeneration' = coordinatorGeneration + 1
  /\ IF feedPhase = "waiting"
     THEN /\ feedPhase' = "coordinator-retired"
          /\ subscribed' = FALSE
     ELSE /\ UNCHANGED <<feedPhase, subscribed>>
  /\ UNCHANGED <<revision, journal, afterRevision, batchLimit,
       outputItems, outputThroughRevision, outputRuntimeRevision,
       outputGeneration, outputHasMore>>
  /\ lastAction' = "coordinator-retire"

Next ==
  \/ \E limit \in 1..MaxBatch: BeginBaseline(limit)
  \/ \E baseline \in 0..MaxRevision, limit \in 1..MaxBatch:
       BeginFollowCurrent(baseline, limit)
  \/ \E baseline \in 0..MaxRevision, limit \in 1..MaxBatch:
       BeginFollowChanges(baseline, limit)
  \/ \E baseline \in 0..MaxRevision, limit \in 1..MaxBatch:
       BeginFollowReset(baseline, limit)
  \/ BeginInvalidCursor
  \/ AgentUpdate
  \/ Timeout
  \/ Cancel
  \/ CoordinatorRetire

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ Retention \in 1..MaxRevision
  /\ MaxBatch \in 1..MaxRevision
  /\ revision \in 0..MaxRevision
  /\ journal \in Seq(0..MaxRevision)
  /\ coordinatorGeneration \in 1..MaxGeneration
  /\ feedPhase \in FeedPhases
  /\ afterRevision \in 0..MaxRevision
  /\ batchLimit \in 1..MaxBatch
  /\ subscribed \in BOOLEAN
  /\ outputItems \in Seq(0..MaxRevision)
  /\ outputThroughRevision \in 0..MaxRevision
  /\ outputRuntimeRevision \in 0..MaxRevision
  /\ outputGeneration \in 0..MaxGeneration
  /\ outputHasMore \in BOOLEAN

JournalIsBoundedOrderedSuffix ==
  /\ Len(journal) <= Retention
  /\ journal = ExpectedJournal(revision)

WaitingHasExactSubscription ==
  subscribed = (feedPhase = "waiting")

WaitingCursorIsCurrent ==
  feedPhase = "waiting" => afterRevision = revision

ChangePageIsOrderedAndBounded ==
  feedPhase = "changes" =>
    /\ Len(outputItems) \in 1..batchLimit
    /\ outputItems = BatchAfter(afterRevision, batchLimit,
         outputRuntimeRevision)
    /\ outputThroughRevision = outputItems[Len(outputItems)]
    /\ outputThroughRevision <= outputRuntimeRevision
    /\ outputHasMore = (outputThroughRevision < outputRuntimeRevision)
    /\ outputGeneration \in 1..coordinatorGeneration

NoChangePageSilentlyCrossesGap ==
  feedPhase = "changes" =>
    \/ outputRuntimeRevision <= Retention
    \/ afterRevision >= outputRuntimeRevision - Retention

ResetIsExplicitAndAdvancesCursor ==
  feedPhase = "reset-required" =>
    /\ outputItems = <<>>
    /\ outputHasMore = FALSE
    /\ outputThroughRevision = outputRuntimeRevision
    /\ outputRuntimeRevision > Retention
    /\ afterRevision < outputRuntimeRevision - Retention
    /\ outputGeneration \in 1..coordinatorGeneration

EmptyPagesDoNotInventChanges ==
  feedPhase \in {"baseline", "current", "reset-required"} =>
    /\ outputItems = <<>>
    /\ outputHasMore = FALSE
    /\ outputThroughRevision = outputRuntimeRevision

NonresultsHaveNoOutput ==
  feedPhase \in {"absent", "waiting", "invalid", "cancelled",
      "coordinator-retired"} =>
    /\ outputItems = <<>>
    /\ outputRuntimeRevision = 0
    /\ outputGeneration = 0
    /\ outputHasMore = FALSE

====================================================================
