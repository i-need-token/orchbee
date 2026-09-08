----------------- MODULE TerminalCloseActivityFreshness -----------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-006, TH-010, and TH-012 for the
\* projection-only close-activity observation. Every accepted non-focus input,
\* including a zero-byte encoding, immediately invalidates an older activity
\* sample. A later successful sample resolves the current platform foreground
\* observation together with Ghostty's current semantic-prompt state, so an
\* inactive terminal may return to idle in the same lifecycle. Platform-active
\* and semantic-command observations remain active, while a failed platform
\* query remains unknown.
\*
\* Input does not synchronously mutate Ghostty's prompt state. A sample may
\* therefore observe the same prompt immediately after input, matching the
\* pinned Ghostty close-confirmation reference. This model claims freshness of
\* the sampled state, not shell consumption of the input.
\*
\* The activity epoch plus its exhausted bit prevents an ABA commit after
\* input invalidation, including at the bounded counter maximum.
\* PrepareSnapshot, BatchFull, EmitSnapshot, FrameApplied, and NeedSnapshot
\* model same-revision snapshots, force-after-flight retry, and exact stale
\* flight abandonment. This Host model does not pretend to synchronously
\* mutate consumer-local presentation state: an input-originating client's
\* local invalidation and causal barrier are verified by the terminal client.

CONSTANTS Attachments, MaxRevision, MaxActivityEpoch, MaxCadence,
          MaxSequence, MaxPendingWrites

AttachmentSymmetry == Permutations(Attachments)

Active == "active"
Idle == "idle"
Unknown == "unknown"

AtPrompt == "at-prompt"
AwayFromPrompt == "away-from-prompt"
PromptUnavailable == "prompt-unavailable"

Ready == "ready"
Provisional == "provisional"
InFlight == "in-flight"

VARIABLES
  revision,
  processActive,
  terminalPrompt,
  pendingWrites,
  cachedActivity,
  activityEpoch,
  activityEpochExhausted,
  cadence,
  sampledCadence,
  delivery,
  snapshotRequired,
  forceSnapshotAfterFlight,
  frameActivity,
  frameEpoch,
  frameEpochExhausted,
  frameRevision,
  frameSequence,
  nextSequence,
  semanticStepSafe

coreVars ==
  <<revision, processActive, terminalPrompt, pendingWrites, cachedActivity,
    activityEpoch, activityEpochExhausted, cadence, sampledCadence,
    delivery, snapshotRequired, forceSnapshotAfterFlight, frameActivity,
    frameEpoch, frameEpochExhausted, frameRevision, frameSequence,
    nextSequence>>

vars == <<coreVars, semanticStepSafe>>

Init ==
  /\ revision = 0
  /\ processActive = FALSE
  /\ terminalPrompt = AtPrompt
  /\ pendingWrites = 0
  /\ cachedActivity = Idle
  /\ activityEpoch = 0
  /\ activityEpochExhausted = FALSE
  /\ cadence = 0
  /\ sampledCadence = 0
  /\ delivery = [attachment \in Attachments |-> Ready]
  /\ snapshotRequired = [attachment \in Attachments |-> TRUE]
  /\ forceSnapshotAfterFlight = [attachment \in Attachments |-> FALSE]
  /\ frameActivity = [attachment \in Attachments |-> Unknown]
  /\ frameEpoch = [attachment \in Attachments |-> 0]
  /\ frameEpochExhausted = [attachment \in Attachments |-> FALSE]
  /\ frameRevision = [attachment \in Attachments |-> 0]
  /\ frameSequence = [attachment \in Attachments |-> 0]
  /\ nextSequence = [attachment \in Attachments |-> 1]
  /\ semanticStepSafe = TRUE

InvalidateReady(nextDelivery, nextRequired, nextForce) ==
  /\ nextDelivery = delivery
  /\ nextRequired =
       [attachment \in Attachments |->
          IF delivery[attachment] = Ready
            THEN TRUE
            ELSE snapshotRequired[attachment]]
  /\ nextForce =
       [attachment \in Attachments |->
          IF delivery[attachment] = InFlight
            THEN TRUE
            ELSE forceSnapshotAfterFlight[attachment]]

