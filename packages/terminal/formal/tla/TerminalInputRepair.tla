--------------------------- MODULE TerminalInputRepair ---------------------------
EXTENDS Naturals, FiniteSets

CONSTANTS Scenario, MaxCapabilityRevision

CompositionPhases == {"idle", "eligible", "cancelled-awaiting-end"}
SettlementStates == {"none", "awaiting-native-end", "beforeinput-observed", "awaiting-beforeinput"}
Correlations == {"key", "composition", "beforeinput", "paste", "mouse"}
Admissions == {"accepting", "repairing", "disposed"}
RepairReasons == {"reset", "textarea-blur", "window-blur", "visibility-hidden", "dispose"}
FocusRepairReasons == {"textarea-blur", "window-blur", "visibility-hidden"}
RepairStages == {"none", "transient-cleared", "keys-released", "mouse-released", "complete"}
ActiveRepairStages == {"transient-cleared", "keys-released", "mouse-released"}
StagedContinuations == {"none", "current", "stale"}
SendStates == {
    "none",
    "key-press",
    "key-release",
    "mouse-press",
    "mouse-release",
    "focus-in",
    "focus-out",
    "stateless"
}
Modes == {
    "composition",
    "key",
    "mouse",
    "focus",
    "review-key",
    "review-mouse",
    "review-focus",
    "review-composition",
    "lifecycle"
}
QueuedOwnershipInputs == {
    "none",
    "key-press",
    "key-release",
    "mouse-press",
    "mouse-release",
    "focus-in"
}
MaxTextPublications == 2

InputActions == {
    "focus",
    "begin-paste-review",
    "resolve-paste-review",
    "reject-paste-review",
    "queue-key-press",
    "queue-key-release",
    "queue-mouse-press",
    "queue-mouse-release",
    "queue-focus-in",
    "begin-queued-send",
    "begin-key-press-send",
    "begin-key-release-send",
    "begin-mouse-press-send",
    "begin-mouse-release-send",
    "begin-focus-send",
    "begin-focus-reconciliation",
    "begin-stateless-send",
    "block-text-publication-focus-mismatch",
    "stage-input-continuation",
    "continue-current-input",
    "start-composition",
    "update-composition",
    "record-key",
    "release-key",
    "record-paste",
    "consume-paste-beforeinput",
    "record-mouse",
    "release-mouse",
    "consume-key-beforeinput",
    "consume-composition-beforeinput",
    "publish-beforeinput-eligible",
    "publish-beforeinput-idle",
    "consume-beforeinput-end",
    "publish-composition-end",
    "consume-composition-origin-beforeinput",
    "end-cancelled-composition"
}

HostInputAttemptActions == {
    "stage-input-continuation",
    "publish-beforeinput-eligible",
    "publish-beforeinput-idle",
    "publish-composition-end",
    "begin-key-press-send",
    "begin-key-release-send",
    "begin-mouse-press-send",
    "begin-mouse-release-send",
    "begin-focus-send",
    "begin-focus-reconciliation",
    "begin-stateless-send",
    "begin-queued-send",
    "begin-repair-key-send",
    "begin-repair-mouse-send",
    "begin-repair-focus-send"
}

VARIABLES
    inputEnabled,
    capabilityRevision,
    phase,
    settlement,
    preeditPublished,
    textareaDirty,
    availableCorrelations,
    lastConsumedCorrelation,
    compositionPublicationCount,
    totalTextPublicationCount,
    keyPressed,
    mousePressed,
    focused,
    localKeyPressed,
    localMousePressed,
    localFocused,
    reviewStalled,
    cancelledReviewPending,
    queuedOwnershipInput,
    needsFocusOut,
    focusOutPublished,
    admission,
    repairStage,
    repairReason,
    disposePending,
    disposed,
    stagedContinuation,
    sendState,
    sendFromRepair,
    lastAction,
    lastAttemptedCapabilityRevision,
    priorPhase,
    priorSettlement,
    priorCompositionPublicationCount,
    priorTotalTextPublicationCount,
    priorFocused,
    priorKeyPressed,
    priorMousePressed,
    priorLocalKeyPressed,
    priorLocalMousePressed,
    priorLocalFocused,
    priorReviewStalled,
    priorCancelledReviewPending,
    priorQueuedOwnershipInput,
    priorNeedsFocusOut,
    priorAdmission,
    priorStagedContinuation,
    priorSendState,
    priorSendFromRepair,
    priorPreeditPublished,
    priorTextareaDirty,
    priorRepairStage,
    priorInputEnabled,
    priorCapabilityRevision,
    priorAttemptedCapabilityRevision

coreRuntimeVars == <<
    phase,
    settlement,
    preeditPublished,
    textareaDirty,
    availableCorrelations,
    lastConsumedCorrelation,
    compositionPublicationCount,
    totalTextPublicationCount,
    keyPressed,
    mousePressed,
    focused,
    needsFocusOut,
    focusOutPublished,
    admission,
    repairStage,
    repairReason,
    disposePending,
    disposed
        >>

dispatcherOwnershipVars == <<
    localKeyPressed,
    localMousePressed,
    localFocused,
    reviewStalled,
    cancelledReviewPending,
    queuedOwnershipInput
        >>

capabilityVars == <<inputEnabled, capabilityRevision>>

sendTransactionVars == <<sendState, sendFromRepair>>

runtimeVars == <<
    coreRuntimeVars,
    capabilityVars,
    stagedContinuation,
    dispatcherOwnershipVars,
    sendTransactionVars
        >>

historyVars == <<
    lastAction,
    lastAttemptedCapabilityRevision,
    priorPhase,
    priorSettlement,
    priorCompositionPublicationCount,
    priorTotalTextPublicationCount,
    priorFocused,
    priorKeyPressed,
    priorMousePressed,
    priorLocalKeyPressed,
    priorLocalMousePressed,
    priorLocalFocused,
    priorReviewStalled,
    priorCancelledReviewPending,
    priorQueuedOwnershipInput,
    priorNeedsFocusOut,
    priorAdmission,
    priorStagedContinuation,
    priorSendState,
    priorSendFromRepair,
    priorPreeditPublished,
    priorTextareaDirty,
    priorRepairStage,
    priorInputEnabled,
    priorCapabilityRevision,
    priorAttemptedCapabilityRevision
        >>

vars == <<runtimeVars, historyVars>>

InputReady == admission = "accepting" /\ repairStage = "none" /\ ~disposed

DispatcherReady ==
    InputReady /\ inputEnabled /\ ~reviewStalled /\
    queuedOwnershipInput = "none" /\ sendState = "none"

FocusConverged == ~localFocused \/ focused

InputPublicationReady == DispatcherReady /\ FocusConverged

ScenarioRepairReasons ==
    IF Scenario = "review-composition"
    THEN {"reset"}
    ELSE RepairReasons \ {"dispose"}

TargetedSendSuccessAllowed ==
    ~(Scenario = "review-composition" /\
      cancelledReviewPending /\
      sendState = "focus-in" /\
      lastAction = "repair-reopen-admission")

RecordHistoryFields(action) ==
    /\ lastAction' = action
    /\ lastAttemptedCapabilityRevision' =
          IF action \in HostInputAttemptActions
          THEN capabilityRevision
          ELSE lastAttemptedCapabilityRevision
    /\ priorPhase' = phase
    /\ priorSettlement' = settlement
    /\ priorCompositionPublicationCount' = compositionPublicationCount
    /\ priorTotalTextPublicationCount' = totalTextPublicationCount
    /\ priorFocused' = focused
    /\ priorKeyPressed' = keyPressed
    /\ priorMousePressed' = mousePressed
    /\ priorLocalKeyPressed' = localKeyPressed
    /\ priorLocalMousePressed' = localMousePressed
    /\ priorLocalFocused' = localFocused
    /\ priorReviewStalled' = reviewStalled
    /\ priorCancelledReviewPending' = cancelledReviewPending
    /\ priorQueuedOwnershipInput' = queuedOwnershipInput
    /\ priorNeedsFocusOut' = needsFocusOut
    /\ priorAdmission' = admission
    /\ priorStagedContinuation' = stagedContinuation
    /\ priorSendState' = sendState
    /\ priorSendFromRepair' = sendFromRepair
    /\ priorPreeditPublished' = preeditPublished
    /\ priorTextareaDirty' = textareaDirty
    /\ priorRepairStage' = repairStage
    /\ priorInputEnabled' = inputEnabled
    /\ priorCapabilityRevision' = capabilityRevision
    /\ priorAttemptedCapabilityRevision' = lastAttemptedCapabilityRevision

RecordHistory(action) ==
    /\ RecordHistoryFields(action)
    /\ UNCHANGED capabilityVars

