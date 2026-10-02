-------------------- MODULE WindowsWorkbenchAutosave --------------------
EXTENDS Naturals, TLC

VARIABLES layout, directory, physical, cachedPhysical, cachedDirectory,
          dirty, debounceReady, periodicDue, phase, kind, captured,
          capturedSource, epoch, stopRequested, quiescent, finalCaptured,
          finalEpoch, stopped, failedStop, stored, storedKind, storedEpoch,
          suppressedInitial, lastEvent
vars == <<layout, directory, physical, cachedPhysical, cachedDirectory,
          dirty, debounceReady, periodicDue, phase, kind, captured,
          capturedSource, epoch, stopRequested, quiescent, finalCaptured,
          finalEpoch, stopped, failedStop, stored, storedKind, storedEpoch,
          suppressedInitial, lastEvent>>
Cwds == {"Unknown", "d0", "d1"}
Empty == [layout |-> 0, cwd |-> "Unknown", physical |-> 1]
Cached == IF cachedPhysical = physical THEN cachedDirectory ELSE "Unknown"
InitialPayload(value) == value.layout = 0 /\ value.cwd = "Unknown"

Init ==
  /\ layout = 0 /\ directory = "d0" /\ physical = 1
  /\ cachedPhysical = 0 /\ cachedDirectory = "Unknown"
  /\ dirty = TRUE /\ debounceReady = FALSE /\ periodicDue = FALSE
  /\ phase = "Idle" /\ kind = "None" /\ captured = Empty /\ capturedSource = 0
  /\ epoch = 1 /\ stopRequested = FALSE /\ quiescent = FALSE
  /\ finalCaptured = FALSE /\ finalEpoch = 0 /\ stopped = FALSE /\ failedStop = FALSE
  /\ stored = Empty /\ storedKind = "None" /\ storedEpoch = 0
  /\ suppressedInitial \in BOOLEAN /\ lastEvent = "Init"

ChangeLayout ==
  /\ ~stopped /\ ~quiescent /\ layout < 2
  /\ layout' = layout + 1 /\ dirty' = TRUE /\ debounceReady' = FALSE
  /\ lastEvent' = "ChangeLayout"
  /\ UNCHANGED <<directory, physical, cachedPhysical, cachedDirectory, periodicDue,
                  phase, kind, captured, capturedSource, epoch, stopRequested,
                  quiescent, finalCaptured, finalEpoch, stopped, failedStop,
                  stored, storedKind, storedEpoch, suppressedInitial>>

ChangeDirectory ==
  /\ ~stopped /\ directory = "d0" /\ directory' = "d1"
  /\ lastEvent' = "ChangeDirectory"
  /\ UNCHANGED <<layout, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, phase, kind, captured, capturedSource,
                  epoch, stopRequested, quiescent, finalCaptured, finalEpoch,
                  stopped, failedStop, stored, storedKind, storedEpoch,
                  suppressedInitial>>

ObserveDirectory ==
  /\ ~stopped /\ ~quiescent
  /\ cachedPhysical' = physical /\ cachedDirectory' = directory
  /\ lastEvent' = "ObserveDirectory"
  /\ UNCHANGED <<layout, directory, physical, dirty, debounceReady, periodicDue,
                  phase, kind, captured, capturedSource, epoch, stopRequested,
                  quiescent, finalCaptured, finalEpoch, stopped, failedStop,
                  stored, storedKind, storedEpoch, suppressedInitial>>

ReplacePhysical ==
  /\ ~stopped /\ ~quiescent /\ physical = 1 /\ physical' = 2
  /\ lastEvent' = "ReplacePhysical"
  /\ UNCHANGED <<layout, directory, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, phase, kind, captured, capturedSource,
                  epoch, stopRequested, quiescent, finalCaptured, finalEpoch,
                  stopped, failedStop, stored, storedKind, storedEpoch,
                  suppressedInitial>>

