---------------------- MODULE WorkspaceTerminalCreate ----------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

(*
One Workspace Create resolves an already accepted primary Directory to one
exact Worker and explicit cwd.  Workbench preparation is pure: a stage may be
dropped after the Site revision or Worker selection changes without consuming
membership, ledger, or terminal state.  The successful transition jointly
commits membership, terminal intent, the Site revision, and a retained pending
result.

The existing standalone first-Open slot is deliberately separate.  It may
start or settle before, during, or after Workspace Create, but Workspace
actions neither read nor notify it.  A standalone start advances the shared
Site revision, which makes a previously prepared Workspace stage stale.  Its
later settlement does not rewrite the revision retained by Workspace Create.

Pending and final exact replays use an independent response flight.  A pending
flight can therefore survive final settlement without being silently changed
into a final response.  Directory removal and Worker loss after commit settle
as failures while retaining the committed membership; HomeCwd is in the state
type so that accidental fallback is a real invariant violation.
*)

CONSTANTS Workers, DirectoryWorker, OtherWorker, NoWorker,
          ExplicitCwd, HomeCwd, NoCwd, Request,
          CompletionSeal, ForeignSeal, NoSeal

ASSUME /\ Cardinality(Workers) = 2
       /\ DirectoryWorker \in Workers
       /\ OtherWorker \in Workers
       /\ DirectoryWorker # OtherWorker
       /\ NoWorker \notin Workers
       /\ ExplicitCwd # HomeCwd
       /\ ExplicitCwd # NoCwd
       /\ HomeCwd # NoCwd
       /\ CompletionSeal # ForeignSeal
       /\ CompletionSeal # NoSeal
       /\ ForeignSeal # NoSeal

StageStates == {"none", "prepared"}
LedgerStates == {"absent", "pending", "final"}
TerminalStates ==
  {"absent", "creating", "started", "runtime-unavailable",
   "launch-failed", "unknown"}
FinalOutcomes ==
  {"started", "runtime-unavailable", "launch-failed", "unknown"}
LedgerOutcomes == {"none", "pending"} \cup FinalOutcomes
InitialReplies ==
  {"none", "pending-visible", "pending-lost",
   "final-visible", "final-lost"}
ReplayFlights ==
  {"none", "pending", "pending-written", "final", "final-written"}
ReplayOutcomes == {"none", "pending"} \cup FinalOutcomes
StandaloneStates == {"empty", "creating", "settled"}
AdmissionObservations == {"none", "wrong-worker", "worker-unavailable"}

VARIABLES
  revision,
  members,
  terminalIntent,
  intentWorker,
  intentCwd,
  stageState,
  stagedBase,
  stagedWorker,
  stagedCwd,
  ledgerStatus,
  ledgerOutcome,
  ledgerResultRevision,
  completionSeal,
  dispatchCount,
  dispatchedWorker,
  dispatchedCwd,
  initialReply,
  replayFlight,
  replayRevision,
  replayOutcome,
  availableWorker,
  directoryPresent,
  standaloneSlot,
  standaloneNotifications,
  admissionObservation,
  wrongSealRejected,
  staleSealRejected,
  lastAction

workspaceOwnerVars ==
  <<members, terminalIntent, intentWorker, intentCwd,
    ledgerStatus, ledgerOutcome, ledgerResultRevision, completionSeal,
    dispatchCount, dispatchedWorker, dispatchedCwd>>

stageVars == <<stageState, stagedBase, stagedWorker, stagedCwd>>

deliveryVars ==
  <<initialReply, replayFlight, replayRevision, replayOutcome>>

environmentVars == <<availableWorker, directoryPresent>>
standaloneVars == <<standaloneSlot, standaloneNotifications>>
observerVars ==
  <<admissionObservation, wrongSealRejected, staleSealRejected>>

vars ==
  <<revision, workspaceOwnerVars, stageVars, deliveryVars,
    environmentVars, standaloneVars, observerVars, lastAction>>

WorkspaceCommitCount == Len(members)
StandaloneCommitCount == IF standaloneSlot = "empty" THEN 0 ELSE 1