TypeOK ==
    /\ Scenario \in Modes
    /\ MaxCapabilityRevision \in (Nat \ {0})
    /\ inputEnabled \in BOOLEAN
    /\ capabilityRevision \in 1..MaxCapabilityRevision
    /\ phase \in CompositionPhases
    /\ settlement \in SettlementStates
    /\ preeditPublished \in BOOLEAN
    /\ textareaDirty \in BOOLEAN
    /\ availableCorrelations \subseteq Correlations
    /\ lastConsumedCorrelation \in Correlations \cup {"none"}
    \* The domain deliberately admits 2 so the at-most-one invariant can fail.
    /\ compositionPublicationCount \in 0..2
    /\ totalTextPublicationCount \in 0..MaxTextPublications
    /\ keyPressed \in BOOLEAN
    /\ mousePressed \in BOOLEAN
    /\ focused \in BOOLEAN
    /\ localKeyPressed \in BOOLEAN
    /\ localMousePressed \in BOOLEAN
    /\ localFocused \in BOOLEAN
    /\ reviewStalled \in BOOLEAN
    /\ cancelledReviewPending \in BOOLEAN
    /\ queuedOwnershipInput \in QueuedOwnershipInputs
    /\ needsFocusOut \in BOOLEAN
    /\ focusOutPublished \in BOOLEAN
    /\ admission \in Admissions
    /\ repairStage \in RepairStages
    /\ repairReason \in RepairReasons \cup {"none"}
    /\ disposePending \in BOOLEAN
    /\ disposed \in BOOLEAN
    /\ stagedContinuation \in StagedContinuations
    /\ sendState \in SendStates
    /\ sendFromRepair \in BOOLEAN
    /\ lastAction \in InputActions \cup {
          "init",
          "repair-eligible",
          "repair-settled",
          "repair-plain",
          "coalesce-focus-repair-during-reset",
          "discard-stale-input-continuation",
          "start-dispose-focused",
          "start-dispose-unfocused",
          "start-dispose-after-repair",
          "request-dispose-during-repair",
          "begin-pending-dispose",
          "repair-release-keys",
          "repair-skip-key-release",
          "repair-release-mouse",
          "repair-skip-mouse-release",
          "repair-complete-without-focusout",
          "repair-publish-focusout",
          "begin-repair-key-send",
          "begin-repair-mouse-send",
          "begin-repair-focus-send",
          "send-success",
          "send-failure",
          "repair-key-send-failure",
          "repair-mouse-send-failure",
          "repair-focus-send-failure",
          "repair-reopen-admission",
          "resolve-cancelled-review",
          "dispose-complete",
          "capability-downgrade",
          "capability-upgrade"
        }
    /\ lastAttemptedCapabilityRevision \in 0..MaxCapabilityRevision
    /\ priorPhase \in CompositionPhases
    /\ priorSettlement \in SettlementStates
    /\ priorCompositionPublicationCount \in 0..2
    /\ priorTotalTextPublicationCount \in 0..MaxTextPublications
    /\ priorFocused \in BOOLEAN
    /\ priorKeyPressed \in BOOLEAN
    /\ priorMousePressed \in BOOLEAN
    /\ priorLocalKeyPressed \in BOOLEAN
    /\ priorLocalMousePressed \in BOOLEAN
    /\ priorLocalFocused \in BOOLEAN
    /\ priorReviewStalled \in BOOLEAN
    /\ priorCancelledReviewPending \in BOOLEAN
    /\ priorQueuedOwnershipInput \in QueuedOwnershipInputs
    /\ priorNeedsFocusOut \in BOOLEAN
    /\ priorAdmission \in Admissions
    /\ priorStagedContinuation \in StagedContinuations
    /\ priorSendState \in SendStates
    /\ priorSendFromRepair \in BOOLEAN
    /\ priorPreeditPublished \in BOOLEAN
    /\ priorTextareaDirty \in BOOLEAN
    /\ priorRepairStage \in RepairStages
    /\ priorInputEnabled \in BOOLEAN
    /\ priorCapabilityRevision \in 1..MaxCapabilityRevision
    /\ priorAttemptedCapabilityRevision \in 0..MaxCapabilityRevision

Init ==
    /\ inputEnabled = TRUE
    /\ capabilityRevision = 1
    /\ phase = "idle"
    /\ settlement = "none"
    /\ preeditPublished = FALSE
    /\ textareaDirty = FALSE
    /\ availableCorrelations = {}
    /\ lastConsumedCorrelation = "none"
    /\ compositionPublicationCount = 0
    /\ totalTextPublicationCount = 0
    /\ keyPressed = FALSE
    /\ mousePressed = FALSE
    /\ focused \in BOOLEAN
    /\ localKeyPressed = keyPressed
    /\ localMousePressed = mousePressed
    /\ localFocused = focused
    /\ reviewStalled = FALSE
    /\ cancelledReviewPending = FALSE
    /\ queuedOwnershipInput = "none"
    /\ needsFocusOut = FALSE
    /\ focusOutPublished = FALSE
    /\ admission = "accepting"
    /\ repairStage = "none"
    /\ repairReason = "none"
    /\ disposePending = FALSE
    /\ disposed = FALSE
    /\ stagedContinuation = "none"
    /\ sendState = "none"
    /\ sendFromRepair = FALSE
    /\ lastAction = "init"
    /\ lastAttemptedCapabilityRevision = 0
    /\ priorPhase = phase
    /\ priorSettlement = settlement
    /\ priorCompositionPublicationCount = compositionPublicationCount
    /\ priorTotalTextPublicationCount = totalTextPublicationCount
    /\ priorFocused = focused
    /\ priorKeyPressed = keyPressed
    /\ priorMousePressed = mousePressed
    /\ priorLocalKeyPressed = localKeyPressed
    /\ priorLocalMousePressed = localMousePressed
    /\ priorLocalFocused = localFocused
    /\ priorReviewStalled = reviewStalled
    /\ priorCancelledReviewPending = cancelledReviewPending
    /\ priorQueuedOwnershipInput = queuedOwnershipInput
    /\ priorNeedsFocusOut = needsFocusOut
    /\ priorAdmission = admission
    /\ priorStagedContinuation = stagedContinuation
    /\ priorSendState = sendState
    /\ priorSendFromRepair = sendFromRepair
    /\ priorPreeditPublished = preeditPublished
    /\ priorTextareaDirty = textareaDirty
    /\ priorRepairStage = repairStage
    /\ priorInputEnabled = inputEnabled
    /\ priorCapabilityRevision = capabilityRevision
    /\ priorAttemptedCapabilityRevision = lastAttemptedCapabilityRevision

ApplyCapabilityDowngrade ==
    /\ InputReady
    /\ inputEnabled
    /\ capabilityRevision < MaxCapabilityRevision
    /\ sendState = "none"
    /\ inputEnabled' = FALSE
    /\ capabilityRevision' = capabilityRevision + 1
    /\ phase' = IF phase = "eligible" THEN "cancelled-awaiting-end" ELSE phase
    /\ settlement' =
          IF phase = "eligible" THEN "awaiting-native-end" ELSE settlement
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = {}
    /\ lastConsumedCorrelation' = "none"
    /\ keyPressed' = FALSE
    /\ mousePressed' = FALSE
    /\ focused' = FALSE
    /\ localKeyPressed' = FALSE
    /\ localMousePressed' = FALSE
    /\ localFocused' = FALSE
    /\ reviewStalled' = FALSE
    /\ cancelledReviewPending' =
          IF reviewStalled THEN TRUE ELSE cancelledReviewPending
    /\ queuedOwnershipInput' = "none"
    /\ needsFocusOut' = FALSE
    /\ focusOutPublished' = FALSE
    /\ stagedContinuation' =
          IF stagedContinuation = "current" THEN "stale" ELSE stagedContinuation
    /\ sendState' = "none"
    /\ sendFromRepair' = FALSE
    /\ RecordHistoryFields("capability-downgrade")
    /\ UNCHANGED <<
        compositionPublicationCount,
        totalTextPublicationCount,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ApplyCapabilityUpgrade ==
    /\ InputReady
    /\ ~inputEnabled
    /\ capabilityRevision < MaxCapabilityRevision
    /\ sendState = "none"
    /\ inputEnabled' = TRUE
    /\ capabilityRevision' = capabilityRevision + 1
    /\ RecordHistoryFields("capability-upgrade")
    /\ UNCHANGED <<coreRuntimeVars, stagedContinuation, dispatcherOwnershipVars, sendTransactionVars>>

GainFocus ==
    /\ InputReady
    /\ ~focused
    /\ phase' = IF phase = "cancelled-awaiting-end" THEN "idle" ELSE phase
    /\ focused' = TRUE
    /\ focusOutPublished' = FALSE
    /\ RecordHistory("focus")
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        needsFocusOut,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

StartComposition ==
    /\ InputReady
    /\ phase \in {"idle", "cancelled-awaiting-end"}
    /\ phase' = "eligible"
    /\ settlement' = "none"
    /\ preeditPublished' = TRUE
    /\ textareaDirty' = TRUE
    /\ availableCorrelations' = availableCorrelations \ {"beforeinput", "composition"}
    /\ compositionPublicationCount' = 0
    /\ RecordHistory("start-composition")
    /\ UNCHANGED <<
        stagedContinuation,
        lastConsumedCorrelation,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

UpdateEligibleComposition ==
    /\ InputReady
    /\ phase = "eligible"
    /\ compositionPublicationCount = 0
    /\ preeditPublished' = TRUE
    /\ textareaDirty' = TRUE
    /\ RecordHistory("update-composition")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

RecordKeyInput ==
    /\ InputReady
    /\ phase # "eligible"
    /\ phase' = "idle"
    /\ availableCorrelations' = availableCorrelations \cup {"key"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "key" THEN "none" ELSE lastConsumedCorrelation
    /\ keyPressed' = TRUE
    /\ RecordHistory("record-key")
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ReleaseKeyInput ==
    /\ InputReady
    /\ keyPressed
    /\ keyPressed' = FALSE
    /\ RecordHistory("release-key")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

