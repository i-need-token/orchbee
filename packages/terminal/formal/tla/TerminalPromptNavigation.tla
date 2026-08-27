----------------------- MODULE TerminalPromptNavigation -----------------------
EXTENDS Naturals, TLC

CONSTANTS MaxRequests, MaxRevision, MaxIntent

Directions == {"previous", "next"}
Phases == {"idle", "reading", "target-ready", "settled", "cancelled", "disposed"}
Outcomes == {"none", "moved", "unchanged", "cancelled", "disposed"}
Actions == {
  "init",
  "start-previous",
  "start-next",
  "read-target",
  "read-not-found",
  "advance-revision",
  "move-viewport",
  "cancel",
  "replace-previous",
  "replace-next",
  "commit",
  "reject-stale-target",
  "dispose"
}

VARIABLES
  active,
  phase,
  direction,
  requestEpoch,
  admissions,
  sourceRevision,
  capturedRevision,
  viewportIntent,
  capturedIntent,
  targetRequest,
  commitCount,
  outcome,
  priorPhase,
  priorRequestEpoch,
  priorAdmissions,
  priorSourceRevision,
  priorCapturedRevision,
  priorViewportIntent,
  priorCapturedIntent,
  priorTargetRequest,
  priorCommitCount,
  lastAction

vars == <<
  active,
  phase,
  direction,
  requestEpoch,
  admissions,
  sourceRevision,
  capturedRevision,
  viewportIntent,
  capturedIntent,
  targetRequest,
  commitCount,
  outcome,
  priorPhase,
  priorRequestEpoch,
  priorAdmissions,
  priorSourceRevision,
  priorCapturedRevision,
  priorViewportIntent,
  priorCapturedIntent,
  priorTargetRequest,
  priorCommitCount,
  lastAction
>>

Init ==
  /\ active = TRUE
  /\ phase = "idle"
  /\ direction = "previous"
  /\ requestEpoch = 0
  /\ admissions = 0
  /\ sourceRevision = 1
  /\ capturedRevision = 1
  /\ viewportIntent = 0
  /\ capturedIntent = 0
  /\ targetRequest = 0
  /\ commitCount = 0
  /\ outcome = "none"
  /\ priorPhase = "idle"
  /\ priorRequestEpoch = 0
  /\ priorAdmissions = 0
  /\ priorSourceRevision = 1
  /\ priorCapturedRevision = 1
  /\ priorViewportIntent = 0
  /\ priorCapturedIntent = 0
  /\ priorTargetRequest = 0
  /\ priorCommitCount = 0
  /\ lastAction = "init"

Record(action) ==
  /\ priorPhase' = phase
  /\ priorRequestEpoch' = requestEpoch
  /\ priorAdmissions' = admissions
  /\ priorSourceRevision' = sourceRevision
  /\ priorCapturedRevision' = capturedRevision
  /\ priorViewportIntent' = viewportIntent
  /\ priorCapturedIntent' = capturedIntent
  /\ priorTargetRequest' = targetRequest
  /\ priorCommitCount' = commitCount
  /\ lastAction' = action

Start(dir, action) ==
  /\ active
  /\ phase \in {"idle", "settled", "cancelled"}
  /\ admissions < MaxRequests
  /\ dir \in Directions
  /\ phase' = "reading"
  /\ direction' = dir
  /\ requestEpoch' = requestEpoch + 1
  /\ admissions' = admissions + 1
  /\ capturedRevision' = sourceRevision
  /\ capturedIntent' = viewportIntent
  /\ targetRequest' = 0
  /\ outcome' = "none"
  /\ UNCHANGED <<active, sourceRevision, viewportIntent, commitCount>>
  /\ Record(action)

StartPrevious == Start("previous", "start-previous")
StartNext == Start("next", "start-next")

ReadTarget ==
  /\ active
  /\ phase = "reading"
  /\ phase' = "target-ready"
  /\ targetRequest' = requestEpoch
  /\ UNCHANGED <<
       active, direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       viewportIntent, capturedIntent, commitCount, outcome
     >>
  /\ Record("read-target")

ReadNotFound ==
  /\ active
  /\ phase = "reading"
  /\ phase' = "settled"
  /\ targetRequest' = 0
  /\ outcome' = "unchanged"
  /\ UNCHANGED <<
       active, direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       viewportIntent, capturedIntent, commitCount
     >>
  /\ Record("read-not-found")

AdvanceRevision ==
  /\ active
  /\ sourceRevision < MaxRevision
  /\ sourceRevision' = sourceRevision + 1
  /\ UNCHANGED <<
       active, phase, direction, requestEpoch, admissions, capturedRevision,
       viewportIntent, capturedIntent, targetRequest, commitCount, outcome
     >>
  /\ Record("advance-revision")

MoveViewport ==
  /\ active
  /\ viewportIntent < MaxIntent
  /\ viewportIntent' = viewportIntent + 1
  /\ UNCHANGED <<
       active, phase, direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       capturedIntent, targetRequest, commitCount, outcome
     >>
  /\ Record("move-viewport")

Cancel ==
  /\ active
  /\ phase \in {"reading", "target-ready"}
  /\ phase' = "cancelled"
  /\ targetRequest' = 0
  /\ outcome' = "cancelled"
  /\ UNCHANGED <<
       active, direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       viewportIntent, capturedIntent, commitCount
     >>
  /\ Record("cancel")

