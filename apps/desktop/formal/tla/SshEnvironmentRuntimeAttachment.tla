------------------- MODULE SshEnvironmentRuntimeAttachment -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction of the Slice 3.1B SSH Environment cohort activation
\* spine. Desktop authenticates one signed Runtime plus Terminal Host catalog
\* before opening a transport. The same SSH child reports one platform and the
\* exact state of both immutable components, requests only absent bytes in
\* fixed role order, and crosses the activation barrier only after the complete
\* cohort is present. A detached per-Environment Runtime daemon outlives
\* replaceable attachment generations; incompatible installed or running state
\* blocks instead of being overwritten, killed, or downgraded.

CONSTANT MaxGeneration, Epochs, Platforms

NoGeneration == 0
NoEpoch == "none"
UnsupportedPlatform == "unsupported"
Components == {"ssh-environment-runtime", "terminal-host"}
RuntimeRole == "runtime"
TerminalHostRole == "terminal-host"
NoBootstrapRole == "none"
BootstrapComplete == "complete"
BootstrapRoles == {NoBootstrapRole, RuntimeRole, TerminalHostRole, BootstrapComplete}
ComponentForRole(role) ==
  IF role = RuntimeRole THEN "ssh-environment-runtime" ELSE "terminal-host"
NextBootstrapRole(role) ==
  IF role = RuntimeRole THEN TerminalHostRole ELSE BootstrapComplete
InstallDigests == {"absent", "expected", "mismatch"}
InstallModes == {"none", "present", "upload"}
NoHello == "no-hello"
ExactHello == "exact"
HelloCandidates == {
  ExactHello, "missing-service", "extra-service", "reversed-services",
  "metadata-mismatch"
}
HelloIsExact(candidate) == candidate = ExactHello
Phases == {
  "stopped", "verifying-catalog", "negotiating", "uploading",
  "daemon-ready", "attaching", "handshaking", "connected",
  "retry-wait", "blocked"
}
Actions == {
  "init", "begin-verification", "verify-catalog", "reject-catalog",
  "report-present", "request-upload", "reject-installed-mismatch",
  "reject-unsupported", "reject-bootstrap-order", "upload-component",
  "concurrent-install-wins",
  "converge-cohort", "reject-concurrent-mismatch", "start-daemon",
  "begin-handshake", "accept-hello", "reject-hello", "heartbeat",
  "lose-attachment", "retry-elapsed", "restart-daemon",
  "reject-competing-attachment", "reject-unadvertised-service", "stop",
  "repeat-stop", "stale-hello"
}

AllExpected(digests) ==
  \A component \in Components : digests[component] = "expected"

AbsentComponents(digests) ==
  {component \in Components : digests[component] = "absent"}

MismatchComponents(digests) ==
  {component \in Components : digests[component] = "mismatch"}

VARIABLES
  phase,
  generation,
  catalogVerified,
  remotePlatform,
  installedDigests,
  installMode,
  requestedUploads,
  loadedArtifacts,
  daemonEpoch,
  attachmentGeneration,
  helloGeneration,
  knownEpoch,
  remoteWorkRetained,
  replayedUnknownEffect,
  advertisedServices,
  bootstrapRole,
  lastAction

vars == <<phase, generation, catalogVerified, remotePlatform,
  installedDigests, installMode, requestedUploads, loadedArtifacts,
  daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
  remoteWorkRetained, replayedUnknownEffect, advertisedServices,
  bootstrapRole, lastAction>>

Init ==
  /\ phase = "stopped"
  /\ generation = NoGeneration
  /\ catalogVerified = FALSE
  /\ remotePlatform \in Platforms \cup {UnsupportedPlatform}
  /\ installedDigests \in [Components -> InstallDigests]
  /\ installMode = "none"
  /\ requestedUploads = {}
  /\ loadedArtifacts = {}
  /\ daemonEpoch = NoEpoch
  /\ attachmentGeneration = NoGeneration
  /\ helloGeneration = NoGeneration
  /\ knownEpoch = NoEpoch
  /\ remoteWorkRetained = TRUE
  /\ replayedUnknownEffect = FALSE
  /\ advertisedServices = NoHello
  /\ bootstrapRole = NoBootstrapRole
  /\ lastAction = "init"

BeginVerification ==
  /\ phase = "stopped"
  /\ generation < MaxGeneration
  /\ phase' = "verifying-catalog"
  /\ generation' = generation + 1
  /\ catalogVerified' = FALSE
  /\ installMode' = "none"
  /\ requestedUploads' = {}
  /\ loadedArtifacts' = {}
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = NoHello
  /\ bootstrapRole' = NoBootstrapRole
  /\ UNCHANGED <<remotePlatform, installedDigests, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "begin-verification"

