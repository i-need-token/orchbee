------------------------- MODULE TerminalHostDelivery ------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-003, TH-010, TH-011, and TH-012. One
\* capability abstracts one attachment-plus-delivery-kind flight; Prepare,
\* HandoffBatch, WriteSucceeded, WriteFailed, Acknowledge, Pause, and Detach
\* preserve the same commit boundary as the runtime model.

CONSTANTS
  SmallCapability,
  LargeCapability,
  OversizeCapability,
  MaxRevision,
  MaxSequence,
  MaxBatchBytes

Capabilities == {SmallCapability, LargeCapability, OversizeCapability}
CapabilityStates == {"absent", "paused", "ready", "in_flight", "blocked"}

RecordBytes(capability) ==
  IF capability = SmallCapability THEN 1
  ELSE IF capability = LargeCapability THEN MaxBatchBytes
  ELSE MaxBatchBytes + 1

BatchBytes(items) ==
  (IF SmallCapability \in items THEN 1 ELSE 0) +
  (IF LargeCapability \in items THEN MaxBatchBytes ELSE 0) +
  (IF OversizeCapability \in items THEN MaxBatchBytes + 1 ELSE 0)

VARIABLES
  brokerActive,
  revision,
  capabilityState,
  dirty,
  nextSequence,
  flightSequence,
  flightRevision,
  lastAppliedSequence,
  blockedRevision,
  handedSequences,
  prepared,
  preparedSequence,
  preparedRevision,
  handed

deliveryVars ==
  <<capabilityState, dirty, nextSequence, flightSequence, flightRevision,
    lastAppliedSequence, blockedRevision, handedSequences>>
batchVars == <<prepared, preparedSequence, preparedRevision, handed>>
vars == <<brokerActive, revision, deliveryVars, batchVars>>

Init ==
  /\ brokerActive = FALSE
  /\ revision = 0
  /\ capabilityState = [capability \in Capabilities |-> "absent"]
  /\ dirty = [capability \in Capabilities |-> FALSE]
  /\ nextSequence = [capability \in Capabilities |-> 1]
  /\ flightSequence = [capability \in Capabilities |-> 0]
  /\ flightRevision = [capability \in Capabilities |-> 0]
  /\ lastAppliedSequence = [capability \in Capabilities |-> 0]
  /\ blockedRevision = [capability \in Capabilities |-> 0]
  /\ handedSequences = [capability \in Capabilities |-> {}]
  /\ prepared = {}
  /\ preparedSequence = [capability \in Capabilities |-> 0]
  /\ preparedRevision = [capability \in Capabilities |-> 0]
  /\ handed = {}

Connect ==
  /\ ~brokerActive
  /\ brokerActive' = TRUE
  /\ UNCHANGED <<revision, deliveryVars, batchVars>>

Attach(capability) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ capabilityState[capability] = "absent"
  /\ capabilityState' =
       [capabilityState EXCEPT ![capability] = "paused"]
  /\ dirty' = [dirty EXCEPT ![capability] = FALSE]
  /\ nextSequence' = [nextSequence EXCEPT ![capability] = 1]
  /\ flightSequence' = [flightSequence EXCEPT ![capability] = 0]
  /\ flightRevision' = [flightRevision EXCEPT ![capability] = 0]
  /\ lastAppliedSequence' =
       [lastAppliedSequence EXCEPT ![capability] = 0]
  /\ blockedRevision' = [blockedRevision EXCEPT ![capability] = 0]
  /\ handedSequences' = [handedSequences EXCEPT ![capability] = {}]
  /\ UNCHANGED <<brokerActive, revision, batchVars>>

Activate(capability) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ capabilityState[capability] = "paused"
  /\ capabilityState' =
       [capabilityState EXCEPT ![capability] = "ready"]
  /\ dirty' = [dirty EXCEPT ![capability] = TRUE]
  /\ UNCHANGED <<brokerActive, revision, nextSequence, flightSequence,
                  flightRevision, lastAppliedSequence, blockedRevision, handedSequences,
                  batchVars>>

