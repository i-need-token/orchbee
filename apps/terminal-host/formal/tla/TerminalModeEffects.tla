-------------------------- MODULE TerminalModeEffects --------------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-006, TH-007, TH-020, and TH-022. The model
\* keeps the five externally observable mode effects in one Host transaction
\* domain: mode-20 expansion is charged at preparation, mode-1004 enable reports
\* the current aggregate and stale prepared focus is rejected, mode-2026 blocks
\* projection only until reset, resize, or its deadline, and mode-2048 reports
\* the exact committed geometry. Mode 40 restores the latest successful
\* resize-owner viewport without letting mode-3 DECCOLM overwrite that target.
\* A required terminal-reply admission failure fences the runtime after
\* retaining the canonical mode or geometry commit.
\* MaxProjections bounds one finite projection transaction. The maintained
\* configuration uses one projection and therefore does not claim repeated
\* synchronized-output re-entry; repeated cycles remain deterministic-test
\* evidence.

CONSTANTS Scenario, MaxInputBytes, MaxReplies, MaxTime, MaxGeometryRevision, MaxProjections

NoDeadline == MaxTime + 1
Scenarios == {"input-focus", "geometry"}
GeometryValues == {"a", "b"}
GeometryActions == {"none", "viewport", "deccolm", "mode40"}

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
  mode40,
  viewportGeometry,
  canonicalGeometry,
  lastGeometryAction,
  geometryRevision,
  replyCount,
  lastFocusReply,
  lastFocusSnapshot,
  lastSizeReplyRevision,
  projectionCount,
  unsafeProjection,
  failureFenced

vars ==
  <<linefeedMode, inputPrepared, preparedSourceBytes,
    preparedCarriageReturns, preparedLinefeedMode, preparedCharge,
    clientInputBytes, focusMode, focused, focusPrepared, preparedFocusMode,
    preparedFocusTarget, syncMode, syncDeadline, time, sizeMode,
    mode40, viewportGeometry, canonicalGeometry, lastGeometryAction,
    geometryRevision, replyCount, lastFocusReply, lastFocusSnapshot,
    lastSizeReplyRevision, projectionCount,
    unsafeProjection, failureFenced>>

UnchangedGeometry ==
  UNCHANGED <<mode40, viewportGeometry, canonicalGeometry,
              lastGeometryAction>>

Init ==
  /\ Scenario \in Scenarios
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
  /\ mode40 = FALSE
  \* Either value represents the launch geometry or a viewport already
  \* replaced by the latest successful resize-owner request before this trace.
  /\ IF Scenario = "geometry"
       THEN viewportGeometry \in GeometryValues
       ELSE viewportGeometry = "a"
  /\ canonicalGeometry = viewportGeometry
  /\ lastGeometryAction = "none"
  /\ geometryRevision = 0
  /\ replyCount = 0
  /\ lastFocusReply = "none"
  /\ lastFocusSnapshot = FALSE
  /\ lastSizeReplyRevision = 0
  /\ projectionCount = 0
  /\ unsafeProjection = FALSE
  /\ failureFenced = FALSE

SetLinefeedMode(enabled) ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

PrepareClientInput(sourceBytes, carriageReturns) ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

CommitPreparedInput ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  projectionCount,
                  unsafeProjection, failureFenced>>

CancelPreparedInput ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

DrainClientInput ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
  /\ clientInputBytes > 0
  /\ clientInputBytes' = 0
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, focusMode, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount,
                  unsafeProjection, failureFenced>>

PrepareFocus(target) ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

CommitPreparedFocus ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  projectionCount,
                  unsafeProjection, failureFenced>>

RejectStalePreparedFocus ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

EnableFocusReporting ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

FailFocusEnableReply ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
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
                  projectionCount,
                  unsafeProjection>>