VerifyCatalog ==
  /\ phase = "verifying-catalog"
  /\ phase' = "negotiating"
  /\ catalogVerified' = TRUE
  /\ bootstrapRole' = RuntimeRole
  /\ UNCHANGED <<generation, remotePlatform, installedDigests, installMode,
       requestedUploads, loadedArtifacts, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "verify-catalog"

RejectCatalog ==
  /\ phase = "verifying-catalog"
  /\ phase' = "blocked"
  /\ catalogVerified' = FALSE
  /\ UNCHANGED <<generation, remotePlatform, installedDigests, installMode,
       requestedUploads, loadedArtifacts, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices, bootstrapRole>>
  /\ lastAction' = "reject-catalog"

ReportPresent(component) ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component = ComponentForRole(bootstrapRole)
  /\ installedDigests[component] = "expected"
  /\ bootstrapRole' = NextBootstrapRole(bootstrapRole)
  /\ installMode' = IF requestedUploads = {} THEN "present" ELSE installMode
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, requestedUploads, loadedArtifacts, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "report-present"

RequestUpload(component) ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component = ComponentForRole(bootstrapRole)
  /\ installedDigests[component] = "absent"
  /\ phase' = "uploading"
  /\ installMode' = "upload"
  /\ requestedUploads' = requestedUploads \cup {component}
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, loadedArtifacts, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices, bootstrapRole>>
  /\ lastAction' = "request-upload"

RejectInstalledMismatch(component) ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component = ComponentForRole(bootstrapRole)
  /\ installedDigests[component] = "mismatch"
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "reject-installed-mismatch"

RejectUnsupported ==
  /\ phase = "negotiating"
  /\ remotePlatform = UnsupportedPlatform
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "reject-unsupported"

RejectBootstrapOrder(component) ==
  /\ phase \in {"negotiating", "uploading"}
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component \in Components \ {ComponentForRole(bootstrapRole)}
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "reject-bootstrap-order"

UploadComponent(component) ==
  /\ phase = "uploading"
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component \in requestedUploads
  /\ component = ComponentForRole(bootstrapRole)
  /\ installedDigests[component] = "absent"
  /\ phase' = "negotiating"
  /\ installedDigests' = [installedDigests EXCEPT ![component] = "expected"]
  /\ loadedArtifacts' = loadedArtifacts \cup {component}
  /\ bootstrapRole' = NextBootstrapRole(bootstrapRole)
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installMode, requestedUploads, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "upload-component"

ConcurrentInstallWins(component) ==
  /\ phase = "uploading"
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component \in requestedUploads
  /\ component = ComponentForRole(bootstrapRole)
  /\ installedDigests[component] = "absent"
  /\ phase' = "negotiating"
  /\ installedDigests' = [installedDigests EXCEPT ![component] = "expected"]
  /\ bootstrapRole' = NextBootstrapRole(bootstrapRole)
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installMode, requestedUploads, loadedArtifacts, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "concurrent-install-wins"

ConvergeCohort ==
  /\ phase = "negotiating"
  /\ bootstrapRole = BootstrapComplete
  /\ AllExpected(installedDigests)
  /\ phase' = "daemon-ready"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "converge-cohort"

RejectConcurrentMismatch(component) ==
  /\ phase = "uploading"
  /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
  /\ component \in requestedUploads
  /\ component = ComponentForRole(bootstrapRole)
  /\ installedDigests[component] = "absent"
  /\ phase' = "blocked"
  /\ installedDigests' = [installedDigests EXCEPT ![component] = "mismatch"]
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform, installMode,
       requestedUploads, loadedArtifacts, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices, bootstrapRole>>
  /\ lastAction' = "reject-concurrent-mismatch"

StartDaemon ==
  /\ phase = "daemon-ready"
  /\ catalogVerified
  /\ remotePlatform \in Platforms
  /\ AllExpected(installedDigests)
  /\ phase' = "attaching"
  /\ attachmentGeneration' = generation
  /\ IF daemonEpoch = NoEpoch
       THEN \E epoch \in Epochs : daemonEpoch' = epoch
       ELSE daemonEpoch' = daemonEpoch
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices, bootstrapRole>>
  /\ lastAction' = "start-daemon"

BeginHandshake ==
  /\ phase = "attaching"
  /\ attachmentGeneration = generation
  /\ phase' = "handshaking"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "begin-handshake"

