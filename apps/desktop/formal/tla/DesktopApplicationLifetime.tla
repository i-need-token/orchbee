--------------------- MODULE DesktopApplicationLifetime ---------------------
EXTENDS FiniteSets, TLC

Resources == {"referenced", "unreferenced"}
Modes == {"none", "user-preserve", "user-stop", "update-restart",
  "development-host-restart"}
DestructiveModes == {"user-stop", "development-host-restart"}
Phases == {"idle", "awaiting-choice", "flushing", "persisting-stop",
  "stopping", "staging-restart", "preparing-plan", "failed", "ready",
  "quitting"}
Actions == {"init", "request-user-protected", "request-user-unprotected",
  "request-update", "request-development",
  "choose-preserve", "choose-stop", "cancel", "flush", "flush-failed",
  "persist-stop", "persist-failed", "dispatch-close", "close-completed",
  "close-failed", "retry", "orphan-detected", "finish-stop",
  "finish-development-stop", "stage-restart", "prepare-plan",
  "prepare-plan-failed", "quit", "quit-anyway"}

VARIABLES
  mode,
  phase,
  prompted,
  topologyDurable,
  restartProvisionDurable,
  restartPlanReady,
  bindings,
  closeIssued,
  hostOrphan,
  effectsStarted,
  forcedQuit,
  quit,
  writable,
  planWritable,
  lastAction,
  previousBindings,
  previousEffectsStarted

vars == <<mode, phase, prompted, topologyDurable, restartProvisionDurable,
  restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
  forcedQuit, quit, writable, planWritable, lastAction, previousBindings,
  previousEffectsStarted>>

Record(action) ==
  /\ lastAction' = action
  /\ previousBindings' = bindings
  /\ previousEffectsStarted' = effectsStarted

Init ==
  /\ mode = "none"
  /\ phase = "idle"
  /\ prompted = FALSE
  /\ topologyDurable = FALSE
  /\ restartProvisionDurable = FALSE
  /\ restartPlanReady = FALSE
  /\ bindings = [resource \in Resources |-> TRUE]
  /\ closeIssued = [resource \in Resources |-> FALSE]
  /\ hostOrphan \in BOOLEAN
  /\ effectsStarted = FALSE
  /\ forcedQuit = FALSE
  /\ quit = FALSE
  /\ writable \in BOOLEAN
  /\ planWritable \in BOOLEAN
  /\ lastAction = "init"
  /\ previousBindings = bindings
  /\ previousEffectsStarted = effectsStarted

\* These two actions model the zero/nonzero result of the Workbench-owned
\* close-confirmation count sampled for one user-quit request.
RequestProtectedUserQuit ==
  /\ phase = "idle"
  /\ mode' = "user-preserve"
  /\ phase' = "awaiting-choice"
  /\ prompted' = TRUE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("request-user-protected")

RequestUnprotectedUserQuit ==
  /\ phase = "idle"
  /\ mode' = "user-preserve"
  /\ phase' = "flushing"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("request-user-unprotected")

RequestUpdateRestart ==
  /\ phase = "idle"
  /\ mode' = "update-restart"
  /\ phase' = "flushing"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("request-update")

RequestDevelopmentHostRestart ==
  /\ phase = "idle"
  /\ mode' = "development-host-restart"
  /\ phase' = "persisting-stop"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("request-development")

ChoosePreserve ==
  /\ phase = "awaiting-choice"
  /\ mode' = "user-preserve"
  /\ phase' = "flushing"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("choose-preserve")

ChooseStop ==
  /\ phase = "awaiting-choice"
  /\ mode' = "user-stop"
  /\ phase' = "persisting-stop"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("choose-stop")

Cancel ==
  /\ phase = "awaiting-choice"
  /\ ~effectsStarted
  /\ mode' = "none"
  /\ phase' = "idle"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("cancel")

Flush ==
  /\ phase = "flushing"
  /\ writable
  /\ topologyDurable' = TRUE
  /\ phase' = "ready"
  /\ UNCHANGED <<mode, prompted, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("flush")

FlushFails ==
  /\ phase = "flushing"
  /\ ~writable
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("flush-failed")

PersistStop ==
  /\ phase = "persisting-stop"
  /\ writable
  /\ topologyDurable' = TRUE
  /\ phase' = "stopping"
  /\ UNCHANGED <<mode, prompted, restartProvisionDurable, restartPlanReady,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("persist-stop")

PersistStopFails ==
  /\ phase = "persisting-stop"
  /\ ~writable
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("persist-failed")

DispatchClose(resource) ==
  /\ mode \in DestructiveModes
  /\ phase = "stopping"
  /\ topologyDurable
  /\ bindings[resource]
  /\ ~closeIssued[resource]
  /\ closeIssued' = [closeIssued EXCEPT ![resource] = TRUE]
  /\ effectsStarted' = TRUE
  /\ UNCHANGED <<mode, phase, prompted, topologyDurable,
       restartProvisionDurable, restartPlanReady, bindings, hostOrphan,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("dispatch-close")

CloseCompletes(resource) ==
  /\ mode \in DestructiveModes
  /\ phase = "stopping"
  /\ closeIssued[resource]
  /\ bindings' = [bindings EXCEPT ![resource] = FALSE]
  /\ closeIssued' = [closeIssued EXCEPT ![resource] = FALSE]
  /\ UNCHANGED <<mode, phase, prompted, topologyDurable,
       restartProvisionDurable, restartPlanReady, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("close-completed")