RecordClipboardPaste ==
    /\ InputReady
    /\ phase = "idle"
    /\ availableCorrelations' = availableCorrelations \cup {"paste"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "paste" THEN "none" ELSE lastConsumedCorrelation
    /\ RecordHistory("record-paste")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ConsumePasteBeforeInputDuplicate ==
    /\ InputReady
    /\ "paste" \in availableCorrelations
    /\ availableCorrelations' = availableCorrelations \ {"paste"}
    /\ lastConsumedCorrelation' = "paste"
    /\ RecordHistory("consume-paste-beforeinput")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

RecordMouseInput ==
    /\ InputReady
    /\ availableCorrelations' = availableCorrelations \cup {"mouse"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "mouse" THEN "none" ELSE lastConsumedCorrelation
    /\ mousePressed' = TRUE
    /\ RecordHistory("record-mouse")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ReleaseMouseInput ==
    /\ InputReady
    /\ mousePressed
    /\ mousePressed' = FALSE
    /\ availableCorrelations' = availableCorrelations \ {"mouse"}
    /\ lastConsumedCorrelation' = "mouse"
    /\ RecordHistory("release-mouse")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ConsumeKeyBeforeInputDuplicate ==
    /\ InputReady
    /\ "key" \in availableCorrelations
    /\ phase' = IF phase = "cancelled-awaiting-end" THEN "idle" ELSE phase
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = availableCorrelations \ {"key"}
    /\ lastConsumedCorrelation' = "key"
    /\ RecordHistory("consume-key-beforeinput")
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        preeditPublished,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ConsumeCompositionBeforeInputDuplicate ==
    /\ InputReady
    /\ "key" \notin availableCorrelations
    /\ "composition" \in availableCorrelations
    /\ phase' = IF phase = "cancelled-awaiting-end" THEN "idle" ELSE phase
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = availableCorrelations \ {"composition"}
    /\ lastConsumedCorrelation' = "composition"
    /\ RecordHistory("consume-composition-beforeinput")
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        preeditPublished,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

PublishBeforeInput(authority, action) ==
    /\ InputReady
    /\ authority \in {"eligible", "idle"}
    /\ "key" \notin availableCorrelations
    /\ "composition" \notin availableCorrelations
    /\ IF authority = "eligible"
          THEN /\ phase = "eligible"
               /\ compositionPublicationCount = 0
          ELSE phase \in {"idle", "cancelled-awaiting-end"}
    /\ totalTextPublicationCount < MaxTextPublications
    /\ phase' = IF authority = "eligible" THEN phase ELSE "idle"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = availableCorrelations \cup {"beforeinput"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "beforeinput" THEN "none" ELSE lastConsumedCorrelation
    /\ compositionPublicationCount' =
          IF authority = "eligible"
          THEN compositionPublicationCount + 1
          ELSE compositionPublicationCount
    /\ totalTextPublicationCount' = totalTextPublicationCount + 1
    /\ RecordHistory(action)
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

PublishEligibleBeforeInput ==
    PublishBeforeInput("eligible", "publish-beforeinput-eligible")

PublishIdleBeforeInput ==
    PublishBeforeInput("idle", "publish-beforeinput-idle")

ConsumeBeforeInputCompositionEndDuplicate ==
    /\ InputReady
    /\ phase = "eligible"
    /\ "beforeinput" \in availableCorrelations
    /\ phase' = "idle"
    /\ settlement' = "none"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = availableCorrelations \ {"beforeinput"}
    /\ lastConsumedCorrelation' = "beforeinput"
    /\ RecordHistory("consume-beforeinput-end")
    /\ UNCHANGED <<
        stagedContinuation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

PublishEligibleCompositionEnd ==
    /\ InputReady
    /\ phase = "eligible"
    /\ compositionPublicationCount = 0
    /\ "beforeinput" \notin availableCorrelations
    /\ totalTextPublicationCount < MaxTextPublications
    /\ phase' = "idle"
    /\ settlement' = "awaiting-beforeinput"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = availableCorrelations \cup {"composition"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "composition" THEN "none" ELSE lastConsumedCorrelation
    /\ compositionPublicationCount' = compositionPublicationCount + 1
    /\ totalTextPublicationCount' = totalTextPublicationCount + 1
    /\ RecordHistory("publish-composition-end")
    /\ UNCHANGED <<
        stagedContinuation,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ConsumeCompositionOriginBeforeInput ==
    /\ InputReady
    /\ phase # "eligible"
    /\ settlement \in {"awaiting-native-end", "awaiting-beforeinput"}
    /\ settlement' =
          IF settlement = "awaiting-native-end" THEN "beforeinput-observed" ELSE "none"
    /\ availableCorrelations' = availableCorrelations \ {"composition"}
    /\ lastConsumedCorrelation' =
          IF "composition" \in availableCorrelations
          THEN "composition"
          ELSE lastConsumedCorrelation
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ RecordHistory("consume-composition-origin-beforeinput")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

EndCancelledComposition ==
    /\ InputReady
    /\ (phase = "cancelled-awaiting-end" \/
        (phase = "idle" /\ settlement \in {"awaiting-native-end", "beforeinput-observed"}))
    /\ phase' = "idle"
    /\ settlement' =
          IF settlement = "beforeinput-observed" THEN "none" ELSE "awaiting-beforeinput"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ RecordHistory("end-cancelled-composition")
    /\ UNCHANGED <<
        stagedContinuation,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

StartRepair(reason, action) ==
    /\ InputReady
    /\ reason \in (RepairReasons \ {"dispose"})
    /\ admission' = "repairing"
    /\ phase' = IF phase = "eligible" THEN "cancelled-awaiting-end" ELSE phase
    /\ settlement' =
          IF phase = "eligible" THEN "awaiting-native-end" ELSE settlement
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = {}
    /\ lastConsumedCorrelation' = "none"
    /\ needsFocusOut' = (focused /\ reason # "reset")
    /\ focusOutPublished' = FALSE
    /\ repairStage' = "transient-cleared"
    /\ repairReason' = reason
    /\ disposePending' = FALSE
    /\ stagedContinuation' =
          IF stagedContinuation = "current" THEN "stale" ELSE stagedContinuation
    /\ RecordHistory(action)
    /\ UNCHANGED <<
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        disposed
        >>

StartEligibleRepair(reason) ==
    /\ phase = "eligible"
    /\ StartRepair(reason, "repair-eligible")

StartSettledRepair(reason) ==
    /\ phase # "eligible"
    /\ settlement # "none"
    /\ StartRepair(reason, "repair-settled")

StartPlainRepair(reason) ==
    /\ phase # "eligible"
    /\ settlement = "none"
    /\ StartRepair(reason, "repair-plain")

StartDispose(action) ==
    /\ ~disposed
    /\ ((InputReady) \/
        (admission = "repairing" /\ repairStage = "complete" /\
         repairReason \in FocusRepairReasons /\ ~disposePending))
    /\ admission' = "disposed"
    /\ phase' = "idle"
    /\ settlement' = "none"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = {}
    /\ lastConsumedCorrelation' = "none"
    /\ needsFocusOut' = focused
    /\ focusOutPublished' = FALSE
    /\ repairStage' = "transient-cleared"
    /\ repairReason' = "dispose"
    /\ disposePending' = FALSE
    /\ stagedContinuation' =
          IF stagedContinuation = "current" THEN "stale" ELSE stagedContinuation
    /\ RecordHistory(action)
    /\ UNCHANGED <<
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        disposed
        >>

StartFocusedDispose ==
    /\ InputReady
    /\ focused
    /\ StartDispose("start-dispose-focused")

StartUnfocusedDispose ==
    /\ InputReady
    /\ ~focused
    /\ StartDispose("start-dispose-unfocused")

StartDisposeAfterCompletedFocusRepair ==
    /\ admission = "repairing"
    /\ repairStage = "complete"
    /\ repairReason \in FocusRepairReasons
    /\ StartDispose("start-dispose-after-repair")

StartCoalescedResetFocusRepair ==
    /\ InputReady
    /\ focused
    /\ StartRepair("window-blur", "coalesce-focus-repair-during-reset")

CoalesceFocusRepairDuringActiveReset ==
    /\ admission = "repairing"
    /\ repairReason = "reset"
    /\ repairStage \in (ActiveRepairStages \cup {"complete"})
    /\ focused
    /\ needsFocusOut' = TRUE
    /\ focusOutPublished' = FALSE
    /\ repairReason' = "window-blur"
    /\ repairStage' = IF repairStage = "complete" THEN "mouse-released" ELSE repairStage
    /\ RecordHistory("coalesce-focus-repair-during-reset")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        admission,
        disposePending,
        disposed
        >>

RequestDisposeDuringRepair ==
    /\ admission = "repairing"
    /\ repairStage \in ActiveRepairStages
    /\ repairReason # "dispose"
    /\ admission' = "disposed"
    /\ phase' = "idle"
    /\ settlement' = "none"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ availableCorrelations' = {}
    /\ lastConsumedCorrelation' = "none"
    /\ needsFocusOut' = focused
    /\ focusOutPublished' = FALSE
    /\ repairReason' = "dispose"
    /\ disposePending' = TRUE
    /\ RecordHistory("request-dispose-during-repair")
    /\ UNCHANGED <<
        stagedContinuation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        repairStage,
        disposed
        >>

BeginPendingDispose ==
    /\ admission = "disposed"
    /\ disposePending
    /\ repairStage = "complete"
    /\ disposePending' = FALSE
    /\ disposed' = TRUE
    /\ RecordHistory("begin-pending-dispose")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason
        >>

