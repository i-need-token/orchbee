-------------------- MODULE TerminalRendererInitialization --------------------
EXTENDS TLC

LifecycleStates == {"live", "frozen", "disposed"}
PendingPhases == {"adapter", "device", "pipeline", "validation", "prepared"}
Phases == PendingPhases \cup {
  "idle", "ready", "fallback", "cancelled", "failed", "disposed"
}
Backends == {"none", "webgpu", "canvas2d"}
Results == {"none", "pending", "ready", "fallback", "cancelled", "failed", "disposed"}
Actions == {
  "init", "start", "adapter-completes", "device-completes",
  "pipeline-completes", "validation-completes", "publish-webgpu",
  "publish-fallback", "fail", "device-lost", "freeze",
  "cancel-retired", "dispose"
}

VARIABLES
  lifecycle,
  phase,
  fallbackAllowed,
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

vars == <<lifecycle, phase, fallbackAllowed, backend, runtimeLeaseOwned,
  lossSubscriptionOwned, resourcesOwned, contextOwned, result, lastAction,
  previousLifecycle, previousPhase, previousBackend>>

Init ==
  /\ lifecycle = "live"
  /\ phase = "idle"
  /\ fallbackAllowed = FALSE
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
  /\ phase = "idle"
  /\ allowFallback \in BOOLEAN
  /\ phase' = "adapter"
  /\ fallbackAllowed' = allowFallback
  /\ runtimeLeaseOwned' = TRUE
  /\ lossSubscriptionOwned' = TRUE
  /\ result' = "pending"
  /\ UNCHANGED <<lifecycle, backend, resourcesOwned, contextOwned>>
  /\ Record("start")

AdapterCompletes ==
  /\ lifecycle = "live"
  /\ phase = "adapter"
  /\ phase' = "device"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("adapter-completes")

DeviceCompletes ==
  /\ lifecycle = "live"
  /\ phase = "device"
  /\ phase' = "pipeline"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("device-completes")

PipelineCompletes ==
  /\ lifecycle = "live"
  /\ phase = "pipeline"
  /\ phase' = "validation"
  /\ resourcesOwned' = TRUE
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, runtimeLeaseOwned,
       lossSubscriptionOwned, contextOwned, result>>
  /\ Record("pipeline-completes")

ValidationCompletes ==
  /\ lifecycle = "live"
  /\ phase = "validation"
  /\ phase' = "prepared"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, runtimeLeaseOwned,
       lossSubscriptionOwned, resourcesOwned, contextOwned, result>>
  /\ Record("validation-completes")

PublishWebGPU ==
  /\ lifecycle = "live"
  /\ phase = "prepared"
  /\ phase' = "ready"
  /\ backend' = "webgpu"
  /\ contextOwned' = TRUE
  /\ result' = "ready"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, runtimeLeaseOwned,
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
  /\ UNCHANGED <<lifecycle, fallbackAllowed>>
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
  /\ UNCHANGED <<lifecycle, fallbackAllowed>>
  /\ Record("device-lost")

Freeze ==
  /\ lifecycle = "live"
  /\ lifecycle' = "frozen"
  /\ UNCHANGED <<phase, fallbackAllowed, backend, runtimeLeaseOwned,
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
  /\ UNCHANGED <<lifecycle, fallbackAllowed>>
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
  /\ UNCHANGED fallbackAllowed
  /\ Record("dispose")

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
  \/ Freeze
  \/ CancelRetired
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ lifecycle \in LifecycleStates
  /\ phase \in Phases
  /\ fallbackAllowed \in BOOLEAN
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

ResultMatchesPhase ==
  /\ (phase = "idle" => result = "none")
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

CancelledOwnsNothing ==
  phase = "cancelled" =>
    /\ backend = "none"
    /\ ~runtimeLeaseOwned
    /\ ~lossSubscriptionOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

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

FreezePreservesPresentedBackend ==
  lastAction = "freeze" /\ previousPhase \in {"ready", "fallback"} =>
    /\ phase = previousPhase
    /\ backend = previousBackend

=============================================================================
