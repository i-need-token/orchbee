------------------- MODULE SshEnvironmentTerminalHostRelay -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction of the Slice 3.1B terminal-host-v1 relay. One Runtime
\* attachment owns one global monotonically allocated channel namespace, at
\* most one opaque Terminal Host broker, and one credit window in each
\* direction. Host setup and local I/O do not remove project-root-v2 authority
\* or disable heartbeat. Attachment loss closes only the broker, retains Host
\* work, marks an admitted-but-unsettled effect unknown, and never replays it.

CONSTANT CreditWindow, HostEpochs, MaxChannelId, RuntimeEpochs

NoChannel == 0
NoHostEpoch == "none"
AttachmentPhases == {"connected", "lost"}
RelayPhases == {"idle", "opening", "open"}
HostLifecycles == {"absent", "starting", "compatible", "incompatible"}
EffectOutcomes == {"none", "pending", "settled", "unknown"}
Actions == {
  "init", "open-relay", "reject-busy", "reject-channel-exhausted",
  "connect-compatible-host", "launch-host", "host-started",
  "host-start-failed", "reject-incompatible-host", "desktop-sends",
  "runtime-drains-host", "host-produces", "desktop-consumes",
  "admit-effect", "settle-effect", "close-relay", "host-disconnects",
  "lose-attachment", "reconnect-attachment", "host-idle-exit",
  "reject-stale-channel", "heartbeat"
}

VARIABLES
  attachmentPhase,
  runtimeEpoch,
  nextChannelId,
  usedChannelIds,
  relayPhase,
  relayChannel,
  hostLifecycle,
  hostEpoch,
  hostHasRetainedWork,
  brokerConnected,
  desktopToHostCredit,
  hostToDesktopCredit,
  desktopToHostBuffered,
  hostToDesktopBuffered,
  effectOutcome,
  replayedUnknownEffect,
  projectRootAvailable,
  lastAction

vars == <<attachmentPhase, runtimeEpoch, nextChannelId, usedChannelIds,
  relayPhase, relayChannel, hostLifecycle, hostEpoch, hostHasRetainedWork,
  brokerConnected, desktopToHostCredit, hostToDesktopCredit,
  desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
  replayedUnknownEffect, projectRootAvailable, lastAction>>

Init ==
  /\ attachmentPhase = "connected"
  /\ runtimeEpoch \in RuntimeEpochs
  /\ nextChannelId = 1
  /\ usedChannelIds = {}
  /\ relayPhase = "idle"
  /\ relayChannel = NoChannel
  /\ hostLifecycle \in {"absent", "compatible", "incompatible"}
  /\ hostHasRetainedWork \in BOOLEAN
  /\ hostHasRetainedWork => hostLifecycle \in {"compatible", "incompatible"}
  /\ IF hostLifecycle \in {"compatible", "incompatible"}
       THEN hostEpoch \in HostEpochs
       ELSE hostEpoch = NoHostEpoch
  /\ brokerConnected = FALSE
  /\ desktopToHostCredit = 0
  /\ hostToDesktopCredit = 0
  /\ desktopToHostBuffered = 0
  /\ hostToDesktopBuffered = 0
  /\ effectOutcome = "none"
  /\ replayedUnknownEffect = FALSE
  /\ projectRootAvailable = TRUE
  /\ lastAction = "init"

OpenRelay ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "idle"
  /\ nextChannelId <= MaxChannelId
  /\ relayPhase' = "opening"
  /\ relayChannel' = nextChannelId
  /\ usedChannelIds' = usedChannelIds \cup {nextChannelId}
  /\ nextChannelId' = nextChannelId + 1
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "open-relay"

RejectBusy ==
  /\ attachmentPhase = "connected"
  /\ relayPhase # "idle"
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "reject-busy"

RejectChannelExhausted ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "idle"
  /\ nextChannelId = MaxChannelId + 1
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "reject-channel-exhausted"

ConnectCompatibleHost ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "compatible"
  /\ relayPhase' = "open"
  /\ brokerConnected' = TRUE
  /\ desktopToHostCredit' = CreditWindow
  /\ hostToDesktopCredit' = CreditWindow
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "connect-compatible-host"

LaunchHost ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "absent"
  /\ hostLifecycle' = "starting"
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "launch-host"

HostStarted ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "starting"
  /\ hostLifecycle' = "compatible"
  /\ \E epoch \in HostEpochs : hostEpoch' = epoch
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostHasRetainedWork,
       brokerConnected, desktopToHostCredit, hostToDesktopCredit,
       desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "host-started"

HostStartFailed ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "starting"
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ hostLifecycle' = "absent"
  /\ hostEpoch' = NoHostEpoch
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, hostHasRetainedWork, brokerConnected,
       desktopToHostCredit, hostToDesktopCredit, desktopToHostBuffered,
       hostToDesktopBuffered, effectOutcome, replayedUnknownEffect,
       projectRootAvailable>>
  /\ lastAction' = "host-start-failed"