MutateTerminal ==
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ capabilityState' = [capability \in Capabilities |->
       IF capabilityState[capability] = "blocked"
       THEN "ready"
       ELSE capabilityState[capability]]
  /\ dirty' = [capability \in Capabilities |->
       IF capabilityState[capability] \in {"ready", "in_flight", "blocked"}
       THEN TRUE
       ELSE dirty[capability]]
  /\ blockedRevision' = [capability \in Capabilities |->
       IF capabilityState[capability] = "blocked"
       THEN 0
       ELSE blockedRevision[capability]]
  /\ UNCHANGED <<brokerActive, nextSequence, flightSequence,
                  flightRevision, lastAppliedSequence, handedSequences,
                  batchVars>>

Prepare(capability) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ capabilityState[capability] = "ready"
  /\ dirty[capability]
  /\ capability \notin prepared
  /\ nextSequence[capability] <= MaxSequence
  /\ RecordBytes(capability) <= MaxBatchBytes
  /\ BatchBytes(prepared) + RecordBytes(capability) <= MaxBatchBytes
  /\ prepared' = prepared \cup {capability}
  /\ preparedSequence' = [preparedSequence EXCEPT
       ![capability] = nextSequence[capability]]
  /\ preparedRevision' = [preparedRevision EXCEPT
       ![capability] = revision]
  /\ UNCHANGED <<brokerActive, revision, deliveryVars, handed>>

AbortBatch ==
  /\ brokerActive
  /\ handed = {}
  /\ prepared # {}
  /\ prepared' = {}
  /\ preparedSequence' = [capability \in Capabilities |-> 0]
  /\ preparedRevision' = [capability \in Capabilities |-> 0]
  /\ UNCHANGED <<brokerActive, revision, deliveryVars, handed>>

HandoffBatch ==
  /\ brokerActive
  /\ handed = {}
  /\ prepared # {}
  /\ handed' = prepared
  /\ capabilityState' = [capability \in Capabilities |->
       IF capability \in prepared THEN "in_flight"
       ELSE capabilityState[capability]]
  /\ dirty' = [capability \in Capabilities |->
       IF capability \in prepared
       THEN revision > preparedRevision[capability]
       ELSE dirty[capability]]
  /\ nextSequence' = [capability \in Capabilities |->
       IF capability \in prepared THEN nextSequence[capability] + 1
       ELSE nextSequence[capability]]
  /\ flightSequence' = [capability \in Capabilities |->
       IF capability \in prepared THEN preparedSequence[capability]
       ELSE flightSequence[capability]]
  /\ flightRevision' = [capability \in Capabilities |->
       IF capability \in prepared THEN preparedRevision[capability]
       ELSE flightRevision[capability]]
  /\ handedSequences' = [capability \in Capabilities |->
       IF capability \in prepared
       THEN handedSequences[capability] \cup {preparedSequence[capability]}
       ELSE handedSequences[capability]]
  /\ prepared' = {}
  /\ preparedSequence' = [capability \in Capabilities |-> 0]
  /\ preparedRevision' = [capability \in Capabilities |-> 0]
  /\ UNCHANGED <<brokerActive, revision, lastAppliedSequence, blockedRevision>>

HandoffFullPrefix(nextCapability) ==
  /\ nextCapability \in Capabilities
  /\ nextCapability \notin prepared
  /\ capabilityState[nextCapability] = "ready"
  /\ dirty[nextCapability]
  /\ RecordBytes(nextCapability) <= MaxBatchBytes
  /\ BatchBytes(prepared) + RecordBytes(nextCapability) > MaxBatchBytes
  /\ HandoffBatch

WriteSucceeded ==
  /\ brokerActive
  /\ handed # {}
  /\ handed' = {}
  /\ UNCHANGED <<brokerActive, revision, deliveryVars, prepared,
                  preparedSequence, preparedRevision>>

