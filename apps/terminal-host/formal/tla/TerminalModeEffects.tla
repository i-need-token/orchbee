-------------------------- MODULE TerminalModeEffects --------------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-006, TH-007, TH-020, and TH-022. The model
\* keeps the four externally observable mode effects in one Host transaction
\* domain: mode-20 expansion is charged at preparation, mode-1004 enable reports
\* the current aggregate and stale prepared focus is rejected, mode-2026 blocks
\* projection only until reset, resize, or its deadline, and mode-2048 reports
\* the exact committed geometry. A required terminal-reply admission failure
\* fences the runtime after retaining the canonical mode or geometry commit.

CONSTANTS MaxInputBytes, MaxReplies, MaxTime, MaxGeometryRevision, MaxProjections

NoDeadline == MaxTime + 1

VARIABLES
  linefeedMode,
  inputPrepared,
  preparedSourceBytes,
  preparedCarriageReturns,
  preparedLinefeedMode,
  preparedCharge,
  clientInputBytes,
  focusMode,
  focused,
  focusPrepared,
  preparedFocusMode,
  preparedFocusTarget,
  syncMode,
  syncDeadline,
  time,
  sizeMode,
  geometryRevision,
  replyCount,
  lastFocusReply,
  lastFocusSnapshot,
  lastSizeReplyRevision,
  projectionCount,
  suppressedProjectionObserved,
  unsafeProjection,
  failureFenced

vars ==
  <<linefeedMode, inputPrepared, preparedSourceBytes,
    preparedCarriageReturns, preparedLinefeedMode, preparedCharge,
    clientInputBytes, focusMode, focused, focusPrepared, preparedFocusMode,
    preparedFocusTarget, syncMode, syncDeadline, time, sizeMode,
    geometryRevision, replyCount, lastFocusReply, lastFocusSnapshot,
    lastSizeReplyRevision, projectionCount, suppressedProjectionObserved,
    unsafeProjection, failureFenced>>

Init ==
  /\ linefeedMode = FALSE
  /\ inputPrepared = FALSE
  /\ preparedSourceBytes = 0
  /\ preparedCarriageReturns = 0
  /\ preparedLinefeedMode = FALSE
  /\ preparedCharge = 0
  /\ clientInputBytes = 0
  /\ focusMode = FALSE
  /\ focused = FALSE
  /\ focusPrepared = FALSE
  /\ preparedFocusMode = FALSE
  /\ preparedFocusTarget = FALSE
  /\ syncMode = FALSE
  /\ syncDeadline = NoDeadline
  /\ time = 0
  /\ sizeMode = FALSE
  /\ geometryRevision = 0
  /\ replyCount = 0
  /\ lastFocusReply = "none"
  /\ lastFocusSnapshot = FALSE
  /\ lastSizeReplyRevision = 0
  /\ projectionCount = 0
  /\ suppressedProjectionObserved = FALSE
  /\ unsafeProjection = FALSE
  /\ failureFenced = FALSE

SetLinefeedMode(enabled) ==
  /\ ~failureFenced
  /\ enabled \in BOOLEAN
  /\ enabled # linefeedMode
  /\ linefeedMode' = enabled
  /\ UNCHANGED <<inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

PrepareClientInput(sourceBytes, carriageReturns) ==
  /\ ~failureFenced
  /\ ~inputPrepared
  /\ sourceBytes \in 1..MaxInputBytes
  /\ carriageReturns \in 0..sourceBytes
  /\ LET charged == sourceBytes +
         (IF linefeedMode THEN carriageReturns ELSE 0)
     IN
       /\ charged <= MaxInputBytes - clientInputBytes
       /\ preparedCharge' = charged
  /\ inputPrepared' = TRUE
  /\ preparedSourceBytes' = sourceBytes
  /\ preparedCarriageReturns' = carriageReturns
  /\ preparedLinefeedMode' = linefeedMode
  /\ UNCHANGED <<linefeedMode, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

CommitPreparedInput ==
  /\ ~failureFenced
  /\ inputPrepared
  /\ clientInputBytes' = clientInputBytes + preparedCharge
  /\ inputPrepared' = FALSE
  /\ preparedSourceBytes' = 0
  /\ preparedCarriageReturns' = 0
  /\ preparedLinefeedMode' = FALSE
  /\ preparedCharge' = 0
  /\ UNCHANGED <<linefeedMode, focusMode, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