RejectIncompatibleHost ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "incompatible"
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, hostLifecycle, hostEpoch, hostHasRetainedWork,
       brokerConnected, desktopToHostCredit, hostToDesktopCredit,
       desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "reject-incompatible-host"

DesktopSends(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..desktopToHostCredit
  /\ desktopToHostCredit' = desktopToHostCredit - bytes
  /\ desktopToHostBuffered' = desktopToHostBuffered + bytes
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, hostToDesktopCredit,
       hostToDesktopBuffered, effectOutcome, replayedUnknownEffect,
       projectRootAvailable>>
  /\ lastAction' = "desktop-sends"

RuntimeDrainsHost(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..desktopToHostBuffered
  /\ desktopToHostBuffered' = desktopToHostBuffered - bytes
  /\ desktopToHostCredit' = desktopToHostCredit + bytes
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, hostToDesktopCredit,
       hostToDesktopBuffered, effectOutcome, replayedUnknownEffect,
       projectRootAvailable>>
  /\ lastAction' = "runtime-drains-host"

HostProduces(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..hostToDesktopCredit
  /\ hostToDesktopCredit' = hostToDesktopCredit - bytes
  /\ hostToDesktopBuffered' = hostToDesktopBuffered + bytes
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       desktopToHostBuffered, effectOutcome, replayedUnknownEffect,
       projectRootAvailable>>
  /\ lastAction' = "host-produces"

DesktopConsumes(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..hostToDesktopBuffered
  /\ hostToDesktopBuffered' = hostToDesktopBuffered - bytes
  /\ hostToDesktopCredit' = hostToDesktopCredit + bytes
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       desktopToHostBuffered, effectOutcome, replayedUnknownEffect,
       projectRootAvailable>>
  /\ lastAction' = "desktop-consumes"

AdmitEffect ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ effectOutcome \in {"none", "settled"}
  /\ effectOutcome' = "pending"
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "admit-effect"

SettleEffect ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ effectOutcome = "pending"
  /\ effectOutcome' = "settled"
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "settle-effect"

CloseRelay ==
  /\ attachmentPhase = "connected"
  /\ relayPhase \in {"opening", "open"}
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ brokerConnected' = FALSE
  /\ desktopToHostCredit' = 0
  /\ hostToDesktopCredit' = 0
  /\ desktopToHostBuffered' = 0
  /\ hostToDesktopBuffered' = 0
  /\ effectOutcome' = IF effectOutcome = "pending" THEN "unknown" ELSE effectOutcome
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, hostLifecycle, hostEpoch, hostHasRetainedWork,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "close-relay"

HostDisconnects ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ brokerConnected' = FALSE
  /\ desktopToHostCredit' = 0
  /\ hostToDesktopCredit' = 0
  /\ desktopToHostBuffered' = 0
  /\ hostToDesktopBuffered' = 0
  /\ effectOutcome' = IF effectOutcome = "pending" THEN "unknown" ELSE effectOutcome
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, hostLifecycle, hostEpoch, hostHasRetainedWork,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "host-disconnects"

LoseAttachment ==
  /\ attachmentPhase = "connected"
  /\ attachmentPhase' = "lost"
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ brokerConnected' = FALSE
  /\ desktopToHostCredit' = 0
  /\ hostToDesktopCredit' = 0
  /\ desktopToHostBuffered' = 0
  /\ hostToDesktopBuffered' = 0
  /\ effectOutcome' = IF effectOutcome = "pending" THEN "unknown" ELSE effectOutcome
  /\ projectRootAvailable' = FALSE
  /\ UNCHANGED <<runtimeEpoch, nextChannelId, usedChannelIds, hostLifecycle,
       hostEpoch, hostHasRetainedWork, replayedUnknownEffect>>
  /\ lastAction' = "lose-attachment"

ReconnectAttachment ==
  /\ attachmentPhase = "lost"
  /\ attachmentPhase' = "connected"
  /\ \E epoch \in RuntimeEpochs : runtimeEpoch' = epoch
  /\ projectRootAvailable' = TRUE
  /\ UNCHANGED <<nextChannelId, usedChannelIds, relayPhase, relayChannel,
       hostLifecycle, hostEpoch, hostHasRetainedWork, brokerConnected,
       desktopToHostCredit, hostToDesktopCredit, desktopToHostBuffered,
       hostToDesktopBuffered, effectOutcome, replayedUnknownEffect>>
  /\ lastAction' = "reconnect-attachment"

HostIdleExit ==
  /\ ~brokerConnected
  /\ relayPhase = "idle"
  /\ ~hostHasRetainedWork
  /\ hostLifecycle = "compatible"
  /\ hostLifecycle' = "absent"
  /\ hostEpoch' = NoHostEpoch
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostHasRetainedWork,
       brokerConnected, desktopToHostCredit, hostToDesktopCredit,
       desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
       replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "host-idle-exit"

RejectStaleChannel(staleChannel) ==
  /\ staleChannel \in usedChannelIds
  /\ staleChannel # relayChannel
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "reject-stale-channel"

Heartbeat ==
  /\ attachmentPhase = "connected"
  /\ UNCHANGED <<attachmentPhase, runtimeEpoch, nextChannelId,
       usedChannelIds, relayPhase, relayChannel, hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, replayedUnknownEffect, projectRootAvailable>>
  /\ lastAction' = "heartbeat"

AdvanceOpening ==
  ConnectCompatibleHost \/ LaunchHost \/ HostStarted \/ HostStartFailed \/
    RejectIncompatibleHost

Next ==
  \/ OpenRelay
  \/ RejectBusy
  \/ RejectChannelExhausted
  \/ AdvanceOpening
  \/ \E bytes \in 1..CreditWindow : DesktopSends(bytes)
  \/ \E bytes \in 1..CreditWindow : RuntimeDrainsHost(bytes)
  \/ \E bytes \in 1..CreditWindow : HostProduces(bytes)
  \/ \E bytes \in 1..CreditWindow : DesktopConsumes(bytes)
  \/ AdmitEffect
  \/ SettleEffect
  \/ CloseRelay
  \/ HostDisconnects
  \/ LoseAttachment
  \/ ReconnectAttachment
  \/ HostIdleExit
  \/ \E staleChannel \in 1..MaxChannelId : RejectStaleChannel(staleChannel)
  \/ Heartbeat

Spec ==
  Init /\ [][Next]_vars /\ WF_vars(AdvanceOpening) /\
    WF_vars(ReconnectAttachment) /\ WF_vars(Heartbeat)

TypeOK ==
  /\ attachmentPhase \in AttachmentPhases
  /\ runtimeEpoch \in RuntimeEpochs
  /\ nextChannelId \in 1..(MaxChannelId + 1)
  /\ usedChannelIds \subseteq 1..MaxChannelId
  /\ relayPhase \in RelayPhases
  /\ relayChannel \in 0..MaxChannelId
  /\ hostLifecycle \in HostLifecycles
  /\ hostEpoch \in HostEpochs \cup {NoHostEpoch}
  /\ hostHasRetainedWork \in BOOLEAN
  /\ brokerConnected \in BOOLEAN
  /\ desktopToHostCredit \in 0..CreditWindow
  /\ hostToDesktopCredit \in 0..CreditWindow
  /\ desktopToHostBuffered \in 0..CreditWindow
  /\ hostToDesktopBuffered \in 0..CreditWindow
  /\ effectOutcome \in EffectOutcomes
  /\ replayedUnknownEffect \in BOOLEAN
  /\ projectRootAvailable \in BOOLEAN
  /\ lastAction \in Actions

OneRelayPerAttachment ==
  relayPhase = "idle" <=> relayChannel = NoChannel

ChannelIdsAreNeverReused ==
  /\ \A channel \in usedChannelIds : channel < nextChannelId
  /\ relayChannel = NoChannel \/ relayChannel \in usedChannelIds

BrokerRequiresCompatibleOpenRelay ==
  brokerConnected =>
    (attachmentPhase = "connected" /\ relayPhase = "open" /\
      hostLifecycle = "compatible")

CreditAndBuffersStayBounded ==
  /\ desktopToHostCredit + desktopToHostBuffered =
       IF relayPhase = "open" THEN CreditWindow ELSE 0
  /\ hostToDesktopCredit + hostToDesktopBuffered =
       IF relayPhase = "open" THEN CreditWindow ELSE 0

HostIdentityMatchesLifecycle ==
  hostLifecycle \in {"absent", "starting"} <=> hostEpoch = NoHostEpoch

RetainedHostSurvivesAttachmentLoss ==
  lastAction # "lose-attachment" \/
    (~hostHasRetainedWork \/ hostLifecycle \in {"compatible", "incompatible"})

CompatibilityFailureNeverKillsHost ==
  lastAction # "reject-incompatible-host" \/ hostLifecycle = "incompatible"

LocalRelayFailurePreservesProjectRoot ==
  lastAction \notin {"host-start-failed", "reject-incompatible-host",
    "host-disconnects"} \/ projectRootAvailable

AttachmentLossClosesOnlyBroker ==
  attachmentPhase # "lost" \/
    (~brokerConnected /\ relayPhase = "idle" /\ relayChannel = NoChannel)

UnknownEffectsAreNeverReplayed ==
  ~replayedUnknownEffect

OpeningEventuallySettles ==
  [](relayPhase = "opening" ~> relayPhase # "opening")

LostAttachmentEventuallyReconnects ==
  [](attachmentPhase = "lost" ~> attachmentPhase = "connected")

=============================================================================
