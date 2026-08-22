----------------- MODULE TerminalCloseActivityFreshness -----------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-006, TH-010, and TH-012 for the
\* projection-only close-activity observation. Every accepted non-focus input,
\* including a zero-byte encoding, permanently latches the current terminal
\* lifecycle. Writer completion, PTY output, semantic prompt markers, a
\* foreground-process transition, and elapsed cadence cannot clear that latch
\* because none is an epoch-tagged shell acknowledgement. A successful sample
\* may publish active while latched only from an independently observed active
\* foreground process; every inactive observation must publish unknown.
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

Ready == "ready"
Provisional == "provisional"
InFlight == "in-flight"

VARIABLES
  revision,
  processActive,
  pendingWrites,
  inputLatched,
  cachedActivity,
  cacheFresh,
  activityEpoch,
  activityEpochExhausted,
  cadence,
  sampledCadence,
  delivery,
  snapshotRequired,
  forceSnapshotAfterFlight,
  frameActivity,
  frameFresh,
  frameEpoch,
  frameEpochExhausted,
  frameRevision,
  frameSequence,
  nextSequence

vars ==
  <<revision, processActive, pendingWrites, inputLatched, cachedActivity,
    cacheFresh, activityEpoch, activityEpochExhausted, cadence, sampledCadence,
    delivery, snapshotRequired, forceSnapshotAfterFlight, frameActivity,
    frameFresh, frameEpoch, frameEpochExhausted, frameRevision, frameSequence,
    nextSequence>>

