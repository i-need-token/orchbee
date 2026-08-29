------------------- MODULE SshEnvironmentRuntimeAttachment -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction of the Slice 2.3C Runtime attachment spine. Desktop
\* authenticates one local release catalog before opening a transport. The same
\* SSH child reports one platform and exact installed-artifact state, requests
\* bytes only when absent, and then carries Hello. Immutable installation and a
\* detached per-Environment daemon outlive replaceable attachment generations.

CONSTANT MaxGeneration, Epochs, Platforms

NoGeneration == 0
NoEpoch == "none"
UnsupportedPlatform == "unsupported"
InstallDigests == {"absent", "expected", "mismatch"}
InstallModes == {"none", "present", "upload"}
ExpectedServices == {"project-root-v1"}
Phases == {
  "stopped", "verifying-catalog", "negotiating", "uploading",
  "daemon-ready", "attaching", "handshaking", "connected",
  "retry-wait", "blocked"
}
Actions == {
  "init", "begin-verification", "verify-catalog", "reject-catalog",
  "report-present", "request-upload", "reject-installed-mismatch",
  "reject-unsupported", "upload-and-install", "concurrent-install-wins",
  "converge-concurrent-install", "reject-concurrent-mismatch", "start-daemon", "begin-handshake",
  "accept-hello", "reject-hello", "heartbeat", "lose-attachment",
  "retry-elapsed", "restart-daemon", "reject-competing-attachment",
  "reject-unadvertised-service", "stop", "repeat-stop", "stale-hello"
}

VARIABLES
  phase,
  generation,
  catalogVerified,
  remotePlatform,
  installedDigest,
  installMode,
  artifactLoaded,
  daemonEpoch,
  attachmentGeneration,
  helloGeneration,
  knownEpoch,
  remoteWorkRetained,
  replayedUnknownEffect,
  advertisedServices,
  lastAction

vars == <<phase, generation, catalogVerified, remotePlatform,
  installedDigest, installMode, artifactLoaded, daemonEpoch,
  attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
  replayedUnknownEffect, advertisedServices, lastAction>>

Init ==
  /\ phase = "stopped"
  /\ generation = NoGeneration
  /\ catalogVerified = FALSE
  /\ remotePlatform \in Platforms \cup {UnsupportedPlatform}
  /\ installedDigest \in InstallDigests
  /\ installMode = "none"
  /\ artifactLoaded = FALSE
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
  /\ artifactLoaded' = FALSE
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = {}
  /\ UNCHANGED <<remotePlatform, installedDigest, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "begin-verification"

VerifyCatalog ==
  /\ phase = "verifying-catalog"
  /\ phase' = "negotiating"
  /\ catalogVerified' = TRUE
  /\ UNCHANGED <<generation, remotePlatform, installedDigest, installMode,
       artifactLoaded, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "verify-catalog"

RejectCatalog ==
  /\ phase = "verifying-catalog"
  /\ phase' = "blocked"
  /\ catalogVerified' = FALSE
  /\ UNCHANGED <<generation, remotePlatform, installedDigest, installMode,
       artifactLoaded, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "reject-catalog"

ReportPresent ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ installedDigest = "expected"
  /\ phase' = "daemon-ready"
  /\ installMode' = "present"
  /\ artifactLoaded' = FALSE
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "report-present"

RequestUpload ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ installedDigest = "absent"
  /\ phase' = "uploading"
  /\ installMode' = "upload"
  /\ artifactLoaded' = FALSE
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "request-upload"

RejectInstalledMismatch ==
  /\ phase = "negotiating"
  /\ remotePlatform \in Platforms
  /\ installedDigest = "mismatch"
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-installed-mismatch"

RejectUnsupported ==
  /\ phase = "negotiating"
  /\ remotePlatform = UnsupportedPlatform
  /\ phase' = "blocked"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-unsupported"

UploadAndInstall ==
  /\ phase = "uploading"
  /\ installedDigest = "absent"
  /\ phase' = "daemon-ready"
  /\ installedDigest' = "expected"
  /\ artifactLoaded' = TRUE
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform, installMode,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "upload-and-install"

ConcurrentInstallWins ==
  /\ phase = "uploading"
  /\ installedDigest = "absent"
  /\ installedDigest' = "expected"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installMode, artifactLoaded, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "concurrent-install-wins"

ConvergeConcurrentInstall ==
  /\ phase = "uploading"
  /\ installedDigest = "expected"
  /\ phase' = "daemon-ready"
  /\ artifactLoaded' = TRUE
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, installMode, daemonEpoch, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "converge-concurrent-install"

