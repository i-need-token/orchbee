---------------- MODULE TerminalHostProcessIdentity ----------------
EXTENDS TLC

\* Refines runtime-model claims TH-008, TH-009, and TH-013 for the POSIX
\* process-identity owner (THCR-006). The finite Scenario family separates two
\* claims that must not share a one-candidate shortcut.
\*
\* "pid-race" acquires one stable kernel reference, observes numeric session
\* membership, and permits numeric PID reuse before signal or liveness effects.
\* It proves only per-candidate targeting and retry; it cannot confirm that the
\* whole session is empty.
\*
\* "member-scan" enumerates two abstract session candidates. Each candidate is
\* acquired, classified or attempted, and released before the scan advances.
\* Errors are retained while the remaining candidates are still visited.
\* Containment may be confirmed only after a complete count scan found no live
\* member, observed no error, and released every acquired reference. These
\* transitions abstract ProcessReference outcomes rather than platform calls.
\*
\* The macOS reusable-PID negative prefilter remains outside this authoritative
\* abstraction. RebindSession separately retains the SID-reuse fence after
\* leader release. Registry identity and shutdown remain in their owner models.

ScenarioKinds == {"pid-race", "member-scan"}
SignalKinds == {"HUP", "STOP", "KILL", "CONT"}
OperationKinds == SignalKinds \union {"count"}
MemberIdentities == {"launch-member", "replacement-member"}
MemberSessions == {"owned", "foreign"}
ScanMembers == {"member-a", "member-b"}
ScanStates == {"idle", "acquire", "membership", "effect", "release", "complete"}
ScanMembershipOutcomes == {"member", "nonmember", "error"}
ScanLivenessOutcomes == {"live", "dead", "error"}

VARIABLES
  scenario,
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
  signalTargets,
  countedTargets,
  scanState,
  scanKind,
  scanRemaining,
  scanAcquired,
  scanAttempted,
  scanReleased,
  scanLiveFound,
  scanFirstError,
  scanSignalTargets

vars ==
  <<scenario, completion, containmentConfirmed, identityRetained, sessionOwner,
    requestInFlight, requestKind, memberPidIdentity, memberPidSession,
    memberRef, membershipObserved, memberSignalObservation,
    memberLiveObservation, signalTargets, countedTargets, scanState, scanKind,
    scanRemaining, scanAcquired, scanAttempted, scanReleased, scanLiveFound,
    scanFirstError, scanSignalTargets>>

Init ==
  /\ scenario \in ScenarioKinds
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
  /\ signalTargets = {}
  /\ countedTargets = {}
  /\ scanState = "idle"
  /\ scanKind = "none"
  /\ scanRemaining = {}
  /\ scanAcquired = {}
  /\ scanAttempted = {}
  /\ scanReleased = {}
  /\ scanLiveFound = FALSE
  /\ scanFirstError = FALSE
  /\ scanSignalTargets = {}

ChildExited ==
  /\ completion = "pending"
  /\ completion' = "normal"
  /\ UNCHANGED <<scenario, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, memberRef,
                  membershipObserved, memberSignalObservation,
                  memberLiveObservation, signalTargets, countedTargets,
                  scanState, scanKind, scanRemaining, scanAcquired,
                  scanAttempted, scanReleased, scanLiveFound,
                  scanFirstError, scanSignalTargets>>

AdmitPidDispatch ==
  /\ scenario = "pid-race"
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
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, memberPidIdentity, memberPidSession,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

AdmitScanDispatch(kind) ==
  /\ scenario = "member-scan"
  /\ kind \in OperationKinds
  /\ ~requestInFlight
  /\ identityRetained
  /\ ~containmentConfirmed
  /\ requestInFlight' = TRUE
  /\ requestKind' = kind
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "none"
  /\ memberLiveObservation' = "none"
  /\ scanState' = "acquire"
  /\ scanKind' = kind
  /\ scanRemaining' = ScanMembers
  /\ scanAcquired' = {}
  /\ scanAttempted' = {}
  /\ scanReleased' = {}
  /\ scanLiveFound' = FALSE
  /\ scanFirstError' = FALSE
  /\ scanSignalTargets' = {}
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, memberPidIdentity, memberPidSession,
                  signalTargets, countedTargets>>

AdmitDispatch ==
  \/ AdmitPidDispatch
  \/ \E kind \in OperationKinds: AdmitScanDispatch(kind)