Init ==
  /\ standaloneSlot \in StandaloneStates
  /\ revision = IF standaloneSlot = "empty" THEN 0 ELSE 1
  /\ standaloneNotifications =
       IF standaloneSlot = "settled" THEN 1 ELSE 0
  /\ members = <<>>
  /\ terminalIntent = "absent"
  /\ intentWorker = NoWorker
  /\ intentCwd = NoCwd
  /\ stageState = "none"
  /\ stagedBase = 0
  /\ stagedWorker = NoWorker
  /\ stagedCwd = NoCwd
  /\ ledgerStatus = "absent"
  /\ ledgerOutcome = "none"
  /\ ledgerResultRevision = 0
  /\ completionSeal = NoSeal
  /\ dispatchCount = 0
  /\ dispatchedWorker = NoWorker
  /\ dispatchedCwd = NoCwd
  /\ initialReply = "none"
  /\ replayFlight = "none"
  /\ replayRevision = 0
  /\ replayOutcome = "none"
  /\ availableWorker = DirectoryWorker
  /\ directoryPresent = TRUE
  /\ admissionObservation = "none"
  /\ wrongSealRejected = FALSE
  /\ staleSealRejected = FALSE
  /\ lastAction = "Init"

StageWorkspaceCreate ==
  /\ stageState = "none"
  /\ ledgerStatus = "absent"
  /\ members = <<>>
  /\ availableWorker = DirectoryWorker
  /\ stageState' = "prepared"
  /\ stagedBase' = revision
  /\ stagedWorker' = DirectoryWorker
  /\ stagedCwd' = ExplicitCwd
  /\ admissionObservation' = "none"
  /\ lastAction' = "StageWorkspaceCreate"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, deliveryVars, environmentVars,
         standaloneVars, wrongSealRejected, staleSealRejected>>

DropStagedCreate ==
  /\ stageState = "prepared"
  /\ stageState' = "none"
  /\ stagedBase' = 0
  /\ stagedWorker' = NoWorker
  /\ stagedCwd' = NoCwd
  /\ lastAction' = "DropStagedCreate"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, deliveryVars, environmentVars,
         standaloneVars, observerVars>>

RefuseWrongWorker ==
  /\ stageState = "none"
  /\ ledgerStatus = "absent"
  /\ members = <<>>
  /\ availableWorker = OtherWorker
  /\ admissionObservation' = "wrong-worker"
  /\ lastAction' = "RefuseWrongWorker"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         environmentVars, standaloneVars, wrongSealRejected,
         staleSealRejected>>

RefuseWorkerUnavailable ==
  /\ stageState = "none"
  /\ ledgerStatus = "absent"
  /\ members = <<>>
  /\ availableWorker = NoWorker
  /\ admissionObservation' = "worker-unavailable"
  /\ lastAction' = "RefuseWorkerUnavailable"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         environmentVars, standaloneVars, wrongSealRejected,
         staleSealRejected>>

JointCommitWorkspaceCreate ==
  /\ stageState = "prepared"
  /\ stagedBase = revision
  /\ stagedWorker = DirectoryWorker
  /\ stagedCwd = ExplicitCwd
  /\ availableWorker = DirectoryWorker
  /\ ledgerStatus = "absent"
  /\ members = <<>>
  /\ revision' = revision + 1
  /\ members' = <<Request>>
  /\ terminalIntent' = "creating"
  /\ intentWorker' = stagedWorker
  /\ intentCwd' = stagedCwd
  /\ ledgerStatus' = "pending"
  /\ ledgerOutcome' = "pending"
  /\ ledgerResultRevision' = revision + 1
  /\ completionSeal' = CompletionSeal
  /\ dispatchCount' = 0
  /\ dispatchedWorker' = NoWorker
  /\ dispatchedCwd' = NoCwd
  /\ stageState' = "none"
  /\ stagedBase' = 0
  /\ stagedWorker' = NoWorker
  /\ stagedCwd' = NoCwd
  /\ initialReply' = "pending-visible"
  /\ admissionObservation' = "none"
  /\ lastAction' = "JointCommitWorkspaceCreate"
  /\ UNCHANGED
       <<replayFlight, replayRevision, replayOutcome, environmentVars,
         standaloneVars, wrongSealRejected, staleSealRejected>>

DispatchLaunch ==
  /\ ledgerStatus = "pending"
  /\ terminalIntent = "creating"
  /\ dispatchCount = 0
  /\ dispatchCount' = 1
  /\ dispatchedWorker' = intentWorker
  /\ dispatchedCwd' = intentCwd
  /\ lastAction' = "DispatchLaunch"
  /\ UNCHANGED
       <<revision, members, terminalIntent, intentWorker, intentCwd,
         ledgerStatus, ledgerOutcome, ledgerResultRevision, completionSeal,
         stageVars, deliveryVars, environmentVars, standaloneVars,
         observerVars>>