Replace(dir, action) ==
  /\ active
  /\ phase \in {"reading", "target-ready"}
  /\ admissions < MaxRequests
  /\ dir \in Directions
  /\ phase' = "reading"
  /\ direction' = dir
  /\ requestEpoch' = requestEpoch + 1
  /\ admissions' = admissions + 1
  /\ capturedRevision' = sourceRevision
  /\ capturedIntent' = viewportIntent
  /\ targetRequest' = 0
  /\ outcome' = "none"
  /\ UNCHANGED <<active, sourceRevision, viewportIntent, commitCount>>
  /\ Record(action)

ReplacePrevious == Replace("previous", "replace-previous")
ReplaceNext == Replace("next", "replace-next")

CoverageStartPrevious ==
  /\ lastAction = "start-previous"
  /\ UNCHANGED vars

CoverageStartNext ==
  /\ lastAction = "start-next"
  /\ UNCHANGED vars

CoverageReplacePrevious ==
  /\ lastAction = "replace-previous"
  /\ UNCHANGED vars

CoverageReplaceNext ==
  /\ lastAction = "replace-next"
  /\ UNCHANGED vars

Commit ==
  /\ active
  /\ phase = "target-ready"
  /\ targetRequest = requestEpoch
  /\ capturedRevision = sourceRevision
  /\ capturedIntent = viewportIntent
  /\ phase' = "settled"
  /\ targetRequest' = 0
  /\ commitCount' = commitCount + 1
  /\ outcome' = "moved"
  /\ UNCHANGED <<
       active, direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       viewportIntent, capturedIntent
     >>
  /\ Record("commit")

RejectStaleTarget ==
  /\ active
  /\ phase = "target-ready"
  /\ targetRequest = requestEpoch
  /\ \/ capturedRevision # sourceRevision
     \/ capturedIntent # viewportIntent
  /\ phase' = "settled"
  /\ targetRequest' = 0
  /\ outcome' = "unchanged"
  /\ UNCHANGED <<
       active, direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       viewportIntent, capturedIntent, commitCount
     >>
  /\ Record("reject-stale-target")

Dispose ==
  /\ active
  /\ active' = FALSE
  /\ phase' = "disposed"
  /\ targetRequest' = 0
  /\ outcome' = "disposed"
  /\ UNCHANGED <<
       direction, requestEpoch, admissions, sourceRevision, capturedRevision,
       viewportIntent, capturedIntent, commitCount
     >>
  /\ Record("dispose")

FinalizeTarget == Commit \/ RejectStaleTarget

Next ==
  \/ StartPrevious
  \/ StartNext
  \/ ReadTarget
  \/ ReadNotFound
  \/ AdvanceRevision
  \/ MoveViewport
  \/ Cancel
  \/ ReplacePrevious
  \/ ReplaceNext
  \/ CoverageStartPrevious
  \/ CoverageStartNext
  \/ CoverageReplacePrevious
  \/ CoverageReplaceNext
  \/ Commit
  \/ RejectStaleTarget
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(FinalizeTarget)

TypeOK ==
  /\ active \in BOOLEAN
  /\ phase \in Phases
  /\ direction \in Directions
  /\ requestEpoch \in 0..MaxRequests
  /\ admissions \in 0..MaxRequests
  /\ sourceRevision \in 1..MaxRevision
  /\ capturedRevision \in 1..MaxRevision
  /\ viewportIntent \in 0..MaxIntent
  /\ capturedIntent \in 0..MaxIntent
  /\ targetRequest \in 0..MaxRequests
  /\ commitCount \in 0..MaxRequests
  /\ outcome \in Outcomes
  /\ priorPhase \in Phases
  /\ priorRequestEpoch \in 0..MaxRequests
  /\ priorAdmissions \in 0..MaxRequests
  /\ priorSourceRevision \in 1..MaxRevision
  /\ priorCapturedRevision \in 1..MaxRevision
  /\ priorViewportIntent \in 0..MaxIntent
  /\ priorCapturedIntent \in 0..MaxIntent
  /\ priorTargetRequest \in 0..MaxRequests
  /\ priorCommitCount \in 0..MaxRequests
  /\ lastAction \in Actions

TargetBelongsToCurrentRequest ==
  phase = "target-ready" => targetRequest = requestEpoch

ViewportChangesOnlyOnCommit ==
  commitCount # priorCommitCount =>
    /\ lastAction = "commit"
    /\ commitCount = priorCommitCount + 1

CommitRequiresCurrentAuthority ==
  lastAction = "commit" =>
    /\ priorPhase = "target-ready"
    /\ priorTargetRequest = priorRequestEpoch
    /\ priorCapturedRevision = priorSourceRevision
    /\ priorCapturedIntent = priorViewportIntent
    /\ requestEpoch = priorRequestEpoch
    /\ admissions = priorAdmissions

ReplacementStartsFresh ==
  lastAction \in {"replace-previous", "replace-next"} =>
    /\ phase = "reading"
    /\ requestEpoch = priorRequestEpoch + 1
    /\ admissions = priorAdmissions + 1
    /\ targetRequest = 0
    /\ capturedRevision = sourceRevision
    /\ capturedIntent = viewportIntent

TerminalPhaseCannotCommit ==
  priorPhase \in {"cancelled", "disposed"} => lastAction # "commit"

AtMostOneCommitPerAdmission == commitCount <= admissions

TargetReadyEventuallyRetires ==
  [](phase = "target-ready" => <>(phase # "target-ready"))

=============================================================================