CloseFails(resource) ==
  /\ mode \in DestructiveModes
  /\ phase = "stopping"
  /\ closeIssued[resource]
  /\ closeIssued' = [closeIssued EXCEPT ![resource] = FALSE]
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, hostOrphan, effectsStarted, forcedQuit,
       quit, writable, planWritable>>
  /\ Record("close-failed")

Retry ==
  /\ mode \in DestructiveModes
  /\ phase = "failed"
  /\ topologyDurable
  /\ restartPlanReady' = FALSE
  /\ phase' = "stopping"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("retry")

DetectOrphan ==
  /\ mode \in DestructiveModes
  /\ phase = "stopping"
  /\ \A resource \in Resources: ~bindings[resource]
  /\ hostOrphan
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("orphan-detected")

FinishStop ==
  /\ mode = "user-stop"
  /\ phase = "stopping"
  /\ \A resource \in Resources: ~bindings[resource]
  /\ ~hostOrphan
  /\ phase' = "ready"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("finish-stop")

FinishDevelopmentStop ==
  /\ mode = "development-host-restart"
  /\ phase = "stopping"
  /\ \A resource \in Resources: ~bindings[resource]
  /\ ~hostOrphan
  /\ phase' = "staging-restart"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("finish-development-stop")

StageDevelopmentRestart ==
  /\ mode = "development-host-restart"
  /\ phase = "staging-restart"
  /\ writable
  /\ restartProvisionDurable' = TRUE
  /\ phase' = "preparing-plan"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartPlanReady, bindings,
       closeIssued, hostOrphan, effectsStarted, forcedQuit, quit, writable,
       planWritable>>
  /\ Record("stage-restart")

PrepareDevelopmentPlan ==
  /\ mode = "development-host-restart"
  /\ phase = "preparing-plan"
  /\ restartProvisionDurable
  /\ planWritable
  /\ restartPlanReady' = TRUE
  /\ phase' = "ready"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       bindings, closeIssued, hostOrphan, effectsStarted, forcedQuit, quit,
       writable, planWritable>>
  /\ Record("prepare-plan")

PrepareDevelopmentPlanFails ==
  /\ mode = "development-host-restart"
  /\ phase = "preparing-plan"
  /\ ~planWritable
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, quit, writable, planWritable>>
  /\ Record("prepare-plan-failed")

Quit ==
  /\ phase = "ready"
  /\ phase' = "quitting"
  /\ quit' = TRUE
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       forcedQuit, writable, planWritable>>
  /\ Record("quit")

QuitAnyway ==
  /\ phase = "failed"
  /\ phase' = "quitting"
  /\ forcedQuit' = TRUE
  /\ quit' = TRUE
  /\ UNCHANGED <<mode, prompted, topologyDurable, restartProvisionDurable,
       restartPlanReady, bindings, closeIssued, hostOrphan, effectsStarted,
       writable, planWritable>>
  /\ Record("quit-anyway")

Next ==
  \/ RequestProtectedUserQuit
  \/ RequestUnprotectedUserQuit
  \/ RequestUpdateRestart
  \/ RequestDevelopmentHostRestart
  \/ ChoosePreserve
  \/ ChooseStop
  \/ Cancel
  \/ Flush
  \/ FlushFails
  \/ PersistStop
  \/ PersistStopFails
  \/ Retry
  \/ DetectOrphan
  \/ FinishStop
  \/ FinishDevelopmentStop
  \/ StageDevelopmentRestart
  \/ PrepareDevelopmentPlan
  \/ PrepareDevelopmentPlanFails
  \/ Quit
  \/ QuitAnyway
  \/ \E resource \in Resources: DispatchClose(resource)
  \/ \E resource \in Resources: CloseCompletes(resource)
  \/ \E resource \in Resources: CloseFails(resource)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ mode \in Modes
  /\ phase \in Phases
  /\ prompted \in BOOLEAN
  /\ topologyDurable \in BOOLEAN
  /\ restartProvisionDurable \in BOOLEAN
  /\ restartPlanReady \in BOOLEAN
  /\ bindings \in [Resources -> BOOLEAN]
  /\ closeIssued \in [Resources -> BOOLEAN]
  /\ hostOrphan \in BOOLEAN
  /\ effectsStarted \in BOOLEAN
  /\ forcedQuit \in BOOLEAN
  /\ quit \in BOOLEAN
  /\ writable \in BOOLEAN
  /\ planWritable \in BOOLEAN
  /\ lastAction \in Actions

UpdateNeverPrompts == mode = "update-restart" => ~prompted

DevelopmentRestartNeverPrompts == mode = "development-host-restart" => ~prompted

ProtectedUserQuitPrompts ==
  lastAction = "request-user-protected" =>
    /\ mode = "user-preserve"
    /\ phase = "awaiting-choice"
    /\ prompted

UnprotectedUserQuitPreservesWithoutPrompt ==
  lastAction = "request-user-unprotected" =>
    /\ mode = "user-preserve"
    /\ phase = "flushing"
    /\ ~prompted

CloseRequiresDurableStop ==
  lastAction = "dispatch-close" => mode \in DestructiveModes /\ topologyDurable

PreserveNeverClosesBindings ==
  mode \in {"user-preserve", "update-restart"} => bindings = previousBindings

CancelPrecedesEffects == lastAction = "cancel" => ~previousEffectsStarted

CleanStopQuitHasNoOwnedBinding ==
  quit /\ mode = "user-stop" /\ ~forcedQuit =>
    \A resource \in Resources: ~bindings[resource]

CleanDevelopmentRestartIsReady ==
  quit /\ mode = "development-host-restart" /\ ~forcedQuit =>
    /\ restartProvisionDurable
    /\ restartPlanReady
    /\ \A resource \in Resources: ~bindings[resource]

=============================================================================
