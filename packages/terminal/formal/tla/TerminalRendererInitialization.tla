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
  "publish-fallback", "fail", "freeze", "cancel-retired", "dispose"
}

VARIABLES
  lifecycle,
  phase,
  fallbackAllowed,
  backend,
  deviceOwned,
  resourcesOwned,
  contextOwned,
  result,
  lastAction,
  previousLifecycle,
  previousPhase,
  previousBackend

vars == <<lifecycle, phase, fallbackAllowed, backend, deviceOwned,
  resourcesOwned, contextOwned, result, lastAction, previousLifecycle,
  previousPhase, previousBackend>>

Init ==
  /\ lifecycle = "live"
  /\ phase = "idle"
  /\ fallbackAllowed = FALSE
  /\ backend = "none"
  /\ deviceOwned = FALSE
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
  /\ result' = "pending"
  /\ UNCHANGED <<lifecycle, backend, deviceOwned, resourcesOwned, contextOwned>>
  /\ Record("start")

AdapterCompletes ==
  /\ lifecycle = "live"
  /\ phase = "adapter"
  /\ phase' = "device"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, deviceOwned,
       resourcesOwned, contextOwned, result>>
  /\ Record("adapter-completes")

DeviceCompletes ==
  /\ lifecycle = "live"
  /\ phase = "device"
  /\ phase' = "pipeline"
  /\ deviceOwned' = TRUE
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, resourcesOwned,
       contextOwned, result>>
  /\ Record("device-completes")

PipelineCompletes ==
  /\ lifecycle = "live"
  /\ phase = "pipeline"
  /\ phase' = "validation"
  /\ resourcesOwned' = TRUE
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, deviceOwned,
       contextOwned, result>>
  /\ Record("pipeline-completes")

ValidationCompletes ==
  /\ lifecycle = "live"
  /\ phase = "validation"
  /\ phase' = "prepared"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, backend, deviceOwned,
       resourcesOwned, contextOwned, result>>
  /\ Record("validation-completes")

PublishWebGPU ==
  /\ lifecycle = "live"
  /\ phase = "prepared"
  /\ phase' = "ready"
  /\ backend' = "webgpu"
  /\ contextOwned' = TRUE
  /\ result' = "ready"
  /\ UNCHANGED <<lifecycle, fallbackAllowed, deviceOwned, resourcesOwned>>
  /\ Record("publish-webgpu")

InitializationFails ==
  /\ lifecycle = "live"
  /\ phase \in PendingPhases
  /\ phase' = IF fallbackAllowed THEN "fallback" ELSE "failed"
  /\ backend' = IF fallbackAllowed THEN "canvas2d" ELSE "none"
  /\ deviceOwned' = FALSE
  /\ resourcesOwned' = FALSE
  /\ contextOwned' = fallbackAllowed
  /\ result' = IF fallbackAllowed THEN "fallback" ELSE "failed"
  /\ UNCHANGED <<lifecycle, fallbackAllowed>>
  /\ Record(IF fallbackAllowed THEN "publish-fallback" ELSE "fail")

Freeze ==
  /\ lifecycle = "live"
  /\ lifecycle' = "frozen"
  /\ UNCHANGED <<phase, fallbackAllowed, backend, deviceOwned,
       resourcesOwned, contextOwned, result>>
  /\ Record("freeze")

CancelRetired ==
  /\ lifecycle = "frozen"
  /\ phase \in PendingPhases
  /\ phase' = "cancelled"
  /\ backend' = "none"
  /\ deviceOwned' = FALSE
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
  /\ deviceOwned' = FALSE
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
  \/ Freeze
  \/ CancelRetired
  \/ Dispose

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ lifecycle \in LifecycleStates
  /\ phase \in Phases
  /\ fallbackAllowed \in BOOLEAN
  /\ backend \in Backends
  /\ deviceOwned \in BOOLEAN
  /\ resourcesOwned \in BOOLEAN
  /\ contextOwned \in BOOLEAN
  /\ result \in Results
  /\ lastAction \in Actions
  /\ previousLifecycle \in LifecycleStates
  /\ previousPhase \in Phases
  /\ previousBackend \in Backends

OwnershipMatchesPhase ==
  /\ (deviceOwned <=> phase \in {"pipeline", "validation", "prepared", "ready"})
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
    /\ ~deviceOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

DisposedOwnsNothing ==
  lifecycle = "disposed" =>
    /\ phase = "disposed"
    /\ backend = "none"
    /\ ~deviceOwned
    /\ ~resourcesOwned
    /\ ~contextOwned

FreezePreservesPresentedBackend ==
  lastAction = "freeze" /\ previousPhase \in {"ready", "fallback"} =>
    /\ phase = previousPhase
    /\ backend = previousBackend

=============================================================================