AcquirePidMemberRef ==
  /\ scenario = "pid-race"
  /\ requestInFlight
  /\ memberRef = "none"
  /\ memberRef' = memberPidIdentity
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, membershipObserved,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

AcquireScanMemberRef(member) ==
  /\ scenario = "member-scan"
  /\ requestInFlight
  /\ scanState = "acquire"
  /\ member \in scanRemaining
  /\ memberRef' = member
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "none"
  /\ memberLiveObservation' = "none"
  /\ scanState' = "membership"
  /\ scanRemaining' = scanRemaining \ {member}
  /\ scanAcquired' = scanAcquired \union {member}
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, signalTargets,
                  countedTargets, scanKind, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

AcquireMemberRef ==
  \/ AcquirePidMemberRef
  \/ \E member \in ScanMembers: AcquireScanMemberRef(member)

ObservePidMembership ==
  /\ scenario = "pid-race"
  /\ requestInFlight
  /\ memberRef \in MemberIdentities
  /\ membershipObserved = "none"
  /\ membershipObserved' =
       IF memberPidSession = "owned" THEN "member" ELSE "nonmember"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, memberRef,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

ObserveScanMembership(outcome) ==
  /\ scenario = "member-scan"
  /\ requestInFlight
  /\ scanState = "membership"
  /\ memberRef \in ScanMembers
  /\ outcome \in ScanMembershipOutcomes
  /\ membershipObserved' = outcome
  /\ scanState' = IF outcome = "member" THEN "effect" ELSE "release"
  /\ scanAttempted' =
       IF outcome = "member" THEN scanAttempted
       ELSE scanAttempted \union {memberRef}
  /\ scanFirstError' = (scanFirstError \/ outcome = "error")
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, memberRef,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanKind, scanRemaining,
                  scanAcquired, scanReleased, scanLiveFound,
                  scanSignalTargets>>

ObserveMembership ==
  \/ ObservePidMembership
  \/ \E outcome \in ScanMembershipOutcomes: ObserveScanMembership(outcome)

RecycleMemberPid ==
  /\ scenario = "pid-race"
  /\ identityRetained
  /\ memberPidIdentity = "launch-member"
  /\ memberPidIdentity' = "replacement-member"
  /\ memberPidSession' \in MemberSessions
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind, memberRef,
                  membershipObserved, memberSignalObservation,
                  memberLiveObservation, signalTargets, countedTargets,
                  scanState, scanKind, scanRemaining, scanAcquired,
                  scanAttempted, scanReleased, scanLiveFound,
                  scanFirstError, scanSignalTargets>>

EmitPidSignal ==
  /\ scenario = "pid-race"
  /\ requestInFlight
  /\ requestKind \in SignalKinds
  /\ membershipObserved = "member"
  /\ memberRef = memberPidIdentity
  /\ signalTargets' = signalTargets \union {memberRef}
  /\ requestInFlight' = FALSE
  /\ requestKind' = "none"
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "sent"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, memberPidIdentity, memberPidSession,
                  memberLiveObservation, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

AttemptScanSignal(succeeds) ==
  /\ scenario = "member-scan"
  /\ requestInFlight
  /\ scanState = "effect"
  /\ scanKind \in SignalKinds
  /\ membershipObserved = "member"
  /\ memberRef \in ScanMembers
  /\ succeeds \in BOOLEAN
  /\ memberSignalObservation' = IF succeeds THEN "sent" ELSE "error"
  /\ scanSignalTargets' =
       IF succeeds THEN scanSignalTargets \union {memberRef}
       ELSE scanSignalTargets
  /\ scanFirstError' = (scanFirstError \/ ~succeeds)
  /\ scanAttempted' = scanAttempted \union {memberRef}
  /\ scanState' = "release"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, memberRef,
                  membershipObserved, memberLiveObservation, signalTargets,
                  countedTargets, scanKind, scanRemaining, scanAcquired,
                  scanReleased, scanLiveFound>>

EmitSignal ==
  \/ EmitPidSignal
  \/ \E succeeds \in BOOLEAN: AttemptScanSignal(succeeds)

CancelSignal ==
  /\ scenario = "pid-race"
  /\ requestInFlight
  /\ requestKind \in SignalKinds
  /\ membershipObserved = "nonmember"
  /\ requestInFlight' = FALSE
  /\ requestKind' = "none"
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "cancelled"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, memberPidIdentity, memberPidSession,
                  memberLiveObservation, signalTargets, countedTargets,
                  scanState, scanKind, scanRemaining, scanAcquired,
                  scanAttempted, scanReleased, scanLiveFound,
                  scanFirstError, scanSignalTargets>>