PublishRepairKeyRelease ==
    /\ repairStage = "transient-cleared"
    /\ keyPressed
    /\ sendState = "none"
    /\ sendState' = "key-release"
    /\ sendFromRepair' = TRUE
    /\ RecordHistory("begin-repair-key-send")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

SkipRepairKeyRelease ==
    /\ repairStage = "transient-cleared"
    /\ sendState = "none"
    /\ ~keyPressed
    /\ keyPressed' = FALSE
    /\ repairStage' = "keys-released"
    /\ RecordHistory("repair-skip-key-release")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairReason,
        disposePending,
        disposed
        >>

ReleaseRepairKeys == PublishRepairKeyRelease \/ SkipRepairKeyRelease

PublishRepairMouseRelease ==
    /\ repairStage = "keys-released"
    /\ mousePressed
    /\ sendState = "none"
    /\ sendState' = "mouse-release"
    /\ sendFromRepair' = TRUE
    /\ RecordHistory("begin-repair-mouse-send")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

SkipRepairMouseRelease ==
    /\ repairStage = "keys-released"
    /\ sendState = "none"
    /\ ~mousePressed
    /\ mousePressed' = FALSE
    /\ repairStage' = "mouse-released"
    /\ RecordHistory("repair-skip-mouse-release")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairReason,
        disposePending,
        disposed
        >>

ReleaseRepairMouse == PublishRepairMouseRelease \/ SkipRepairMouseRelease

CompleteRepairWithoutFocusOut ==
    /\ repairStage = "mouse-released"
    /\ sendState = "none"
    /\ ~needsFocusOut
    /\ repairStage' = "complete"
    /\ RecordHistory("repair-complete-without-focusout")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairReason,
        disposePending,
        disposed
        >>

PublishFocusOut ==
    /\ repairStage = "mouse-released"
    /\ needsFocusOut
    /\ focused
    /\ repairReason \in FocusRepairReasons \cup {"dispose"}
    /\ sendState = "none"
    /\ sendState' = "focus-out"
    /\ sendFromRepair' = TRUE
    /\ RecordHistory("begin-repair-focus-send")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ReopenInputAdmission ==
    /\ admission = "repairing"
    /\ repairStage = "complete"
    /\ repairReason # "dispose"
    /\ ~disposePending
    /\ sendState = "none"
    /\ admission' = "accepting"
    /\ repairStage' = "none"
    /\ repairReason' = "none"
    /\ focusOutPublished' = FALSE
    /\ sendState' = IF localFocused /\ ~focused THEN "focus-in" ELSE "none"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("repair-reopen-admission")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        disposePending,
        disposed
        >>

CompleteDispose ==
    /\ admission = "disposed"
    /\ repairStage = "complete"
    /\ repairReason = "dispose"
    /\ ~disposePending
    /\ ~needsFocusOut
    /\ sendState = "none"
    /\ disposed' = TRUE
    /\ RecordHistory("dispose-complete")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending
        >>

KeepDispatcherOwnership(action) ==
    /\ action
    /\ UNCHANGED dispatcherOwnershipVars
    /\ UNCHANGED sendTransactionVars

PreserveDispatcherOwnership(action) ==
    /\ action
    /\ UNCHANGED dispatcherOwnershipVars

DispatchAction(action) ==
    /\ DispatcherReady
    /\ action
    /\ UNCHANGED dispatcherOwnershipVars
    /\ UNCHANGED sendTransactionVars

PublishInputAction(action) ==
    /\ InputPublicationReady
    /\ action
    /\ UNCHANGED dispatcherOwnershipVars
    /\ UNCHANGED sendTransactionVars

PublishGainFocus ==
    /\ DispatcherReady
    /\ GainFocus
    /\ localFocused' = TRUE
    /\ UNCHANGED <<
        localKeyPressed,
        localMousePressed,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput
        >>

PublishKeyInput ==
    /\ DispatcherReady
    /\ RecordKeyInput
    /\ localKeyPressed' = TRUE
    /\ UNCHANGED <<
        localMousePressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput
        >>

PublishKeyRelease ==
    /\ DispatcherReady
    /\ ReleaseKeyInput
    /\ localKeyPressed' = FALSE
    /\ UNCHANGED <<
        localMousePressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput
        >>

PublishMouseInput ==
    /\ DispatcherReady
    /\ RecordMouseInput
    /\ localMousePressed' = TRUE
    /\ UNCHANGED <<
        localKeyPressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput
        >>

PublishMouseRelease ==
    /\ DispatcherReady
    /\ ReleaseMouseInput
    /\ localMousePressed' = FALSE
    /\ UNCHANGED <<
        localKeyPressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput
        >>

BeginKeyPressSend ==
    /\ DispatcherReady
    /\ FocusConverged
    /\ phase # "eligible"
    /\ ~localKeyPressed
    /\ phase' = "idle"
    /\ availableCorrelations' = availableCorrelations \cup {"key"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "key" THEN "none" ELSE lastConsumedCorrelation
    /\ localKeyPressed' = TRUE
    /\ sendState' = "key-press"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-key-press-send")
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        localMousePressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

BeginKeyReleaseSend ==
    /\ DispatcherReady
    /\ FocusConverged
    /\ localKeyPressed
    /\ keyPressed
    /\ localKeyPressed' = FALSE
    /\ sendState' = "key-release"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-key-release-send")
    /\ UNCHANGED <<
        coreRuntimeVars,
        stagedContinuation,
        localMousePressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput
        >>

BeginMousePressSend ==
    /\ DispatcherReady
    /\ FocusConverged
    /\ ~localMousePressed
    /\ availableCorrelations' = availableCorrelations \cup {"mouse"}
    /\ lastConsumedCorrelation' =
          IF lastConsumedCorrelation = "mouse" THEN "none" ELSE lastConsumedCorrelation
    /\ localMousePressed' = TRUE
    /\ sendState' = "mouse-press"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-mouse-press-send")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        localKeyPressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

BeginMouseReleaseSend ==
    /\ DispatcherReady
    /\ FocusConverged
    /\ localMousePressed
    /\ mousePressed
    /\ localMousePressed' = FALSE
    /\ availableCorrelations' = availableCorrelations \ {"mouse"}
    /\ lastConsumedCorrelation' = "mouse"
    /\ sendState' = "mouse-release"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-mouse-release-send")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        localKeyPressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

BeginFocusSend ==
    /\ DispatcherReady
    /\ ~localFocused
    /\ ~focused
    /\ localFocused' = TRUE
    /\ phase' = IF phase = "cancelled-awaiting-end" THEN "idle" ELSE phase
    /\ sendState' = "focus-in"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-focus-send")
    /\ UNCHANGED <<
        stagedContinuation,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        localKeyPressed,
        localMousePressed,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

BeginFocusReconciliation ==
    /\ DispatcherReady
    /\ localFocused
    /\ ~focused
    /\ sendState' = "focus-in"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-focus-reconciliation")
    /\ UNCHANGED <<coreRuntimeVars, stagedContinuation, dispatcherOwnershipVars>>

BeginStatelessSend ==
    /\ DispatcherReady
    /\ FocusConverged
    /\ sendState' = "stateless"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-stateless-send")
    /\ UNCHANGED <<coreRuntimeVars, stagedContinuation, dispatcherOwnershipVars>>

CompleteSendSuccess ==
    /\ sendState # "none"
    /\ TargetedSendSuccessAllowed
    /\ keyPressed' =
          IF sendState = "key-press" THEN TRUE
          ELSE IF sendState = "key-release" THEN FALSE
          ELSE keyPressed
    /\ mousePressed' =
          IF sendState = "mouse-press" THEN TRUE
          ELSE IF sendState = "mouse-release" THEN FALSE
          ELSE mousePressed
    /\ focused' =
          IF sendState = "focus-in" THEN TRUE
          ELSE IF sendState = "focus-out" THEN FALSE
          ELSE focused
    /\ needsFocusOut' =
          IF sendFromRepair /\ sendState = "focus-out" THEN FALSE ELSE needsFocusOut
    /\ focusOutPublished' =
          IF sendFromRepair /\ sendState = "focus-out" THEN TRUE
          ELSE IF sendState = "focus-in" THEN FALSE
          ELSE focusOutPublished
    /\ repairStage' =
          IF sendFromRepair /\ sendState = "key-release" THEN "keys-released"
          ELSE IF sendFromRepair /\ sendState = "mouse-release" THEN "mouse-released"
          ELSE IF sendFromRepair /\ sendState = "focus-out" THEN "complete"
          ELSE repairStage
    /\ sendState' = "none"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory(
          IF sendFromRepair /\ sendState = "key-release" THEN "repair-release-keys"
          ELSE IF sendFromRepair /\ sendState = "mouse-release" THEN "repair-release-mouse"
          ELSE IF sendFromRepair /\ sendState = "focus-out" THEN "repair-publish-focusout"
          ELSE "send-success")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        localKeyPressed,
        localMousePressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput,
        admission,
        repairReason,
        disposePending,
        disposed
        >>

