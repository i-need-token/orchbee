---------------------------- MODULE TerminalHostProcessIdentity ------------------------
EXTENDS TLC

\* Refines runtime-model claims TH-008, TH-009, and TH-013 for the POSIX
\* process-identity owner (THCR-006). completion abstracts the engine's
\* completion-classification sampling; containmentConfirmed abstracts the
\* empty-session containment confirmation; identityRetained abstracts retention
\* of the session-leader identity until waitpid releases it; sessionOwner
\* abstracts the numeric session identifier, which an unrelated same-user
\* process may rebind to itself after release (SID/PID churn); and
\* AdmitDispatch / EmitSignal / CancelSignal abstract the Host graceful and
\* force termination requests and their serialized platform effect.
\* Registry identity, broker retention, and multi-runtime shutdown are
\* intentionally owned by TerminalHostRegistry and TerminalHostShutdown.
\* EmitSignal is the check-and-signal step of the serialized process-identity
\* fence: it emits a process signal only while the launch identity is retained
\* and containment is unconfirmed. After identity release or confirmed
\* containment, the request completes as CancelSignal, an idempotent no-op that
\* never consults the numeric identifier. NoForeignSignal is the safety claim
\* that a rebound foreign session can never receive a signal from a settled
\* terminal.

VARIABLES
  completion,
  containmentConfirmed,
  identityRetained,
  sessionOwner,
  requestInFlight,
  signalsEmitted,
  foreignSignals

vars == <<completion, containmentConfirmed, identityRetained, sessionOwner,
          requestInFlight, signalsEmitted, foreignSignals>>

Init ==
  /\ completion = "pending"
  /\ containmentConfirmed = FALSE
  /\ identityRetained = TRUE
  /\ sessionOwner = "launch"
  /\ requestInFlight = FALSE
  /\ signalsEmitted = FALSE
  /\ foreignSignals = FALSE

\* The engine samples child completion.
ChildExited ==
  /\ completion = "pending"
  /\ completion' = "normal"
  /\ UNCHANGED <<containmentConfirmed, identityRetained, sessionOwner,
                 requestInFlight, signalsEmitted, foreignSignals>>

\* The wait worker confirms the owned session has no live members.
ConfirmContainment ==
  /\ completion = "normal"
  /\ ~containmentConfirmed
  /\ identityRetained
  /\ containmentConfirmed' = TRUE
  /\ UNCHANGED <<completion, identityRetained, sessionOwner,
                 requestInFlight, signalsEmitted, foreignSignals>>

\* The wait worker reaps the session leader: the numeric identity is freed.
ReleaseIdentity ==
  /\ completion = "normal"
  /\ containmentConfirmed
  /\ identityRetained
  /\ identityRetained' = FALSE
  /\ UNCHANGED <<completion, containmentConfirmed, sessionOwner,
                 requestInFlight, signalsEmitted, foreignSignals>>

\* An unrelated same-user process calls setsid() and rebinds the freed numeric
\* identifier to its own session.
RebindSession ==
  /\ ~identityRetained
  /\ sessionOwner = "launch"
  /\ sessionOwner' = "foreign"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 requestInFlight, signalsEmitted, foreignSignals>>

\* The Host observes the stale pending engine and admits a graceful or force
\* request for dispatch. The engine may still be finishing independently.
AdmitDispatch ==
  /\ ~requestInFlight
  /\ completion = "pending"
  /\ requestInFlight' = TRUE
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, signalsEmitted, foreignSignals>>

\* The serialized check-and-signal step of the identity fence: the platform
\* effect emits a signal only while the launch identity is retained and
\* containment is unconfirmed.
EmitSignal ==
  /\ requestInFlight
  /\ ~containmentConfirmed
  /\ identityRetained
  /\ signalsEmitted' = TRUE
  /\ requestInFlight' = FALSE
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, foreignSignals>>

\* Once containment is confirmed or the identity is released, the request
\* succeeds as an idempotent no-op without consulting the numeric identifier.
CancelSignal ==
  /\ requestInFlight
  /\ (containmentConfirmed \/ ~identityRetained)
  /\ requestInFlight' = FALSE
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, signalsEmitted, foreignSignals>>

Next ==
  \/ ChildExited
  \/ ConfirmContainment
  \/ ReleaseIdentity
  \/ RebindSession
  \/ AdmitDispatch
  \/ EmitSignal
  \/ CancelSignal

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ completion \in {"pending", "normal"}
  /\ containmentConfirmed \in BOOLEAN
  /\ identityRetained \in BOOLEAN
  /\ sessionOwner \in {"launch", "foreign"}
  /\ requestInFlight \in BOOLEAN
  /\ signalsEmitted \in BOOLEAN
  /\ foreignSignals \in BOOLEAN

\* A foreign session can adopt the numeric identifier only after the launch
\* identity has been released.
ForeignBindRequiresRelease ==
  sessionOwner = "foreign" => ~identityRetained

\* A signal is emitted only through EmitSignal, which observes the retained
\* launch identity in the same serialized step; a released or contained
\* terminal never emits a signal that could reach a rebound session.
\* foreignSignals is the event-tracked defect flag: it is set only if the
\* platform effect were ever emitted without the retained launch identity.
NoForeignSignal ==
  ~foreignSignals

=============================================================================