LosePendingResult ==
  /\ ledgerStatus = "pending"
  /\ initialReply = "pending-visible"
  /\ initialReply' = "pending-lost"
  /\ lastAction' = "LosePendingResult"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars,
         replayFlight, replayRevision, replayOutcome,
         environmentVars, standaloneVars, observerVars>>

ReplayPending ==
  /\ ledgerStatus = "pending"
  /\ replayFlight = "none"
  /\ replayFlight' = "pending"
  /\ replayRevision' = ledgerResultRevision
  /\ replayOutcome' = "pending"
  /\ lastAction' = "ReplayPending"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, initialReply,
         environmentVars, standaloneVars, observerVars>>

SettleWorkspaceCreate(outcome, actionName) ==
  /\ outcome \in FinalOutcomes
  /\ ledgerStatus = "pending"
  /\ terminalIntent = "creating"
  /\ dispatchCount = 1
  /\ completionSeal = CompletionSeal
  /\ terminalIntent' = outcome
  /\ ledgerStatus' = "final"
  /\ ledgerOutcome' = outcome
  /\ completionSeal' = NoSeal
  /\ initialReply' = "final-visible"
  /\ lastAction' = actionName
  /\ UNCHANGED
       <<revision, members, intentWorker, intentCwd, ledgerResultRevision,
         dispatchCount, dispatchedWorker, dispatchedCwd, stageVars,
         replayFlight, replayRevision, replayOutcome,
         environmentVars, standaloneVars, observerVars>>

CompleteStarted ==
  /\ availableWorker = DirectoryWorker
  /\ directoryPresent
  /\ SettleWorkspaceCreate("started", "CompleteStarted")

CompleteRuntimeUnavailable ==
  /\ availableWorker # DirectoryWorker
  /\ SettleWorkspaceCreate(
       "runtime-unavailable", "CompleteRuntimeUnavailable")

CompleteLaunchFailed ==
  /\ availableWorker = DirectoryWorker
  /\ ~directoryPresent
  /\ SettleWorkspaceCreate("launch-failed", "CompleteLaunchFailed")

CompleteOutcomeUnknown ==
  /\ dispatchCount = 1
  /\ SettleWorkspaceCreate("unknown", "CompleteOutcomeUnknown")

LoseFinalResult ==
  /\ ledgerStatus = "final"
  /\ initialReply = "final-visible"
  /\ initialReply' = "final-lost"
  /\ lastAction' = "LoseFinalResult"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars,
         replayFlight, replayRevision, replayOutcome,
         environmentVars, standaloneVars, observerVars>>

ReplayFinal ==
  /\ ledgerStatus = "final"
  /\ replayFlight = "none"
  /\ replayFlight' = "final"
  /\ replayRevision' = ledgerResultRevision
  /\ replayOutcome' = ledgerOutcome
  /\ lastAction' = "ReplayFinal"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, initialReply,
         environmentVars, standaloneVars, observerVars>>

WriteReplayResponse ==
  /\ replayFlight \in {"pending", "final"}
  /\ replayFlight' =
       IF replayFlight = "pending" THEN "pending-written"
       ELSE "final-written"
  /\ lastAction' = "WriteReplayResponse"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, initialReply,
         replayRevision, replayOutcome, environmentVars, standaloneVars,
         observerVars>>

ClearReplayResponse ==
  /\ replayFlight \in {"pending-written", "final-written"}
  /\ replayFlight' = "none"
  /\ replayRevision' = 0
  /\ replayOutcome' = "none"
  /\ lastAction' = "ClearReplayResponse"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, initialReply,
         environmentVars, standaloneVars, observerVars>>

RejectWrongCompletionSeal ==
  /\ ledgerStatus = "pending"
  /\ dispatchCount = 1
  /\ ForeignSeal # completionSeal
  /\ ~wrongSealRejected
  /\ wrongSealRejected' = TRUE
  /\ lastAction' = "RejectWrongCompletionSeal"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         environmentVars, standaloneVars, admissionObservation,
         staleSealRejected>>

RejectStaleCompletion ==
  /\ ledgerStatus = "final"
  /\ CompletionSeal # completionSeal
  /\ ~staleSealRejected
  /\ staleSealRejected' = TRUE
  /\ lastAction' = "RejectStaleCompletion"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         environmentVars, standaloneVars, admissionObservation,
         wrongSealRejected>>