RejectConcurrentMismatch ==
  /\ phase = "uploading"
  /\ installedDigest = "absent"
  /\ phase' = "blocked"
  /\ installedDigest' = "mismatch"
  /\ artifactLoaded' = TRUE
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform, installMode,
       daemonEpoch, attachmentGeneration, helloGeneration, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-concurrent-mismatch"

StartDaemon ==
  /\ phase = "daemon-ready"
  /\ catalogVerified
  /\ remotePlatform \in Platforms
  /\ installedDigest = "expected"
  /\ phase' = "attaching"
  /\ attachmentGeneration' = generation
  /\ IF daemonEpoch = NoEpoch
       THEN \E epoch \in Epochs : daemonEpoch' = epoch
       ELSE daemonEpoch' = daemonEpoch
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "start-daemon"

BeginHandshake ==
  /\ phase = "attaching"
  /\ attachmentGeneration = generation
  /\ phase' = "handshaking"
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
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
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "accept-hello"

RejectHello ==
  /\ phase = "handshaking"
  /\ phase' = "blocked"
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = {}
  /\ UNCHANGED <<generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "reject-hello"

Heartbeat ==
  /\ phase = "connected"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "heartbeat"

LoseAttachment ==
  /\ phase = "connected"
  /\ generation < MaxGeneration
  /\ phase' = "retry-wait"
  /\ generation' = generation + 1
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ installMode' = "none"
  /\ artifactLoaded' = FALSE
  /\ UNCHANGED <<catalogVerified, remotePlatform, installedDigest,
       daemonEpoch, knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "lose-attachment"

RetryElapsed ==
  /\ phase = "retry-wait"
  /\ phase' = "verifying-catalog"
  /\ catalogVerified' = FALSE
  /\ UNCHANGED <<generation, remotePlatform, installedDigest, installMode,
       artifactLoaded, daemonEpoch, attachmentGeneration, helloGeneration,
       knownEpoch, remoteWorkRetained, replayedUnknownEffect,
       advertisedServices>>
  /\ lastAction' = "retry-elapsed"

RestartDaemon ==
  /\ phase = "retry-wait"
  /\ daemonEpoch \in Epochs
  /\ \E epoch \in (Epochs \ {daemonEpoch}) : daemonEpoch' = epoch
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, attachmentGeneration,
       helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "restart-daemon"

RejectCompetingAttachment ==
  /\ attachmentGeneration = generation
  /\ phase \in {"attaching", "handshaking", "connected"}
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-competing-attachment"

RejectUnadvertisedService ==
  /\ phase = "connected"
  /\ advertisedServices = ExpectedServices
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "reject-unadvertised-service"

Stop ==
  /\ phase # "stopped"
  /\ phase' = "stopped"
  /\ generation' = IF generation < MaxGeneration THEN generation + 1 ELSE generation
  /\ catalogVerified' = FALSE
  /\ installMode' = "none"
  /\ artifactLoaded' = FALSE
  /\ attachmentGeneration' = NoGeneration
  /\ helloGeneration' = NoGeneration
  /\ advertisedServices' = {}
  /\ UNCHANGED <<remotePlatform, installedDigest, daemonEpoch, knownEpoch,
       remoteWorkRetained, replayedUnknownEffect>>
  /\ lastAction' = "stop"

RepeatStop ==
  /\ phase = "stopped"
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "repeat-stop"

StaleHello(staleGeneration) ==
  /\ staleGeneration \in 0..MaxGeneration
  /\ staleGeneration # generation
  /\ UNCHANGED <<phase, generation, catalogVerified, remotePlatform,
       installedDigest, installMode, artifactLoaded, daemonEpoch,
       attachmentGeneration, helloGeneration, knownEpoch, remoteWorkRetained,
       replayedUnknownEffect, advertisedServices>>
  /\ lastAction' = "stale-hello"

Next ==
  \/ BeginVerification
  \/ VerifyCatalog
  \/ RejectCatalog
  \/ ReportPresent
  \/ RequestUpload
  \/ RejectInstalledMismatch
  \/ RejectUnsupported
  \/ UploadAndInstall
  \/ ConcurrentInstallWins
  \/ ConvergeConcurrentInstall
  \/ RejectConcurrentMismatch
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
  /\ installedDigest \in InstallDigests
  /\ installMode \in InstallModes
  /\ artifactLoaded \in BOOLEAN
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
UploadReadsOnlyOnRequest == artifactLoaded => installMode = "upload"
PresentSkipsArtifactRead == installMode # "present" \/ ~artifactLoaded
AttachmentRequiresExactInstall ==
  phase \notin {"daemon-ready", "attaching", "handshaking", "connected"} \/
    (remotePlatform \in Platforms /\ installedDigest = "expected")
ConcurrentMismatchIsNotClobbered ==
  lastAction # "reject-concurrent-mismatch" \/
    (phase = "blocked" /\ installedDigest = "mismatch")
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