CompleteSendFailure ==
    /\ sendState # "none"
    /\ availableCorrelations' =
          IF ~sendFromRepair /\ sendState = "key-press"
          THEN availableCorrelations \ {"key"}
          ELSE IF ~sendFromRepair /\ sendState = "mouse-press"
          THEN availableCorrelations \ {"mouse"}
          ELSE availableCorrelations
    /\ localKeyPressed' =
          IF ~sendFromRepair /\ sendState = "key-press" THEN FALSE ELSE localKeyPressed
    /\ localMousePressed' =
          IF ~sendFromRepair /\ sendState = "mouse-press" THEN FALSE ELSE localMousePressed
    /\ needsFocusOut' =
          IF sendFromRepair /\ sendState = "focus-out" THEN FALSE ELSE needsFocusOut
    /\ repairStage' =
          IF sendFromRepair /\ sendState = "key-release" THEN "keys-released"
          ELSE IF sendFromRepair /\ sendState = "mouse-release" THEN "mouse-released"
          ELSE IF sendFromRepair /\ sendState = "focus-out" THEN "complete"
          ELSE repairStage
    /\ sendState' = "none"
    /\ sendFromRepair' = FALSE
    /\ RecordHistory(
          IF sendFromRepair /\ sendState = "key-release" THEN "repair-key-send-failure"
          ELSE IF sendFromRepair /\ sendState = "mouse-release" THEN "repair-mouse-send-failure"
          ELSE IF sendFromRepair /\ sendState = "focus-out" THEN "repair-focus-send-failure"
          ELSE "send-failure")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        queuedOwnershipInput,
        focusOutPublished,
        admission,
        repairReason,
        disposePending,
        disposed
        >>

CompleteSend == CompleteSendSuccess \/ CompleteSendFailure

BeginPasteReview ==
    /\ Scenario \in {"review-key", "review-mouse", "review-focus", "review-composition"}
    /\ DispatcherReady
    /\ stagedContinuation = "none"
    /\ reviewStalled' = TRUE
    /\ RecordHistory("begin-paste-review")
    /\ UNCHANGED <<
        coreRuntimeVars,
        stagedContinuation,
        localKeyPressed,
        localMousePressed,
        localFocused,
        cancelledReviewPending,
        queuedOwnershipInput,
        sendTransactionVars
        >>

ResolvePasteReview ==
    /\ Scenario \in {"review-key", "review-mouse", "review-focus", "review-composition"}
    /\ InputReady
    /\ reviewStalled
    /\ reviewStalled' = FALSE
    /\ RecordHistory("resolve-paste-review")
    /\ UNCHANGED <<
        coreRuntimeVars,
        stagedContinuation,
        localKeyPressed,
        localMousePressed,
        localFocused,
        cancelledReviewPending,
        queuedOwnershipInput,
        sendTransactionVars
        >>

RejectPasteReview ==
    /\ Scenario \in {"review-key", "review-mouse", "review-focus", "review-composition"}
    /\ InputReady
    /\ reviewStalled
    /\ reviewStalled' = FALSE
    /\ RecordHistory("reject-paste-review")
    /\ UNCHANGED <<
        coreRuntimeVars,
        stagedContinuation,
        localKeyPressed,
        localMousePressed,
        localFocused,
        cancelledReviewPending,
        queuedOwnershipInput,
        sendTransactionVars
        >>

QueueOwnershipInput(kind, action) ==
    /\ InputReady
    /\ reviewStalled
    /\ queuedOwnershipInput = "none"
    /\ kind \in (QueuedOwnershipInputs \ {"none"})
    /\ queuedOwnershipInput' = kind
    /\ RecordHistory(action)
    /\ UNCHANGED <<
        coreRuntimeVars,
        stagedContinuation,
        cancelledReviewPending,
        reviewStalled,
        sendTransactionVars
        >>

QueueKeyPress ==
    /\ Scenario = "review-key"
    /\ QueueOwnershipInput("key-press", "queue-key-press")
    /\ ~localKeyPressed
    /\ ~keyPressed
    /\ localKeyPressed' = TRUE
    /\ UNCHANGED <<localMousePressed, localFocused>>

QueueKeyRelease ==
    /\ Scenario = "review-key"
    /\ QueueOwnershipInput("key-release", "queue-key-release")
    /\ localKeyPressed
    /\ keyPressed
    /\ localKeyPressed' = FALSE
    /\ UNCHANGED <<localMousePressed, localFocused>>

QueueMousePress ==
    /\ Scenario = "review-mouse"
    /\ QueueOwnershipInput("mouse-press", "queue-mouse-press")
    /\ ~localMousePressed
    /\ ~mousePressed
    /\ localMousePressed' = TRUE
    /\ UNCHANGED <<localKeyPressed, localFocused>>

QueueMouseRelease ==
    /\ Scenario = "review-mouse"
    /\ QueueOwnershipInput("mouse-release", "queue-mouse-release")
    /\ localMousePressed
    /\ mousePressed
    /\ localMousePressed' = FALSE
    /\ UNCHANGED <<localKeyPressed, localFocused>>

QueueFocusIn ==
    /\ Scenario \in {"review-focus", "review-composition"}
    /\ QueueOwnershipInput("focus-in", "queue-focus-in")
    /\ ~localFocused
    /\ ~focused
    /\ localFocused' = TRUE
    /\ UNCHANGED <<localKeyPressed, localMousePressed>>

PublishQueuedOwnershipBase(kind) ==
    /\ InputReady
    /\ ~reviewStalled
    /\ queuedOwnershipInput = kind
    /\ sendState = "none"
    /\ queuedOwnershipInput' = "none"
    /\ sendState' = kind
    /\ sendFromRepair' = FALSE
    /\ RecordHistory("begin-queued-send")
    /\ UNCHANGED <<
        stagedContinuation,
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        localKeyPressed,
        localMousePressed,
        localFocused,
        reviewStalled,
        cancelledReviewPending,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

PublishQueuedKeyPress ==
    /\ PublishQueuedOwnershipBase("key-press")
    /\ UNCHANGED <<keyPressed, mousePressed, focused>>

PublishQueuedKeyRelease ==
    /\ PublishQueuedOwnershipBase("key-release")
    /\ UNCHANGED <<keyPressed, mousePressed, focused>>

PublishQueuedMousePress ==
    /\ PublishQueuedOwnershipBase("mouse-press")
    /\ UNCHANGED <<keyPressed, mousePressed, focused>>

PublishQueuedMouseRelease ==
    /\ PublishQueuedOwnershipBase("mouse-release")
    /\ UNCHANGED <<keyPressed, mousePressed, focused>>

PublishQueuedFocusIn ==
    /\ PublishQueuedOwnershipBase("focus-in")
    /\ UNCHANGED <<keyPressed, mousePressed, focused>>

PublishQueuedOwnershipInput ==
    \/ PublishQueuedKeyPress
    \/ PublishQueuedKeyRelease
    \/ PublishQueuedMousePress
    \/ PublishQueuedMouseRelease
    \/ PublishQueuedFocusIn

ResolveCancelledReview ==
    /\ Scenario \in {"review-key", "review-mouse", "review-focus", "review-composition"}
    /\ cancelledReviewPending
    /\ cancelledReviewPending' = FALSE
    /\ RecordHistory("resolve-cancelled-review")
    /\ UNCHANGED <<
        coreRuntimeVars,
        stagedContinuation,
        localKeyPressed,
        localMousePressed,
        localFocused,
        reviewStalled,
        queuedOwnershipInput,
        sendTransactionVars
        >>

InvalidateDispatcherForLifecycle(action) ==
    /\ action
    /\ localKeyPressed' = FALSE
    /\ localMousePressed' = FALSE
    /\ localFocused' = IF repairReason' = "reset" THEN localFocused ELSE FALSE
    /\ reviewStalled' = FALSE
    /\ cancelledReviewPending' = (cancelledReviewPending \/ reviewStalled)
    /\ queuedOwnershipInput' = "none"
    /\ UNCHANGED sendTransactionVars

CoverageCompositionAuthorityAntecedent ==
    /\ compositionPublicationCount > priorCompositionPublicationCount
    /\ UNCHANGED vars

CoverageExplicitPublicationOriginAntecedent ==
    /\ totalTextPublicationCount > priorTotalTextPublicationCount
    /\ UNCHANGED vars

CoverageEligibleRepairGuardAntecedent ==
    /\ lastAction = "repair-eligible"
    /\ UNCHANGED vars

CoverageSettledRepairGuardAntecedent ==
    /\ lastAction = "repair-settled"
    /\ UNCHANGED vars

CoverageFocusOutOwnershipAntecedent ==
    /\ lastAction = "repair-publish-focusout"
    /\ UNCHANGED vars

CoverageUnfocusedDisposeAntecedent ==
    /\ lastAction = "start-dispose-unfocused"
    /\ UNCHANGED vars

CoverageDisposeAfterFocusRepairAntecedent ==
    /\ lastAction = "start-dispose-after-repair"
    /\ UNCHANGED vars

CoveragePendingDisposeRequestAntecedent ==
    /\ lastAction = "request-dispose-during-repair"
    /\ UNCHANGED vars

CoveragePendingDisposeBeginAntecedent ==
    /\ lastAction = "begin-pending-dispose"
    /\ UNCHANGED vars

CoverageFocusedActiveResetState ==
    /\ admission = "repairing"
    /\ repairReason = "reset"
    /\ repairStage \in (ActiveRepairStages \cup {"complete"})
    /\ focused
    /\ UNCHANGED vars

CoverageStartCoalescedResetFocusRepair ==
    /\ lastAction = "coalesce-focus-repair-during-reset"
    /\ priorAdmission = "accepting"
    /\ UNCHANGED vars

CoverageCoalesceFocusRepairDuringActiveReset ==
    /\ lastAction = "coalesce-focus-repair-during-reset"
    /\ priorAdmission = "repairing"
    /\ UNCHANGED vars

