---------------- MODULE AgentIntegrationLaunchReminder ----------------
EXTENDS TLC, Naturals, Sequences

Adapters == {"claude-code", "codex"}
IntegrationStates == {"not-installed", "disabled", "enabled", "conflict", "unknown"}
None == "none"
Targets == Adapters \cup {None}

VARIABLES
  integrationState,
  mainLaunched,
  rendererLaunched,
  launchQueue,
  responsePending,
  responseLaunches,
  reminder,
  reminderConsumed,
  dismissed,
  pendingEnable,
  lastAction,
  previousIntegrationState,
  previousMainLaunched,
  previousRendererLaunched,
  previousLaunchQueue,
  previousResponsePending,
  previousResponseLaunches,
  previousReminder,
  previousReminderConsumed,
  previousDismissed,
  previousPendingEnable

vars == <<integrationState, mainLaunched, rendererLaunched, launchQueue,
  responsePending, responseLaunches, reminder, reminderConsumed, dismissed,
  pendingEnable, lastAction, previousIntegrationState, previousMainLaunched,
  previousRendererLaunched, previousLaunchQueue, previousResponsePending,
  previousResponseLaunches, previousReminder, previousReminderConsumed,
  previousDismissed, previousPendingEnable>>

SeqSet(sequence) == {sequence[index] : index \in 1..Len(sequence)}

UniqueSequence(sequence) ==
  \A left, right \in 1..Len(sequence):
    left # right => sequence[left] # sequence[right]

CanonicalSubsequence(subsequence, sequence) ==
  subsequence = SelectSeq(sequence, LAMBDA item: item \in SeqSet(subsequence))

MergeLaunches(current, incoming) ==
  current \o SelectSeq(incoming, LAMBDA item: item \notin SeqSet(current))

Eligible(adapter) ==
  integrationState[adapter] \in {"not-installed", "disabled"}

EligibleRendererLaunches ==
  SelectSeq(
    rendererLaunched,
    LAMBDA adapter: Eligible(adapter) /\ adapter \notin dismissed
  )

Record(action) ==
  /\ lastAction' = action
  /\ previousIntegrationState' = integrationState
  /\ previousMainLaunched' = mainLaunched
  /\ previousRendererLaunched' = rendererLaunched
  /\ previousLaunchQueue' = launchQueue
  /\ previousResponsePending' = responsePending
  /\ previousResponseLaunches' = responseLaunches
  /\ previousReminder' = reminder
  /\ previousReminderConsumed' = reminderConsumed
  /\ previousDismissed' = dismissed
  /\ previousPendingEnable' = pendingEnable

Init ==
  /\ integrationState \in [Adapters -> IntegrationStates]
  /\ mainLaunched = <<>>
  /\ rendererLaunched = <<>>
  /\ launchQueue = <<>>
  /\ responsePending = FALSE
  /\ responseLaunches = <<>>
  /\ reminder = None
  /\ reminderConsumed = FALSE
  /\ dismissed \in SUBSET Adapters
  /\ pendingEnable = None
  /\ lastAction = "init"
  /\ previousIntegrationState = integrationState
  /\ previousMainLaunched = mainLaunched
  /\ previousRendererLaunched = rendererLaunched
  /\ previousLaunchQueue = launchQueue
  /\ previousResponsePending = responsePending
  /\ previousResponseLaunches = responseLaunches
  /\ previousReminder = reminder
  /\ previousReminderConsumed = reminderConsumed
  /\ previousDismissed = dismissed
  /\ previousPendingEnable = pendingEnable

ObserveDirectLaunch(adapter) ==
  /\ adapter \in Adapters
  /\ adapter \notin SeqSet(mainLaunched)
  /\ mainLaunched' = Append(mainLaunched, adapter)
  /\ launchQueue' = Append(launchQueue, adapter)
  /\ UNCHANGED <<integrationState, rendererLaunched, responsePending,
    responseLaunches, reminder, reminderConsumed, dismissed, pendingEnable>>
  /\ Record("observe-direct-launch")

ObserveDuplicateLaunch(adapter) ==
  /\ adapter \in SeqSet(mainLaunched)
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminder, reminderConsumed, dismissed,
    pendingEnable>>
  /\ Record("observe-duplicate-launch")

CaptureManagementResponse ==
  /\ ~responsePending
  /\ responsePending' = TRUE
  /\ responseLaunches' = mainLaunched
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    reminder, reminderConsumed, dismissed, pendingEnable>>
  /\ Record("capture-management-response")

DeliverLaunchObservation ==
  /\ Len(launchQueue) > 0
  /\ rendererLaunched' = MergeLaunches(rendererLaunched, <<Head(launchQueue)>>)
  /\ launchQueue' = Tail(launchQueue)
  /\ UNCHANGED <<integrationState, mainLaunched, responsePending,
    responseLaunches, reminder, reminderConsumed, dismissed, pendingEnable>>
  /\ Record("deliver-launch-observation")

DeliverManagementResponse ==
  /\ responsePending
  /\ rendererLaunched' = MergeLaunches(rendererLaunched, responseLaunches)
  /\ responsePending' = FALSE
  /\ responseLaunches' = <<>>
  /\ UNCHANGED <<integrationState, mainLaunched, launchQueue, reminder,
    reminderConsumed, dismissed, pendingEnable>>
  /\ Record("deliver-management-response")