CancelPreparedInput ==
  /\ inputPrepared
  /\ inputPrepared' = FALSE
  /\ preparedSourceBytes' = 0
  /\ preparedCarriageReturns' = 0
  /\ preparedLinefeedMode' = FALSE
  /\ preparedCharge' = 0
  /\ UNCHANGED <<linefeedMode, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

DrainClientInput ==
  /\ clientInputBytes > 0
  /\ clientInputBytes' = 0
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, focusMode, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

PrepareFocus(target) ==
  /\ ~failureFenced
  /\ ~focusPrepared
  /\ target \in BOOLEAN
  /\ focusPrepared' = TRUE
  /\ preparedFocusMode' = focusMode
  /\ preparedFocusTarget' = target
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

CommitPreparedFocus ==
  /\ ~failureFenced
  /\ focusPrepared
  /\ preparedFocusMode = focusMode
  /\ focused' = preparedFocusTarget
  /\ focusPrepared' = FALSE
  /\ preparedFocusMode' = FALSE
  /\ preparedFocusTarget' = FALSE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

RejectStalePreparedFocus ==
  /\ focusPrepared
  /\ preparedFocusMode # focusMode
  /\ focusPrepared' = FALSE
  /\ preparedFocusMode' = FALSE
  /\ preparedFocusTarget' = FALSE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

EnableFocusReporting ==
  /\ ~failureFenced
  /\ ~focusMode
  /\ replyCount < MaxReplies
  /\ focusMode' = TRUE
  /\ replyCount' = replyCount + 1
  /\ lastFocusReply' = IF focused THEN "in" ELSE "out"
  /\ lastFocusSnapshot' = focused
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

FailFocusEnableReply ==
  /\ ~failureFenced
  /\ ~focusMode
  /\ replyCount = MaxReplies
  /\ focusMode' = TRUE
  /\ failureFenced' = TRUE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection>>

DisableFocusReporting ==
  /\ ~failureFenced
  /\ focusMode
  /\ focusMode' = FALSE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

EnableSynchronizedOutput ==
  /\ ~failureFenced
  /\ time < MaxTime
  /\ syncMode' = TRUE
  /\ syncDeadline' = time + 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

DisableSynchronizedOutput ==
  /\ ~failureFenced
  /\ syncMode
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

AdvanceTime ==
  /\ time < MaxTime
  /\ time' = time + 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

SuppressProjection ==
  /\ ~failureFenced
  /\ syncMode
  /\ time < syncDeadline
  /\ suppressedProjectionObserved' = TRUE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

ExpireSynchronizedOutput ==
  /\ ~failureFenced
  /\ syncMode
  /\ time >= syncDeadline
  /\ projectionCount < MaxProjections
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ projectionCount' = projectionCount + 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

CaptureProjection ==
  /\ ~failureFenced
  /\ ~syncMode
  /\ projectionCount < MaxProjections
  /\ projectionCount' = projectionCount + 1
  /\ unsafeProjection' = unsafeProjection \/ syncMode
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, suppressedProjectionObserved,
                  failureFenced>>

EnableInBandSizeReports ==
  /\ ~failureFenced
  /\ ~sizeMode
  /\ replyCount < MaxReplies
  /\ sizeMode' = TRUE
  /\ replyCount' = replyCount + 1
  /\ lastSizeReplyRevision' = geometryRevision
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, geometryRevision,
                  lastFocusReply, lastFocusSnapshot, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

FailInBandSizeEnableReply ==
  /\ ~failureFenced
  /\ ~sizeMode
  /\ replyCount = MaxReplies
  /\ sizeMode' = TRUE
  /\ failureFenced' = TRUE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection>>

DisableInBandSizeReports ==
  /\ ~failureFenced
  /\ sizeMode
  /\ sizeMode' = FALSE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

ResizeGeometry ==
  /\ ~failureFenced
  /\ geometryRevision < MaxGeometryRevision
  /\ ~sizeMode \/ replyCount < MaxReplies
  /\ geometryRevision' = geometryRevision + 1
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ replyCount' = IF sizeMode THEN replyCount + 1 ELSE replyCount
  /\ lastSizeReplyRevision' =
       IF sizeMode THEN geometryRevision + 1 ELSE lastSizeReplyRevision
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, lastFocusReply, lastFocusSnapshot,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

FailResizeSizeReply ==
  /\ ~failureFenced
  /\ sizeMode
  /\ geometryRevision < MaxGeometryRevision
  /\ replyCount = MaxReplies
  /\ geometryRevision' = geometryRevision + 1
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ failureFenced' = TRUE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection>>

