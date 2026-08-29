------------------- MODULE SshEnvironmentProjectRootService -------------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded abstraction of the project-root inspection and directory-listing
\* service. One authenticated Runtime attachment advertises one exact service.
\* Desktop opens bounded single-request channels, both peers consume explicit
\* byte credit, filesystem workers remain bounded, and attachment replacement
\* never replays a request or publishes a stale worker result.

CONSTANT Channels, Epochs, MaxConcurrent

NoEpoch == "none"
NoOutcome == "none"
ExpectedService == "project-root-v2"
ExpectedServices == {ExpectedService}

ChannelPhases == {
  "unused", "opening", "open", "request-sent", "inspecting",
  "result-ready", "result-published", "closing", "closed",
  "attachment-lost"
}

WorkerOutcomes == {
  "resolved", "listed", "invalid-path", "not-found", "not-directory",
  "permission-denied", "timed-out", "busy", "unavailable"
}

Settlements == WorkerOutcomes \cup {NoOutcome, "cancelled", "attachment-lost"}

Actions == {
  "init", "connect", "reconnect", "open-channel", "accept-channel",
  "send-request", "start-inspection", "reject-busy", "reject-invalid-payload",
  "complete-inspection", "timeout-inspection", "publish-result",
  "cancel-channel", "begin-completed-close", "acknowledge-close",
  "lose-attachment", "retire-lost-channel", "retire-late-worker",
  "reject-replay", "heartbeat", "transport-violation", "stale-result"
}

VARIABLES
  connected,
  runtimeEpoch,
  advertisedServices,
  usedChannels,
  channelPhase,
  channelEpoch,
  requestCredit,
  resultCredit,
  workerRunning,
  workerOutcome,
  settlement,
  replayedChannels,
  lastAction

vars == <<connected, runtimeEpoch, advertisedServices, usedChannels,
  channelPhase, channelEpoch, requestCredit, resultCredit, workerRunning,
  workerOutcome, settlement, replayedChannels, lastAction>>

ActiveWorkers == {channel \in Channels : workerRunning[channel]}

OpenChannelPhases == {
  "opening", "open", "request-sent", "inspecting", "result-ready",
  "result-published", "closing"
}

Init ==
  /\ connected = FALSE
  /\ runtimeEpoch = NoEpoch
  /\ advertisedServices = {}
  /\ usedChannels = {}
  /\ channelPhase = [channel \in Channels |-> "unused"]
  /\ channelEpoch = [channel \in Channels |-> NoEpoch]
  /\ requestCredit = [channel \in Channels |-> 0]
  /\ resultCredit = [channel \in Channels |-> 0]
  /\ workerRunning = [channel \in Channels |-> FALSE]
  /\ workerOutcome = [channel \in Channels |-> NoOutcome]
  /\ settlement = [channel \in Channels |-> NoOutcome]
  /\ replayedChannels = {}
  /\ lastAction = "init"

Connect ==
  /\ ~connected
  /\ usedChannels = {}
  /\ \E epoch \in Epochs : runtimeEpoch' = epoch
  /\ connected' = TRUE
  /\ advertisedServices' = ExpectedServices
  /\ UNCHANGED <<usedChannels, channelPhase, channelEpoch, requestCredit,
       resultCredit, workerRunning, workerOutcome, settlement,
       replayedChannels>>
  /\ lastAction' = "connect"

Reconnect ==
  /\ ~connected
  /\ usedChannels # {}
  /\ \E epoch \in Epochs : runtimeEpoch' = epoch
  /\ connected' = TRUE
  /\ advertisedServices' = ExpectedServices
  /\ UNCHANGED <<usedChannels, channelPhase, channelEpoch, requestCredit,
       resultCredit, workerRunning, workerOutcome, settlement,
       replayedChannels>>
  /\ lastAction' = "reconnect"

OpenChannel(channel) ==
  /\ connected
  /\ advertisedServices = ExpectedServices
  /\ channel \in Channels \ usedChannels
  /\ usedChannels' = usedChannels \cup {channel}
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "opening"]
  /\ channelEpoch' = [channelEpoch EXCEPT ![channel] = runtimeEpoch]
  /\ resultCredit' = [resultCredit EXCEPT ![channel] = 1]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, requestCredit,
       workerRunning, workerOutcome, settlement, replayedChannels>>
  /\ lastAction' = "open-channel"

