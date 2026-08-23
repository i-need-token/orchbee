------------------------- MODULE DesktopTerminalArchive -------------------------
EXTENDS Naturals, TLC

MaxRows == 3
MaxBytes == 3

HostStates == {"running", "exited", "disposed"}
Intents == {"retain", "stop", "stopped", "close"}
PresentationStates == {"live", "stopping", "stopped", "fresh", "closed"}
CaptureStates == {"none", "capturing", "stored", "failed", "removed"}
SurfaceStates == {"live", "archive", "empty", "none"}

VARIABLES host, intent, presentation, capture, archivedRows, archivedBytes,
  surface, inputEnabled, mergedHistory, lastAction

vars == <<host, intent, presentation, capture, archivedRows, archivedBytes,
  surface, inputEnabled, mergedHistory, lastAction>>

Init ==
  /\ host = "running"
  /\ intent = "retain"
  /\ presentation = "live"
  /\ capture = "none"
  /\ archivedRows = 0
  /\ archivedBytes = 0
  /\ surface = "live"
  /\ inputEnabled = TRUE
  /\ mergedHistory = FALSE
  /\ lastAction = "init"

RequestStop ==
  /\ presentation = "live"
  /\ intent = "retain"
  /\ intent' = "stop"
  /\ presentation' = "stopping"
  /\ surface' = "none"
  /\ inputEnabled' = FALSE
  /\ lastAction' = "request-stop"
  /\ UNCHANGED <<host, capture, archivedRows, archivedBytes, mergedHistory>>

RequestClose ==
  /\ presentation = "live"
  /\ intent = "retain"
  /\ intent' = "close"
  /\ presentation' = "stopping"
  /\ surface' = "none"
  /\ inputEnabled' = FALSE
  /\ lastAction' = "request-close"
  /\ UNCHANGED <<host, capture, archivedRows, archivedBytes, mergedHistory>>

HostExit ==
  /\ presentation = "stopping"
  /\ host = "running"
  /\ host' = "exited"
  /\ lastAction' = "host-exit"
  /\ UNCHANGED <<intent, presentation, capture, archivedRows, archivedBytes,
       surface, inputEnabled, mergedHistory>>

BeginCapture ==
  /\ intent = "stop"
  /\ host = "exited"
  /\ capture = "none"
  /\ capture' = "capturing"
  /\ lastAction' = "begin-capture"
  /\ UNCHANGED <<host, intent, presentation, archivedRows, archivedBytes,
       surface, inputEnabled, mergedHistory>>

CaptureSucceeds(rows, bytes) ==
  /\ capture = "capturing"
  /\ rows \in 1..MaxRows
  /\ bytes \in 1..MaxBytes
  /\ capture' = "stored"
  /\ archivedRows' = rows
  /\ archivedBytes' = bytes
  /\ lastAction' = "capture-succeeds"
  /\ UNCHANGED <<host, intent, presentation, surface, inputEnabled, mergedHistory>>

CaptureFails ==
  /\ capture = "capturing"
  /\ capture' = "failed"
  /\ archivedRows' = 0
  /\ archivedBytes' = 0
  /\ lastAction' = "capture-fails"
  /\ UNCHANGED <<host, intent, presentation, surface, inputEnabled, mergedHistory>>

RejectCorruptArchive ==
  /\ capture = "stored"
  /\ capture' = "failed"
  /\ archivedRows' = 0
  /\ archivedBytes' = 0
  /\ surface' = IF presentation = "stopped" THEN "empty" ELSE surface
  /\ lastAction' = "reject-corrupt"
  /\ UNCHANGED <<host, intent, presentation, inputEnabled, mergedHistory>>

EvictArchive ==
  /\ capture = "stored"
  /\ capture' = "removed"
  /\ archivedRows' = 0
  /\ archivedBytes' = 0
  /\ surface' = IF presentation = "stopped" THEN "empty" ELSE surface
  /\ lastAction' = "evict-archive"
  /\ UNCHANGED <<host, intent, presentation, inputEnabled, mergedHistory>>

DisposeHost ==
  /\ host = "exited"
  /\ IF intent = "stop" THEN capture \in {"stored", "failed", "removed"}
       ELSE intent = "close" /\ capture = "none"
  /\ host' = "disposed"
  /\ lastAction' = "dispose-host"
  /\ UNCHANGED <<intent, presentation, capture, archivedRows, archivedBytes,
       surface, inputEnabled, mergedHistory>>

