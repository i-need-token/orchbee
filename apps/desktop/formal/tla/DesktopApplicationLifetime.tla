--------------------- MODULE DesktopApplicationLifetime ---------------------
EXTENDS FiniteSets, TLC

Resources == {"referenced", "unreferenced"}
Modes == {"none", "user-preserve", "user-stop", "update-restart"}
Phases == {"idle", "awaiting-choice", "flushing", "persisting-stop",
  "stopping", "failed", "ready", "quitting"}
Actions == {"init", "request-user", "request-update", "choose-preserve",
  "choose-stop", "cancel", "flush", "flush-failed", "persist-stop",
  "persist-failed", "dispatch-close", "close-completed", "close-failed",
  "retry", "orphan-detected", "finish-stop", "quit", "quit-anyway"}

VARIABLES
  mode,
  phase,
  prompted,
  topologyDurable,
  bindings,
  closeIssued,
  hostOrphan,
  effectsStarted,
  forcedQuit,
  quit,
  writable,
  lastAction,
  previousBindings,
  previousEffectsStarted

vars == <<mode, phase, prompted, topologyDurable, bindings, closeIssued,
  hostOrphan, effectsStarted, forcedQuit, quit, writable, lastAction,
  previousBindings, previousEffectsStarted>>

Record(action) ==
  /\ lastAction' = action
  /\ previousBindings' = bindings
  /\ previousEffectsStarted' = effectsStarted

Init ==
  /\ mode = "none"
  /\ phase = "idle"
  /\ prompted = FALSE
  /\ topologyDurable = FALSE
  /\ bindings = [resource \in Resources |-> TRUE]
  /\ closeIssued = [resource \in Resources |-> FALSE]
  /\ hostOrphan \in BOOLEAN
  /\ effectsStarted = FALSE
  /\ forcedQuit = FALSE
  /\ quit = FALSE
  /\ writable \in BOOLEAN
  /\ lastAction = "init"
  /\ previousBindings = bindings
  /\ previousEffectsStarted = effectsStarted

RequestUserQuit ==
  /\ phase = "idle"
  /\ mode' = "user-preserve"
  /\ phase' = "awaiting-choice"
  /\ prompted' = TRUE
  /\ UNCHANGED <<topologyDurable, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("request-user")

RequestUpdateRestart ==
  /\ phase = "idle"
  /\ mode' = "update-restart"
  /\ phase' = "flushing"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("request-update")

ChoosePreserve ==
  /\ phase = "awaiting-choice"
  /\ mode' = "user-preserve"
  /\ phase' = "flushing"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("choose-preserve")

ChooseStop ==
  /\ phase = "awaiting-choice"
  /\ mode' = "user-stop"
  /\ phase' = "persisting-stop"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("choose-stop")

Cancel ==
  /\ phase = "awaiting-choice"
  /\ ~effectsStarted
  /\ mode' = "none"
  /\ phase' = "idle"
  /\ prompted' = FALSE
  /\ UNCHANGED <<topologyDurable, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("cancel")

Flush ==
  /\ phase = "flushing"
  /\ writable
  /\ topologyDurable' = TRUE
  /\ phase' = "ready"
  /\ UNCHANGED <<mode, prompted, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("flush")

FlushFails ==
  /\ phase = "flushing"
  /\ ~writable
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, forcedQuit, quit, writable>>
  /\ Record("flush-failed")

PersistStop ==
  /\ phase = "persisting-stop"
  /\ writable
  /\ topologyDurable' = TRUE
  /\ phase' = "stopping"
  /\ UNCHANGED <<mode, prompted, bindings, closeIssued, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("persist-stop")

PersistStopFails ==
  /\ phase = "persisting-stop"
  /\ ~writable
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, forcedQuit, quit, writable>>
  /\ Record("persist-failed")

DispatchClose(resource) ==
  /\ mode = "user-stop"
  /\ phase = "stopping"
  /\ topologyDurable
  /\ bindings[resource]
  /\ ~closeIssued[resource]
  /\ closeIssued' = [closeIssued EXCEPT ![resource] = TRUE]
  /\ effectsStarted' = TRUE
  /\ UNCHANGED <<mode, phase, prompted, topologyDurable, bindings,
       hostOrphan, forcedQuit, quit, writable>>
  /\ Record("dispatch-close")

CloseCompletes(resource) ==
  /\ mode = "user-stop"
  /\ phase = "stopping"
  /\ closeIssued[resource]
  /\ bindings' = [bindings EXCEPT ![resource] = FALSE]
  /\ closeIssued' = [closeIssued EXCEPT ![resource] = FALSE]
  /\ UNCHANGED <<mode, phase, prompted, topologyDurable, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("close-completed")

CloseFails(resource) ==
  /\ mode = "user-stop"
  /\ phase = "stopping"
  /\ closeIssued[resource]
  /\ closeIssued' = [closeIssued EXCEPT ![resource] = FALSE]
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, hostOrphan,
       effectsStarted, forcedQuit, quit, writable>>
  /\ Record("close-failed")

Retry ==
  /\ mode = "user-stop"
  /\ phase = "failed"
  /\ topologyDurable
  /\ phase' = "stopping"
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, forcedQuit, quit, writable>>
  /\ Record("retry")

DetectOrphan ==
  /\ mode = "user-stop"
  /\ phase = "stopping"
  /\ \A resource \in Resources: ~bindings[resource]
  /\ hostOrphan
  /\ phase' = "failed"
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, forcedQuit, quit, writable>>
  /\ Record("orphan-detected")

FinishStop ==
  /\ mode = "user-stop"
  /\ phase = "stopping"
  /\ \A resource \in Resources: ~bindings[resource]
  /\ ~hostOrphan
  /\ phase' = "ready"
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, forcedQuit, quit, writable>>
  /\ Record("finish-stop")

Quit ==
  /\ phase = "ready"
  /\ phase' = "quitting"
  /\ quit' = TRUE
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, forcedQuit, writable>>
  /\ Record("quit")

QuitAnyway ==
  /\ phase = "failed"
  /\ phase' = "quitting"
  /\ forcedQuit' = TRUE
  /\ quit' = TRUE
  /\ UNCHANGED <<mode, prompted, topologyDurable, bindings, closeIssued,
       hostOrphan, effectsStarted, writable>>
  /\ Record("quit-anyway")

Next ==
  \/ RequestUserQuit
  \/ RequestUpdateRestart
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
  /\ bindings \in [Resources -> BOOLEAN]
  /\ closeIssued \in [Resources -> BOOLEAN]
  /\ hostOrphan \in BOOLEAN
  /\ effectsStarted \in BOOLEAN
  /\ forcedQuit \in BOOLEAN
  /\ quit \in BOOLEAN
  /\ writable \in BOOLEAN
  /\ lastAction \in Actions

UpdateNeverPrompts == mode = "update-restart" => ~prompted

CloseRequiresDurableStop ==
  lastAction = "dispatch-close" => mode = "user-stop" /\ topologyDurable

PreserveNeverClosesBindings ==
  mode \in {"user-preserve", "update-restart"} => bindings = previousBindings

CancelPrecedesEffects == lastAction = "cancel" => ~previousEffectsStarted

CleanStopQuitHasNoOwnedBinding ==
  quit /\ mode = "user-stop" /\ ~forcedQuit =>
    \A resource \in Resources: ~bindings[resource]

=============================================================================