ResetBrokerCapabilities ==
  /\ brokerActive' = FALSE
  /\ capabilityState' = [capability \in Capabilities |-> "absent"]
  /\ dirty' = [capability \in Capabilities |-> FALSE]
  /\ nextSequence' = [capability \in Capabilities |-> 1]
  /\ flightSequence' = [capability \in Capabilities |-> 0]
  /\ flightRevision' = [capability \in Capabilities |-> 0]
  /\ lastAppliedSequence' = [capability \in Capabilities |-> 0]
  /\ blockedRevision' = [capability \in Capabilities |-> 0]
  /\ handedSequences' = [capability \in Capabilities |-> {}]
  /\ prepared' = {}
  /\ preparedSequence' = [capability \in Capabilities |-> 0]
  /\ preparedRevision' = [capability \in Capabilities |-> 0]
  /\ handed' = {}
  /\ UNCHANGED revision

WriteFailed ==
  /\ brokerActive
  /\ handed # {}
  /\ ResetBrokerCapabilities

Disconnect ==
  /\ brokerActive
  /\ handed = {}
  /\ ResetBrokerCapabilities

Acknowledge(capability, sequence) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ sequence \in 1..MaxSequence
  /\ capabilityState[capability] # "absent"
  /\ IF capabilityState[capability] = "in_flight"
        /\ sequence = flightSequence[capability]
     THEN
       /\ capabilityState' =
            [capabilityState EXCEPT ![capability] = "ready"]
       /\ lastAppliedSequence' =
            [lastAppliedSequence EXCEPT ![capability] = sequence]
       /\ flightSequence' = [flightSequence EXCEPT ![capability] = 0]
       /\ flightRevision' = [flightRevision EXCEPT ![capability] = 0]
       /\ UNCHANGED <<brokerActive, revision, dirty, nextSequence,
                       blockedRevision, handedSequences, batchVars>>
     ELSE
       /\ UNCHANGED vars

Pause(capability) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ capabilityState[capability] \in {
       "ready", "in_flight", "blocked"
     }
  /\ capabilityState' =
       [capabilityState EXCEPT ![capability] = "paused"]
  /\ dirty' = [dirty EXCEPT ![capability] = FALSE]
  /\ flightSequence' = [flightSequence EXCEPT ![capability] = 0]
  /\ flightRevision' = [flightRevision EXCEPT ![capability] = 0]
  /\ blockedRevision' = [blockedRevision EXCEPT ![capability] = 0]
  /\ prepared' = prepared \ {capability}
  /\ preparedSequence' =
       [preparedSequence EXCEPT ![capability] = 0]
  /\ preparedRevision' =
       [preparedRevision EXCEPT ![capability] = 0]
  /\ UNCHANGED <<brokerActive, revision, nextSequence,
                  lastAppliedSequence, handedSequences, handed>>

Detach(capability) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ capabilityState[capability] # "absent"
  /\ capabilityState' =
       [capabilityState EXCEPT ![capability] = "absent"]
  /\ dirty' = [dirty EXCEPT ![capability] = FALSE]
  /\ nextSequence' = [nextSequence EXCEPT ![capability] = 1]
  /\ flightSequence' = [flightSequence EXCEPT ![capability] = 0]
  /\ flightRevision' = [flightRevision EXCEPT ![capability] = 0]
  /\ lastAppliedSequence' =
       [lastAppliedSequence EXCEPT ![capability] = 0]
  /\ blockedRevision' = [blockedRevision EXCEPT ![capability] = 0]
  /\ handedSequences' = [handedSequences EXCEPT ![capability] = {}]
  /\ prepared' = prepared \ {capability}
  /\ preparedSequence' =
       [preparedSequence EXCEPT ![capability] = 0]
  /\ preparedRevision' =
       [preparedRevision EXCEPT ![capability] = 0]
  /\ UNCHANGED <<brokerActive, revision, handed>>

BlockOversize(capability) ==
  /\ brokerActive
  /\ handed = {}
  /\ capability \in Capabilities
  /\ capabilityState[capability] = "ready"
  /\ dirty[capability]
  /\ capability \notin prepared
  /\ RecordBytes(capability) > MaxBatchBytes
  /\ capabilityState' =
       [capabilityState EXCEPT ![capability] = "blocked"]
  /\ dirty' = [dirty EXCEPT ![capability] = FALSE]
  /\ blockedRevision' = [blockedRevision EXCEPT ![capability] = revision]
  /\ UNCHANGED <<brokerActive, revision, nextSequence,
                  flightSequence, flightRevision, lastAppliedSequence,
                  handedSequences, batchVars>>

