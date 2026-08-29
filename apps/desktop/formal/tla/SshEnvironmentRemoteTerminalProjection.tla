---------------- MODULE SshEnvironmentRemoteTerminalProjection ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded model of Slice 3.2C. A projection request carries only a
\* Presentation identity and geometry. Main resolves its local or exact SSH
\* Environment target from the canonical document, requires an already-prepared
\* remote runtime, and rechecks that target before publishing a live attachment.
\* Canonical deletion may race an already-admitted terminal start, but it can
\* never publish the stale attachment. Disconnect retires remote attachments and
\* projection work never activates SSH or falls back to the local runtime.

CONSTANT Requests, Environments

LocalTarget == "local"
NoTarget == "no-target"
NoRequest == "no-request"
Targets == {LocalTarget} \cup Environments
CanonicalTargets == Targets \cup {NoTarget}
RequestPhases == {
  "idle", "requested", "resolved", "opening", "attached",
  "published", "failed", "released"
}
Actions == {
  "init", "connect", "prepare-remote-runtime", "request-projection",
  "resolve-canonical-target", "reject-missing-target",
  "reject-unprepared-remote", "begin-local-open", "begin-remote-open",
  "start-terminal", "attach-projection", "delete-canonical-target",
  "reject-client-retarget", "publish-projection",
  "discard-stale-projection", "disconnect",
  "fail-remote-open-after-disconnect", "release-projection"
}

VARIABLE s

vars == <<s>>

Init ==
  \E target \in Targets:
    s = [
      canonicalTarget |-> target,
      connected |-> FALSE,
      remoteReady |-> FALSE,
      requestPhase |-> [request \in Requests |-> "idle"],
      resolvedTarget |-> [request \in Requests |-> NoTarget],
      leaseTarget |-> [request \in Requests |-> NoTarget],
      attachments |-> {},
      startedTargets |-> {},
      startCount |-> [candidate \in Targets |-> 0],
      lastAction |-> "init",
      lastRequest |-> NoRequest,
      previousConnected |-> FALSE,
      previousRemoteReady |-> FALSE
    ]

Record(next, action, request) ==
  [next EXCEPT
    !.lastAction = action,
    !.lastRequest = request,
    !.previousConnected = s.connected,
    !.previousRemoteReady = s.remoteReady]

Connect ==
  /\ ~s.connected
  /\ s' = Record([s EXCEPT !.connected = TRUE], "connect", NoRequest)

PrepareRemoteRuntime ==
  /\ s.connected
  /\ ~s.remoteReady
  /\ s.canonicalTarget \in Environments
  /\ s' = Record([s EXCEPT !.remoteReady = TRUE], "prepare-remote-runtime", NoRequest)

RequestProjection(request) ==
  /\ s.requestPhase[request] = "idle"
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "requested"],
       "request-projection",
       request)

ResolveCanonicalTarget(request) ==
  /\ s.requestPhase[request] = "requested"
  /\ s.canonicalTarget \in Targets
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[request] = "resolved",
         !.resolvedTarget[request] = s.canonicalTarget],
       "resolve-canonical-target",
       request)

RejectMissingTarget(request) ==
  /\ s.requestPhase[request] = "requested"
  /\ s.canonicalTarget = NoTarget
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "failed"],
       "reject-missing-target",
       request)

RejectUnpreparedRemote(request) ==
  /\ s.requestPhase[request] = "resolved"
  /\ s.resolvedTarget[request] \in Environments
  /\ ~s.remoteReady
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "failed"],
       "reject-unprepared-remote",
       request)

BeginLocalOpen(request) ==
  /\ s.requestPhase[request] = "resolved"
  /\ s.resolvedTarget[request] = LocalTarget
  /\ s.canonicalTarget = LocalTarget
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "opening"],
       "begin-local-open",
       request)

BeginRemoteOpen(request) ==
  /\ s.requestPhase[request] = "resolved"
  /\ s.resolvedTarget[request] \in Environments
  /\ s.canonicalTarget = s.resolvedTarget[request]
  /\ s.remoteReady
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "opening"],
       "begin-remote-open",
       request)

StartTerminal(request) ==
  LET target == s.resolvedTarget[request] IN
    /\ s.requestPhase[request] = "opening"
    /\ target \in Targets
    /\ s.canonicalTarget = target
    /\ target \notin s.startedTargets
    /\ target \in Environments => s.remoteReady
    /\ s' = Record(
         [s EXCEPT
           !.startedTargets = @ \cup {target},
           !.startCount[target] = @ + 1],
         "start-terminal",
         request)

AttachProjection(request) ==
  LET target == s.resolvedTarget[request] IN
    /\ s.requestPhase[request] = "opening"
    /\ target \in s.startedTargets
    /\ s.canonicalTarget = target
    /\ target \in Environments => s.remoteReady
    /\ s' = Record(
         [s EXCEPT
           !.requestPhase[request] = "attached",
           !.leaseTarget[request] = target,
           !.attachments = @ \cup {request}],
         "attach-projection",
         request)

DeleteCanonicalTarget ==
  /\ s.canonicalTarget \in Targets
  /\ s' = Record(
       [s EXCEPT !.canonicalTarget = NoTarget],
       "delete-canonical-target",
       NoRequest)

RejectClientRetarget ==
  /\ s.canonicalTarget \in Environments
  /\ s' = Record(s, "reject-client-retarget", NoRequest)