FinishStop ==
  /\ host = "disposed"
  /\ intent = "stop"
  /\ presentation = "stopping"
  /\ intent' = "stopped"
  /\ presentation' = "stopped"
  /\ surface' = IF capture = "stored" THEN "archive" ELSE "empty"
  /\ inputEnabled' = FALSE
  /\ lastAction' = "finish-stop"
  /\ UNCHANGED <<host, capture, archivedRows, archivedBytes, mergedHistory>>

FinishClose ==
  /\ host = "disposed"
  /\ intent = "close"
  /\ presentation = "stopping"
  /\ presentation' = "closed"
  /\ capture' = "removed"
  /\ archivedRows' = 0
  /\ archivedBytes' = 0
  /\ surface' = "none"
  /\ inputEnabled' = FALSE
  /\ lastAction' = "finish-close"
  /\ UNCHANGED <<host, intent, mergedHistory>>

StartFresh ==
  /\ presentation = "stopped"
  /\ intent = "stopped"
  /\ presentation' = "fresh"
  /\ intent' = "retain"
  /\ capture' = "removed"
  /\ archivedRows' = 0
  /\ archivedBytes' = 0
  /\ surface' = "live"
  /\ inputEnabled' = TRUE
  /\ mergedHistory' = FALSE
  /\ lastAction' = "start-fresh"
  /\ UNCHANGED host

CloseStopped ==
  /\ presentation = "stopped"
  /\ intent = "stopped"
  /\ presentation' = "closed"
  /\ intent' = "close"
  /\ capture' = "removed"
  /\ archivedRows' = 0
  /\ archivedBytes' = 0
  /\ surface' = "none"
  /\ inputEnabled' = FALSE
  /\ mergedHistory' = FALSE
  /\ lastAction' = "close-stopped"
  /\ UNCHANGED host

Next ==
  \/ RequestStop
  \/ RequestClose
  \/ HostExit
  \/ BeginCapture
  \/ \E rows \in 1..MaxRows, bytes \in 1..MaxBytes: CaptureSucceeds(rows, bytes)
  \/ CaptureFails
  \/ RejectCorruptArchive
  \/ EvictArchive
  \/ DisposeHost
  \/ FinishStop
  \/ FinishClose
  \/ StartFresh
  \/ CloseStopped

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(HostExit)
  /\ WF_vars(BeginCapture)
  /\ WF_vars(CaptureFails)
  /\ WF_vars(DisposeHost)
  /\ WF_vars(FinishStop)
  /\ WF_vars(FinishClose)

TypeOK ==
  /\ host \in HostStates
  /\ intent \in Intents
  /\ presentation \in PresentationStates
  /\ capture \in CaptureStates
  /\ archivedRows \in 0..MaxRows
  /\ archivedBytes \in 0..MaxBytes
  /\ surface \in SurfaceStates
  /\ inputEnabled \in BOOLEAN
  /\ mergedHistory \in BOOLEAN

CapturePrecedesDispose == host = "disposed" => capture # "capturing"

ArchiveBounds ==
  /\ capture = "stored" => archivedRows \in 1..MaxRows /\ archivedBytes \in 1..MaxBytes
  /\ capture \in {"none", "failed", "removed"} => archivedRows = 0 /\ archivedBytes = 0

CloseNeverArchives == intent = "close" => capture \in {"none", "removed"}

ArchiveSurfaceIsReadOnly == surface = "archive" => ~inputEnabled

StoppedProjection ==
  presentation = "stopped" =>
    /\ ~inputEnabled
    /\ surface = IF capture = "stored" THEN "archive" ELSE "empty"

FreshNeverMergesArchive ==
  presentation = "fresh" =>
    /\ capture = "removed"
    /\ archivedRows = 0
    /\ archivedBytes = 0
    /\ surface = "live"
    /\ inputEnabled
    /\ ~mergedHistory

NoTransitionMergesHistory == ~mergedHistory

ClosedHasNoArchive ==
  presentation = "closed" =>
    /\ capture = "removed"
    /\ archivedRows = 0
    /\ archivedBytes = 0
    /\ surface = "none"

CaptureEventuallySettles == capture = "capturing" ~> capture # "capturing"

ExitedEventuallyDisposes == host = "exited" ~> host = "disposed"

StoppingEventuallySettles ==
  presentation = "stopping" ~> presentation \in {"stopped", "closed"}

=============================================================================
