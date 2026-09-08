------------------- MODULE SshEnvironmentTerminalHostRelay -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction of the Slice 3.1B terminal-host-v1 relay. One Runtime
\* attachment owns one global monotonically allocated channel namespace, at
\* most one opaque Terminal Host broker, and one credit window in each
\* direction. Host setup and local I/O do not remove project-root-v2 authority
\* or disable heartbeat. Attachment loss closes only the broker, retains Host
\* work, marks an admitted-but-unsettled effect unknown, and never replays it.

CONSTANT CreditWindow, EffectIds, HostEpochs, MaxAttachmentGeneration,
         MaxChannelId, RuntimeEpochs

ASSUME Cardinality(EffectIds) = 2
ASSUME MaxAttachmentGeneration \in Nat \ {0}

NoChannel == 0
NoEffect == "no-effect"
NoHostEpoch == "none"
AttachmentGenerations == 1..MaxAttachmentGeneration
ChannelKeys == AttachmentGenerations \X (1..MaxChannelId)
AttachmentPhases == {"connected", "lost"}
RelayPhases == {"idle", "opening", "open"}
HostLifecycles == {"absent", "starting", "compatible", "incompatible"}
EffectOutcomes == {"none", "pending", "settled", "unknown"}
UnknownAfterInterruption(outcomes, effect) ==
  IF effect \in EffectIds
  THEN IF outcomes[effect] = "pending"
       THEN [outcomes EXCEPT ![effect] = "unknown"]
       ELSE outcomes
  ELSE outcomes
Actions == {
  "init", "open-relay", "reject-busy", "reject-channel-exhausted",
  "connect-compatible-host", "launch-host", "host-started",
  "host-start-failed", "reject-incompatible-host", "desktop-sends",
  "runtime-drains-host", "host-produces", "desktop-consumes",
  "admit-effect", "settle-effect", "reject-effect-replay", "close-relay",
  "host-disconnects",
  "lose-attachment", "reconnect-attachment", "host-idle-exit",
  "reject-stale-channel", "heartbeat"
}

VARIABLES
  attachmentPhase,
  attachmentGeneration,
  runtimeEpoch,
  nextChannelId,
  usedChannelIds,
  usedChannelKeys,
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
  activeEffect,
  effectSendCount,
  projectRootAvailable,
  lastAction

vars == <<attachmentPhase, attachmentGeneration, runtimeEpoch, nextChannelId,
  usedChannelIds, usedChannelKeys,
  relayPhase, relayChannel, hostLifecycle, hostEpoch, hostHasRetainedWork,
  brokerConnected, desktopToHostCredit, hostToDesktopCredit,
  desktopToHostBuffered, hostToDesktopBuffered, effectOutcome, activeEffect,
  effectSendCount, projectRootAvailable, lastAction>>

Init ==
  /\ attachmentPhase = "connected"
  /\ attachmentGeneration = 1
  /\ runtimeEpoch \in RuntimeEpochs
  /\ nextChannelId = 1
  /\ usedChannelIds = {}
  /\ usedChannelKeys = {}
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
  /\ effectOutcome = [effect \in EffectIds |-> "none"]
  /\ activeEffect = NoEffect
  /\ effectSendCount = [effect \in EffectIds |-> 0]
  /\ projectRootAvailable = TRUE
  /\ lastAction = "init"

OpenRelay ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "idle"
  /\ nextChannelId <= MaxChannelId
  /\ relayPhase' = "opening"
  /\ relayChannel' = nextChannelId
  /\ usedChannelIds' = usedChannelIds \cup {nextChannelId}
  /\ usedChannelKeys' =
       usedChannelKeys \cup {<<attachmentGeneration, nextChannelId>>}
  /\ nextChannelId' = nextChannelId + 1
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "open-relay"

RejectBusy ==
  /\ attachmentPhase = "connected"
  /\ relayPhase # "idle"
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "reject-busy"

RejectChannelExhausted ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "idle"
  /\ nextChannelId = MaxChannelId + 1
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "reject-channel-exhausted"

ConnectCompatibleHost ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "compatible"
  /\ relayPhase' = "open"
  /\ brokerConnected' = TRUE
  /\ desktopToHostCredit' = CreditWindow
  /\ hostToDesktopCredit' = CreditWindow
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "connect-compatible-host"

LaunchHost ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "absent"
  /\ hostLifecycle' = "starting"
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase,
       relayChannel, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "launch-host"

HostStarted ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "starting"
  /\ hostLifecycle' = "compatible"
  /\ \E epoch \in HostEpochs : hostEpoch' = epoch
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase,
       relayChannel, hostHasRetainedWork,
       brokerConnected, desktopToHostCredit, hostToDesktopCredit,
       desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
       activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "host-started"