AcceptChannel(channel) ==
  /\ connected
  /\ channelPhase[channel] = "opening"
  /\ channelEpoch[channel] = runtimeEpoch
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "open"]
  /\ requestCredit' = [requestCredit EXCEPT ![channel] = 1]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, resultCredit, workerRunning, workerOutcome, settlement,
       replayedChannels>>
  /\ lastAction' = "accept-channel"

SendRequest(channel) ==
  /\ connected
  /\ channelPhase[channel] = "open"
  /\ channelEpoch[channel] = runtimeEpoch
  /\ requestCredit[channel] = 1
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "request-sent"]
  /\ requestCredit' = [requestCredit EXCEPT ![channel] = 0]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, resultCredit, workerRunning, workerOutcome, settlement,
       replayedChannels>>
  /\ lastAction' = "send-request"

StartInspection(channel) ==
  /\ connected
  /\ channelPhase[channel] = "request-sent"
  /\ channelEpoch[channel] = runtimeEpoch
  /\ Cardinality(ActiveWorkers) < MaxConcurrent
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "inspecting"]
  /\ workerRunning' = [workerRunning EXCEPT ![channel] = TRUE]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, workerOutcome, settlement,
       replayedChannels>>
  /\ lastAction' = "start-inspection"

RejectBusy(channel) ==
  /\ connected
  /\ channelPhase[channel] = "request-sent"
  /\ channelEpoch[channel] = runtimeEpoch
  /\ Cardinality(ActiveWorkers) >= MaxConcurrent
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "result-ready"]
  /\ workerOutcome' = [workerOutcome EXCEPT ![channel] = "busy"]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, workerRunning, settlement,
       replayedChannels>>
  /\ lastAction' = "reject-busy"

RejectInvalidPayload(channel) ==
  /\ connected
  /\ channelPhase[channel] = "request-sent"
  /\ channelEpoch[channel] = runtimeEpoch
  /\ Cardinality(ActiveWorkers) < MaxConcurrent
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "result-ready"]
  /\ workerOutcome' = [workerOutcome EXCEPT ![channel] = "invalid-path"]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, workerRunning, settlement,
       replayedChannels>>
  /\ lastAction' = "reject-invalid-payload"

CompleteInspection(channel, outcome) ==
  /\ channelPhase[channel] = "inspecting"
  /\ workerRunning[channel]
  /\ outcome \in WorkerOutcomes \ {"timed-out", "busy", "invalid-path"}
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "result-ready"]
  /\ workerRunning' = [workerRunning EXCEPT ![channel] = FALSE]
  /\ workerOutcome' = [workerOutcome EXCEPT ![channel] = outcome]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, settlement,
       replayedChannels>>
  /\ lastAction' = "complete-inspection"

TimeoutInspection(channel) ==
  /\ channelPhase[channel] = "inspecting"
  /\ workerRunning[channel]
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "result-ready"]
  /\ workerOutcome' = [workerOutcome EXCEPT ![channel] = "timed-out"]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, workerRunning, settlement,
       replayedChannels>>
  /\ lastAction' = "timeout-inspection"

PublishResult(channel) ==
  /\ connected
  /\ channelPhase[channel] = "result-ready"
  /\ channelEpoch[channel] = runtimeEpoch
  /\ resultCredit[channel] = 1
  /\ settlement[channel] = NoOutcome
  /\ workerOutcome[channel] \in WorkerOutcomes
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "result-published"]
  /\ resultCredit' = [resultCredit EXCEPT ![channel] = 0]
  /\ settlement' = [settlement EXCEPT ![channel] = workerOutcome[channel]]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, workerRunning, workerOutcome,
       replayedChannels>>
  /\ lastAction' = "publish-result"

CancelChannel(channel) ==
  /\ channelPhase[channel] \in OpenChannelPhases \ {"result-published", "closing"}
  /\ settlement[channel] = NoOutcome
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "closing"]
  /\ requestCredit' = [requestCredit EXCEPT ![channel] = 0]
  /\ resultCredit' = [resultCredit EXCEPT ![channel] = 0]
  /\ settlement' = [settlement EXCEPT ![channel] = "cancelled"]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, workerRunning, workerOutcome, replayedChannels>>
  /\ lastAction' = "cancel-channel"

BeginCompletedClose(channel) ==
  /\ channelPhase[channel] = "result-published"
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "closing"]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, workerRunning,
       workerOutcome, settlement, replayedChannels>>
  /\ lastAction' = "begin-completed-close"