DisableFocusReporting ==
  /\ Scenario = "input-focus"
  /\ UnchangedGeometry
  /\ ~failureFenced
  /\ focusMode
  /\ focusMode' = FALSE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, syncMode,
                  syncDeadline, time, sizeMode, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount,
                  unsafeProjection, failureFenced>>

EnableSynchronizedOutput ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
  /\ ~failureFenced
  /\ time < MaxTime
  /\ syncMode' = TRUE
  /\ syncDeadline' = time + 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

DisableSynchronizedOutput ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
  /\ ~failureFenced
  /\ syncMode
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

AdvanceTime ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
  /\ time < MaxTime
  /\ time' = time + 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

SuppressProjection ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
  /\ ~failureFenced
  /\ syncMode
  /\ time < syncDeadline
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

ExpireSynchronizedOutput ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
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
                  lastFocusSnapshot, lastSizeReplyRevision, unsafeProjection,
                  failureFenced>>

CaptureProjection ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
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
                  lastSizeReplyRevision,
                  failureFenced>>

EnableInBandSizeReports ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
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
                  lastFocusReply, lastFocusSnapshot, projectionCount, unsafeProjection,
                  failureFenced>>

FailInBandSizeEnableReply ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
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
                  projectionCount,
                  unsafeProjection>>

DisableInBandSizeReports ==
  /\ Scenario = "geometry"
  /\ UnchangedGeometry
  /\ ~failureFenced
  /\ sizeMode
  /\ sizeMode' = FALSE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount,
                  unsafeProjection, failureFenced>>

CommitGeometry(target, nextViewport, nextMode40, source) ==
  /\ target \in GeometryValues
  /\ nextViewport \in GeometryValues
  /\ nextMode40 \in BOOLEAN
  /\ source \in GeometryActions \ {"none"}
  /\ geometryRevision < MaxGeometryRevision
  /\ ~sizeMode \/ replyCount < MaxReplies
  /\ canonicalGeometry' = target
  /\ viewportGeometry' = nextViewport
  /\ mode40' = nextMode40
  /\ lastGeometryAction' = source
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
                  projectionCount,
                  unsafeProjection, failureFenced>>

FailGeometryReply(target, nextViewport, nextMode40, source) ==
  /\ target \in GeometryValues
  /\ nextViewport \in GeometryValues
  /\ nextMode40 \in BOOLEAN
  /\ source \in GeometryActions \ {"none"}
  /\ sizeMode
  /\ geometryRevision < MaxGeometryRevision
  /\ replyCount = MaxReplies
  /\ canonicalGeometry' = target
  /\ viewportGeometry' = nextViewport
  /\ mode40' = nextMode40
  /\ lastGeometryAction' = source
  /\ geometryRevision' = geometryRevision + 1
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ failureFenced' = TRUE
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget, time,
                  sizeMode, replyCount, lastFocusReply, lastFocusSnapshot,
                  lastSizeReplyRevision, projectionCount, unsafeProjection>>

ResizeGeometry(target) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ target \in GeometryValues
  /\ target # canonicalGeometry
  /\ CommitGeometry(target, target, mode40, "viewport")

FailResizeSizeReply(target) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ target \in GeometryValues
  /\ target # canonicalGeometry
  /\ FailGeometryReply(target, target, mode40, "viewport")

RefreshViewportGeometry(target) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ target \in GeometryValues
  /\ target = canonicalGeometry
  /\ target # viewportGeometry
  /\ viewportGeometry' = target
  /\ lastGeometryAction' = "viewport"
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, mode40,
                  canonicalGeometry, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount,
                  unsafeProjection, failureFenced>>

SetColumnModeGeometry(target) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ mode40
  /\ target \in GeometryValues
  /\ target # canonicalGeometry
  /\ CommitGeometry(target, viewportGeometry, mode40, "deccolm")

FailColumnModeSizeReply(target) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ mode40
  /\ target \in GeometryValues
  /\ target # canonicalGeometry
  /\ FailGeometryReply(target, viewportGeometry, mode40, "deccolm")

