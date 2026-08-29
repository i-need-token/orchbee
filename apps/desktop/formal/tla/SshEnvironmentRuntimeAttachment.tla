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
InstallDigests == {"absent", "expected", "mismatch"}
InstallModes == {"none", "present", "upload"}
ExpectedServices == {"project-root-v1", "terminal-host-v1"}
Phases == {
  "stopped", "verifying-catalog", "negotiating", "uploading",
  "daemon-ready", "attaching", "handshaking", "connected",
  "retry-wait", "blocked"
}
Actions == {
  "init", "begin-verification", "verify-catalog", "reject-catalog",
  "report-present", "request-upload", "reject-installed-mismatch",
  "reject-unsupported", "upload-component", "concurrent-install-wins",
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
  lastAction

vars == <<phase, generation, catalogVerified, remotePlatform,
  installedDigests, installMode, requestedUploads, loadedArtifacts,
  daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
  remoteWorkRetained, replayedUnknownEffect, advertisedServices, lastAction>>

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
  /\ advertisedServices = {}
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
  /\ advertisedServices' = {}
  /\ UNCHANGED <<remotePlatform, installedDigests, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "begin-verification"

VerifyCatalog ==
  /\ phase = "verifying-catalog"
  /\ phase' = "negotiating"
  /\ catalogVerified' = TRUE
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
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-catalog"

ReportPresent ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ AllExpected(installedDigests)
  /\ phase' = "daemon-ready"
  /\ installMode' = "present"
  /\ requestedUploads' = {}
  /\ loadedArtifacts' = {}
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "report-present"

RequestUpload ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ MismatchComponents(installedDigests) = {}
  /\ AbsentComponents(installedDigests) # {}
  /\ phase' = "uploading"
  /\ installMode' = "upload"
  /\ requestedUploads' = AbsentComponents(installedDigests)
  /\ loadedArtifacts' = {}
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "request-upload"

RejectInstalledMismatch ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ MismatchComponents(installedDigests) # {}
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-installed-mismatch"

RejectUnsupported ==
  /\ phase = "negotiating"
  /\ remotePlatform = UnsupportedPlatform
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-unsupported"

UploadComponent(component) ==
  /\ phase = "uploading"
  /\ component \in requestedUploads
  /\ installedDigests[component] = "absent"
  /\ installedDigests' = [installedDigests EXCEPT ![component] = "expected"]
  /\ loadedArtifacts' = loadedArtifacts \cup {component}
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installMode, requestedUploads, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "upload-component"

ConcurrentInstallWins(component) ==
  /\ phase = "uploading"
  /\ component \in requestedUploads
  /\ installedDigests[component] = "absent"
  /\ installedDigests' = [installedDigests EXCEPT ![component] = "expected"]
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installMode, requestedUploads, loadedArtifacts, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "concurrent-install-wins"

ConvergeCohort ==
  /\ phase = "uploading"
  /\ AllExpected(installedDigests)
  /\ phase' = "daemon-ready"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "converge-cohort"

RejectConcurrentMismatch(component) ==
  /\ phase = "uploading"
  /\ component \in requestedUploads
  /\ installedDigests[component] = "absent"
  /\ phase' = "blocked"
  /\ installedDigests' = [installedDigests EXCEPT ![component] = "mismatch"]
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform, installMode,
       requestedUploads, loadedArtifacts, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
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
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "start-daemon"

BeginHandshake ==
  /\ phase = "attaching"
  /\ attachmentGeneration = generation
  /\ phase' = "handshaking"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "begin-handshake"

AcceptHello ==
  /\ phase = "handshaking"
  /\ attachmentGeneration = generation
  /\ daemonEpoch \in Epochs
  /\ phase' = "connected"
  /\ helloGeneration' = generation
  /\ knownEpoch' = daemonEpoch
  /\ advertisedServices' = ExpectedServices
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, remoteWorkRetained,
       replayedUnknownEffect>>
  /\ lastAction' = "accept-hello"

RejectHello ==
  /\ phase = "handshaking"
  /\ phase' = "blocked"
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = {}
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, knownEpoch, remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "reject-hello"

Heartbeat ==
  /\ phase = "connected"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
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
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "retry-elapsed"

RestartDaemon ==
  /\ phase = "retry-wait"
  /\ daemonEpoch \in Epochs
  /\ \E epoch \in (Epochs \ {daemonEpoch}) : daemonEpoch' = epoch
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "restart-daemon"

RejectCompetingAttachment ==
  /\ attachmentGeneration = generation
  /\ phase \in {"attaching", "handshaking", "connected"}
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-competing-attachment"

RejectUnadvertisedService ==
  /\ phase = "connected"
  /\ advertisedServices = ExpectedServices
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
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
  /\ advertisedServices' = {}
  /\ UNCHANGED <<remotePlatform, installedDigests, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "stop"

RepeatStop ==
  /\ phase = "stopped"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "repeat-stop"

StaleHello(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigests, installMode, requestedUploads, loadedArtifacts,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "stale-hello"

Next ==
  \/ BeginVerification
  \/ VerifyCatalog
  \/ RejectCatalog
  \/ ReportPresent
  \/ RequestUpload
  \/ RejectInstalledMismatch
  \/ RejectUnsupported
  \/ \E component \in Components : UploadComponent(component)
  \/ \E component \in Components : ConcurrentInstallWins(component)
  \/ ConvergeCohort
  \/ \E component \in Components : RejectConcurrentMismatch(component)
  \/ StartDaemon
  \/ BeginHandshake
  \/ AcceptHello
  \/ RejectHello
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
  /\ advertisedServices \subseteq ExpectedServices
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
    (remotePlatform \in Platforms /\ AllExpected(installedDigests))

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
      knownEpoch = daemonEpoch)

AttachmentLossDoesNotDeleteDaemon ==
  phase # "retry-wait" \/ daemonEpoch \in Epochs

StopRetiresOnlyAttachment ==
  phase # "stopped" \/
    (attachmentGeneration = NoGeneration /\ helloGeneration = NoGeneration)

RemoteWorkIsRetained == remoteWorkRetained

UnknownEffectsAreNeverReplayed == ~replayedUnknownEffect

ExactServiceAuthorityInSlice ==
  phase # "connected" \/ advertisedServices = ExpectedServices

RetryWaitEventuallyLeaves == [](phase = "retry-wait" ~> phase # "retry-wait")

=============================================================================