Next ==
  \/ Connect
  \/ \E capability \in Capabilities: Attach(capability)
  \/ \E capability \in Capabilities: Activate(capability)
  \/ MutateTerminal
  \/ \E capability \in Capabilities: Prepare(capability)
  \/ AbortBatch
  \/ HandoffBatch
  \/ \E capability \in Capabilities: HandoffFullPrefix(capability)
  \/ WriteSucceeded
  \/ WriteFailed
  \/ Disconnect
  \/ \E capability \in Capabilities, sequence \in 1..MaxSequence:
       Acknowledge(capability, sequence)
  \/ \E capability \in Capabilities: Pause(capability)
  \/ \E capability \in Capabilities: Detach(capability)
  \/ \E capability \in Capabilities: BlockOversize(capability)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(HandoffBatch)

TypeOK ==
  /\ brokerActive \in BOOLEAN
  /\ revision \in 0..MaxRevision
  /\ capabilityState \in [Capabilities -> CapabilityStates]
  /\ dirty \in [Capabilities -> BOOLEAN]
  /\ nextSequence \in [Capabilities -> 1..(MaxSequence + 1)]
  /\ flightSequence \in [Capabilities -> 0..MaxSequence]
  /\ flightRevision \in [Capabilities -> 0..MaxRevision]
  /\ lastAppliedSequence \in [Capabilities -> 0..MaxSequence]
  /\ blockedRevision \in [Capabilities -> 0..MaxRevision]
  /\ handedSequences \in [Capabilities -> SUBSET (1..MaxSequence)]
  /\ prepared \subseteq Capabilities
  /\ preparedSequence \in [Capabilities -> 0..MaxSequence]
  /\ preparedRevision \in [Capabilities -> 0..MaxRevision]
  /\ handed \subseteq Capabilities

BrokerScopeIntegrity ==
  /\ ~brokerActive =>
       /\ prepared = {}
       /\ handed = {}
       /\ \A capability \in Capabilities:
            capabilityState[capability] = "absent"
  /\ handed # {} => brokerActive

BatchIntegrity ==
  /\ prepared \cap handed = {}
  /\ BatchBytes(prepared) <= MaxBatchBytes
  /\ BatchBytes(handed) <= MaxBatchBytes
  /\ \A capability \in Capabilities:
       /\ capability \in prepared =>
            /\ capabilityState[capability] = "ready"
            /\ dirty[capability]
            /\ preparedSequence[capability] = nextSequence[capability]
       /\ capability \notin prepared =>
            /\ preparedSequence[capability] = 0
            /\ preparedRevision[capability] = 0
       /\ capability \in handed =>
            /\ capabilityState[capability] = "in_flight"
            /\ flightSequence[capability] \in
                 handedSequences[capability]

DeliveryIntegrity ==
  \A capability \in Capabilities:
    /\ nextSequence[capability] =
         Cardinality(handedSequences[capability]) + 1
    /\ (lastAppliedSequence[capability] = 0) \/
         (lastAppliedSequence[capability] \in
            handedSequences[capability])
    /\ (capabilityState[capability] = "in_flight") <=>
         (flightSequence[capability] # 0)
    /\ capabilityState[capability] = "in_flight" =>
         flightSequence[capability] \in handedSequences[capability]
    /\ capabilityState[capability] \in {
         "absent", "paused", "blocked"
       } => ~dirty[capability]
    /\ capabilityState[capability] = "blocked" =>
         blockedRevision[capability] = revision
    /\ capabilityState[capability] # "blocked" =>
         blockedRevision[capability] = 0

OversizeNeverConsumesSequence ==
  handedSequences[OversizeCapability] = {}

AbortDoesNotCommit ==
  [][AbortBatch =>
       UNCHANGED <<deliveryVars, brokerActive, revision, handed>>]_vars

HandoffDoesNotDisconnect ==
  [][HandoffBatch => UNCHANGED brokerActive]_vars

TransportFailurePreservesTerminal ==
  [][(WriteFailed \/ Disconnect) => UNCHANGED revision]_vars

=============================================================================