SetMode40AtViewport(enabled) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ enabled \in BOOLEAN
  /\ canonicalGeometry = viewportGeometry
  /\ mode40' = enabled
  /\ lastGeometryAction' = "mode40"
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, viewportGeometry,
                  canonicalGeometry, geometryRevision, replyCount,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount,
                  unsafeProjection, failureFenced>>

RestoreViewportOnMode40(enabled) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ enabled \in BOOLEAN
  /\ canonicalGeometry # viewportGeometry
  /\ CommitGeometry(viewportGeometry, viewportGeometry, enabled, "mode40")

FailMode40SizeReply(enabled) ==
  /\ Scenario = "geometry"
  /\ ~failureFenced
  /\ enabled \in BOOLEAN
  /\ canonicalGeometry # viewportGeometry
  /\ FailGeometryReply(viewportGeometry, viewportGeometry, enabled, "mode40")

ResetModes ==
  /\ ~failureFenced
  /\ linefeedMode \/ focusMode \/ syncMode \/ sizeMode \/ mode40
  /\ linefeedMode' = FALSE
  /\ focusMode' = FALSE
  /\ syncMode' = FALSE
  /\ syncDeadline' = NoDeadline
  /\ sizeMode' = FALSE
  /\ mode40' = FALSE
  /\ UNCHANGED <<inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focused, focusPrepared,
                  preparedFocusMode, preparedFocusTarget, time,
                  viewportGeometry, canonicalGeometry, lastGeometryAction,
                  geometryRevision, replyCount, lastFocusReply,
                  lastFocusSnapshot, lastSizeReplyRevision, projectionCount, unsafeProjection,
                  failureFenced>>

DrainReply ==
  /\ UnchangedGeometry
  /\ replyCount > 0
  /\ replyCount' = replyCount - 1
  /\ UNCHANGED <<linefeedMode, inputPrepared, preparedSourceBytes,
                  preparedCarriageReturns, preparedLinefeedMode,
                  preparedCharge, clientInputBytes, focusMode, focused,
                  focusPrepared, preparedFocusMode, preparedFocusTarget,
                  syncMode, syncDeadline, time, sizeMode, geometryRevision,
                  lastFocusReply, lastFocusSnapshot, lastSizeReplyRevision,
                  projectionCount,
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
  \/ DrainReply
  \/ \E target \in GeometryValues: ResizeGeometry(target)
  \/ \E target \in GeometryValues: FailResizeSizeReply(target)
  \/ \E target \in GeometryValues: RefreshViewportGeometry(target)
  \/ \E target \in GeometryValues: SetColumnModeGeometry(target)
  \/ \E target \in GeometryValues: FailColumnModeSizeReply(target)
  \/ \E enabled \in BOOLEAN: SetMode40AtViewport(enabled)
  \/ \E enabled \in BOOLEAN: RestoreViewportOnMode40(enabled)
  \/ \E enabled \in BOOLEAN: FailMode40SizeReply(enabled)
  \/ ResetModes

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
  /\ mode40 \in BOOLEAN
  /\ viewportGeometry \in GeometryValues
  /\ canonicalGeometry \in GeometryValues
  /\ lastGeometryAction \in GeometryActions
  /\ geometryRevision \in 0..MaxGeometryRevision
  /\ replyCount \in 0..MaxReplies
  /\ lastFocusReply \in {"none", "in", "out"}
  /\ lastFocusSnapshot \in BOOLEAN
  /\ lastSizeReplyRevision \in 0..MaxGeometryRevision
  /\ projectionCount \in 0..MaxProjections
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

Mode40RestoreUsesViewportGeometry ==
  lastGeometryAction = "mode40" => canonicalGeometry = viewportGeometry

RequiredReplyFailureIsContained ==
  failureFenced => (focusMode \/ sizeMode)

FailureFenceIsPermanent ==
  [] [failureFenced => failureFenced']_vars

=============================================================================