AcknowledgeClose(channel) ==
  /\ channelPhase[channel] = "closing"
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "closed"]
  /\ requestCredit' = [requestCredit EXCEPT ![channel] = 0]
  /\ resultCredit' = [resultCredit EXCEPT ![channel] = 0]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, workerRunning, workerOutcome, settlement,
       replayedChannels>>
  /\ lastAction' = "acknowledge-close"

LoseAttachment ==
  /\ connected
  /\ connected' = FALSE
  /\ runtimeEpoch' = NoEpoch
  /\ advertisedServices' = {}
  /\ channelPhase' = [channel \in Channels |->
       IF channel \in usedChannels /\ channelPhase[channel] \in OpenChannelPhases
       THEN "attachment-lost"
       ELSE channelPhase[channel]]
  /\ settlement' = [channel \in Channels |->
       IF channel \in usedChannels /\ settlement[channel] = NoOutcome /\
          channelPhase[channel] \in OpenChannelPhases
       THEN "attachment-lost"
       ELSE settlement[channel]]
  /\ requestCredit' = [channel \in Channels |->
       IF channel \in usedChannels /\ channelPhase[channel] \in OpenChannelPhases
       THEN 0
       ELSE requestCredit[channel]]
  /\ resultCredit' = [channel \in Channels |->
       IF channel \in usedChannels /\ channelPhase[channel] \in OpenChannelPhases
       THEN 0
       ELSE resultCredit[channel]]
  /\ UNCHANGED <<usedChannels, channelEpoch, workerRunning, workerOutcome,
       replayedChannels>>
  /\ lastAction' = "lose-attachment"

RetireLostChannel(channel) ==
  /\ channelPhase[channel] = "attachment-lost"
  /\ channelPhase' = [channelPhase EXCEPT ![channel] = "closed"]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelEpoch, requestCredit, resultCredit, workerRunning,
       workerOutcome, settlement, replayedChannels>>
  /\ lastAction' = "retire-lost-channel"

RetireLateWorker(channel) ==
  /\ workerRunning[channel]
  /\ channelPhase[channel] # "inspecting"
  /\ workerRunning' = [workerRunning EXCEPT ![channel] = FALSE]
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelPhase, channelEpoch, requestCredit, resultCredit, workerOutcome,
       settlement, replayedChannels>>
  /\ lastAction' = "retire-late-worker"

RejectReplay(channel) ==
  /\ connected
  /\ channel \in usedChannels
  /\ settlement[channel] = "attachment-lost"
  /\ channelEpoch[channel] # runtimeEpoch
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelPhase, channelEpoch, requestCredit, resultCredit, workerRunning,
       workerOutcome, settlement, replayedChannels>>
  /\ lastAction' = "reject-replay"

Heartbeat ==
  /\ connected
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelPhase, channelEpoch, requestCredit, resultCredit, workerRunning,
       workerOutcome, settlement, replayedChannels>>
  /\ lastAction' = "heartbeat"

TransportViolation ==
  /\ connected
  /\ usedChannels # {}
  /\ connected' = FALSE
  /\ runtimeEpoch' = NoEpoch
  /\ advertisedServices' = {}
  /\ channelPhase' = [channel \in Channels |->
       IF channel \in usedChannels /\ channelPhase[channel] \in OpenChannelPhases
       THEN "attachment-lost"
       ELSE channelPhase[channel]]
  /\ settlement' = [channel \in Channels |->
       IF channel \in usedChannels /\ settlement[channel] = NoOutcome /\
          channelPhase[channel] \in OpenChannelPhases
       THEN "attachment-lost"
       ELSE settlement[channel]]
  /\ requestCredit' = [channel \in Channels |->
       IF channel \in usedChannels /\ channelPhase[channel] \in OpenChannelPhases
       THEN 0
       ELSE requestCredit[channel]]
  /\ resultCredit' = [channel \in Channels |->
       IF channel \in usedChannels /\ channelPhase[channel] \in OpenChannelPhases
       THEN 0
       ELSE resultCredit[channel]]
  /\ UNCHANGED <<usedChannels, channelEpoch, workerRunning, workerOutcome,
       replayedChannels>>
  /\ lastAction' = "transport-violation"

