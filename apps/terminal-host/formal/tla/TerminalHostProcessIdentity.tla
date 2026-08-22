---------------------------- MODULE TerminalHostProcessIdentity ------------------------
EXTENDS TLC

\* Refines runtime-model claims TH-008, TH-009, and TH-013 for the POSIX
\* process-identity owner (THCR-006). The retained leader identity fences the
\* numeric session identifier until containment and reap. Each independently
\* enumerated member is acquired as a stable kernel reference before the
\* authoritative numeric session-membership observation; signal and liveness
\* effects then use only that reference. The macOS reusable-PID negative
\* prefilter precedes and is outside this authoritative abstraction.
\* RecycleMemberPid may replace the numeric member PID at any
\* point while the leader identity remains retained, including after membership
\* observation and immediately before the effect.
\*
\* The model deliberately distinguishes the numeric PID binding from the stable
\* member reference. EmitSignal and ObserveMemberLiveness can act on the captured
\* identity only while it is still live. If the numeric PID has been recycled,
\* both signaling and containment liveness retry from a fresh reference rather
\* than treating a possibly same-session replacement as success or empty.
\* Neither operation retargets the replacement through the stale reference.
\* RebindSession
\* separately retains the existing SID-reuse fence after leader release.
\* Registry identity, broker retention, and multi-runtime shutdown remain owned
\* by TerminalHostRegistry and TerminalHostShutdown.

SignalKinds == {"HUP", "STOP", "KILL", "CONT"}
OperationKinds == SignalKinds \union {"count"}
MemberIdentities == {"launch-member", "replacement-member"}
MemberSessions == {"owned", "foreign"}

VARIABLES
  completion,
  containmentConfirmed,
  identityRetained,
  sessionOwner,
  requestInFlight,
  requestKind,
  memberPidIdentity,
  memberPidSession,
  memberRef,
  membershipObserved,
  memberSignalObservation,
  memberLiveObservation,
  signaledSessionOwners,
  signalTargets,
  countedTargets

vars == <<completion, containmentConfirmed, identityRetained, sessionOwner,
          requestInFlight, requestKind, memberPidIdentity, memberPidSession, memberRef,
          membershipObserved, memberSignalObservation, memberLiveObservation,
          signaledSessionOwners,
          signalTargets, countedTargets>>

Init ==
  /\ completion = "pending"
  /\ containmentConfirmed = FALSE
  /\ identityRetained = TRUE
  /\ sessionOwner = "launch"
  /\ requestInFlight = FALSE
  /\ requestKind = "none"
  /\ memberPidIdentity = "launch-member"
  /\ memberPidSession = "owned"
  /\ memberRef = "none"
  /\ membershipObserved = "none"
  /\ memberSignalObservation = "none"
  /\ memberLiveObservation = "none"
  /\ signaledSessionOwners = {}
  /\ signalTargets = {}
  /\ countedTargets = {}

\* The engine samples child completion without reaping the retained leader.
ChildExited ==
  /\ completion = "pending"
  /\ completion' = "normal"
  /\ UNCHANGED <<containmentConfirmed, identityRetained, sessionOwner,
                 requestInFlight, requestKind, memberPidIdentity, memberPidSession, memberRef,
                 membershipObserved, memberSignalObservation, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

\* Graceful, force-step, and liveness operations share the same stable-member
\* scan workflow while the retained leader/SID fence is open.
AdmitDispatch ==
  /\ ~requestInFlight
  /\ identityRetained
  /\ ~containmentConfirmed
  /\ requestInFlight' = TRUE
  /\ IF memberLiveObservation = "retry"
       THEN requestKind' = "count"
       ELSE requestKind' \in OperationKinds
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "none"
  /\ memberLiveObservation' = "none"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, memberPidIdentity, memberPidSession, signaledSessionOwners,
                 signalTargets, countedTargets>>