CoverageStagedInvalidationAntecedent ==
    /\ priorStagedContinuation = "current"
    /\ stagedContinuation = "stale"
    /\ UNCHANGED vars

CoverageCurrentContinuationAntecedent ==
    /\ lastAction = "continue-current-input"
    /\ UNCHANGED vars

CoverageStaleDiscardAntecedent ==
    /\ lastAction = "discard-stale-input-continuation"
    /\ UNCHANGED vars

CoverageStalledReviewState ==
    /\ reviewStalled
    /\ UNCHANGED vars

CoverageQueuedKeyPressMismatch ==
    /\ queuedOwnershipInput = "key-press"
    /\ localKeyPressed
    /\ ~keyPressed
    /\ UNCHANGED vars

CoverageQueuedKeyReleaseMismatch ==
    /\ queuedOwnershipInput = "key-release"
    /\ ~localKeyPressed
    /\ keyPressed
    /\ UNCHANGED vars

CoverageQueuedMousePressMismatch ==
    /\ queuedOwnershipInput = "mouse-press"
    /\ localMousePressed
    /\ ~mousePressed
    /\ UNCHANGED vars

CoverageQueuedMouseReleaseMismatch ==
    /\ queuedOwnershipInput = "mouse-release"
    /\ ~localMousePressed
    /\ mousePressed
    /\ UNCHANGED vars

CoverageQueuedFocusMismatch ==
    /\ queuedOwnershipInput = "focus-in"
    /\ localFocused
    /\ ~focused
    /\ UNCHANGED vars

CoveragePublishedKeyCleanupAntecedent ==
    /\ lastAction = "repair-release-keys"
    /\ priorKeyPressed
    /\ UNCHANGED vars

CoverageSkippedKeyCleanupAntecedent ==
    /\ lastAction = "repair-skip-key-release"
    /\ ~priorKeyPressed
    /\ UNCHANGED vars

CoveragePublishedMouseCleanupAntecedent ==
    /\ lastAction = "repair-release-mouse"
    /\ priorMousePressed
    /\ UNCHANGED vars

CoverageSkippedMouseCleanupAntecedent ==
    /\ lastAction = "repair-skip-mouse-release"
    /\ ~priorMousePressed
    /\ UNCHANGED vars

CoverageCancelledReviewResolution ==
    /\ lastAction = "resolve-cancelled-review"
    /\ UNCHANGED vars

CoverageCancelledReviewBypassState ==
    /\ cancelledReviewPending
    /\ DispatcherReady
    /\ UNCHANGED vars

CoverageStatelessSendSuccess ==
    /\ lastAction = "send-success"
    /\ priorSendState = "stateless"
    /\ UNCHANGED vars

CoverageSendFailure ==
    /\ lastAction = "send-failure"
    /\ priorSendState \in {"key-press", "key-release", "mouse-press", "mouse-release", "focus-in"}
    /\ UNCHANGED vars

CoverageFailedKeyPressRollback ==
    /\ lastAction = "send-failure"
    /\ priorSendState = "key-press"
    /\ ~localKeyPressed
    /\ "key" \notin availableCorrelations
    /\ UNCHANGED vars

CoverageFailedMousePressRollback ==
    /\ lastAction = "send-failure"
    /\ priorSendState = "mouse-press"
    /\ ~localMousePressed
    /\ "mouse" \notin availableCorrelations
    /\ UNCHANGED vars

CoveragePublishedKeyReleaseAntecedent ==
    /\ lastAction = "begin-key-release-send"
    /\ priorKeyPressed
    /\ UNCHANGED vars

CoveragePublishedMouseReleaseAntecedent ==
    /\ lastAction = "begin-mouse-release-send"
    /\ priorMousePressed
    /\ UNCHANGED vars

CoverageReentrantRepairDuringSend ==
    /\ admission # "accepting"
    /\ sendState # "none"
    /\ UNCHANGED vars

CoverageFocusReconciliation ==
    /\ lastAction = "begin-focus-reconciliation"
    /\ priorLocalFocused
    /\ ~priorFocused
    /\ UNCHANGED vars

CoverageRepairReopenFocusReconciliation ==
    /\ lastAction = "repair-reopen-admission"
    /\ priorLocalFocused
    /\ ~priorFocused
    /\ sendState = "focus-in"
    /\ UNCHANGED vars

CoveragePasteReviewRejection ==
    /\ lastAction = "reject-paste-review"
    /\ ~reviewStalled
    /\ UNCHANGED vars

CoverageRepairSendFailure ==
    /\ lastAction \in
          {"repair-key-send-failure", "repair-mouse-send-failure", "repair-focus-send-failure"}
    /\ UNCHANGED vars

CoverageCurrentContinuationEffect ==
    /\ lastAction = "continue-current-input"
    /\ priorPreeditPublished
    /\ priorTextareaDirty
    /\ ~preeditPublished
    /\ ~textareaDirty
    /\ UNCHANGED vars

CoverageStaleContinuationEffect ==
    /\ lastAction = "discard-stale-input-continuation"
    /\ preeditPublished = priorPreeditPublished
    /\ textareaDirty = priorTextareaDirty
    /\ UNCHANGED vars

CoverageReviewCompositionResetFocusFailure ==
    /\ Scenario = "review-composition"
    /\ lastAction = "send-failure"
    /\ priorSendState = "focus-in"
    /\ cancelledReviewPending
    /\ DispatcherReady
    /\ localFocused
    /\ ~focused
    /\ UNCHANGED vars

CoverageLocalCompositionProgressWithFocusMismatch ==
    /\ Scenario = "review-composition"
    /\ lastAction = "start-composition"
    /\ cancelledReviewPending
    /\ phase = "eligible"
    /\ localFocused
    /\ ~focused
    /\ UNCHANGED vars

CoverageLocalSettlementWithFocusMismatch ==
    /\ Scenario = "review-composition"
    /\ lastAction = "end-cancelled-composition"
    /\ priorLocalFocused
    /\ ~priorFocused
    /\ totalTextPublicationCount = priorTotalTextPublicationCount
    /\ UNCHANGED vars

CoveragePostConvergenceCompositionPublication ==
    /\ Scenario = "review-composition"
    /\ lastAction = "stage-input-continuation"
    /\ cancelledReviewPending
    /\ totalTextPublicationCount > priorTotalTextPublicationCount
    /\ priorLocalFocused
    /\ priorFocused
    /\ UNCHANGED vars

BlockTextPublicationOnFocusMismatch ==
    /\ Scenario = "review-composition"
    /\ DispatcherReady
    /\ cancelledReviewPending
    /\ localFocused
    /\ ~focused
    /\ phase = "eligible"
    /\ stagedContinuation = "none"
    /\ compositionPublicationCount = 0
    /\ totalTextPublicationCount < MaxTextPublications
    /\ RecordHistory("block-text-publication-focus-mismatch")
    /\ UNCHANGED runtimeVars

StageInputContinuation ==
    /\ InputReady
    /\ stagedContinuation = "none"
    /\ phase = "eligible"
    /\ compositionPublicationCount = 0
    /\ totalTextPublicationCount < MaxTextPublications
    /\ stagedContinuation' = "current"
    /\ compositionPublicationCount' = compositionPublicationCount + 1
    /\ totalTextPublicationCount' = totalTextPublicationCount + 1
    /\ RecordHistory("stage-input-continuation")
    /\ UNCHANGED <<
        phase,
        settlement,
        preeditPublished,
        textareaDirty,
        availableCorrelations,
        lastConsumedCorrelation,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

ContinueCurrentInput ==
    /\ InputReady
    /\ stagedContinuation = "current"
    /\ stagedContinuation' = "none"
    /\ preeditPublished' = FALSE
    /\ textareaDirty' = FALSE
    /\ RecordHistory("continue-current-input")
    /\ UNCHANGED <<
        phase,
        settlement,
        availableCorrelations,
        lastConsumedCorrelation,
        compositionPublicationCount,
        totalTextPublicationCount,
        keyPressed,
        mousePressed,
        focused,
        needsFocusOut,
        focusOutPublished,
        admission,
        repairStage,
        repairReason,
        disposePending,
        disposed
        >>

DiscardStaleInputContinuation ==
    /\ stagedContinuation = "stale"
    /\ stagedContinuation' = "none"
    /\ RecordHistory("discard-stale-input-continuation")
    /\ UNCHANGED coreRuntimeVars