InputCommit(hasBytes) ==
  /\ hasBytes \in BOOLEAN
  /\ ~hasBytes \/ pendingWrites < MaxPendingWrites
  /\ pendingWrites' = IF hasBytes THEN pendingWrites + 1 ELSE pendingWrites
  /\ cachedActivity' = Unknown
  /\ activityEpoch' =
       IF activityEpoch < MaxActivityEpoch
         THEN activityEpoch + 1
         ELSE activityEpoch
  /\ activityEpochExhausted' =
       (activityEpochExhausted \/ activityEpoch = MaxActivityEpoch)
  /\ InvalidateReady(delivery', snapshotRequired', forceSnapshotAfterFlight')
  /\ UNCHANGED <<revision, processActive, terminalPrompt, cadence, sampledCadence,
                  frameActivity, frameEpoch, frameRevision,
                  frameEpochExhausted, frameSequence, nextSequence, semanticStepSafe>>

\* Physical delivery does not itself refresh the activity observation.
WriterCompletesInput ==
  /\ pendingWrites > 0
  /\ pendingWrites' = pendingWrites - 1
  /\ UNCHANGED <<revision, processActive, terminalPrompt, cachedActivity,
                  activityEpoch, activityEpochExhausted, cadence,
                  sampledCadence, delivery, snapshotRequired,
                  forceSnapshotAfterFlight, frameActivity,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence, semanticStepSafe>>

\* Output may move Ghostty's current semantic-prompt observation. It does not
\* refresh the cached close activity until the next bounded sample.
OutputCommit(promptState) ==
  /\ promptState \in {AtPrompt, AwayFromPrompt, PromptUnavailable}
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ terminalPrompt' = promptState
  /\ UNCHANGED <<processActive, pendingWrites, cachedActivity,
                  activityEpoch, activityEpochExhausted, cadence,
                  sampledCadence, delivery, snapshotRequired,
                  forceSnapshotAfterFlight, frameActivity,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence, semanticStepSafe>>

SilentForegroundChange(active) ==
  /\ active \in BOOLEAN
  /\ active # processActive
  /\ processActive' = active
  /\ UNCHANGED <<revision, terminalPrompt, pendingWrites, cachedActivity,
                  activityEpoch, activityEpochExhausted, cadence,
                  sampledCadence, delivery, snapshotRequired,
                  forceSnapshotAfterFlight, frameActivity,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence, semanticStepSafe>>

AdvanceCadence ==
  /\ cadence < MaxCadence
  /\ cadence' = cadence + 1
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cachedActivity, activityEpoch,
                  activityEpochExhausted, sampledCadence, delivery,
                  snapshotRequired, forceSnapshotAfterFlight, frameActivity,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence, semanticStepSafe>>

SampleSuccess ==
  /\ sampledCadence < cadence
  /\ ~activityEpochExhausted
  /\ LET requestedActivity ==
           IF processActive
             THEN Active
             ELSE IF terminalPrompt = AwayFromPrompt THEN Active ELSE Idle
         epochOverflow ==
           requestedActivity # cachedActivity /\
             activityEpoch = MaxActivityEpoch
         nextActivity == IF epochOverflow THEN Unknown ELSE requestedActivity
         changed == nextActivity # cachedActivity
     IN
       /\ cachedActivity' = nextActivity
       /\ activityEpoch' =
            IF requestedActivity # cachedActivity /\
                 activityEpoch < MaxActivityEpoch
              THEN activityEpoch + 1
              ELSE activityEpoch
       /\ activityEpochExhausted' = epochOverflow
       /\ IF changed
            THEN InvalidateReady(
                   delivery',
                   snapshotRequired',
                   forceSnapshotAfterFlight')
            ELSE /\ delivery' = delivery
                 /\ snapshotRequired' = snapshotRequired
                 /\ forceSnapshotAfterFlight' = forceSnapshotAfterFlight
  /\ sampledCadence' = cadence
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cadence, frameActivity, frameEpoch,
                  frameEpochExhausted, frameRevision, frameSequence,
                  nextSequence>>
  /\ semanticStepSafe' =
       semanticStepSafe /\
         (processActive \/ terminalPrompt = AwayFromPrompt \/
          (cachedActivity # Idle /\ activityEpoch = MaxActivityEpoch) \/
          cachedActivity' = Idle)

SampleFailure ==
  /\ sampledCadence < cadence
  /\ ~activityEpochExhausted
  /\ LET epochOverflow ==
           cachedActivity # Unknown /\ activityEpoch = MaxActivityEpoch
         changed == cachedActivity # Unknown
     IN
       /\ cachedActivity' = Unknown
       /\ activityEpoch' =
            IF cachedActivity # Unknown /\ activityEpoch < MaxActivityEpoch
              THEN activityEpoch + 1
              ELSE activityEpoch
       /\ activityEpochExhausted' = epochOverflow
       /\ IF changed
            THEN InvalidateReady(
                   delivery',
                   snapshotRequired',
                   forceSnapshotAfterFlight')
            ELSE /\ delivery' = delivery
                 /\ snapshotRequired' = snapshotRequired
                 /\ forceSnapshotAfterFlight' = forceSnapshotAfterFlight
  /\ sampledCadence' = cadence
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cadence, frameActivity, frameEpoch,
                  frameEpochExhausted, frameRevision, frameSequence,
                  nextSequence, semanticStepSafe>>

PrepareSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = Ready
  /\ snapshotRequired[attachment]
  /\ nextSequence[attachment] <= MaxSequence
  /\ delivery' = [delivery EXCEPT ![attachment] = Provisional]
  /\ frameActivity' = [frameActivity EXCEPT ![attachment] = cachedActivity]
  /\ frameEpoch' = [frameEpoch EXCEPT ![attachment] = activityEpoch]
  /\ frameEpochExhausted' =
       [frameEpochExhausted EXCEPT ![attachment] = activityEpochExhausted]
  /\ frameRevision' = [frameRevision EXCEPT ![attachment] = revision]
  /\ frameSequence' =
       [frameSequence EXCEPT ![attachment] = nextSequence[attachment]]
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cachedActivity, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  snapshotRequired, forceSnapshotAfterFlight, nextSequence, semanticStepSafe>>

BatchFull(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = Provisional
  /\ delivery' = [delivery EXCEPT ![attachment] = Ready]
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cachedActivity, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  snapshotRequired, forceSnapshotAfterFlight, frameActivity,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence, semanticStepSafe>>

EmitSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = Provisional
  /\ frameEpoch[attachment] = activityEpoch
  /\ frameEpochExhausted[attachment] = activityEpochExhausted
  /\ delivery' = [delivery EXCEPT ![attachment] = InFlight]
  /\ snapshotRequired' = [snapshotRequired EXCEPT ![attachment] = FALSE]
  /\ nextSequence' = [nextSequence EXCEPT ![attachment] = @ + 1]
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cachedActivity, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  forceSnapshotAfterFlight, frameActivity,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, semanticStepSafe>>

FlightStatePreserved ==
  /\ delivery' = delivery
  /\ snapshotRequired' = snapshotRequired
  /\ forceSnapshotAfterFlight' = forceSnapshotAfterFlight
  /\ frameActivity' = frameActivity
  /\ frameEpoch' = frameEpoch
  /\ frameEpochExhausted' = frameEpochExhausted
  /\ frameRevision' = frameRevision
  /\ frameSequence' = frameSequence
  /\ nextSequence' = nextSequence

ExactFrameAppliedInput(attachment, receivedSequence, receivedRevision) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ receivedSequence = frameSequence[attachment]
  /\ receivedRevision = frameRevision[attachment]

MonitorFrameAppliedInput(attachment, receivedSequence, receivedRevision) ==
  semanticStepSafe' =
    semanticStepSafe
      /\ (ExactFrameAppliedInput(attachment, receivedSequence,
                                 receivedRevision)
          \/ FlightStatePreserved)

FrameApplied(attachment, receivedSequence, receivedRevision) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ receivedSequence = frameSequence[attachment]
  /\ receivedRevision = frameRevision[attachment]
  /\ delivery' = [delivery EXCEPT ![attachment] = Ready]
  /\ snapshotRequired' = [snapshotRequired EXCEPT
       ![attachment] = forceSnapshotAfterFlight[attachment]]
  /\ forceSnapshotAfterFlight' =
       [forceSnapshotAfterFlight EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cachedActivity, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  frameActivity, frameEpoch, frameEpochExhausted,
                  frameRevision, frameSequence, nextSequence>>
  /\ MonitorFrameAppliedInput(attachment, receivedSequence, receivedRevision)

RejectFrameApplied(attachment, receivedSequence, receivedRevision) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ receivedSequence \in 0..(MaxSequence + 1)
  /\ receivedRevision \in 0..MaxRevision
  /\ (receivedSequence # frameSequence[attachment]
        \/ receivedRevision # frameRevision[attachment])
  /\ UNCHANGED coreVars
  /\ MonitorFrameAppliedInput(attachment, receivedSequence, receivedRevision)

ExactNeedSnapshotInput(attachment, receivedSequence) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ receivedSequence = frameSequence[attachment]

MonitorNeedSnapshotInput(attachment, receivedSequence) ==
  semanticStepSafe' =
    semanticStepSafe
      /\ (ExactNeedSnapshotInput(attachment, receivedSequence)
          \/ FlightStatePreserved)

NeedSnapshot(attachment, receivedSequence) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ receivedSequence = frameSequence[attachment]
  /\ delivery' = [delivery EXCEPT ![attachment] = Ready]
  /\ snapshotRequired' = [snapshotRequired EXCEPT ![attachment] = TRUE]
  /\ forceSnapshotAfterFlight' =
       [forceSnapshotAfterFlight EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, processActive, terminalPrompt, pendingWrites,
                  cachedActivity, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  frameActivity, frameEpoch, frameEpochExhausted,
                  frameRevision, frameSequence, nextSequence>>
  /\ MonitorNeedSnapshotInput(attachment, receivedSequence)

RejectNeedSnapshot(attachment, receivedSequence) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ receivedSequence \in 0..(MaxSequence + 1)
  /\ receivedSequence # frameSequence[attachment]
  /\ UNCHANGED coreVars
  /\ MonitorNeedSnapshotInput(attachment, receivedSequence)

Next ==
  \/ \E hasBytes \in BOOLEAN: InputCommit(hasBytes)
  \/ WriterCompletesInput
  \/ \E promptState \in {AtPrompt, AwayFromPrompt, PromptUnavailable}:
       OutputCommit(promptState)
  \/ AdvanceCadence
  \/ SampleSuccess
  \/ SampleFailure
  \/ \E active \in BOOLEAN: SilentForegroundChange(active)
  \/ \E attachment \in Attachments:
       \/ PrepareSnapshot(attachment)
       \/ BatchFull(attachment)
       \/ EmitSnapshot(attachment)
       \/ \E receivedSequence \in 0..(MaxSequence + 1),
             receivedRevision \in 0..MaxRevision:
            \/ FrameApplied(attachment, receivedSequence, receivedRevision)
            \/ RejectFrameApplied(attachment, receivedSequence, receivedRevision)
       \/ \E receivedSequence \in 0..(MaxSequence + 1):
            \/ NeedSnapshot(attachment, receivedSequence)
            \/ RejectNeedSnapshot(attachment, receivedSequence)

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ processActive \in BOOLEAN
  /\ terminalPrompt \in {AtPrompt, AwayFromPrompt, PromptUnavailable}
  /\ pendingWrites \in 0..MaxPendingWrites
  /\ cachedActivity \in {Active, Idle, Unknown}
  /\ activityEpoch \in 0..MaxActivityEpoch
  /\ activityEpochExhausted \in BOOLEAN
  /\ cadence \in 0..MaxCadence
  /\ sampledCadence \in 0..MaxCadence
  /\ delivery \in [Attachments -> {Ready, Provisional, InFlight}]
  /\ snapshotRequired \in [Attachments -> BOOLEAN]
  /\ forceSnapshotAfterFlight \in [Attachments -> BOOLEAN]
  /\ frameActivity \in [Attachments -> {Active, Idle, Unknown}]
  /\ frameEpoch \in [Attachments -> 0..MaxActivityEpoch]
  /\ frameEpochExhausted \in [Attachments -> BOOLEAN]
  /\ frameRevision \in [Attachments -> 0..MaxRevision]
  /\ frameSequence \in [Attachments -> 0..MaxSequence]
  /\ nextSequence \in [Attachments -> 1..(MaxSequence + 1)]
  /\ semanticStepSafe \in BOOLEAN

CurrentTokenFrameMatchesCache ==
  \A attachment \in Attachments:
    delivery[attachment] \in {Provisional, InFlight} /\
      frameEpoch[attachment] = activityEpoch /\
      frameEpochExhausted[attachment] = activityEpochExhausted =>
        frameActivity[attachment] = cachedActivity

EpochExhaustionIsUnknown ==
  activityEpochExhausted => cachedActivity = Unknown

EveryReadAttachmentTracksActivityInvalidation ==
  \A attachment \in Attachments:
    delivery[attachment] = Ready /\ ~snapshotRequired[attachment] =>
      (frameEpoch[attachment] = activityEpoch /\
         frameEpochExhausted[attachment] = activityEpochExhausted) \/
        frameActivity[attachment] = Unknown

InFlightInvalidationForcesSnapshotRetry ==
  \A attachment \in Attachments:
    forceSnapshotAfterFlight[attachment] =>
      delivery[attachment] = InFlight

PreparedFrameEpochCannotLead ==
  \A attachment \in Attachments:
    delivery[attachment] = Provisional =>
      frameEpoch[attachment] <= activityEpoch

BatchFullCannotConsumeSequence ==
  \A attachment \in Attachments:
    delivery[attachment] = Provisional =>
      frameSequence[attachment] = nextSequence[attachment]

SemanticStepWitness == semanticStepSafe

=============================================================================
