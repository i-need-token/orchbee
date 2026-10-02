------------------- MODULE NativeTerminalSelectionEffects -------------------
EXTENDS FiniteSets, Naturals, Sequences

CONSTANTS ClipboardSink, AllowProgramWrites, MaxChunks, MaxEffectId,
          MaxQueuedEffects

ASSUME
  /\ ClipboardSink \in {"windows", "osc52"}
  /\ AllowProgramWrites \in BOOLEAN
  /\ MaxChunks \in 1..3
  /\ MaxEffectId \in 2..4
  /\ MaxQueuedEffects \in 1..MaxEffectId

CopyPhases == {
  "idle", "starting", "receiving", "decoding", "ready", "queued",
  "committing", "done", "cancelled", "failed", "unknown"
}
ActiveCopyPhases == {"starting", "receiving", "decoding", "ready", "queued"}
EffectKinds == {"selection", "program", "bell"}
EffectSinks == {"windows", "osc52", "terminal"}
NoEffect == [id |-> 0, kind |-> "none", sink |-> "none"]
EffectRecords == [id: 1..MaxEffectId, kind: EffectKinds, sink: EffectSinks]

Effect(id, kind) ==
  [id |-> id,
   kind |-> kind,
   sink |-> IF kind = "bell" THEN "terminal" ELSE ClipboardSink]
UsesTerminalOutput(effect) == effect.kind = "bell" \/ effect.sink = "osc52"

VARIABLES
  copyPhase, earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
  finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
  attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
  lastBellSequence, lastProgramSequence, rejectedEffects,
  deniedProgramWrites, frameOpen, lifecycle

vars ==
  <<copyPhase, earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
    finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
    attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
    lastBellSequence, lastProgramSequence, rejectedEffects,
    deniedProgramWrites, frameOpen, lifecycle>>

Init ==
  /\ copyPhase = "idle"
  /\ earlyChunk = 0
  /\ expectedChunks = 0
  /\ nextChunk = 0
  /\ finalChunkSeen = FALSE
  /\ finishSeen = FALSE
  /\ utf8Validated = FALSE
  /\ effectQueue = <<>>
  /\ nextEffectId = 1
  /\ inFlight = NoEffect
  /\ attemptedOrder = <<>>
  /\ settledIds = {}
  /\ copyCommits = 0
  /\ programCommits = 0
  /\ bellCommits = 0
  /\ lastBellSequence = 0
  /\ lastProgramSequence = 0
  /\ rejectedEffects = 0
  /\ deniedProgramWrites = 0
  /\ frameOpen = FALSE
  /\ lifecycle = "active"

AcceptSelectionEnd ==
  /\ lifecycle = "active"
  /\ copyPhase = "idle"
  /\ copyPhase' = "starting"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectSelectionEnd ==
  /\ lifecycle = "active"
  /\ copyPhase = "idle"
  /\ rejectedEffects < 2
  /\ rejectedEffects' = rejectedEffects + 1
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, lastProgramSequence,
       deniedProgramWrites, frameOpen, lifecycle>>

ReceiveEarlyChunk ==
  /\ lifecycle = "active"
  /\ copyPhase = "starting"
  /\ earlyChunk = 0
  /\ earlyChunk' = 1
  /\ UNCHANGED <<copyPhase, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectSecondEarlyChunk ==
  /\ lifecycle = "active"
  /\ copyPhase = "starting"
  /\ earlyChunk = 1
  /\ copyPhase' = "failed"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

StartExport ==
  /\ lifecycle = "active"
  /\ copyPhase = "starting"
  /\ \E total \in 1..MaxChunks:
       /\ total >= earlyChunk
       /\ copyPhase' = "receiving"
       /\ expectedChunks' = total
       /\ nextChunk' = earlyChunk
       /\ finalChunkSeen' = (earlyChunk = total)
  /\ UNCHANGED <<earlyChunk, finishSeen, utf8Validated, effectQueue,
       nextEffectId, inFlight, attemptedOrder, settledIds, copyCommits,
       programCommits, bellCommits, lastBellSequence, lastProgramSequence,
       rejectedEffects, deniedProgramWrites, frameOpen, lifecycle>>

ReceiveNextChunk ==
  /\ lifecycle = "active"
  /\ copyPhase = "receiving"
  /\ nextChunk < expectedChunks
  /\ nextChunk' = nextChunk + 1
  /\ finalChunkSeen' = (nextChunk + 1 = expectedChunks)
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, finishSeen,
       utf8Validated, effectQueue, nextEffectId, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectOutOfOrderChunk ==
  /\ lifecycle = "active"
  /\ copyPhase = "receiving"
  /\ copyPhase' = "failed"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