SelectOtherWorker ==
  /\ availableWorker = DirectoryWorker
  /\ availableWorker' = OtherWorker
  /\ lastAction' = "SelectOtherWorker"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         directoryPresent, standaloneVars, observerVars>>

LoseDirectoryWorker ==
  /\ availableWorker = DirectoryWorker
  /\ availableWorker' = NoWorker
  /\ lastAction' = "LoseDirectoryWorker"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         directoryPresent, standaloneVars, observerVars>>

RestoreDirectoryWorker ==
  /\ availableWorker # DirectoryWorker
  /\ availableWorker' = DirectoryWorker
  /\ lastAction' = "RestoreDirectoryWorker"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         directoryPresent, standaloneVars, observerVars>>

RemoveDirectory ==
  /\ directoryPresent
  /\ directoryPresent' = FALSE
  /\ lastAction' = "RemoveDirectory"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         availableWorker, standaloneVars, observerVars>>

StandaloneBegin ==
  /\ standaloneSlot = "empty"
  /\ standaloneSlot' = "creating"
  /\ revision' = revision + 1
  /\ lastAction' = "StandaloneBegin"
  /\ UNCHANGED
       <<workspaceOwnerVars, stageVars, deliveryVars, environmentVars,
         standaloneNotifications, observerVars>>

StandaloneSettle ==
  /\ standaloneSlot = "creating"
  /\ standaloneSlot' = "settled"
  /\ standaloneNotifications' = 1
  /\ lastAction' = "StandaloneSettle"
  /\ UNCHANGED
       <<revision, workspaceOwnerVars, stageVars, deliveryVars,
         environmentVars, observerVars>>

(* Direct Next branches make each semantic witness visible to action coverage. *)
ObservePendingReplaySurvivesSettlement ==
  /\ ledgerStatus = "final"
  /\ replayFlight \in {"pending", "pending-written"}
  /\ replayRevision = ledgerResultRevision
  /\ UNCHANGED vars

ObserveWorkspaceCommitWhileStandaloneCreating ==
  /\ ledgerStatus \in {"pending", "final"}
  /\ standaloneSlot = "creating"
  /\ UNCHANGED vars

ObserveWorkspaceCompletionWhileStandaloneSettled ==
  /\ ledgerStatus = "final"
  /\ standaloneSlot = "settled"
  /\ UNCHANGED vars

ObserveLaunchFailureRetainsMembership ==
  /\ ledgerStatus = "final"
  /\ ledgerOutcome \in
       {"runtime-unavailable", "launch-failed", "unknown"}
  /\ members = <<Request>>
  /\ UNCHANGED vars

ObserveWrongWorkerWithoutHome ==
  /\ admissionObservation = "wrong-worker"
  /\ stageState = "none"
  /\ intentCwd # HomeCwd
  /\ dispatchedCwd # HomeCwd
  /\ UNCHANGED vars

ObserveResultRevisionSurvivesStandaloneAdvance ==
  /\ ledgerStatus \in {"pending", "final"}
  /\ revision > ledgerResultRevision
  /\ standaloneSlot \in {"creating", "settled"}
  /\ UNCHANGED vars