PublishProjection(request) ==
  /\ s.requestPhase[request] = "attached"
  /\ request \in s.attachments
  /\ s.leaseTarget[request] = s.resolvedTarget[request]
  /\ s.canonicalTarget = s.leaseTarget[request]
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "published"],
       "publish-projection",
       request)

DiscardStaleProjection(request) ==
  /\ s.requestPhase[request] \in {"resolved", "opening", "attached"}
  /\ s.resolvedTarget[request] \in Targets
  /\ s.canonicalTarget # s.resolvedTarget[request]
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[request] = "failed",
         !.attachments = @ \ {request}],
       "discard-stale-projection",
       request)

RemoteAttachments ==
  {request \in s.attachments : s.leaseTarget[request] \in Environments}

Disconnect ==
  /\ s.connected
  /\ s' = Record(
       [s EXCEPT
         !.connected = FALSE,
         !.remoteReady = FALSE,
         !.requestPhase = [request \in Requests |->
           IF request \in RemoteAttachments
           THEN "failed"
           ELSE s.requestPhase[request]],
         !.attachments = @ \ RemoteAttachments],
       "disconnect",
       NoRequest)

FailRemoteOpenAfterDisconnect(request) ==
  /\ s.requestPhase[request] \in {"resolved", "opening"}
  /\ s.resolvedTarget[request] \in Environments
  /\ ~s.remoteReady
  /\ s' = Record(
       [s EXCEPT !.requestPhase[request] = "failed"],
       "fail-remote-open-after-disconnect",
       request)

ReleaseProjection(request) ==
  /\ s.requestPhase[request] = "published"
  /\ request \in s.attachments
  /\ s' = Record(
       [s EXCEPT
         !.requestPhase[request] = "released",
         !.attachments = @ \ {request}],
       "release-projection",
       request)

Next ==
  \/ Connect
  \/ PrepareRemoteRuntime
  \/ \E request \in Requests: RequestProjection(request)
  \/ \E request \in Requests: ResolveCanonicalTarget(request)
  \/ \E request \in Requests: RejectMissingTarget(request)
  \/ \E request \in Requests: RejectUnpreparedRemote(request)
  \/ \E request \in Requests: BeginLocalOpen(request)
  \/ \E request \in Requests: BeginRemoteOpen(request)
  \/ \E request \in Requests: StartTerminal(request)
  \/ \E request \in Requests: AttachProjection(request)
  \/ DeleteCanonicalTarget
  \/ RejectClientRetarget
  \/ \E request \in Requests: PublishProjection(request)
  \/ \E request \in Requests: DiscardStaleProjection(request)
  \/ Disconnect
  \/ \E request \in Requests: FailRemoteOpenAfterDisconnect(request)
  \/ \E request \in Requests: ReleaseProjection(request)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.canonicalTarget \in CanonicalTargets
  /\ s.connected \in BOOLEAN
  /\ s.remoteReady \in BOOLEAN
  /\ s.requestPhase \in [Requests -> RequestPhases]
  /\ s.resolvedTarget \in [Requests -> CanonicalTargets]
  /\ s.leaseTarget \in [Requests -> CanonicalTargets]
  /\ s.attachments \subseteq Requests
  /\ s.startedTargets \subseteq Targets
  /\ s.startCount \in [Targets -> 0..1]
  /\ s.lastAction \in Actions
  /\ s.lastRequest \in Requests \cup {NoRequest}
  /\ s.previousConnected \in BOOLEAN
  /\ s.previousRemoteReady \in BOOLEAN

AttachmentMatchesResolvedTarget ==
  \A request \in s.attachments:
    /\ s.requestPhase[request] \in {"attached", "published"}
    /\ s.leaseTarget[request] = s.resolvedTarget[request]
    /\ s.leaseTarget[request] \in Targets

NoLocalFallback ==
  \A request \in Requests:
    s.resolvedTarget[request] \in Environments =>
      s.leaseTarget[request] # LocalTarget

StartOccursAtMostOncePerTarget ==
  \A target \in Targets:
    /\ s.startCount[target] \in 0..1
    /\ (target \in s.startedTargets) = (s.startCount[target] = 1)

StartRequiresCurrentAuthority ==
  s.lastAction = "start-terminal" =>
    /\ s.lastRequest \in Requests
    /\ s.resolvedTarget[s.lastRequest] = s.canonicalTarget

RemoteStartRequiresPreparedRuntime ==
  /\ s.lastAction = "start-terminal"
  /\ s.lastRequest \in Requests
  /\ s.resolvedTarget[s.lastRequest] \in Environments
  => s.remoteReady

PublishRequiresCurrentAuthority ==
  s.lastAction = "publish-projection" =>
    /\ s.lastRequest \in s.attachments
    /\ s.requestPhase[s.lastRequest] = "published"
    /\ s.leaseTarget[s.lastRequest] = s.canonicalTarget

ProjectionDoesNotActivateSsh ==
  s.lastAction \notin {"connect", "prepare-remote-runtime", "disconnect"} =>
    /\ s.connected = s.previousConnected
    /\ s.remoteReady = s.previousRemoteReady

DisconnectRetiresRemoteAttachments ==
  s.lastAction = "disconnect" =>
    \A request \in s.attachments:
      s.leaseTarget[request] \notin Environments

=============================================================================