DebounceExpires ==
  /\ ~stopped /\ ~stopRequested /\ dirty /\ ~debounceReady
  /\ debounceReady' = TRUE /\ lastEvent' = "DebounceExpires"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  periodicDue, phase, kind, captured, capturedSource, epoch,
                  stopRequested, quiescent, finalCaptured, finalEpoch, stopped,
                  failedStop, stored, storedKind, storedEpoch, suppressedInitial>>

PeriodicExpires ==
  /\ ~stopped /\ ~stopRequested /\ ~periodicDue
  /\ periodicDue' = TRUE /\ lastEvent' = "PeriodicExpires"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, phase, kind, captured, capturedSource, epoch,
                  stopRequested, quiescent, finalCaptured, finalEpoch, stopped,
                  failedStop, stored, storedKind, storedEpoch, suppressedInitial>>

CaptureBackground ==
  /\ ~stopRequested /\ ~stopped /\ phase = "Idle"
  /\ ((dirty /\ debounceReady) \/ periodicDue)
  /\ captured' = [layout |-> layout, cwd |-> Cached, physical |-> physical]
  /\ capturedSource' = cachedPhysical /\ kind' = "Background" /\ phase' = "Captured"
  /\ dirty' = FALSE /\ debounceReady' = FALSE /\ periodicDue' = FALSE
  /\ lastEvent' = "CaptureBackground"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory,
                  epoch, stopRequested, quiescent, finalCaptured, finalEpoch,
                  stopped, failedStop, stored, storedKind, storedEpoch,
                  suppressedInitial>>

SuppressUnchanged ==
  /\ phase = "Captured" /\ kind = "Background"
  /\ suppressedInitial /\ InitialPayload(captured)
  /\ phase' = "Idle" /\ kind' = "None" /\ lastEvent' = "SuppressUnchanged"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, captured, capturedSource, epoch,
                  stopRequested, quiescent, finalCaptured, finalEpoch, stopped,
                  failedStop, stored, storedKind, storedEpoch, suppressedInitial>>

BeginWrite ==
  /\ phase = "Captured"
  \* Empty-baseline suppression is background-only; explicit final capture always passes this guard.
  /\ ~(kind = "Background" /\ suppressedInitial /\ InitialPayload(captured))
  /\ phase' = "Writing" /\ lastEvent' = "BeginWrite"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, kind, captured, capturedSource, epoch,
                  stopRequested, quiescent, finalCaptured, finalEpoch, stopped,
                  failedStop, stored, storedKind, storedEpoch, suppressedInitial>>

FinishWrite ==
  /\ phase = "Writing"
  /\ stored' = captured /\ storedKind' = kind /\ storedEpoch' = epoch
  /\ phase' = "Idle" /\ kind' = "None"
  /\ suppressedInitial' = FALSE
  /\ lastEvent' = IF kind = "Background" THEN "BackgroundCommitted" ELSE "FinalCommitted"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, captured, capturedSource, epoch,
                  stopRequested, quiescent, finalCaptured, finalEpoch, stopped, failedStop>>

WriteFailure ==
  /\ phase \in {"Captured", "Writing"}
  /\ failedStop' = (failedStop \/ kind = "Final")
  /\ phase' = "Idle" /\ kind' = "None" /\ lastEvent' = "WriteFailure"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, captured, capturedSource, epoch,
                  stopRequested, quiescent, finalCaptured, finalEpoch, stopped,
                  stored, storedKind, storedEpoch, suppressedInitial>>

RequestStop ==
  /\ ~stopRequested /\ ~stopped
  /\ stopRequested' = TRUE /\ lastEvent' = "RequestStop"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, phase, kind, captured, capturedSource,
                  epoch, quiescent, finalCaptured, finalEpoch, stopped, failedStop,
                  stored, storedKind, storedEpoch, suppressedInitial>>

SettleRequests ==
  /\ stopRequested /\ ~quiescent /\ quiescent' = TRUE
  /\ lastEvent' = "SettleRequests"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, phase, kind, captured, capturedSource,
                  epoch, stopRequested, finalCaptured, finalEpoch, stopped, failedStop,
                  stored, storedKind, storedEpoch, suppressedInitial>>