AcceptHello(candidate) ==
  /\ phase = "handshaking"
  /\ attachmentGeneration = generation
  /\ daemonEpoch \in Epochs
  /\ candidate \in HelloCandidates
  /\ HelloIsExact(candidate)
  /\ phase' = "connected"
  /\ helloGeneration' = generation
  /\ knownEpoch' = daemonEpoch
  /\ advertisedServices' = candidate
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, remoteWorkRetained,
       replayedUnknownEffect, bootstrapRole>>
  /\ lastAction' = "accept-hello"

RejectHello(candidate) ==
  /\ phase = "handshaking"
  /\ candidate \in HelloCandidates
  /\ ~HelloIsExact(candidate)
  /\ phase' = "blocked"
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = candidate
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       bootstrapRole>>
  /\ lastAction' = "reject-hello"

\* Named aliases keep all invalid Hello equivalence classes visible to the
\* inventory coverage gate without changing the transition graph.
RejectHelloMissingService ==
  /\ attachmentGeneration = generation
  /\ daemonEpoch \in Epochs
  /\ RejectHello("missing-service")

RejectHelloExtraService ==
  /\ attachmentGeneration = generation
  /\ daemonEpoch \in Epochs
  /\ RejectHello("extra-service")

RejectHelloReversedServices ==
  /\ attachmentGeneration = generation
  /\ daemonEpoch \in Epochs
  /\ RejectHello("reversed-services")

RejectHelloMetadataMismatch ==
  /\ attachmentGeneration = generation
  /\ daemonEpoch \in Epochs
  /\ RejectHello("metadata-mismatch")

Heartbeat ==
  /\ phase = "connected"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "heartbeat"

LoseAttachment ==
  /\ phase = "connected"
  /\ generation < MaxGeneration
  /\ phase' = "retry-wait"
  /\ generation' = generation + 1
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ installMode' = "none"
  /\ requestedUploads' = {}
  /\ loadedArtifacts' = {}
  /\ bootstrapRole' = NoBootstrapRole
  /\ UNCHANGED <<catalogVerified, remotePlatform, installedDigests,
       daemonEpoch, knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "lose-attachment"

RetryElapsed ==
  /\ phase = "retry-wait"
  /\ phase' = "verifying-catalog"
  /\ catalogVerified' = FALSE
  /\ UNCHANGED <<generation, remotePlatform, installedDigests, installMode,
       requestedUploads, loadedArtifacts, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices, bootstrapRole>>
  /\ lastAction' = "retry-elapsed"

RestartDaemon ==
  /\ phase = "retry-wait"
  /\ daemonEpoch \in Epochs
  /\ \E epoch \in (Epochs \ {daemonEpoch}) : daemonEpoch' = epoch
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices, bootstrapRole>>
  /\ lastAction' = "restart-daemon"

RejectCompetingAttachment ==
  /\ attachmentGeneration = generation
  /\ phase \in {"attaching", "handshaking", "connected"}
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "reject-competing-attachment"

RejectUnadvertisedService ==
  /\ phase = "connected"
  /\ HelloIsExact(advertisedServices)
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "reject-unadvertised-service"

Stop ==
  /\ phase # "stopped"
  /\ phase' = "stopped"
  /\ generation' = IF generation < MaxGeneration THEN generation + 1 ELSE generation
  /\ catalogVerified' = FALSE
  /\ installMode' = "none"
  /\ requestedUploads' = {}
  /\ loadedArtifacts' = {}
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = NoHello
  /\ bootstrapRole' = NoBootstrapRole
  /\ UNCHANGED <<remotePlatform, installedDigests, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "stop"

RepeatStop ==
  /\ phase = "stopped"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "repeat-stop"