OfferReminder ==
  /\ reminder = None
  /\ ~reminderConsumed
  /\ Len(EligibleRendererLaunches) > 0
  /\ reminder' = Head(EligibleRendererLaunches)
  /\ reminderConsumed' = TRUE
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, dismissed, pendingEnable>>
  /\ Record("offer-reminder")

DismissReminder ==
  /\ reminder # None
  /\ dismissed' = dismissed \cup {reminder}
  /\ reminder' = None
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminderConsumed, pendingEnable>>
  /\ Record("dismiss-reminder")

RequestReminderEnable ==
  /\ reminder # None
  /\ pendingEnable = None
  /\ pendingEnable' = reminder
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminder, reminderConsumed, dismissed>>
  /\ Record("request-reminder-enable")

EnableFails ==
  /\ pendingEnable # None
  /\ pendingEnable' = None
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminder, reminderConsumed, dismissed>>
  /\ Record("enable-fails")

EnableSucceeds ==
  /\ pendingEnable # None
  /\ integrationState' =
    [integrationState EXCEPT ![pendingEnable] = "enabled"]
  /\ reminder' = IF reminder = pendingEnable THEN None ELSE reminder
  /\ pendingEnable' = None
  /\ UNCHANGED <<mainLaunched, rendererLaunched, launchQueue, responsePending,
    responseLaunches, reminderConsumed, dismissed>>
  /\ Record("enable-succeeds")

Next ==
  \/ \E adapter \in Adapters: ObserveDirectLaunch(adapter)
  \/ \E adapter \in Adapters: ObserveDuplicateLaunch(adapter)
  \/ CaptureManagementResponse
  \/ DeliverLaunchObservation
  \/ DeliverManagementResponse
  \/ OfferReminder
  \/ DismissReminder
  \/ RequestReminderEnable
  \/ EnableFails
  \/ EnableSucceeds

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ integrationState \in [Adapters -> IntegrationStates]
  /\ mainLaunched \in Seq(Adapters)
  /\ rendererLaunched \in Seq(Adapters)
  /\ launchQueue \in Seq(Adapters)
  /\ responsePending \in BOOLEAN
  /\ responseLaunches \in Seq(Adapters)
  /\ reminder \in Targets
  /\ reminderConsumed \in BOOLEAN
  /\ dismissed \subseteq Adapters
  /\ pendingEnable \in Targets
  /\ UniqueSequence(mainLaunched)
  /\ UniqueSequence(rendererLaunched)
  /\ UniqueSequence(launchQueue)
  /\ UniqueSequence(responseLaunches)
  /\ CanonicalSubsequence(rendererLaunched, mainLaunched)
  /\ CanonicalSubsequence(responseLaunches, mainLaunched)
  /\ (responsePending \/ responseLaunches = <<>>)
  /\ lastAction \in {
       "init", "observe-direct-launch", "observe-duplicate-launch",
       "capture-management-response", "deliver-launch-observation",
       "deliver-management-response", "offer-reminder", "dismiss-reminder",
       "request-reminder-enable", "enable-fails", "enable-succeeds"
     }
  /\ previousIntegrationState \in [Adapters -> IntegrationStates]
  /\ previousMainLaunched \in Seq(Adapters)
  /\ previousRendererLaunched \in Seq(Adapters)
  /\ previousLaunchQueue \in Seq(Adapters)
  /\ previousResponsePending \in BOOLEAN
  /\ previousResponseLaunches \in Seq(Adapters)
  /\ previousReminder \in Targets
  /\ previousReminderConsumed \in BOOLEAN
  /\ previousDismissed \subseteq Adapters
  /\ previousPendingEnable \in Targets

IntegrationStateChangesOnlyAfterVerifiedEnable ==
  integrationState # previousIntegrationState => lastAction = "enable-succeeds"

LaunchObservationIsReadOnly ==
  lastAction \in {"observe-direct-launch", "observe-duplicate-launch"} =>
    integrationState = previousIntegrationState

RendererLaunchEvidenceNeverDisappears ==
  previousRendererLaunched =
    SubSeq(rendererLaunched, 1, Len(previousRendererLaunched))

ReminderRequiresObservedEligibleLaunch ==
  reminder # None =>
    /\ reminder \in SeqSet(rendererLaunched)
    /\ Eligible(reminder)
    /\ reminder \notin dismissed
    /\ reminderConsumed

ReminderUsesFirstObservedEligibleLaunch ==
  reminder # None => reminder = Head(EligibleRendererLaunches)

ReminderConsumptionIsMonotonic ==
  previousReminderConsumed => reminderConsumed

ReminderEnableIsExplicitAndExact ==
  lastAction = "request-reminder-enable" =>
    /\ previousReminder # None
    /\ pendingEnable = previousReminder
    /\ integrationState = previousIntegrationState

EnableFailurePreservesRetryState ==
  lastAction = "enable-fails" =>
    /\ integrationState = previousIntegrationState
    /\ reminder = previousReminder
    /\ pendingEnable = None

EnableSuccessIsVerifiedForTheRequestedAgent ==
  lastAction = "enable-succeeds" =>
    /\ previousPendingEnable # None
    /\ integrationState =
      [previousIntegrationState EXCEPT ![previousPendingEnable] = "enabled"]
    /\ pendingEnable = None

=============================================================================