RetrySignal ==
  /\ scenario = "pid-race"
  /\ requestInFlight
  /\ requestKind \in SignalKinds
  /\ membershipObserved = "member"
  /\ memberRef # memberPidIdentity
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ memberSignalObservation' = "retry"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, memberLiveObservation,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

ObservePidLiveness ==
  /\ scenario = "pid-race"
  /\ requestInFlight
  /\ requestKind = "count"
  /\ membershipObserved \in {"member", "nonmember"}
  /\ memberLiveObservation' =
       IF membershipObserved = "member" /\ memberRef = memberPidIdentity
       THEN "live"
       ELSE IF membershipObserved = "member" THEN "retry"
       ELSE "dead"
  /\ countedTargets' =
       IF membershipObserved = "member" /\ memberRef = memberPidIdentity
       THEN countedTargets \union {memberRef}
       ELSE countedTargets
  /\ requestInFlight' = FALSE
  /\ requestKind' = "none"
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, memberPidIdentity, memberPidSession,
                  memberSignalObservation, signalTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

ObserveScanLiveness(outcome) ==
  /\ scenario = "member-scan"
  /\ requestInFlight
  /\ scanState = "effect"
  /\ scanKind = "count"
  /\ membershipObserved = "member"
  /\ memberRef \in ScanMembers
  /\ outcome \in ScanLivenessOutcomes
  /\ memberLiveObservation' = outcome
  /\ scanLiveFound' = (scanLiveFound \/ outcome = "live")
  /\ scanFirstError' = (scanFirstError \/ outcome = "error")
  /\ scanAttempted' = scanAttempted \union {memberRef}
  /\ scanState' = "release"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, requestInFlight, requestKind,
                  memberPidIdentity, memberPidSession, memberRef,
                  membershipObserved, memberSignalObservation, signalTargets,
                  countedTargets, scanKind, scanRemaining, scanAcquired,
                  scanReleased, scanSignalTargets>>

ObserveMemberLiveness ==
  \/ ObservePidLiveness
  \/ \E outcome \in ScanLivenessOutcomes: ObserveScanLiveness(outcome)

ReleaseScannedMember ==
  /\ scenario = "member-scan"
  /\ requestInFlight
  /\ scanState = "release"
  /\ memberRef \in ScanMembers
  /\ memberRef \in scanAttempted
  /\ memberRef \notin scanReleased
  /\ scanReleased' = scanReleased \union {memberRef}
  /\ memberRef' = "none"
  /\ membershipObserved' = "none"
  /\ scanState' = IF scanRemaining = {} THEN "complete" ELSE "acquire"
  /\ requestInFlight' = IF scanRemaining = {} THEN FALSE ELSE requestInFlight
  /\ requestKind' = IF scanRemaining = {} THEN "none" ELSE requestKind
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  sessionOwner, memberPidIdentity, memberPidSession,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanKind, scanRemaining,
                  scanAcquired, scanAttempted, scanLiveFound,
                  scanFirstError, scanSignalTargets>>

ConfirmContainment ==
  /\ scenario = "member-scan"
  /\ completion = "normal"
  /\ ~containmentConfirmed
  /\ identityRetained
  /\ ~requestInFlight
  /\ scanState = "complete"
  /\ scanKind = "count"
  /\ scanRemaining = {}
  /\ scanAcquired = ScanMembers
  /\ scanAttempted = ScanMembers
  /\ scanReleased = ScanMembers
  /\ ~scanLiveFound
  /\ ~scanFirstError
  /\ containmentConfirmed' = TRUE
  /\ UNCHANGED <<scenario, completion, identityRetained, sessionOwner,
                  requestInFlight, requestKind, memberPidIdentity,
                  memberPidSession, memberRef, membershipObserved,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

ReleaseIdentity ==
  /\ scenario = "member-scan"
  /\ completion = "normal"
  /\ containmentConfirmed
  /\ identityRetained
  /\ ~requestInFlight
  /\ identityRetained' = FALSE
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, sessionOwner,
                  requestInFlight, requestKind, memberPidIdentity,
                  memberPidSession, memberRef, membershipObserved,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