HostStartFailed ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "starting"
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ hostLifecycle' = "absent"
  /\ hostEpoch' = NoHostEpoch
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, hostHasRetainedWork,
       brokerConnected,
       desktopToHostCredit, hostToDesktopCredit, desktopToHostBuffered,
       hostToDesktopBuffered, effectOutcome, activeEffect, effectSendCount,
       projectRootAvailable>>
  /\ lastAction' = "host-start-failed"

RejectIncompatibleHost ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "opening"
  /\ hostLifecycle = "incompatible"
  /\ relayPhase' = "idle"
  /\ relayChannel' = NoChannel
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, hostLifecycle, hostEpoch,
       hostHasRetainedWork,
       brokerConnected, desktopToHostCredit, hostToDesktopCredit,
       desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
       activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "reject-incompatible-host"

DesktopSends(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..desktopToHostCredit
  /\ desktopToHostCredit' = desktopToHostCredit - bytes
  /\ desktopToHostBuffered' = desktopToHostBuffered + bytes
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, hostToDesktopCredit,
       hostToDesktopBuffered, effectOutcome, activeEffect, effectSendCount,
       projectRootAvailable>>
  /\ lastAction' = "desktop-sends"

RuntimeDrainsHost(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..desktopToHostBuffered
  /\ desktopToHostBuffered' = desktopToHostBuffered - bytes
  /\ desktopToHostCredit' = desktopToHostCredit + bytes
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, hostToDesktopCredit,
       hostToDesktopBuffered, effectOutcome, activeEffect, effectSendCount,
       projectRootAvailable>>
  /\ lastAction' = "runtime-drains-host"

HostProduces(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..hostToDesktopCredit
  /\ hostToDesktopCredit' = hostToDesktopCredit - bytes
  /\ hostToDesktopBuffered' = hostToDesktopBuffered + bytes
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       desktopToHostBuffered, effectOutcome, activeEffect, effectSendCount,
       projectRootAvailable>>
  /\ lastAction' = "host-produces"

DesktopConsumes(bytes) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ bytes \in 1..hostToDesktopBuffered
  /\ hostToDesktopBuffered' = hostToDesktopBuffered - bytes
  /\ hostToDesktopCredit' = hostToDesktopCredit + bytes
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       desktopToHostBuffered, effectOutcome, activeEffect, effectSendCount,
       projectRootAvailable>>
  /\ lastAction' = "desktop-consumes"

AdmitEffect(effect) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ effect \in EffectIds
  /\ activeEffect = NoEffect
  /\ effectOutcome[effect] = "none"
  /\ effectOutcome' = [effectOutcome EXCEPT ![effect] = "pending"]
  /\ activeEffect' = effect
  /\ effectSendCount' = [effectSendCount EXCEPT ![effect] = @ + 1]
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       projectRootAvailable>>
  /\ lastAction' = "admit-effect"

AdmitFreshEffectAfterUnknown(effect) ==
  /\ \E priorEffect \in EffectIds \ {effect}:
       effectOutcome[priorEffect] = "unknown"
  /\ AdmitEffect(effect)

SettleEffect(effect) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ effect = activeEffect
  /\ effect \in EffectIds
  /\ effectOutcome[effect] = "pending"
  /\ effectOutcome' = [effectOutcome EXCEPT ![effect] = "settled"]
  /\ activeEffect' = NoEffect
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectSendCount, projectRootAvailable>>
  /\ lastAction' = "settle-effect"

RejectEffectReplay(effect) ==
  /\ attachmentPhase = "connected"
  /\ relayPhase = "open"
  /\ effect \in EffectIds
  /\ effectOutcome[effect] = "unknown"
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch, hostHasRetainedWork, brokerConnected,
       desktopToHostCredit, hostToDesktopCredit, desktopToHostBuffered,
       hostToDesktopBuffered, effectOutcome, activeEffect, effectSendCount,
       projectRootAvailable>>
  /\ lastAction' = "reject-effect-replay"

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
  /\ effectOutcome' = UnknownAfterInterruption(effectOutcome, activeEffect)
  /\ activeEffect' = NoEffect
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, hostLifecycle, hostEpoch,
       hostHasRetainedWork, effectSendCount, projectRootAvailable>>
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
  /\ effectOutcome' = UnknownAfterInterruption(effectOutcome, activeEffect)
  /\ activeEffect' = NoEffect
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, hostLifecycle, hostEpoch,
       hostHasRetainedWork, effectSendCount, projectRootAvailable>>
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
  /\ effectOutcome' = UnknownAfterInterruption(effectOutcome, activeEffect)
  /\ activeEffect' = NoEffect
  /\ projectRootAvailable' = FALSE
  /\ UNCHANGED <<attachmentGeneration, runtimeEpoch, nextChannelId,
       usedChannelIds, usedChannelKeys, hostLifecycle, hostEpoch,
       hostHasRetainedWork, effectSendCount>>
  /\ lastAction' = "lose-attachment"

ReconnectAttachment ==
  /\ attachmentPhase = "lost"
  /\ attachmentGeneration < MaxAttachmentGeneration
  /\ attachmentPhase' = "connected"
  /\ attachmentGeneration' = attachmentGeneration + 1
  /\ \E epoch \in RuntimeEpochs : runtimeEpoch' = epoch
  /\ nextChannelId' = 1
  /\ usedChannelIds' = {}
  /\ projectRootAvailable' = TRUE
  /\ UNCHANGED <<usedChannelKeys, relayPhase, relayChannel, hostLifecycle,
       hostEpoch, hostHasRetainedWork, brokerConnected,
       desktopToHostCredit, hostToDesktopCredit, desktopToHostBuffered,
       hostToDesktopBuffered, effectOutcome, activeEffect, effectSendCount>>
  /\ lastAction' = "reconnect-attachment"

HostIdleExit ==
  /\ ~brokerConnected
  /\ relayPhase = "idle"
  /\ ~hostHasRetainedWork
  /\ hostLifecycle = "compatible"
  /\ hostLifecycle' = "absent"
  /\ hostEpoch' = NoHostEpoch
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase,
       relayChannel, hostHasRetainedWork,
       brokerConnected, desktopToHostCredit, hostToDesktopCredit,
       desktopToHostBuffered, hostToDesktopBuffered, effectOutcome,
       activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "host-idle-exit"

RejectStaleChannel(staleGeneration, staleChannel) ==
  /\ <<staleGeneration, staleChannel>> \in usedChannelKeys
  /\ (staleGeneration # attachmentGeneration \/ staleChannel # relayChannel)
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
  /\ lastAction' = "reject-stale-channel"

Heartbeat ==
  /\ attachmentPhase = "connected"
  /\ UNCHANGED <<attachmentPhase, attachmentGeneration, runtimeEpoch,
       nextChannelId, usedChannelIds, usedChannelKeys, relayPhase, relayChannel,
       hostLifecycle, hostEpoch,
       hostHasRetainedWork, brokerConnected, desktopToHostCredit,
       hostToDesktopCredit, desktopToHostBuffered, hostToDesktopBuffered,
       effectOutcome, activeEffect, effectSendCount, projectRootAvailable>>
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
  \/ \E effect \in EffectIds : AdmitEffect(effect)
  \/ \E effect \in EffectIds : AdmitFreshEffectAfterUnknown(effect)
  \/ \E effect \in EffectIds : SettleEffect(effect)
  \/ \E effect \in EffectIds : RejectEffectReplay(effect)
  \/ CloseRelay
  \/ HostDisconnects
  \/ LoseAttachment
  \/ ReconnectAttachment
  \/ HostIdleExit
  \/ \E staleGeneration \in AttachmentGenerations,
       staleChannel \in 1..MaxChannelId :
       RejectStaleChannel(staleGeneration, staleChannel)
  \/ Heartbeat

Spec ==
  Init /\ [][Next]_vars /\ WF_vars(AdvanceOpening) /\
    WF_vars(Heartbeat)

TypeOK ==
  /\ attachmentPhase \in AttachmentPhases
  /\ attachmentGeneration \in AttachmentGenerations
  /\ runtimeEpoch \in RuntimeEpochs
  /\ nextChannelId \in 1..(MaxChannelId + 1)
  /\ usedChannelIds \subseteq 1..MaxChannelId
  /\ usedChannelKeys \subseteq ChannelKeys
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
  /\ effectOutcome \in [EffectIds -> EffectOutcomes]
  /\ activeEffect \in EffectIds \cup {NoEffect}
  /\ effectSendCount \in [EffectIds -> 0..2]
  /\ projectRootAvailable \in BOOLEAN
  /\ lastAction \in Actions

OneRelayPerAttachment ==
  relayPhase = "idle" <=> relayChannel = NoChannel

ChannelIdsAreNeverReusedWithinAttachment ==
  /\ \A channel \in usedChannelIds : channel < nextChannelId
  /\ relayChannel = NoChannel \/ relayChannel \in usedChannelIds
  /\ \A channel \in usedChannelIds:
       <<attachmentGeneration, channel>> \in usedChannelKeys

ReconnectResetsChannelNamespace ==
  lastAction = "reconnect-attachment" =>
    /\ nextChannelId = 1
    /\ usedChannelIds = {}

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
  /\ \A effect \in EffectIds: effectSendCount[effect] <= 1
  /\ \A effect \in EffectIds:
       effectOutcome[effect] = "unknown" => effectSendCount[effect] = 1

PendingEffectHasExactIdentity ==
  /\ (activeEffect = NoEffect =>
       \A effect \in EffectIds: effectOutcome[effect] # "pending")
  /\ (activeEffect \in EffectIds => effectOutcome[activeEffect] = "pending")

OpeningEventuallySettles ==
  [](relayPhase = "opening" ~> relayPhase # "opening")

=============================================================================
