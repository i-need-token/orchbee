-------------------- MODULE TerminalRendererInitialization --------------------
EXTENDS TLC

LifecycleStates == {"live", "frozen", "disposed"}
PendingPhases == {"adapter", "device", "pipeline", "validation", "prepared"}
Phases == PendingPhases \cup {
  "idle", "parked", "ready", "fallback", "cancelled", "failed", "disposed"
}
Backends == {"none", "webgpu", "canvas2d"}
Results == {"none", "pending", "ready", "fallback", "cancelled", "failed", "disposed"}
VisibilityStates == {"visible", "hidden"}
Actions == {
  "init", "start", "adapter-completes", "device-completes",
  "pipeline-completes", "validation-completes", "publish-webgpu",
  "publish-fallback", "fail", "device-lost", "hide", "show", "freeze",
  "cancel-retired", "dispose"
}

VARIABLES
  lifecycle,
  phase,
  fallbackAllowed,
  webGPUConfirmed,
  visibility,
  backend,
  runtimeLeaseOwned,
  lossSubscriptionOwned,
  resourcesOwned,
  contextOwned,
  result,
  lastAction,
  previousLifecycle,
  previousPhase,
  previousBackend

vars == <<lifecycle, phase, fallbackAllowed, webGPUConfirmed, visibility,
  backend, runtimeLeaseOwned,
  lossSubscriptionOwned, resourcesOwned, contextOwned, result, lastAction,
  previousLifecycle, previousPhase, previousBackend>>

Init ==
  /\ lifecycle = "live"
  /\ phase = "idle"
  /\ fallbackAllowed = FALSE
  /\ webGPUConfirmed = FALSE
  /\ visibility = "visible"
  /\ backend = "none"
  /\ runtimeLeaseOwned = FALSE
  /\ lossSubscriptionOwned = FALSE
  /\ resourcesOwned = FALSE
  /\ contextOwned = FALSE
  /\ result = "none"
  /\ lastAction = "init"
  /\ previousLifecycle = lifecycle
  /\ previousPhase = phase
  /\ previousBackend = backend

Record(action) ==
  /\ lastAction' = action
  /\ previousLifecycle' = lifecycle
  /\ previousPhase' = phase
  /\ previousBackend' = backend

Start(allowFallback) ==
  /\ lifecycle = "live"
  /\ visibility = "visible"
  /\ phase = "idle"
  /\ allowFallback \in BOOLEAN
  /\ phase' = "adapter"
  /\ fallbackAllowed' = allowFallback /\ ~webGPUConfirmed
  /\ runtimeLeaseOwned' = TRUE
  /\ lossSubscriptionOwned' = TRUE
  /\ result' = "pending"
  /\ UNCHANGED <<lifecycle, webGPUConfirmed, visibility, backend,
       resourcesOwned, contextOwned>>
  /\ Record("start")

AdapterCompletes ==
  /\ lifecycle = "live"
  /\ phase = "adapter"
  /\ phase' = "device"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility,
       backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("adapter-completes")

DeviceCompletes ==
  /\ lifecycle = "live"
  /\ phase = "device"
  /\ phase' = "pipeline"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility,
       backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("device-completes")

PipelineCompletes ==
  /\ lifecycle = "live"
  /\ phase = "pipeline"
  /\ phase' = "validation"
  /\ resourcesOwned' = TRUE
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility,
       backend, runtimeLeaseOwned,
       lossSubscriptionOwned, contextOwned, result>>
  /\ Record("pipeline-completes")

ValidationCompletes ==
  /\ lifecycle = "live"
  /\ phase = "validation"
  /\ phase' = "prepared"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility,
       backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("validation-completes")

PublishWebGPU ==
  /\ lifecycle = "live"
  /\ phase = "prepared"
  /\ phase' = "ready"
  /\ backend' = "webgpu"
  /\ webGPUConfirmed' = TRUE
  /\ contextOwned' = TRUE
  /\ result' = "ready"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, visibility, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned>>
  /\ Record("publish-webgpu")

InitializationFails ==
  /\ lifecycle = "live"
  /\ phase \in PendingPhases
  /\ phase' = IF fallbackAllowed THEN "fallback" ELSE "failed"
  /\ backend' = IF fallbackAllowed THEN "canvas2d" ELSE "none"
  /\ runtimeLeaseOwned' = FALSE
  /\ lossSubscriptionOwned' = FALSE
  /\ resourcesOwned' = FALSE
  /\ contextOwned' = fallbackAllowed
  /\ result' = IF fallbackAllowed THEN "fallback" ELSE "failed"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility>>
  /\ Record(IF fallbackAllowed THEN "publish-fallback" ELSE "fail")

DeviceLost ==
  /\ lifecycle = "live"
  /\ phase = "ready"
  /\ phase' = "idle"
  /\ backend' = "none"
  /\ runtimeLeaseOwned' = FALSE
  /\ lossSubscriptionOwned' = FALSE
  /\ resourcesOwned' = FALSE
  /\ contextOwned' = FALSE
  /\ result' = "none"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility>>
  /\ Record("device-lost")

Hide ==
  /\ lifecycle = "live"
  /\ visibility = "visible"
  /\ visibility' = "hidden"
  /\ phase' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN "parked" ELSE phase
  /\ backend' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN "none" ELSE backend
  /\ runtimeLeaseOwned' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN FALSE ELSE runtimeLeaseOwned
  /\ lossSubscriptionOwned' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN FALSE ELSE lossSubscriptionOwned
  /\ resourcesOwned' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN FALSE ELSE resourcesOwned
  /\ contextOwned' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN FALSE ELSE contextOwned
  /\ result' = IF phase \in PendingPhases \cup {"idle", "ready"}
       THEN "none" ELSE result
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed>>
  /\ Record("hide")