FinishExport ==
  /\ lifecycle = "active"
  /\ copyPhase = "receiving"
  /\ nextChunk = expectedChunks
  /\ finalChunkSeen
  /\ copyPhase' = "decoding"
  /\ finishSeen' = TRUE
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       utf8Validated, effectQueue, nextEffectId, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectPrematureFinish ==
  /\ lifecycle = "active"
  /\ copyPhase = "receiving"
  /\ (nextChunk # expectedChunks \/ ~finalChunkSeen)
  /\ copyPhase' = "failed"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectWrongExportIdentity ==
  /\ lifecycle = "active"
  /\ copyPhase \in {"starting", "receiving", "decoding"}
  /\ copyPhase' = "failed"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

ValidateUtf8 ==
  /\ lifecycle = "active"
  /\ copyPhase = "decoding"
  /\ finishSeen
  /\ copyPhase' = "ready"
  /\ utf8Validated' = TRUE
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, effectQueue, nextEffectId, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectInvalidUtf8 ==
  /\ lifecycle = "active"
  /\ copyPhase = "decoding"
  /\ finishSeen
  /\ copyPhase' = "failed"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, inFlight,
       attemptedOrder, settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

EnqueueSelectionCopy ==
  /\ lifecycle = "active"
  /\ copyPhase = "ready"
  /\ Len(effectQueue) < MaxQueuedEffects
  /\ nextEffectId <= MaxEffectId
  /\ effectQueue' = Append(effectQueue, Effect(nextEffectId, "selection"))
  /\ nextEffectId' = nextEffectId + 1
  /\ copyPhase' = "queued"
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, inFlight, attemptedOrder, settledIds,
       copyCommits, programCommits, bellCommits, lastBellSequence,
       lastProgramSequence, rejectedEffects, deniedProgramWrites,
       frameOpen, lifecycle>>

CancelSelectionCopy ==
  /\ lifecycle = "active"
  /\ copyPhase \in ActiveCopyPhases
  /\ copyPhase' = "cancelled"
  /\ effectQueue' = SelectSeq(effectQueue, LAMBDA effect: effect.kind # "selection")
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, nextEffectId, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

ReceiveBell ==
  /\ lifecycle = "active"
  /\ Len(effectQueue) < MaxQueuedEffects
  /\ nextEffectId <= MaxEffectId
  /\ lastBellSequence < MaxEffectId
  /\ \E sequence \in (lastBellSequence + 1)..MaxEffectId:
       /\ effectQueue' = Append(effectQueue, Effect(nextEffectId, "bell"))
       /\ nextEffectId' = nextEffectId + 1
       /\ lastBellSequence' = sequence
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastProgramSequence, rejectedEffects, deniedProgramWrites,
       frameOpen, lifecycle>>

QueueProgramWrite ==
  /\ AllowProgramWrites
  /\ lifecycle = "active"
  /\ Len(effectQueue) < MaxQueuedEffects
  /\ nextEffectId <= MaxEffectId
  /\ lastProgramSequence < MaxEffectId
  /\ \E sequence \in (lastProgramSequence + 1)..MaxEffectId:
       /\ effectQueue' = Append(effectQueue, Effect(nextEffectId, "program"))
       /\ nextEffectId' = nextEffectId + 1
       /\ lastProgramSequence' = sequence
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, rejectedEffects, deniedProgramWrites,
       frameOpen, lifecycle>>

DenyProgramWrite ==
  /\ ~AllowProgramWrites
  /\ lifecycle = "active"
  /\ deniedProgramWrites < 2
  /\ lastProgramSequence < MaxEffectId
  /\ \E sequence \in (lastProgramSequence + 1)..MaxEffectId:
       /\ lastProgramSequence' = sequence
       /\ deniedProgramWrites' = deniedProgramWrites + 1
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, rejectedEffects, frameOpen, lifecycle>>

RejectForeignEffect ==
  /\ lifecycle = "active"
  /\ rejectedEffects < 2
  /\ rejectedEffects' = rejectedEffects + 1
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, lastProgramSequence,
       deniedProgramWrites, frameOpen, lifecycle>>

RejectStaleEffect ==
  /\ lifecycle = "active"
  /\ rejectedEffects < 2
  /\ (lastBellSequence > 0 \/ lastProgramSequence > 0)
  /\ rejectedEffects' = rejectedEffects + 1
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, lastProgramSequence,
       deniedProgramWrites, frameOpen, lifecycle>>

BeginFrame ==
  /\ lifecycle = "active"
  /\ ~frameOpen
  /\ (inFlight = NoEffect \/ ~UsesTerminalOutput(inFlight))
  /\ frameOpen' = TRUE
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, lifecycle>>

EndFrameAndFlush ==
  /\ lifecycle = "active"
  /\ frameOpen
  /\ frameOpen' = FALSE
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, lifecycle>>

DispatchEffect ==
  /\ lifecycle = "active"
  /\ ~frameOpen
  /\ inFlight = NoEffect
  /\ Len(effectQueue) > 0
  /\ LET effect == Head(effectQueue)
     IN /\ effectQueue' = Tail(effectQueue)
        /\ inFlight' = effect
        /\ attemptedOrder' = Append(attemptedOrder, effect.id)
        /\ copyPhase' = IF effect.kind = "selection" THEN "committing" ELSE copyPhase
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, nextEffectId, settledIds, copyCommits,
       programCommits, bellCommits, lastBellSequence, lastProgramSequence,
       rejectedEffects, deniedProgramWrites, frameOpen, lifecycle>>

EffectSucceeds ==
  /\ lifecycle \in {"active", "draining"}
  /\ inFlight # NoEffect
  /\ settledIds' = settledIds \cup {inFlight.id}
  /\ copyCommits' = copyCommits + (IF inFlight.kind = "selection" THEN 1 ELSE 0)
  /\ programCommits' = programCommits + (IF inFlight.kind = "program" THEN 1 ELSE 0)
  /\ bellCommits' = bellCommits + (IF inFlight.kind = "bell" THEN 1 ELSE 0)
  /\ copyPhase' = IF inFlight.kind = "selection" THEN "done" ELSE copyPhase
  /\ inFlight' = NoEffect
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, attemptedOrder,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen, lifecycle>>

EffectFailsUnknown ==
  /\ lifecycle \in {"active", "draining"}
  /\ inFlight # NoEffect
  /\ settledIds' = settledIds \cup {inFlight.id}
  /\ copyPhase' = IF inFlight.kind = "selection" THEN "unknown" ELSE copyPhase
  /\ inFlight' = NoEffect
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, effectQueue, nextEffectId, attemptedOrder,
       copyCommits, programCommits, bellCommits, lastBellSequence,
       lastProgramSequence, rejectedEffects, deniedProgramWrites,
       frameOpen, lifecycle>>

RequestRetirement ==
  /\ lifecycle = "active"
  /\ lifecycle' = "draining"
  /\ frameOpen' = FALSE
  /\ effectQueue' = <<>>
  /\ copyPhase' =
       IF copyPhase = "committing"
       THEN copyPhase
       ELSE IF copyPhase \in ActiveCopyPhases THEN "cancelled" ELSE copyPhase
  /\ UNCHANGED <<earlyChunk, expectedChunks, nextChunk, finalChunkSeen,
       finishSeen, utf8Validated, nextEffectId, inFlight, attemptedOrder,
       settledIds, copyCommits, programCommits, bellCommits,
       lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites>>

FinishRetirement ==
  /\ lifecycle = "draining"
  /\ inFlight = NoEffect
  /\ lifecycle' = "retired"
  /\ UNCHANGED <<copyPhase, earlyChunk, expectedChunks, nextChunk,
       finalChunkSeen, finishSeen, utf8Validated, effectQueue, nextEffectId,
       inFlight, attemptedOrder, settledIds, copyCommits, programCommits,
       bellCommits, lastBellSequence, lastProgramSequence, rejectedEffects,
       deniedProgramWrites, frameOpen>>

CoverageEffectWaitsForFrameClose ==
  /\ frameOpen
  /\ Len(effectQueue) > 0
  /\ inFlight = NoEffect
  /\ UNCHANGED vars

CoverageSelectionQueuedBehindBell ==
  /\ Len(effectQueue) >= 2
  /\ effectQueue[1].kind = "bell"
  /\ effectQueue[2].kind = "selection"
  /\ UNCHANGED vars

CoverageCancelQueuedSelection ==
  /\ copyPhase = "queued"
  /\ CancelSelectionCopy

CoverageUnknownSelectionDispatch ==
  /\ inFlight.kind = "selection"
  /\ EffectFailsUnknown

CoverageRetirementWaitsForDispatchedEffect ==
  /\ lifecycle = "draining"
  /\ inFlight # NoEffect
  /\ UNCHANGED vars

Next ==
  \/ AcceptSelectionEnd
  \/ RejectSelectionEnd
  \/ ReceiveEarlyChunk
  \/ RejectSecondEarlyChunk
  \/ StartExport
  \/ ReceiveNextChunk
  \/ RejectOutOfOrderChunk
  \/ FinishExport
  \/ RejectPrematureFinish
  \/ RejectWrongExportIdentity
  \/ ValidateUtf8
  \/ RejectInvalidUtf8
  \/ EnqueueSelectionCopy
  \/ CancelSelectionCopy
  \/ ReceiveBell
  \/ QueueProgramWrite
  \/ DenyProgramWrite
  \/ RejectForeignEffect
  \/ RejectStaleEffect
  \/ BeginFrame
  \/ EndFrameAndFlush
  \/ DispatchEffect
  \/ EffectSucceeds
  \/ EffectFailsUnknown
  \/ RequestRetirement
  \/ FinishRetirement
  \/ CoverageEffectWaitsForFrameClose
  \/ CoverageSelectionQueuedBehindBell
  \/ CoverageCancelQueuedSelection
  \/ CoverageUnknownSelectionDispatch
  \/ CoverageRetirementWaitsForDispatchedEffect

Spec == Init /\ [][Next]_vars

QueueIds == {effectQueue[index].id: index \in 1..Len(effectQueue)}
AttemptedIds == {attemptedOrder[index]: index \in 1..Len(attemptedOrder)}
Increasing(sequence) ==
  \A left, right \in 1..Len(sequence):
    left < right => sequence[left] < sequence[right]
ValidSink(effect) ==
  IF effect.kind = "bell"
  THEN effect.sink = "terminal"
  ELSE effect.sink = ClipboardSink

TypeOK ==
  /\ copyPhase \in CopyPhases
  /\ earlyChunk \in 0..1
  /\ expectedChunks \in 0..MaxChunks
  /\ nextChunk \in 0..MaxChunks
  /\ finalChunkSeen \in BOOLEAN
  /\ finishSeen \in BOOLEAN
  /\ utf8Validated \in BOOLEAN
  /\ effectQueue \in Seq(EffectRecords)
  /\ nextEffectId \in 1..(MaxEffectId + 1)
  /\ inFlight \in EffectRecords \cup {NoEffect}
  /\ attemptedOrder \in Seq(1..MaxEffectId)
  /\ settledIds \subseteq 1..MaxEffectId
  /\ copyCommits \in 0..1
  /\ programCommits \in 0..MaxEffectId
  /\ bellCommits \in 0..MaxEffectId
  /\ lastBellSequence \in 0..MaxEffectId
  /\ lastProgramSequence \in 0..MaxEffectId
  /\ rejectedEffects \in 0..2
  /\ deniedProgramWrites \in 0..2
  /\ frameOpen \in BOOLEAN
  /\ lifecycle \in {"active", "draining", "retired"}

EffectQueueIsBounded == Len(effectQueue) <= MaxQueuedEffects
QueuedEffectIdsIncrease == Increasing([index \in 1..Len(effectQueue) |-> effectQueue[index].id])
AttemptOrderIncreases == Increasing(attemptedOrder)
AttemptedEffectsAreUnique == Cardinality(AttemptedIds) = Len(attemptedOrder)
AttemptedEffectsNeverQueue ==
  /\ QueueIds \cap AttemptedIds = {}
  /\ (inFlight = NoEffect \/ inFlight.id \in AttemptedIds)
SettledEffectsWereAttempted ==
  /\ settledIds \subseteq AttemptedIds
  /\ (inFlight = NoEffect \/ inFlight.id \notin settledIds)
SelectionEffectRequiresCompleteExport ==
  /\ (\A index \in 1..Len(effectQueue):
       effectQueue[index].kind = "selection" =>
         expectedChunks > 0 /\ nextChunk = expectedChunks /\ finalChunkSeen
         /\ finishSeen /\ utf8Validated)
  /\ (inFlight.kind = "selection" =>
       expectedChunks > 0 /\ nextChunk = expectedChunks /\ finalChunkSeen
       /\ finishSeen /\ utf8Validated)
SelectionCopyCommitsAtMostOnce == copyCommits <= 1
TerminalCopyNeverCommits ==
  copyPhase \in {"cancelled", "failed", "unknown"} => copyCommits = 0
EffectsUseOneSink ==
  /\ (\A index \in 1..Len(effectQueue): ValidSink(effectQueue[index]))
  /\ (inFlight = NoEffect \/ ValidSink(inFlight))
DeniedProgramWritesHaveNoEffect ==
  ~AllowProgramWrites =>
    /\ programCommits = 0
    /\ inFlight.kind # "program"
    /\ \A index \in 1..Len(effectQueue): effectQueue[index].kind # "program"
FrameAndTerminalWriterAreExclusive ==
  frameOpen => (inFlight = NoEffect \/ ~UsesTerminalOutput(inFlight))
DrainingClientHasNoUndispatchedEffects ==
  lifecycle = "draining" => Len(effectQueue) = 0 /\ ~frameOpen
RetiredClientHasNoEffects ==
  lifecycle = "retired" =>
    /\ Len(effectQueue) = 0
    /\ inFlight = NoEffect
    /\ ~frameOpen
CopyPhaseMatchesSelectionEffect ==
  /\ (copyPhase = "queued" =>
       \E index \in 1..Len(effectQueue): effectQueue[index].kind = "selection")
  /\ (copyPhase = "committing" => inFlight.kind = "selection")
  /\ (copyPhase = "done" => copyCommits = 1)

=============================================================================
