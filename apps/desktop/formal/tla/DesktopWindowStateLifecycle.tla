--------------------- MODULE DesktopWindowStateLifecycle ---------------------
EXTENDS Naturals, Sequences, TLC

CONSTANT MaxCaptures

DurableModes == {"normal", "maximized"}
WindowModes == DurableModes \union {"full-screen"}
NoFlushCaptureCount == MaxCaptures + 1

VARIABLES
  mode,
  captures,
  processedCount,
  durableOrdinal,
  flushRequested,
  flushCaptureCount,
  flushed,
  disposed,
  writable

vars == <<
  mode,
  captures,
  processedCount,
  durableOrdinal,
  flushRequested,
  flushCaptureCount,
  flushed,
  disposed,
  writable
>>

Init ==
  /\ mode \in WindowModes
  /\ captures = <<>>
  /\ processedCount = 0
  /\ durableOrdinal = 0
  /\ flushRequested = FALSE
  /\ flushCaptureCount = NoFlushCaptureCount
  /\ flushed = FALSE
  /\ disposed = FALSE
  /\ writable \in BOOLEAN

SetWindowMode(nextMode) ==
  /\ nextMode \in WindowModes
  /\ ~flushRequested
  /\ ~disposed
  /\ mode' = nextMode
  /\ UNCHANGED <<captures, processedCount, durableOrdinal,
       flushRequested, flushCaptureCount, flushed, disposed, writable>>

Capture ==
  /\ mode \in DurableModes
  /\ Len(captures) < MaxCaptures
  /\ ~flushRequested
  /\ ~disposed
  /\ captures' = Append(captures, mode)
  /\ UNCHANGED <<mode, processedCount, durableOrdinal,
       flushRequested, flushCaptureCount, flushed, disposed, writable>>

ProcessNext ==
  /\ processedCount < Len(captures)
  /\ ~disposed
  /\ processedCount' = processedCount + 1
  /\ durableOrdinal' = IF writable THEN processedCount' ELSE durableOrdinal
  /\ UNCHANGED <<mode, captures, flushRequested, flushCaptureCount,
       flushed, disposed, writable>>

RequestFlush ==
  /\ ~flushRequested
  /\ ~disposed
  /\ flushRequested' = TRUE
  /\ flushCaptureCount' = Len(captures)
  /\ UNCHANGED <<mode, captures, processedCount, durableOrdinal,
       flushed, disposed, writable>>

CompleteFlush ==
  /\ flushRequested
  /\ ~flushed
  /\ processedCount = Len(captures)
  /\ flushed' = TRUE
  /\ UNCHANGED <<mode, captures, processedCount, durableOrdinal,
       flushRequested, flushCaptureCount, disposed, writable>>

Dispose ==
  /\ flushed
  /\ ~disposed
  /\ disposed' = TRUE
  /\ UNCHANGED <<mode, captures, processedCount, durableOrdinal,
       flushRequested, flushCaptureCount, flushed, writable>>

Next ==
  \/ \E nextMode \in WindowModes: SetWindowMode(nextMode)
  \/ Capture
  \/ ProcessNext
  \/ RequestFlush
  \/ CompleteFlush
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(ProcessNext)
  /\ WF_vars(CompleteFlush)
  /\ WF_vars(Dispose)

TypeOK ==
  /\ mode \in WindowModes
  /\ captures \in Seq(DurableModes)
  /\ processedCount \in 0..MaxCaptures
  /\ durableOrdinal \in 0..MaxCaptures
  /\ flushRequested \in BOOLEAN
  /\ flushCaptureCount \in (0..MaxCaptures) \union {NoFlushCaptureCount}
  /\ flushed \in BOOLEAN
  /\ disposed \in BOOLEAN
  /\ writable \in BOOLEAN

CaptureBound == Len(captures) <= MaxCaptures

WritesProcessInOrder ==
  /\ processedCount <= Len(captures)
  /\ durableOrdinal <= processedCount

FullScreenIsTransient ==
  \A ordinal \in 1..Len(captures): captures[ordinal] \in DurableModes

WritablePersistsLatest ==
  writable /\ processedCount > 0 => durableOrdinal = processedCount

UnavailableStoreNeverAdvances ==
  ~writable => durableOrdinal = 0

FlushFreezesCaptures ==
  flushRequested => Len(captures) = flushCaptureCount

FlushCompletesQueue ==
  flushed => processedCount = Len(captures)

DisposeFollowsFlush == disposed => flushed

FlushEventuallyDisposes == flushRequested ~> disposed

=============================================================================