RebindSession ==
  /\ scenario = "member-scan"
  /\ ~identityRetained
  /\ sessionOwner = "launch"
  /\ sessionOwner' = "foreign"
  /\ UNCHANGED <<scenario, completion, containmentConfirmed, identityRetained,
                  requestInFlight, requestKind, memberPidIdentity,
                  memberPidSession, memberRef, membershipObserved,
                  memberSignalObservation, memberLiveObservation,
                  signalTargets, countedTargets, scanState, scanKind,
                  scanRemaining, scanAcquired, scanAttempted, scanReleased,
                  scanLiveFound, scanFirstError, scanSignalTargets>>

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
  \/ ReleaseScannedMember
  \/ ConfirmContainment
  \/ ReleaseIdentity
  \/ RebindSession

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ scenario \in ScenarioKinds
  /\ completion \in {"pending", "normal"}
  /\ containmentConfirmed \in BOOLEAN
  /\ identityRetained \in BOOLEAN
  /\ sessionOwner \in {"launch", "foreign"}
  /\ requestInFlight \in BOOLEAN
  /\ requestKind \in OperationKinds \union {"none"}
  /\ memberPidIdentity \in MemberIdentities
  /\ memberPidSession \in MemberSessions
  /\ memberRef \in MemberIdentities \union ScanMembers \union {"none"}
  /\ membershipObserved \in {"none", "member", "nonmember", "error"}
  /\ memberSignalObservation \in {"none", "sent", "cancelled", "retry", "error"}
  /\ memberLiveObservation \in {"none", "live", "dead", "retry", "error"}
  /\ signalTargets \subseteq MemberIdentities
  /\ countedTargets \subseteq MemberIdentities
  /\ scanState \in ScanStates
  /\ scanKind \in OperationKinds \union {"none"}
  /\ scanRemaining \subseteq ScanMembers
  /\ scanAcquired \subseteq ScanMembers
  /\ scanAttempted \subseteq ScanMembers
  /\ scanReleased \subseteq ScanMembers
  /\ scanLiveFound \in BOOLEAN
  /\ scanFirstError \in BOOLEAN
  /\ scanSignalTargets \subseteq ScanMembers

ForeignBindRequiresRelease ==
  sessionOwner = "foreign" => ~identityRetained

ActiveReferenceOwned ==
  memberRef # "none" => requestInFlight

ObservedMembershipHasReference ==
  membershipObserved # "none" => memberRef # "none"

PidRaceCannotConfirmWholeSession ==
  scenario = "pid-race" => ~containmentConfirmed

ContainmentRequiresCompleteScan ==
  containmentConfirmed =>
    /\ scenario = "member-scan"
    /\ scanState = "complete"
    /\ scanKind = "count"
    /\ scanRemaining = {}
    /\ scanAcquired = ScanMembers
    /\ scanAttempted = ScanMembers
    /\ scanReleased = ScanMembers
    /\ ~scanLiveFound
    /\ ~scanFirstError

InvalidatedReferenceCannotConfirmContainment ==
  (memberLiveObservation = "retry" \/ memberSignalObservation = "retry")
    => ~containmentConfirmed

SignalRetryRetainsRequest ==
  memberSignalObservation = "retry" => requestInFlight

OwnedReplacementCannotAppearDead ==
  (scenario = "pid-race" /\
   memberPidIdentity = "replacement-member" /\
   memberPidSession = "owned") => memberLiveObservation # "dead"

NoForeignSignal ==
  memberPidSession = "foreign" => memberPidIdentity \notin signalTargets

NoForeignCount ==
  memberPidSession = "foreign" => memberPidIdentity \notin countedTargets

ScanReferenceAccounting ==
  scenario = "member-scan" /\ scanState # "idle" =>
    /\ scanAcquired = ScanMembers \ scanRemaining
    /\ scanReleased \subseteq scanAttempted
    /\ scanAttempted \subseteq scanAcquired

ScanPhaseOwnsReference ==
  scenario = "member-scan" =>
    /\ (scanState \in {"membership", "effect", "release"}) <=>
         (memberRef \in ScanMembers)
    /\ scanState \in {"idle", "acquire", "complete"} => memberRef = "none"

ScanCompletionExhaustsCandidates ==
  scenario = "member-scan" /\ scanState = "complete" =>
    /\ ~requestInFlight
    /\ scanRemaining = {}
    /\ scanAcquired = ScanMembers
    /\ scanAttempted = ScanMembers
    /\ scanReleased = ScanMembers
    /\ memberRef = "none"

ScanErrorCannotShortCircuit ==
  scenario = "member-scan" /\ scanFirstError /\ scanState = "complete" =>
    /\ scanAttempted = ScanMembers
    /\ scanReleased = ScanMembers

=============================================================================