CaptureFinal ==
  /\ stopRequested /\ quiescent /\ phase = "Idle" /\ ~finalCaptured
  /\ captured' = [layout |-> layout, cwd |-> directory, physical |-> physical]
  /\ capturedSource' = physical /\ kind' = "Final" /\ phase' = "Captured"
  /\ finalCaptured' = TRUE /\ finalEpoch' = epoch /\ lastEvent' = "CaptureFinal"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, epoch, stopRequested, quiescent,
                  stopped, failedStop, stored, storedKind, storedEpoch,
                  suppressedInitial>>

CompleteStop ==
  /\ stopRequested /\ phase = "Idle" /\ ~failedStop /\ ~stopped
  /\ finalCaptured /\ storedKind = "Final" /\ storedEpoch = epoch
  /\ stopped' = TRUE /\ lastEvent' = "CompleteStop"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, phase, kind, captured, capturedSource,
                  epoch, stopRequested, quiescent, finalCaptured, finalEpoch, failedStop,
                  stored, storedKind, storedEpoch, suppressedInitial>>

RetryFailedStop ==
  /\ failedStop /\ phase = "Idle" /\ epoch = 1 /\ ~stopped
  /\ epoch' = 2
  /\ finalCaptured' = FALSE /\ failedStop' = FALSE
  /\ lastEvent' = "RetryFailedStop"
  /\ UNCHANGED <<layout, directory, physical, cachedPhysical, cachedDirectory, dirty,
                  debounceReady, periodicDue, phase, kind, captured, capturedSource,
                  stopRequested, quiescent, finalEpoch, stopped, stored, storedKind,
                  storedEpoch, suppressedInitial>>

Next == ChangeLayout \/ ChangeDirectory \/ ObserveDirectory \/ ReplacePhysical
        \/ DebounceExpires \/ PeriodicExpires \/ CaptureBackground \/ SuppressUnchanged \/ BeginWrite
        \/ FinishWrite \/ WriteFailure \/ RequestStop \/ SettleRequests
        \/ CaptureFinal \/ CompleteStop \/ RetryFailedStop
SettleWrite == SuppressUnchanged \/ BeginWrite \/ FinishWrite \/ WriteFailure
Spec == Init /\ [][Next]_vars /\ WF_vars(SettleWrite)

TypeOK ==
  /\ layout \in 0..2 /\ directory \in {"d0", "d1"} /\ physical \in 1..2
  /\ cachedPhysical \in 0..2 /\ cachedDirectory \in Cwds
  /\ phase \in {"Idle", "Captured", "Writing"} /\ kind \in {"None", "Background", "Final"}
  /\ captured \in [layout : 0..2, cwd : Cwds, physical : 1..2]
  /\ stored \in [layout : 0..2, cwd : Cwds, physical : 1..2]
  /\ capturedSource \in 0..2 /\ epoch \in 1..2 /\ finalEpoch \in 0..2 /\ storedEpoch \in 0..2
  /\ storedKind \in {"None", "Background", "Final"}
  /\ <<dirty, debounceReady, periodicDue, stopRequested, quiescent,
         finalCaptured, stopped, failedStop, suppressedInitial>> \in [1..9 -> BOOLEAN]

DirectoryFence == captured.cwd # "Unknown" => captured.physical = capturedSource
OneWriter == (phase = "Idle") <=> (kind = "None")
FinalAfterDrain == finalCaptured => (phase = "Idle" \/ kind = "Final")
NoLateBackground == lastEvent = "BackgroundCommitted" => storedEpoch > finalEpoch
StoppedHasFinal == stopped => (storedKind = "Final" /\ storedEpoch = epoch /\ phase = "Idle")
SuppressedInitialEmpty ==
  suppressedInitial => ~(storedKind = "Background" /\ InitialPayload(stored))
SuppressedBackgroundCannotWrite ==
  (phase = "Captured" /\ kind = "Background" /\ suppressedInitial /\ InitialPayload(captured))
    => ~ENABLED BeginWrite
FinalCanWrite == (phase = "Captured" /\ kind = "Final") => ENABLED BeginWrite
WriteSettles == (phase # "Idle") ~> (phase = "Idle")
==========================================================================