StaleResult(channel) ==
  /\ channel \in usedChannels
  /\ (~connected \/ channelEpoch[channel] # runtimeEpoch)
  /\ UNCHANGED <<connected, runtimeEpoch, advertisedServices, usedChannels,
       channelPhase, channelEpoch, requestCredit, resultCredit, workerRunning,
       workerOutcome, settlement, replayedChannels>>
  /\ lastAction' = "stale-result"

Next ==
  \/ Connect
  \/ Reconnect
  \/ \E channel \in Channels : OpenChannel(channel)
  \/ \E channel \in Channels : AcceptChannel(channel)
  \/ \E channel \in Channels : SendRequest(channel)
  \/ \E channel \in Channels : StartInspection(channel)
  \/ \E channel \in Channels : RejectBusy(channel)
  \/ \E channel \in Channels : RejectInvalidPayload(channel)
  \/ \E channel \in Channels, outcome \in WorkerOutcomes :
       CompleteInspection(channel, outcome)
  \/ \E channel \in Channels : TimeoutInspection(channel)
  \/ \E channel \in Channels : PublishResult(channel)
  \/ \E channel \in Channels : CancelChannel(channel)
  \/ \E channel \in Channels : BeginCompletedClose(channel)
  \/ \E channel \in Channels : AcknowledgeClose(channel)
  \/ LoseAttachment
  \/ \E channel \in Channels : RetireLostChannel(channel)
  \/ \E channel \in Channels : RetireLateWorker(channel)
  \/ \E channel \in Channels : RejectReplay(channel)
  \/ Heartbeat
  \/ TransportViolation
  \/ \E channel \in Channels : StaleResult(channel)

WorkerStops(channel) ==
  (\E outcome \in WorkerOutcomes : CompleteInspection(channel, outcome)) \/
    RetireLateWorker(channel)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A channel \in Channels : WF_vars(WorkerStops(channel))

TypeOK ==
  /\ connected \in BOOLEAN
  /\ runtimeEpoch \in Epochs \cup {NoEpoch}
  /\ advertisedServices \subseteq ExpectedServices
  /\ usedChannels \subseteq Channels
  /\ channelPhase \in [Channels -> ChannelPhases]
  /\ channelEpoch \in [Channels -> Epochs \cup {NoEpoch}]
  /\ requestCredit \in [Channels -> 0..1]
  /\ resultCredit \in [Channels -> 0..1]
  /\ workerRunning \in [Channels -> BOOLEAN]
  /\ workerOutcome \in [Channels -> WorkerOutcomes \cup {NoOutcome}]
  /\ settlement \in [Channels -> Settlements]
  /\ replayedChannels \subseteq Channels
  /\ lastAction \in Actions

ExactCapabilityWhenConnected ==
  connected => advertisedServices = ExpectedServices

NoChannelReuse ==
  \A channel \in usedChannels : channelPhase[channel] # "unused"

ChannelEpochIsPinned ==
  \A channel \in usedChannels : channelEpoch[channel] \in Epochs

ConcurrencyIsBounded == Cardinality(ActiveWorkers) <= MaxConcurrent

RequestConsumesCredit ==
  \A channel \in Channels :
    channelPhase[channel] \in {
      "request-sent", "inspecting", "result-ready", "result-published",
      "closing", "closed", "attachment-lost"
    } => requestCredit[channel] = 0

ResultConsumesCredit ==
  \A channel \in Channels :
    channelPhase[channel] = "result-published" => resultCredit[channel] = 0

PublishedResultUsesCurrentEpoch ==
  \A channel \in Channels :
    channelPhase[channel] = "result-published" =>
      (connected /\ channelEpoch[channel] = runtimeEpoch)

SettlementIsSingleAssignment == replayedChannels = {}

CancelledNeverPublishes ==
  \A channel \in Channels :
    settlement[channel] = "cancelled" =>
      channelPhase[channel] \notin {"result-ready", "result-published"}

AttachmentLossNeverPublishes ==
  \A channel \in Channels :
    settlement[channel] = "attachment-lost" =>
      channelPhase[channel] \notin {"result-ready", "result-published"}

WorkerCannotGrantAuthority ==
  \A channel \in Channels :
    workerRunning[channel] => settlement[channel] \in {
      NoOutcome, "cancelled", "attachment-lost", "timed-out"
    }

NoAutomaticReplay == replayedChannels = {}

WorkerEventuallyStops ==
  \A channel \in Channels : [](workerRunning[channel] ~> ~workerRunning[channel])

=============================================================================
