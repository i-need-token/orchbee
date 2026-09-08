---------------- MODULE AgentIntegrationLaunchReminder ----------------
EXTENDS TLC, Naturals, Sequences

\* Two adapter identities expose order, deduplication, and dismissal
\* isolation. rendererLaunched records provider-scoped inspection
\* publications in renderer delivery order. Independent inspections may
\* complete in a different order from Main's raw launch observations.
\* integrationState is the bounded, already-inspected content carried by
\* those publications; item/revision reconciliation is outside this model.

Adapters == {"claude-code", "codex"}
IntegrationStates == {
  "not-installed", "disabled", "enabled", "update-required", "conflict", "unknown"
}
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
  offerCount,
  dismissed,
  lastSettingsTarget,
  lastAction,
  previousIntegrationState,
  previousRendererLaunched,
  previousReminder,
  previousReminderConsumed,
  previousOfferCount,
  previousDismissed

vars == <<integrationState, mainLaunched, rendererLaunched, launchQueue,
  responsePending, responseLaunches, reminder, reminderConsumed, offerCount,
  dismissed, lastSettingsTarget, lastAction, previousIntegrationState,
  previousRendererLaunched, previousReminder, previousReminderConsumed,
  previousOfferCount, previousDismissed>>

SeqSet(sequence) == {sequence[index] : index \in 1..Len(sequence)}

UniqueSequence(sequence) ==
  \A left, right \in 1..Len(sequence):
    left # right => sequence[left] # sequence[right]

CanonicalSubsequence(subsequence, sequence) ==
  subsequence = SelectSeq(sequence, LAMBDA item: item \in SeqSet(subsequence))

MergeLaunches(current, incoming) ==
  current \o SelectSeq(incoming, LAMBDA item: item \notin SeqSet(current))