\* Capture the identity currently bound to the numeric PID before asking for
\* its numeric session membership.
AcquireMemberRef ==
  /\ requestInFlight
  /\ memberRef = "none"
  /\ memberRef' = memberPidIdentity
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, requestInFlight, requestKind, memberPidIdentity,
                 memberPidSession,
                 membershipObserved, memberSignalObservation, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

\* getsid-style membership observation occurs only after stable acquisition.
\* Membership is independent of the identity currently bound to the numeric
\* PID: a replacement may either remain in the owned session or be foreign.
ObserveMembership ==
  /\ requestInFlight
  /\ memberRef # "none"
  /\ membershipObserved = "none"
  /\ membershipObserved' =
       IF memberPidSession = "owned" THEN "member" ELSE "nonmember"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, requestInFlight, requestKind, memberPidIdentity,
                 memberPidSession, memberRef, memberSignalObservation,
                 memberLiveObservation, signaledSessionOwners,
                 signalTargets, countedTargets>>

\* Arbitrary PID reuse is allowed while the session leader itself is still
\* retained. In particular, this may occur between ObserveMembership and the
\* signal or liveness effect.
RecycleMemberPid ==
  /\ identityRetained
  /\ memberPidIdentity = "launch-member"
  /\ memberPidIdentity' = "replacement-member"
  /\ memberPidSession' \in MemberSessions
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, requestInFlight, requestKind, memberRef,
                 membershipObserved, memberSignalObservation, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

\* A signal can be emitted only through the still-live captured reference.
\* Equality here is reference liveness, not a second numeric-PID lookup.
EmitSignal ==
  /\ requestInFlight
  /\ requestKind \in SignalKinds
  /\ membershipObserved = "member"
  /\ memberRef = memberPidIdentity
  /\ signaledSessionOwners' = signaledSessionOwners \union {sessionOwner}
  /\ signalTargets' = signalTargets \union {memberRef}
  /\ requestInFlight' = FALSE
  /\ requestKind' = "none"
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "sent"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, memberPidIdentity, memberPidSession, memberLiveObservation,
                 countedTargets>>

\* An authoritatively observed non-member makes signaling an idempotent no-op.
CancelSignal ==
  /\ requestInFlight
  /\ requestKind \in SignalKinds
  /\ membershipObserved = "nonmember"
  /\ requestInFlight' = FALSE
  /\ requestKind' = "none"
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "cancelled"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, memberPidIdentity, memberPidSession, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

\* A matched member reference invalidated before the effect cannot report
\* signal success. Clear only the stale reference and repeat acquisition and
\* authoritative membership observation for the same in-flight signal.
RetrySignal ==
  /\ requestInFlight
  /\ requestKind \in SignalKinds
  /\ membershipObserved = "member"
  /\ memberRef # memberPidIdentity
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "retry"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, requestInFlight, requestKind, memberPidIdentity,
                 memberPidSession, memberLiveObservation, signaledSessionOwners,
                 signalTargets, countedTargets>>

\* Liveness counts the captured identity only if the observed member reference
\* is still live. An invalidated member reference requests a fresh scan instead
\* of treating a possibly same-session replacement as an empty observation.
ObserveMemberLiveness ==
  /\ requestInFlight
  /\ requestKind = "count"
  /\ membershipObserved # "none"
  /\ memberLiveObservation' =
       IF membershipObserved = "member" /\ memberRef = memberPidIdentity
       THEN "live"
       ELSE IF membershipObserved = "member"
       THEN "retry"
       ELSE "dead"
  /\ countedTargets' =
       IF membershipObserved = "member" /\ memberRef = memberPidIdentity
       THEN countedTargets \union {memberRef}
       ELSE countedTargets
  /\ requestInFlight' = FALSE
  /\ requestKind' = "none"
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 sessionOwner, memberPidIdentity, memberPidSession, signaledSessionOwners,
                 memberSignalObservation, signalTargets>>