Init ==
  /\ revision = 0
  /\ processActive = FALSE
  /\ pendingWrites = 0
  /\ inputLatched = FALSE
  /\ cachedActivity = Idle
  /\ cacheFresh = TRUE
  /\ activityEpoch = 0
  /\ activityEpochExhausted = FALSE
  /\ cadence = 0
  /\ sampledCadence = 0
  /\ delivery = [attachment \in Attachments |-> Ready]
  /\ snapshotRequired = [attachment \in Attachments |-> TRUE]
  /\ forceSnapshotAfterFlight = [attachment \in Attachments |-> FALSE]
  /\ frameActivity = [attachment \in Attachments |-> Unknown]
  /\ frameFresh = [attachment \in Attachments |-> FALSE]
  /\ frameEpoch = [attachment \in Attachments |-> 0]
  /\ frameEpochExhausted = [attachment \in Attachments |-> FALSE]
  /\ frameRevision = [attachment \in Attachments |-> 0]
  /\ frameSequence = [attachment \in Attachments |-> 0]
  /\ nextSequence = [attachment \in Attachments |-> 1]

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
  /\ inputLatched' = TRUE
  /\ pendingWrites' = IF hasBytes THEN pendingWrites + 1 ELSE pendingWrites
  /\ cachedActivity' = Unknown
  /\ cacheFresh' = FALSE
  /\ activityEpoch' =
       IF activityEpoch < MaxActivityEpoch
         THEN activityEpoch + 1
         ELSE activityEpoch
  /\ activityEpochExhausted' =
       (activityEpochExhausted \/ activityEpoch = MaxActivityEpoch)
  /\ InvalidateReady(delivery', snapshotRequired', forceSnapshotAfterFlight')
  /\ UNCHANGED <<revision, processActive, cadence, sampledCadence,
                  frameActivity, frameFresh, frameEpoch, frameRevision,
                  frameEpochExhausted, frameSequence, nextSequence>>

\* Physical delivery remains modeled only to prove that it cannot clear the
\* lifecycle latch or make a later inactive observation authoritative.
WriterCompletesInput ==
  /\ pendingWrites > 0
  /\ pendingWrites' = pendingWrites - 1
  /\ UNCHANGED <<revision, processActive, inputLatched, cachedActivity,
                  cacheFresh, activityEpoch, activityEpochExhausted, cadence,
                  sampledCadence, delivery, snapshotRequired,
                  forceSnapshotAfterFlight, frameActivity, frameFresh,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence>>

\* Output may include buffered bytes, echo, OSC 133 markers, or any other PTY
\* mutation. Without an input epoch in the shell protocol it is not an ACK.
OutputCommit ==
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ UNCHANGED <<processActive, pendingWrites, inputLatched, cachedActivity,
                  cacheFresh, activityEpoch, activityEpochExhausted, cadence,
                  sampledCadence, delivery, snapshotRequired,
                  forceSnapshotAfterFlight, frameActivity, frameFresh,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence>>

SilentForegroundChange(active) ==
  /\ active \in BOOLEAN
  /\ active # processActive
  /\ processActive' = active
  /\ UNCHANGED <<revision, pendingWrites, inputLatched, cachedActivity,
                  cacheFresh, activityEpoch, activityEpochExhausted, cadence,
                  sampledCadence, delivery, snapshotRequired,
                  forceSnapshotAfterFlight, frameActivity, frameFresh,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence>>

AdvanceCadence ==
  /\ cadence < MaxCadence
  /\ cadence' = cadence + 1
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cachedActivity, cacheFresh, activityEpoch,
                  activityEpochExhausted, sampledCadence, delivery,
                  snapshotRequired, forceSnapshotAfterFlight, frameActivity,
                  frameFresh, frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence>>

SampleSuccess ==
  /\ sampledCadence < cadence
  /\ ~activityEpochExhausted
  /\ LET requestedActivity ==
           IF processActive
             THEN Active
             ELSE IF inputLatched THEN Unknown ELSE Idle
         epochOverflow ==
           requestedActivity # cachedActivity /\
             activityEpoch = MaxActivityEpoch
         nextActivity == IF epochOverflow THEN Unknown ELSE requestedActivity
         nextFresh == nextActivity # Unknown
         changed == (nextActivity # cachedActivity \/ nextFresh # cacheFresh)
     IN
       /\ cachedActivity' = nextActivity
       /\ cacheFresh' = nextFresh
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
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cadence, frameActivity, frameFresh, frameEpoch,
                  frameEpochExhausted, frameRevision, frameSequence,
                  nextSequence>>

SampleFailure ==
  /\ sampledCadence < cadence
  /\ ~activityEpochExhausted
  /\ LET epochOverflow ==
           cachedActivity # Unknown /\ activityEpoch = MaxActivityEpoch
         changed == (cachedActivity # Unknown \/ cacheFresh)
     IN
       /\ cachedActivity' = Unknown
       /\ cacheFresh' = FALSE
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
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cadence, frameActivity, frameFresh, frameEpoch,
                  frameEpochExhausted, frameRevision, frameSequence,
                  nextSequence>>

PrepareSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = Ready
  /\ snapshotRequired[attachment]
  /\ nextSequence[attachment] <= MaxSequence
  /\ delivery' = [delivery EXCEPT ![attachment] = Provisional]
  /\ frameActivity' = [frameActivity EXCEPT ![attachment] = cachedActivity]
  /\ frameFresh' = [frameFresh EXCEPT ![attachment] = cacheFresh]
  /\ frameEpoch' = [frameEpoch EXCEPT ![attachment] = activityEpoch]
  /\ frameEpochExhausted' =
       [frameEpochExhausted EXCEPT ![attachment] = activityEpochExhausted]
  /\ frameRevision' = [frameRevision EXCEPT ![attachment] = revision]
  /\ frameSequence' =
       [frameSequence EXCEPT ![attachment] = nextSequence[attachment]]
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cachedActivity, cacheFresh, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  snapshotRequired, forceSnapshotAfterFlight, nextSequence>>

BatchFull(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = Provisional
  /\ delivery' = [delivery EXCEPT ![attachment] = Ready]
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cachedActivity, cacheFresh, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  snapshotRequired, forceSnapshotAfterFlight, frameActivity,
                  frameFresh, frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence, nextSequence>>

EmitSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = Provisional
  /\ frameEpoch[attachment] = activityEpoch
  /\ frameEpochExhausted[attachment] = activityEpochExhausted
  /\ delivery' = [delivery EXCEPT ![attachment] = InFlight]
  /\ snapshotRequired' = [snapshotRequired EXCEPT ![attachment] = FALSE]
  /\ nextSequence' = [nextSequence EXCEPT ![attachment] = @ + 1]
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cachedActivity, cacheFresh, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  forceSnapshotAfterFlight, frameActivity, frameFresh,
                  frameEpoch, frameEpochExhausted, frameRevision,
                  frameSequence>>

FrameApplied(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ delivery' = [delivery EXCEPT ![attachment] = Ready]
  /\ snapshotRequired' = [snapshotRequired EXCEPT
       ![attachment] = forceSnapshotAfterFlight[attachment]]
  /\ forceSnapshotAfterFlight' =
       [forceSnapshotAfterFlight EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cachedActivity, cacheFresh, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  frameActivity, frameFresh, frameEpoch, frameEpochExhausted,
                  frameRevision, frameSequence, nextSequence>>

NeedSnapshot(attachment) ==
  /\ attachment \in Attachments
  /\ delivery[attachment] = InFlight
  /\ delivery' = [delivery EXCEPT ![attachment] = Ready]
  /\ snapshotRequired' = [snapshotRequired EXCEPT ![attachment] = TRUE]
  /\ forceSnapshotAfterFlight' =
       [forceSnapshotAfterFlight EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, processActive, pendingWrites, inputLatched,
                  cachedActivity, cacheFresh, activityEpoch,
                  activityEpochExhausted, cadence, sampledCadence,
                  frameActivity, frameFresh, frameEpoch, frameEpochExhausted,
                  frameRevision, frameSequence, nextSequence>>

Next ==
  \/ \E hasBytes \in BOOLEAN: InputCommit(hasBytes)
  \/ WriterCompletesInput
  \/ OutputCommit
  \/ AdvanceCadence
  \/ SampleSuccess
  \/ SampleFailure
  \/ \E active \in BOOLEAN: SilentForegroundChange(active)
  \/ \E attachment \in Attachments:
       \/ PrepareSnapshot(attachment)
       \/ BatchFull(attachment)
       \/ EmitSnapshot(attachment)
       \/ FrameApplied(attachment)
       \/ NeedSnapshot(attachment)

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ processActive \in BOOLEAN
  /\ pendingWrites \in 0..MaxPendingWrites
  /\ inputLatched \in BOOLEAN
  /\ cachedActivity \in {Active, Idle, Unknown}
  /\ cacheFresh \in BOOLEAN
  /\ activityEpoch \in 0..MaxActivityEpoch
  /\ activityEpochExhausted \in BOOLEAN
  /\ cadence \in 0..MaxCadence
  /\ sampledCadence \in 0..MaxCadence
  /\ delivery \in [Attachments -> {Ready, Provisional, InFlight}]
  /\ snapshotRequired \in [Attachments -> BOOLEAN]
  /\ forceSnapshotAfterFlight \in [Attachments -> BOOLEAN]
  /\ frameActivity \in [Attachments -> {Active, Idle, Unknown}]
  /\ frameFresh \in [Attachments -> BOOLEAN]
  /\ frameEpoch \in [Attachments -> 0..MaxActivityEpoch]
  /\ frameEpochExhausted \in [Attachments -> BOOLEAN]
  /\ frameRevision \in [Attachments -> 0..MaxRevision]
  /\ frameSequence \in [Attachments -> 0..MaxSequence]
  /\ nextSequence \in [Attachments -> 1..(MaxSequence + 1)]

StaleEncodesUnknown ==
  ~cacheFresh => cachedActivity = Unknown

FreshIdleRequiresPristineLifecycle ==
  cachedActivity = Idle => cacheFresh /\ ~inputLatched

LatchedCacheNeverIdle ==
  inputLatched => cachedActivity # Idle

CurrentTokenFrameNeverIdleAfterLatch ==
  inputLatched =>
    \A attachment \in Attachments:
      delivery[attachment] \in {Provisional, InFlight} /\
        frameEpoch[attachment] = activityEpoch /\
        frameEpochExhausted[attachment] = activityEpochExhausted =>
          frameActivity[attachment] # Idle

EpochExhaustionIsUnknown ==
  activityEpochExhausted => cachedActivity = Unknown /\ ~cacheFresh

EveryReadAttachmentTracksActivityInvalidation ==
  \A attachment \in Attachments:
    delivery[attachment] = Ready /\ ~snapshotRequired[attachment] =>
      (frameEpoch[attachment] = activityEpoch /\
         frameEpochExhausted[attachment] = activityEpochExhausted) \/
        (~frameFresh[attachment] /\ frameActivity[attachment] = Unknown)

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

=============================================================================