RemoveLaunch(sequence, adapter) ==
  SelectSeq(sequence, LAMBDA item: item # adapter)

Eligible(adapter) ==
  integrationState[adapter] \in {"not-installed", "disabled", "update-required"}

EligibleRendererLaunches ==
  SelectSeq(
    rendererLaunched,
    LAMBDA adapter: Eligible(adapter) /\ adapter \notin dismissed
  )

Record(action) ==
  /\ lastAction' = action
  /\ previousIntegrationState' = integrationState
  /\ previousRendererLaunched' = rendererLaunched
  /\ previousReminder' = reminder
  /\ previousReminderConsumed' = reminderConsumed
  /\ previousOfferCount' = offerCount
  /\ previousDismissed' = dismissed

Init ==
  /\ integrationState \in [Adapters -> IntegrationStates]
  /\ mainLaunched = <<>>
  /\ rendererLaunched = <<>>
  /\ launchQueue = <<>>
  /\ responsePending = FALSE
  /\ responseLaunches = <<>>
  /\ reminder = None
  /\ reminderConsumed = FALSE
  /\ offerCount = 0
  /\ dismissed \in SUBSET Adapters
  /\ lastSettingsTarget = None
  /\ lastAction = "init"
  /\ previousIntegrationState = integrationState
  /\ previousRendererLaunched = rendererLaunched
  /\ previousReminder = reminder
  /\ previousReminderConsumed = reminderConsumed
  /\ previousOfferCount = offerCount
  /\ previousDismissed = dismissed

ObserveDirectLaunch(adapter) ==
  /\ adapter \in Adapters
  /\ adapter \notin SeqSet(mainLaunched)
  /\ mainLaunched' = Append(mainLaunched, adapter)
  /\ launchQueue' = Append(launchQueue, adapter)
  /\ UNCHANGED <<integrationState, rendererLaunched, responsePending,
    responseLaunches, reminder, reminderConsumed, offerCount, dismissed,
    lastSettingsTarget>>
  /\ Record("observe-direct-launch")

ObserveDuplicateLaunch(adapter) ==
  /\ adapter \in SeqSet(mainLaunched)
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminder, reminderConsumed, offerCount,
    dismissed, lastSettingsTarget>>
  /\ Record("observe-duplicate-launch")

CaptureManagementResponse ==
  /\ ~responsePending
  /\ responsePending' = TRUE
  /\ responseLaunches' = mainLaunched
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    reminder, reminderConsumed, offerCount, dismissed, lastSettingsTarget>>
  /\ Record("capture-management-response")

DeliverLaunchObservation(adapter) ==
  /\ adapter \in SeqSet(launchQueue)
  /\ rendererLaunched' = MergeLaunches(rendererLaunched, <<adapter>>)
  /\ launchQueue' = RemoveLaunch(launchQueue, adapter)
  /\ UNCHANGED <<integrationState, mainLaunched, responsePending,
    responseLaunches, reminder, reminderConsumed, offerCount, dismissed,
    lastSettingsTarget>>
  /\ Record("deliver-launch-observation")

DeliverManagementResponse ==
  /\ responsePending
  /\ rendererLaunched' = MergeLaunches(rendererLaunched, responseLaunches)
  /\ responsePending' = FALSE
  /\ responseLaunches' = <<>>
  /\ UNCHANGED <<integrationState, mainLaunched, launchQueue, reminder,
    reminderConsumed, offerCount, dismissed, lastSettingsTarget>>
  /\ Record("deliver-management-response")

OfferReminder ==
  /\ reminder = None
  /\ ~reminderConsumed
  /\ Len(EligibleRendererLaunches) > 0
  /\ reminder' = Head(EligibleRendererLaunches)
  /\ reminderConsumed' = TRUE
  /\ offerCount' = offerCount + 1
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, dismissed, lastSettingsTarget>>
  /\ Record("offer-reminder")

DismissReminder ==
  /\ reminder # None
  /\ dismissed' = dismissed \cup {reminder}
  /\ reminder' = None
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminderConsumed, offerCount,
    lastSettingsTarget>>
  /\ Record("dismiss-reminder")

OpenIntegrationSettings ==
  /\ reminder # None
  /\ reminder' = None
  /\ lastSettingsTarget' = reminder
  /\ UNCHANGED <<integrationState, mainLaunched, rendererLaunched, launchQueue,
    responsePending, responseLaunches, reminderConsumed, offerCount, dismissed>>
  /\ Record("open-integration-settings")

Next ==
  \/ \E adapter \in Adapters: ObserveDirectLaunch(adapter)
  \/ \E adapter \in Adapters: ObserveDuplicateLaunch(adapter)
  \/ CaptureManagementResponse
  \/ \E adapter \in Adapters: DeliverLaunchObservation(adapter)
  \/ DeliverManagementResponse
  \/ OfferReminder
  \/ DismissReminder
  \/ OpenIntegrationSettings

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
  /\ offerCount \in 0..2
  /\ dismissed \subseteq Adapters
  /\ lastSettingsTarget \in Targets
  /\ UniqueSequence(mainLaunched)
  /\ UniqueSequence(rendererLaunched)
  /\ UniqueSequence(launchQueue)
  /\ UniqueSequence(responseLaunches)
  /\ SeqSet(rendererLaunched) \subseteq SeqSet(mainLaunched)
  /\ SeqSet(launchQueue) \subseteq SeqSet(mainLaunched)
  /\ CanonicalSubsequence(responseLaunches, mainLaunched)
  /\ (responsePending \/ responseLaunches = <<>>)
  /\ lastAction \in {
       "init", "observe-direct-launch", "observe-duplicate-launch",
       "capture-management-response", "deliver-launch-observation",
       "deliver-management-response", "offer-reminder", "dismiss-reminder",
       "open-integration-settings"
     }
  /\ previousIntegrationState \in [Adapters -> IntegrationStates]
  /\ previousRendererLaunched \in Seq(Adapters)
  /\ previousReminder \in Targets
  /\ previousReminderConsumed \in BOOLEAN
  /\ previousOfferCount \in 0..2
  /\ previousDismissed \subseteq Adapters

IntegrationStateIsReadOnly == integrationState = previousIntegrationState

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

AtMostOneReminderOffer == offerCount <= 1

ReminderConsumptionTracksOffer ==
  reminderConsumed <=> offerCount = 1

ReminderOfferWasFresh ==
  lastAction = "offer-reminder" =>
    /\ previousReminder = None
    /\ ~previousReminderConsumed
    /\ previousOfferCount = 0

DismissalIsExactAndReadOnly ==
  lastAction = "dismiss-reminder" =>
    /\ previousReminder # None
    /\ dismissed = previousDismissed \cup {previousReminder}
    /\ integrationState = previousIntegrationState

SettingsNavigationIsReadOnlyAndExact ==
  lastAction = "open-integration-settings" =>
    /\ previousReminder # None
    /\ reminder = None
    /\ lastSettingsTarget = previousReminder
    /\ dismissed = previousDismissed
    /\ integrationState = previousIntegrationState

=============================================================================