\* The wait worker confirms the owned process domain only from a completed
\* stable-reference liveness observation that found no live member.
ConfirmContainment ==
  /\ completion = "normal"
  /\ ~containmentConfirmed
  /\ identityRetained
  /\ ~requestInFlight
  /\ memberLiveObservation = "dead"
  /\ containmentConfirmed' = TRUE
  /\ UNCHANGED <<completion, identityRetained, sessionOwner,
                 requestInFlight, requestKind, memberPidIdentity, memberPidSession, memberRef,
                 membershipObserved, memberSignalObservation, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

\* The wait worker reaps the session leader: the numeric identity is freed.
ReleaseIdentity ==
  /\ completion = "normal"
  /\ containmentConfirmed
  /\ identityRetained
  /\ ~requestInFlight
  /\ identityRetained' = FALSE
  /\ UNCHANGED <<completion, containmentConfirmed, sessionOwner,
                 requestInFlight, requestKind, memberPidIdentity, memberPidSession, memberRef,
                 membershipObserved, memberSignalObservation, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

\* An unrelated same-user process calls setsid() and rebinds the released
\* numeric session identifier to its own session.
RebindSession ==
  /\ ~identityRetained
  /\ sessionOwner = "launch"
  /\ sessionOwner' = "foreign"
  /\ UNCHANGED <<completion, containmentConfirmed, identityRetained,
                 requestInFlight, requestKind, memberPidIdentity, memberPidSession, memberRef,
                 membershipObserved, memberSignalObservation, memberLiveObservation,
                 signaledSessionOwners, signalTargets, countedTargets>>

Next ==
  \/ ChildExited
  \/ AdmitDispatch
  \/ AcquireMemberRef
  \/ ObserveMembership
  \/ RecycleMemberPid
  \/ EmitSignal
  \/ CancelSignal
  \/ RetrySignal
  \/ ObserveMemberLiveness
  \/ ConfirmContainment
  \/ ReleaseIdentity
  \/ RebindSession

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ completion \in {"pending", "normal"}
  /\ containmentConfirmed \in BOOLEAN
  /\ identityRetained \in BOOLEAN
  /\ sessionOwner \in {"launch", "foreign"}
  /\ requestInFlight \in BOOLEAN
  /\ requestKind \in OperationKinds \union {"none"}
  /\ memberPidIdentity \in MemberIdentities
  /\ memberPidSession \in MemberSessions
  /\ memberRef \in MemberIdentities \union {"none"}
  /\ membershipObserved \in {"none", "member", "nonmember"}
  /\ memberSignalObservation \in {"none", "sent", "cancelled", "retry"}
  /\ memberLiveObservation \in {"none", "live", "dead", "retry"}
  /\ signaledSessionOwners \subseteq {"launch", "foreign"}
  /\ signalTargets \subseteq MemberIdentities
  /\ countedTargets \subseteq MemberIdentities

ForeignBindRequiresRelease ==
  sessionOwner = "foreign" => ~identityRetained

ActiveReferenceOwned ==
  memberRef # "none" => requestInFlight

ObservedMembershipHasReference ==
  membershipObserved # "none" => memberRef # "none"

ContainmentRequiresStableEmptyObservation ==
  containmentConfirmed => memberLiveObservation = "dead"

InvalidatedReferenceCannotConfirmContainment ==
  (memberLiveObservation = "retry" \/ memberSignalObservation = "retry")
    => ~containmentConfirmed

SignalRetryRetainsRequest ==
  memberSignalObservation = "retry" => requestInFlight

OwnedReplacementCannotAppearDead ==
  (memberPidIdentity = "replacement-member" /\ memberPidSession = "owned")
    => memberLiveObservation # "dead"

NoForeignSignal ==
  memberPidSession = "foreign" => memberPidIdentity \notin signalTargets

NoForeignSessionSignal ==
  "foreign" \notin signaledSessionOwners

NoForeignCount ==
  memberPidSession = "foreign" => memberPidIdentity \notin countedTargets

=============================================================================