ModeStep ==
    \/ ApplyCapabilityDowngrade
    \/ ApplyCapabilityUpgrade
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ PublishInputAction(StageInputContinuation)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ DispatchAction(ContinueCurrentInput)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ KeepDispatcherOwnership(DiscardStaleInputContinuation)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ DispatchAction(StartComposition)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ DispatchAction(UpdateEligibleComposition)
    \/ /\ Scenario \in {"focus", "review-focus", "lifecycle"}
       /\ BeginFocusSend
    \/ /\ Scenario \in {"focus", "review-focus", "review-composition", "lifecycle"}
       /\ BeginFocusReconciliation
    \/ /\ Scenario \in {"key", "review-key", "lifecycle"}
       /\ BeginKeyPressSend
    \/ /\ Scenario \in {"key", "review-key", "lifecycle"}
       /\ BeginKeyReleaseSend
    \/ /\ Scenario \in {"composition", "review-key", "review-mouse", "review-focus"}
       /\ DispatchAction(RecordClipboardPaste)
    \/ /\ Scenario \in {"composition", "review-key", "review-mouse", "review-focus"}
       /\ DispatchAction(ConsumePasteBeforeInputDuplicate)
    \/ /\ Scenario \in {"mouse", "review-mouse", "lifecycle"}
       /\ BeginMousePressSend
    \/ /\ Scenario \in {"mouse", "review-mouse", "lifecycle"}
       /\ BeginMouseReleaseSend
    \/ /\ Scenario = "key"
       /\ BeginStatelessSend
    \/ /\ Scenario = "composition"
       /\ DispatchAction(ConsumeKeyBeforeInputDuplicate)
    \/ /\ Scenario = "composition"
       /\ DispatchAction(ConsumeCompositionBeforeInputDuplicate)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ PublishInputAction(PublishEligibleBeforeInput)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ PublishInputAction(PublishIdleBeforeInput)
    \/ /\ Scenario = "composition"
       /\ DispatchAction(ConsumeBeforeInputCompositionEndDuplicate)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ PublishInputAction(PublishEligibleCompositionEnd)
    \/ /\ Scenario = "composition"
       /\ DispatchAction(ConsumeCompositionOriginBeforeInput)
    \/ /\ Scenario \in {"composition", "review-composition"}
       /\ DispatchAction(EndCancelledComposition)
    \/ BeginPasteReview
    \/ BlockTextPublicationOnFocusMismatch
    \/ ResolvePasteReview
    \/ RejectPasteReview
    \/ ResolveCancelledReview
    \/ QueueKeyPress
    \/ QueueKeyRelease
    \/ QueueMousePress
    \/ QueueMouseRelease
    \/ QueueFocusIn
    \/ PublishQueuedOwnershipInput
    \/ CompleteSend
    \/ \E reason \in ScenarioRepairReasons :
          InvalidateDispatcherForLifecycle(StartEligibleRepair(reason))
    \/ \E reason \in ScenarioRepairReasons :
          InvalidateDispatcherForLifecycle(StartSettledRepair(reason))
    \/ \E reason \in ScenarioRepairReasons :
          InvalidateDispatcherForLifecycle(StartPlainRepair(reason))
    \/ InvalidateDispatcherForLifecycle(StartFocusedDispose)
    \/ InvalidateDispatcherForLifecycle(StartUnfocusedDispose)
    \/ InvalidateDispatcherForLifecycle(StartDisposeAfterCompletedFocusRepair)
    \/ InvalidateDispatcherForLifecycle(StartCoalescedResetFocusRepair)
    \/ InvalidateDispatcherForLifecycle(CoalesceFocusRepairDuringActiveReset)
    \/ InvalidateDispatcherForLifecycle(RequestDisposeDuringRepair)
    \/ KeepDispatcherOwnership(BeginPendingDispose)
    \/ PreserveDispatcherOwnership(PublishRepairKeyRelease)
    \/ KeepDispatcherOwnership(SkipRepairKeyRelease)
    \/ PreserveDispatcherOwnership(PublishRepairMouseRelease)
    \/ KeepDispatcherOwnership(SkipRepairMouseRelease)
    \/ KeepDispatcherOwnership(CompleteRepairWithoutFocusOut)
    \/ PreserveDispatcherOwnership(PublishFocusOut)
    \/ PreserveDispatcherOwnership(ReopenInputAdmission)
    \/ KeepDispatcherOwnership(CompleteDispose)
    \/ CoverageCompositionAuthorityAntecedent
    \/ CoverageExplicitPublicationOriginAntecedent
    \/ CoverageEligibleRepairGuardAntecedent
    \/ CoverageSettledRepairGuardAntecedent
    \/ CoverageFocusOutOwnershipAntecedent
    \/ CoverageUnfocusedDisposeAntecedent
    \/ CoverageDisposeAfterFocusRepairAntecedent
    \/ CoveragePendingDisposeRequestAntecedent
    \/ CoveragePendingDisposeBeginAntecedent
    \/ CoverageFocusedActiveResetState
    \/ CoverageStartCoalescedResetFocusRepair
    \/ CoverageCoalesceFocusRepairDuringActiveReset
    \/ CoverageStagedInvalidationAntecedent
    \/ CoverageCurrentContinuationAntecedent
    \/ CoverageStaleDiscardAntecedent
    \/ CoverageStalledReviewState
    \/ CoverageQueuedKeyPressMismatch
    \/ CoverageQueuedKeyReleaseMismatch
    \/ CoverageQueuedMousePressMismatch
    \/ CoverageQueuedMouseReleaseMismatch
    \/ CoverageQueuedFocusMismatch
    \/ CoveragePublishedKeyCleanupAntecedent
    \/ CoverageSkippedKeyCleanupAntecedent
    \/ CoveragePublishedMouseCleanupAntecedent
    \/ CoverageSkippedMouseCleanupAntecedent
    \/ CoverageCancelledReviewResolution
    \/ CoverageCancelledReviewBypassState
    \/ CoverageStatelessSendSuccess
    \/ CoverageSendFailure
    \/ CoverageFailedKeyPressRollback
    \/ CoverageFailedMousePressRollback
    \/ CoveragePublishedKeyReleaseAntecedent
    \/ CoveragePublishedMouseReleaseAntecedent
    \/ CoverageReentrantRepairDuringSend
    \/ CoverageFocusReconciliation
    \/ CoverageRepairReopenFocusReconciliation
    \/ CoveragePasteReviewRejection
    \/ CoverageRepairSendFailure
    \/ CoverageCurrentContinuationEffect
    \/ CoverageStaleContinuationEffect
    \/ CoverageReviewCompositionResetFocusFailure
    \/ CoverageLocalCompositionProgressWithFocusMismatch
    \/ CoverageLocalSettlementWithFocusMismatch
    \/ CoveragePostConvergenceCompositionPublication

Next == ModeStep

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(CompleteSend)
    /\ WF_vars(PreserveDispatcherOwnership(PublishRepairKeyRelease))
    /\ WF_vars(KeepDispatcherOwnership(SkipRepairKeyRelease))
    /\ WF_vars(PreserveDispatcherOwnership(PublishRepairMouseRelease))
    /\ WF_vars(KeepDispatcherOwnership(SkipRepairMouseRelease))
    /\ WF_vars(KeepDispatcherOwnership(CompleteRepairWithoutFocusOut))
    /\ WF_vars(PreserveDispatcherOwnership(PublishFocusOut))
    /\ WF_vars(PreserveDispatcherOwnership(ReopenInputAdmission))
    /\ WF_vars(KeepDispatcherOwnership(BeginPendingDispose))
    /\ WF_vars(KeepDispatcherOwnership(CompleteDispose))

AtMostOneCompositionPublication == compositionPublicationCount <= 1

AllPublicationsAreAccounted == compositionPublicationCount <= totalTextPublicationCount

HostInputAttemptUsesCurrentCapabilityRevision ==
    lastAction \in HostInputAttemptActions =>
        lastAttemptedCapabilityRevision = capabilityRevision

CapabilityRevisionStampChangesOnlyOnInputAttempt ==
    lastAction \notin HostInputAttemptActions =>
        lastAttemptedCapabilityRevision = priorAttemptedCapabilityRevision

CapabilityRevisionChangesOnlyOnReplacement ==
    /\ capabilityRevision >= priorCapabilityRevision
    /\ (capabilityRevision # priorCapabilityRevision =>
          capabilityRevision = priorCapabilityRevision + 1 /\
          lastAction \in {"capability-downgrade", "capability-upgrade"})

CapabilityReplacementAlternatesAuthority ==
    /\ (lastAction = "capability-downgrade" => priorInputEnabled /\ ~inputEnabled)
    /\ (lastAction = "capability-upgrade" => ~priorInputEnabled /\ inputEnabled)

CapabilityDowngradeRepairsImmediately ==
    lastAction = "capability-downgrade" =>
        ~inputEnabled /\ ~preeditPublished /\ ~textareaDirty /\
        availableCorrelations = {} /\ ~keyPressed /\ ~mousePressed /\ ~focused /\
        ~localKeyPressed /\ ~localMousePressed /\ ~localFocused /\
        ~reviewStalled /\ queuedOwnershipInput = "none" /\
        ~needsFocusOut /\ stagedContinuation # "current"

ReadOnlyHasNoInputOwnership ==
    ~inputEnabled =>
        ~preeditPublished /\ ~textareaDirty /\ availableCorrelations = {} /\
        ~keyPressed /\ ~mousePressed /\ ~focused /\
        ~localKeyPressed /\ ~localMousePressed /\ ~localFocused /\
        ~reviewStalled /\ queuedOwnershipInput = "none" /\
        stagedContinuation # "current"

ReadOnlyRejectsHostInputAttempts ==
    ~priorInputEnabled => lastAction \notin HostInputAttemptActions

CapabilityUpgradeDoesNotReplayInput ==
    lastAction = "capability-upgrade" =>
        ~keyPressed /\ ~mousePressed /\ ~focused /\
        ~localKeyPressed /\ ~localMousePressed /\ ~localFocused /\
        ~reviewStalled /\ queuedOwnershipInput = "none" /\
        totalTextPublicationCount = priorTotalTextPublicationCount /\
        lastAttemptedCapabilityRevision = priorAttemptedCapabilityRevision

CompositionPublicationRequiresEligibleAuthority ==
    compositionPublicationCount > priorCompositionPublicationCount =>
        priorPhase = "eligible" /\
        lastAction \in {
            "publish-beforeinput-eligible",
            "publish-composition-end",
            "stage-input-continuation"
        }