Next ==
  \/ StageWorkspaceCreate
  \/ DropStagedCreate
  \/ RefuseWrongWorker
  \/ RefuseWorkerUnavailable
  \/ JointCommitWorkspaceCreate
  \/ DispatchLaunch
  \/ LosePendingResult
  \/ ReplayPending
  \/ CompleteStarted
  \/ CompleteRuntimeUnavailable
  \/ CompleteLaunchFailed
  \/ CompleteOutcomeUnknown
  \/ LoseFinalResult
  \/ ReplayFinal
  \/ WriteReplayResponse
  \/ ClearReplayResponse
  \/ RejectWrongCompletionSeal
  \/ RejectStaleCompletion
  \/ SelectOtherWorker
  \/ LoseDirectoryWorker
  \/ RestoreDirectoryWorker
  \/ RemoveDirectory
  \/ StandaloneBegin
  \/ StandaloneSettle
  \/ ObservePendingReplaySurvivesSettlement
  \/ ObserveWorkspaceCommitWhileStandaloneCreating
  \/ ObserveWorkspaceCompletionWhileStandaloneSettled
  \/ ObserveLaunchFailureRetainsMembership
  \/ ObserveWrongWorkerWithoutHome
  \/ ObserveResultRevisionSurvivesStandaloneAdvance

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ revision \in 0..2
  /\ members \in {<<>>, <<Request>>}
  /\ terminalIntent \in TerminalStates
  /\ intentWorker \in Workers \cup {NoWorker}
  /\ intentCwd \in {ExplicitCwd, HomeCwd, NoCwd}
  /\ stageState \in StageStates
  /\ stagedBase \in 0..2
  /\ stagedWorker \in Workers \cup {NoWorker}
  /\ stagedCwd \in {ExplicitCwd, HomeCwd, NoCwd}
  /\ ledgerStatus \in LedgerStates
  /\ ledgerOutcome \in LedgerOutcomes
  /\ ledgerResultRevision \in 0..2
  /\ completionSeal \in {CompletionSeal, ForeignSeal, NoSeal}
  /\ dispatchCount \in 0..1
  /\ dispatchedWorker \in Workers \cup {NoWorker}
  /\ dispatchedCwd \in {ExplicitCwd, HomeCwd, NoCwd}
  /\ initialReply \in InitialReplies
  /\ replayFlight \in ReplayFlights
  /\ replayRevision \in 0..2
  /\ replayOutcome \in ReplayOutcomes
  /\ availableWorker \in Workers \cup {NoWorker}
  /\ directoryPresent \in BOOLEAN
  /\ standaloneSlot \in StandaloneStates
  /\ standaloneNotifications \in 0..1
  /\ admissionObservation \in AdmissionObservations
  /\ wrongSealRejected \in BOOLEAN
  /\ staleSealRejected \in BOOLEAN
  /\ lastAction \in
       {"Init", "StageWorkspaceCreate", "DropStagedCreate",
        "RefuseWrongWorker", "RefuseWorkerUnavailable",
        "JointCommitWorkspaceCreate", "DispatchLaunch",
        "LosePendingResult", "ReplayPending", "CompleteStarted",
        "CompleteRuntimeUnavailable", "CompleteLaunchFailed",
        "CompleteOutcomeUnknown", "LoseFinalResult", "ReplayFinal",
        "WriteReplayResponse", "ClearReplayResponse",
        "RejectWrongCompletionSeal", "RejectStaleCompletion",
        "SelectOtherWorker", "LoseDirectoryWorker",
        "RestoreDirectoryWorker", "RemoveDirectory",
        "StandaloneBegin", "StandaloneSettle"}

RevisionAccounting ==
  revision = WorkspaceCommitCount + StandaloneCommitCount

LedgerOwnsMembership ==
  (ledgerStatus = "absent") <=> (members = <<>>)

TerminalIntentMatchesLedger ==
  /\ (ledgerStatus = "absent" =>
        /\ terminalIntent = "absent"
        /\ ledgerOutcome = "none"
        /\ ledgerResultRevision = 0
        /\ completionSeal = NoSeal)
  /\ (ledgerStatus = "pending" =>
        /\ terminalIntent = "creating"
        /\ ledgerOutcome = "pending"
        /\ ledgerResultRevision \in 1..revision
        /\ completionSeal = CompletionSeal)
  /\ (ledgerStatus = "final" =>
        /\ terminalIntent = ledgerOutcome
        /\ ledgerOutcome \in FinalOutcomes
        /\ ledgerResultRevision \in 1..revision
        /\ completionSeal = NoSeal)

ExactCanonicalPlacement ==
  /\ (stageState = "prepared" =>
        /\ stagedWorker = DirectoryWorker
        /\ stagedCwd = ExplicitCwd)
  /\ (members = <<Request>> =>
        /\ intentWorker = DirectoryWorker
        /\ intentCwd = ExplicitCwd)
  /\ (dispatchCount = 1 =>
        /\ dispatchedWorker = DirectoryWorker
        /\ dispatchedCwd = ExplicitCwd)

NoHomeFallback ==
  /\ stagedCwd # HomeCwd
  /\ intentCwd # HomeCwd
  /\ dispatchedCwd # HomeCwd

EmptyFieldsAreCanonical ==
  /\ (stageState = "none" =>
        /\ stagedBase = 0
        /\ stagedWorker = NoWorker
        /\ stagedCwd = NoCwd)
  /\ (members = <<>> =>
        /\ intentWorker = NoWorker
        /\ intentCwd = NoCwd)
  /\ (dispatchCount = 0 =>
        /\ dispatchedWorker = NoWorker
        /\ dispatchedCwd = NoCwd)

DispatchIsOnceAndAfterCommit ==
  /\ (dispatchCount = 1 =>
        /\ members = <<Request>>
        /\ ledgerStatus \in {"pending", "final"})
  /\ (ledgerStatus = "final" => dispatchCount = 1)