StaleHello(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices,
       bootstrapRole>>
  /\ lastAction' = "stale-hello"

Next ==
  \/ BeginVerification
  \/ VerifyCatalog
  \/ RejectCatalog
  \/ \E component \in Components: ReportPresent(component)
  \/ \E component \in Components: RequestUpload(component)
  \/ \E component \in Components: RejectInstalledMismatch(component)
  \/ RejectUnsupported
  \/ \E component \in Components: RejectBootstrapOrder(component)
  \/ \E component \in Components : UploadComponent(component)
  \/ \E component \in Components : ConcurrentInstallWins(component)
  \/ ConvergeCohort
  \/ \E component \in Components : RejectConcurrentMismatch(component)
  \/ StartDaemon
  \/ BeginHandshake
  \/ \E candidate \in HelloCandidates: AcceptHello(candidate)
  \/ \E candidate \in HelloCandidates: RejectHello(candidate)
  \/ RejectHelloMissingService
  \/ RejectHelloExtraService
  \/ RejectHelloReversedServices
  \/ RejectHelloMetadataMismatch
  \/ Heartbeat
  \/ LoseAttachment
  \/ RetryElapsed
  \/ RestartDaemon
  \/ RejectCompetingAttachment
  \/ RejectUnadvertisedService
  \/ Stop
  \/ RepeatStop
  \/ \E staleGeneration \in 0..MaxGeneration : StaleHello(staleGeneration)

Spec == Init /\ [][Next]_vars /\ WF_vars(RetryElapsed)

TypeOK ==
  /\ phase \in Phases
  /\ generation \in 0..MaxGeneration
  /\ catalogVerified \in BOOLEAN
  /\ remotePlatform \in Platforms \cup {UnsupportedPlatform}
  /\ installedDigests \in [Components -> InstallDigests]
  /\ installMode \in InstallModes
  /\ requestedUploads \subseteq Components
  /\ loadedArtifacts \subseteq Components
  /\ daemonEpoch \in Epochs \cup {NoEpoch}
  /\ attachmentGeneration \in 0..MaxGeneration
  /\ helloGeneration \in 0..MaxGeneration
  /\ knownEpoch \in Epochs \cup {NoEpoch}
  /\ remoteWorkRetained \in BOOLEAN
  /\ replayedUnknownEffect \in BOOLEAN
  /\ advertisedServices \in HelloCandidates \cup {NoHello}
  /\ bootstrapRole \in BootstrapRoles
  /\ lastAction \in Actions

SingleCurrentAttachment ==
  attachmentGeneration \in {NoGeneration, generation}

HelloBelongsToAttachment ==
  helloGeneration = NoGeneration \/ helloGeneration = attachmentGeneration

CatalogPrecedesTransport ==
  phase \notin {"negotiating", "uploading", "daemon-ready", "attaching",
    "handshaking", "connected"} \/ catalogVerified

UploadReadsOnlyOnRequest ==
  loadedArtifacts \subseteq requestedUploads /\
    (loadedArtifacts = {} \/ installMode = "upload")

PresentSkipsArtifactRead ==
  installMode # "present" \/ loadedArtifacts = {}

ActivationRequiresWholeCohort ==
  phase \notin {"daemon-ready", "attaching", "handshaking", "connected"} \/
    (remotePlatform \in Platforms /\ AllExpected(installedDigests) /\
      bootstrapRole = BootstrapComplete)

BootstrapUsesFixedRoleOrder ==
  /\ (phase = "uploading" =>
       /\ bootstrapRole \in {RuntimeRole, TerminalHostRole}
       /\ ComponentForRole(bootstrapRole) \in requestedUploads)
  /\ (phase \in {"daemon-ready", "attaching", "handshaking", "connected"} =>
       bootstrapRole = BootstrapComplete)

RejectedBootstrapOrderFailsClosed ==
  lastAction = "reject-bootstrap-order" => phase = "blocked"

PartialInstallNeverActivates ==
  ~AllExpected(installedDigests) =>
    phase \notin {"daemon-ready", "attaching", "handshaking", "connected"}

ConcurrentMismatchIsNotClobbered ==
  lastAction # "reject-concurrent-mismatch" \/
    (phase = "blocked" /\ MismatchComponents(installedDigests) # {})

ConnectedRequiresVerifiedHello ==
  phase # "connected" \/
    (catalogVerified /\ daemonEpoch \in Epochs /\
      attachmentGeneration = generation /\ helloGeneration = generation /\
      knownEpoch = daemonEpoch /\ HelloIsExact(advertisedServices))

AttachmentLossDoesNotDeleteDaemon ==
  phase # "retry-wait" \/ daemonEpoch \in Epochs

StopRetiresOnlyAttachment ==
  phase # "stopped" \/
    (attachmentGeneration = NoGeneration /\ helloGeneration = NoGeneration)

RemoteWorkIsRetained == remoteWorkRetained

UnknownEffectsAreNeverReplayed == ~replayedUnknownEffect

ExactServiceAuthorityInSlice ==
  phase # "connected" \/ HelloIsExact(advertisedServices)

RejectedHelloIsInexact ==
  lastAction = "reject-hello" => ~HelloIsExact(advertisedServices)

RetryWaitEventuallyLeaves == [](phase = "retry-wait" ~> phase # "retry-wait")

=============================================================================