EveryPublicationHasAnExplicitOrigin ==
    totalTextPublicationCount > priorTotalTextPublicationCount =>
        lastAction \in {
            "publish-beforeinput-eligible",
            "publish-beforeinput-idle",
            "publish-composition-end",
            "stage-input-continuation"
        }

IneligibleUpdateCannotPublishPreedit ==
    preeditPublished =>
        phase = "eligible" /\
        (compositionPublicationCount = 0 \/ stagedContinuation = "current")

RepairRetainsSettlementGuard ==
    /\ lastAction = "repair-eligible" => settlement = "awaiting-native-end"
    /\ lastAction = "repair-settled" => settlement = priorSettlement /\ settlement # "none"

RepairClearsTransientInput ==
    repairStage # "none" =>
        ~preeditPublished /\ ~textareaDirty /\ availableCorrelations = {}

RepairClearsLocalOwnershipAndQueue ==
    repairStage # "none" =>
        ~localKeyPressed /\ ~localMousePressed /\ ~reviewStalled /\
        queuedOwnershipInput = "none" /\
        (repairReason = "reset" \/ ~localFocused)

QueuedOwnershipIsNotPublished ==
    /\ queuedOwnershipInput = "key-press" => localKeyPressed /\ ~keyPressed
    /\ queuedOwnershipInput = "key-release" => ~localKeyPressed /\ keyPressed
    /\ queuedOwnershipInput = "mouse-press" => localMousePressed /\ ~mousePressed
    /\ queuedOwnershipInput = "mouse-release" => ~localMousePressed /\ mousePressed
    /\ queuedOwnershipInput = "focus-in" => localFocused /\ ~focused

HostOwnershipChangesOnlyAtPublication ==
    /\ keyPressed # priorKeyPressed =>
          lastAction \in {"send-success", "repair-release-keys", "capability-downgrade"}
    /\ mousePressed # priorMousePressed =>
          lastAction \in {"send-success", "repair-release-mouse", "capability-downgrade"}
    /\ focused # priorFocused =>
          lastAction \in {"send-success", "repair-publish-focusout", "capability-downgrade"}

StatelessPublicationDoesNotChangeHeldOwnership ==
    (priorSendState = "stateless" /\ lastAction \in {"send-success", "send-failure"}) =>
        keyPressed = priorKeyPressed /\ mousePressed = priorMousePressed /\ focused = priorFocused

FailedSendPreservesPublishedOwnership ==
    lastAction \in {
        "send-failure",
        "repair-key-send-failure",
        "repair-mouse-send-failure",
        "repair-focus-send-failure"
    } =>
        keyPressed = priorKeyPressed /\ mousePressed = priorMousePressed /\ focused = priorFocused

FailedPressRollsBackLocalOwnership ==
    /\ (lastAction = "send-failure" /\ priorSendState = "key-press") =>
          ~localKeyPressed /\ "key" \notin availableCorrelations
    /\ (lastAction = "send-failure" /\ priorSendState = "mouse-press") =>
          ~localMousePressed /\ "mouse" \notin availableCorrelations

OrdinaryReleaseRequiresPublishedOwnership ==
    /\ lastAction = "begin-key-release-send" => priorKeyPressed
    /\ lastAction = "begin-mouse-release-send" => priorMousePressed

RepairReopenReconcilesFocus ==
    (lastAction = "repair-reopen-admission" /\ localFocused /\ ~focused) =>
        sendState = "focus-in" /\ ~sendFromRepair

InputPublicationRequiresFocusConvergence ==
    lastAction \in {
        "begin-key-press-send",
        "begin-key-release-send",
        "begin-mouse-press-send",
        "begin-mouse-release-send",
        "begin-stateless-send"
    } => (~priorLocalFocused \/ priorFocused)

TextPublicationRequiresFocusConvergence ==
    totalTextPublicationCount > priorTotalTextPublicationCount =>
        ~priorLocalFocused \/ priorFocused

CleanupReleaseRequiresPublishedOwnership ==
    /\ lastAction = "repair-release-keys" => priorKeyPressed
    /\ lastAction = "repair-release-mouse" => priorMousePressed
    /\ lastAction = "repair-skip-key-release" => ~priorKeyPressed
    /\ lastAction = "repair-skip-mouse-release" => ~priorMousePressed

LifecycleInvalidationDetachesQueuedGeneration ==
    lastAction \in {
        "repair-eligible",
        "repair-settled",
        "repair-plain",
        "coalesce-focus-repair-during-reset",
        "start-dispose-focused",
        "start-dispose-unfocused",
        "start-dispose-after-repair",
        "request-dispose-during-repair"
    } =>
        ~reviewStalled /\ queuedOwnershipInput = "none" /\
        keyPressed = priorKeyPressed /\ mousePressed = priorMousePressed /\
        focused = priorFocused

CancelledReviewResolutionIsInert ==
    lastAction = "resolve-cancelled-review" =>
        keyPressed = priorKeyPressed /\ mousePressed = priorMousePressed /\
        focused = priorFocused /\ localKeyPressed = priorLocalKeyPressed /\
        localMousePressed = priorLocalMousePressed /\ localFocused = priorLocalFocused /\
        totalTextPublicationCount = priorTotalTextPublicationCount /\
        queuedOwnershipInput = priorQueuedOwnershipInput

CancelledReviewDoesNotBlockCurrentGeneration ==
    (cancelledReviewPending /\ inputEnabled /\ InputReady /\ ~reviewStalled /\
        queuedOwnershipInput = "none" /\ sendState = "none") => DispatcherReady

DedupCorrelationIsSingleUse ==
    lastConsumedCorrelation = "none" \/
    lastConsumedCorrelation \notin availableCorrelations

RepairConsequenceOrder ==
    /\ lastAction \in {"begin-repair-mouse-send", "repair-skip-mouse-release"} =>
          priorRepairStage = "keys-released"
    /\ lastAction \in {
          "begin-repair-focus-send",
          "repair-complete-without-focusout"
       } => priorRepairStage = "mouse-released"
    /\ focusOutPublished =>
          repairStage = "complete" /\ ~focused

FocusOutRequiresPriorOwnership ==
    lastAction = "repair-publish-focusout" => priorFocused /\ priorNeedsFocusOut

FocusOutIsConditional ==
    needsFocusOut => focused /\ repairReason \in FocusRepairReasons \cup {"dispose"}

ResetNeverPublishesFocusOut == repairReason = "reset" => ~needsFocusOut /\ ~focusOutPublished

UnfocusedDisposeRequiresNoFocusOut ==
    lastAction = "start-dispose-unfocused" => ~needsFocusOut /\ ~focusOutPublished

InputEventsRequireOpenAdmission ==
    lastAction \in InputActions => priorAdmission = "accepting"

InputEventsRequireCurrentCapability ==
    lastAction \in InputActions => priorInputEnabled

RepairAndDisposeCloseAdmissionFirst ==
    lastAction \in {
        "repair-eligible",
        "repair-settled",
        "repair-plain",
        "coalesce-focus-repair-during-reset",
        "start-dispose-focused",
        "start-dispose-unfocused",
        "start-dispose-after-repair",
        "request-dispose-during-repair",
        "begin-pending-dispose"
    } => admission # "accepting"

PendingFocusOutIsNotErased ==
    needsFocusOut => repairStage \in ActiveRepairStages

SettlementMatchesCompositionPhase ==
    /\ phase = "eligible" => settlement = "none"
    /\ phase = "cancelled-awaiting-end" =>
          settlement \in {"awaiting-native-end", "beforeinput-observed"}

DisposedAcceptsNothing ==
    disposed =>
        admission = "disposed" /\ repairStage = "complete" /\ repairReason = "dispose" /\
        ~preeditPublished /\ ~textareaDirty /\ availableCorrelations = {} /\
        ~localKeyPressed /\ ~localMousePressed /\ ~localFocused /\
        ~reviewStalled /\ queuedOwnershipInput = "none" /\
        ~needsFocusOut /\ stagedContinuation # "current"

CurrentStagedContinuationHasAuthority ==
    stagedContinuation = "current" =>
        InputPublicationReady

RepairInvalidatesCurrentContinuation ==
    (lastAction \in {
        "repair-eligible",
        "repair-settled",
        "repair-plain",
        "coalesce-focus-repair-during-reset",
        "start-dispose-focused",
        "start-dispose-unfocused"
    } /\ priorStagedContinuation = "current") => stagedContinuation = "stale"

CurrentContinuationRequiresAuthority ==
    lastAction = "continue-current-input" =>
        priorStagedContinuation = "current" /\ priorAdmission = "accepting"

StaleContinuationOnlyCancels ==
    lastAction = "discard-stale-input-continuation" =>
        priorStagedContinuation = "stale" /\
        preeditPublished = priorPreeditPublished /\ textareaDirty = priorTextareaDirty

CurrentContinuationClearsOnlyItsPresentation ==
    lastAction = "continue-current-input" =>
        priorStagedContinuation = "current" /\
        ~preeditPublished /\ ~textareaDirty

RepairEventuallyClosesAdmission ==
    [](admission = "repairing" => <> (admission # "repairing"))

PendingFocusOutEventuallySettles ==
    [](needsFocusOut => <> (~needsFocusOut \/ focusOutPublished))

DisposalEventuallyCompletes ==
    [](admission = "disposed" => <> disposed)

SynchronousSendEventuallySettles ==
    [](sendState # "none" => <> (sendState = "none"))

=============================================================================