Show ==
  /\ lifecycle = "live"
  /\ visibility = "hidden"
  /\ visibility' = "visible"
  /\ phase' = IF phase = "parked" THEN "idle" ELSE phase
  /\ result' = IF phase = "parked" THEN "none" ELSE result
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, backend,
       runtimeLeaseOwned, lossSubscriptionOwned, resourcesOwned, contextOwned>>
  /\ Record("show")

Freeze ==
  /\ lifecycle = "live"
  /\ lifecycle' = "frozen"
  /\ UNCHANGED <<phase, fallbackAllowed, webGPUConfirmed, visibility,
       backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("freeze")

CancelRetired ==
  /\ lifecycle = "frozen"
  /\ phase \in PendingPhases
  /\ phase' = "cancelled"
  /\ backend' = "none"
  /\ runtimeLeaseOwned' = FALSE
  /\ lossSubscriptionOwned' = FALSE
  /\ resourcesOwned' = FALSE
  /\ contextOwned' = FALSE
  /\ result' = "cancelled"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, webGPUConfirmed, visibility>>
  /\ Record("cancel-retired")

Dispose ==
  /\ lifecycle # "disposed"
  /\ lifecycle' = "disposed"
  /\ phase' = "disposed"
  /\ backend' = "none"
  /\ runtimeLeaseOwned' = FALSE
  /\ lossSubscriptionOwned' = FALSE
  /\ resourcesOwned' = FALSE
  /\ contextOwned' = FALSE
  /\ result' = "disposed"
  /\ UNCHANGED <<fallbackAllowed, webGPUConfirmed, visibility>>
  /\ Record("dispose")

CoveragePublishFallback ==
  /\ lastAction = "publish-fallback"
  /\ UNCHANGED vars

CoverageHardFailure ==
  /\ lastAction = "fail"
  /\ UNCHANGED vars

Next ==
  \/ Start(TRUE)
  \/ Start(FALSE)
  \/ AdapterCompletes
  \/ DeviceCompletes
  \/ PipelineCompletes
  \/ ValidationCompletes
  \/ PublishWebGPU
  \/ InitializationFails
  \/ DeviceLost
  \/ Hide
  \/ Show
  \/ Freeze
  \/ CancelRetired
  \/ Dispose
  \/ CoveragePublishFallback
  \/ CoverageHardFailure

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ lifecycle \in LifecycleStates
  /\ phase \in Phases
  /\ fallbackAllowed \in BOOLEAN
  /\ webGPUConfirmed \in BOOLEAN
  /\ visibility \in VisibilityStates
  /\ backend \in Backends
  /\ runtimeLeaseOwned \in BOOLEAN
  /\ lossSubscriptionOwned \in BOOLEAN
  /\ resourcesOwned \in BOOLEAN
  /\ contextOwned \in BOOLEAN
  /\ result \in Results
  /\ lastAction \in Actions
  /\ previousLifecycle \in LifecycleStates
  /\ previousPhase \in Phases
  /\ previousBackend \in Backends

OwnershipMatchesPhase ==
  /\ (runtimeLeaseOwned <=> phase \in PendingPhases \cup {"ready"})
  /\ (lossSubscriptionOwned <=> runtimeLeaseOwned)
  /\ (resourcesOwned <=> phase \in {"validation", "prepared", "ready"})
  /\ (contextOwned <=> phase \in {"ready", "fallback"})

BackendMatchesPublication ==
  /\ (backend = "webgpu" <=> phase = "ready")
  /\ (backend = "canvas2d" <=> phase = "fallback")
  /\ (backend = "canvas2d" => ~webGPUConfirmed)

ResultMatchesPhase ==
  /\ (phase = "idle" => result = "none")
  /\ (phase = "parked" => result = "none")
  /\ (phase \in PendingPhases => result = "pending")
  /\ (phase \in {"ready", "fallback", "cancelled", "failed", "disposed"} =>
       result = phase)

PublicationRequiresLive ==
  lastAction = "publish-webgpu" =>
    /\ previousLifecycle = "live"
    /\ lifecycle = "live"

FallbackRequiresLive ==
  lastAction = "publish-fallback" =>
    /\ previousLifecycle = "live"
    /\ lifecycle = "live"

DisposedOwnsNothing ==
  lifecycle = "disposed" =>
    /\ phase = "disposed"
    /\ backend = "none"
    /\ ~runtimeLeaseOwned
    /\ ~lossSubscriptionOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

LossRetiresGeneration ==
  lastAction = "device-lost" =>
    /\ previousPhase = "ready"
    /\ phase = "idle"
    /\ backend = "none"
    /\ ~runtimeLeaseOwned
    /\ ~lossSubscriptionOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

HideReleasesWebGPU ==
  lastAction = "hide" /\ previousPhase = "ready" =>
    /\ phase = "parked"
    /\ backend = "none"
    /\ ~runtimeLeaseOwned
    /\ ~lossSubscriptionOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

HideCancelsPendingInitialization ==
  lastAction = "hide" /\ previousPhase \in PendingPhases =>
    /\ phase = "parked"
    /\ backend = "none"
    /\ ~runtimeLeaseOwned
    /\ ~resourcesOwned

ShowPublishesNothing ==
  lastAction = "show" /\ previousPhase = "parked" =>
    /\ phase = "idle"
    /\ backend = "none"
    /\ ~runtimeLeaseOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

FreezePreservesPresentedBackend ==
  lastAction = "freeze" /\ previousPhase \in {"ready", "fallback"} =>
    /\ phase = previousPhase
    /\ backend = previousBackend

=============================================================================