ResetModes ==
  /\ ~failureFenced
  /\ linefeedMode \/ focusMode \/ syncMode \/ sizeMode
  /\ linefeedMode' = FALSE
  /\ focusMode' = FALSE
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ sizeMode' = FALSE
  /\ UNCHANGED <<inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, time,
                  geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision, projectionCount,
                  suppressedProjectionObserved, unsafeProjection,
                  failureFenced>>

DrainReply ==
  /\ replyCount > 0
  /\ replyCount' = replyCount - 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount, suppressedProjectionObserved,
                  unsafeProjection, failureFenced>>

Next ==
  \/ \E enabled \in BOOLEAN: SetLinefeedMode(enabled)
  \/ \E sourceBytes \in 1..MaxInputBytes:
       \E carriageReturns \in 0..sourceBytes:
         PrepareClientInput(sourceBytes, carriageReturns)
  \/ CommitPreparedInput
  \/ CancelPreparedInput
  \/ DrainClientInput
  \/ \E target \in BOOLEAN: PrepareFocus(target)
  \/ CommitPreparedFocus
  \/ RejectStalePreparedFocus
  \/ EnableFocusReporting
  \/ FailFocusEnableReply
  \/ DisableFocusReporting
  \/ EnableSynchronizedOutput
  \/ DisableSynchronizedOutput
  \/ AdvanceTime
  \/ SuppressProjection
  \/ ExpireSynchronizedOutput
  \/ CaptureProjection
  \/ EnableInBandSizeReports
  \/ FailInBandSizeEnableReply
  \/ DisableInBandSizeReports
  \/ ResizeGeometry
  \/ FailResizeSizeReply
  \/ ResetModes
  \/ DrainReply

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ linefeedMode \in BOOLEAN
  /\ inputPrepared \in BOOLEAN
  /\ preparedSourceBytes \in 0..MaxInputBytes
  /\ preparedCarriageReturns \in 0..MaxInputBytes
  /\ preparedLinefeedMode \in BOOLEAN
  /\ preparedCharge \in 0..MaxInputBytes
  /\ clientInputBytes \in 0..MaxInputBytes
  /\ focusMode \in BOOLEAN
  /\ focused \in BOOLEAN
  /\ focusPrepared \in BOOLEAN
  /\ preparedFocusMode \in BOOLEAN
  /\ preparedFocusTarget \in BOOLEAN
  /\ syncMode \in BOOLEAN
  /\ syncDeadline \in 0..NoDeadline
  /\ time \in 0..MaxTime
  /\ sizeMode \in BOOLEAN
  /\ geometryRevision \in 0..MaxGeometryRevision
  /\ replyCount \in 0..MaxReplies
  /\ lastFocusReply \in {"none", "in", "out"}
  /\ lastFocusSnapshot \in BOOLEAN
  /\ lastSizeReplyRevision \in 0..MaxGeometryRevision
  /\ projectionCount \in 0..MaxProjections
  /\ suppressedProjectionObserved \in BOOLEAN
  /\ unsafeProjection \in BOOLEAN
  /\ failureFenced \in BOOLEAN

PreparedInputIsChargedAfterExpansion ==
  inputPrepared =>
    preparedCharge = preparedSourceBytes +
      (IF preparedLinefeedMode THEN preparedCarriageReturns ELSE 0)

PreparedInputFitsClientBudget ==
  inputPrepared => preparedCharge <= MaxInputBytes - clientInputBytes

NoPreparedInputHasResidualFields ==
  ~inputPrepared =>
    preparedSourceBytes = 0 /\ preparedCarriageReturns = 0 /\
      preparedCharge = 0

FocusEnableReplyMatchesAggregateSnapshot ==
  /\ (lastFocusReply = "in" => lastFocusSnapshot)
  /\ (lastFocusReply = "out" => ~lastFocusSnapshot)

SynchronizedOutputOwnsOneDeadline ==
  syncMode <=> syncDeadline # NoDeadline

ProjectionNeverCapturesWhileSynchronized ==
  ~unsafeProjection

SizeReplyNeverLeadsGeometry ==
  lastSizeReplyRevision <= geometryRevision

RequiredReplyFailureIsContained ==
  failureFenced => (focusMode \/ sizeMode)

FailureFenceIsPermanent ==
  [] [failureFenced => failureFenced']_vars

=============================================================================