InitialReplyMatchesLedger ==
  /\ (ledgerStatus = "absent" => initialReply = "none")
  /\ (ledgerStatus = "pending" =>
        initialReply \in {"pending-visible", "pending-lost"})
  /\ (ledgerStatus = "final" =>
        initialReply \in {"final-visible", "final-lost"})

ReplayFlightIsCorrelated ==
  /\ (replayFlight = "none" =>
        /\ replayRevision = 0
        /\ replayOutcome = "none")
  /\ (replayFlight \in {"pending", "pending-written"} =>
        /\ ledgerStatus \in {"pending", "final"}
        /\ replayRevision = ledgerResultRevision
        /\ replayOutcome = "pending")
  /\ (replayFlight \in {"final", "final-written"} =>
        /\ ledgerStatus = "final"
        /\ replayRevision = ledgerResultRevision
        /\ replayOutcome = ledgerOutcome)

StandaloneNotificationMatchesSlot ==
  (standaloneNotifications = 1) <=> (standaloneSlot = "settled")

FailureCompletionRetainsMembership ==
  ledgerOutcome \in {"runtime-unavailable", "launch-failed", "unknown"}
    => /\ ledgerStatus = "final"
       /\ members = <<Request>>

JointCommitIsAtomic == [][
  members = <<>> /\ members' = <<Request>> =>
    /\ revision' = revision + 1
    /\ terminalIntent' = "creating"
    /\ intentWorker' = DirectoryWorker
    /\ intentCwd' = ExplicitCwd
    /\ ledgerStatus' = "pending"
    /\ ledgerOutcome' = "pending"
    /\ ledgerResultRevision' = revision'
    /\ completionSeal' = CompletionSeal
    /\ stageState' = "none"
    /\ UNCHANGED standaloneVars]_vars

PrecommitPreservesOwners == [][
  lastAction' \in
    {"StageWorkspaceCreate", "DropStagedCreate",
     "RefuseWrongWorker", "RefuseWorkerUnavailable"}
  => UNCHANGED <<revision, workspaceOwnerVars>>]_vars

InvalidStageCannotCommit == [][
  stageState = "prepared"
    /\ (stagedBase # revision \/ availableWorker # DirectoryWorker)
  => UNCHANGED workspaceOwnerVars]_vars

CompletionPreservesCommittedTuple == [][
  lastAction' \in
    {"CompleteStarted", "CompleteRuntimeUnavailable",
     "CompleteLaunchFailed", "CompleteOutcomeUnknown"}
  => UNCHANGED
       <<revision, members, intentWorker, intentCwd, ledgerResultRevision,
         dispatchCount, dispatchedWorker, dispatchedCwd>>]_vars

LedgerResultRevisionIsImmutable == [][
  ledgerStatus # "absent" =>
    ledgerResultRevision' = ledgerResultRevision]_vars

PendingReplaySurvivesSettlement == [][
  (replayFlight \in {"pending", "pending-written"}
   /\ ledgerStatus = "pending"
   /\ ledgerStatus' = "final")
  =>
  (replayFlight' = replayFlight
   /\ replayRevision' = replayRevision
   /\ replayOutcome' = replayOutcome)]_vars

ReplayAndLossPreserveOwners == [][
  lastAction' \in
    {"LosePendingResult", "ReplayPending", "LoseFinalResult",
     "ReplayFinal", "WriteReplayResponse", "ClearReplayResponse"}
  => UNCHANGED <<revision, workspaceOwnerVars, standaloneVars>>]_vars

RejectedCompletionPreservesOwners == [][
  lastAction' \in
    {"RejectWrongCompletionSeal", "RejectStaleCompletion"}
  => UNCHANGED <<revision, workspaceOwnerVars, standaloneVars>>]_vars

WorkspaceActionsPreserveStandalone == [][
  lastAction' \notin {"Init", "StandaloneBegin", "StandaloneSettle"}
  => UNCHANGED standaloneVars]_vars

StandaloneActionsAreIndependent == [][
  lastAction' \in {"StandaloneBegin", "StandaloneSettle"}
  => UNCHANGED
       <<workspaceOwnerVars, stageVars, deliveryVars,
         environmentVars, observerVars>>]_vars

DispatchCannotRepeat == [][
  dispatchCount = 1 => dispatchCount' = 1]_vars

FinalLedgerIsImmutable == [][
  ledgerStatus = "final" => UNCHANGED workspaceOwnerVars]_vars

=============================================================================
