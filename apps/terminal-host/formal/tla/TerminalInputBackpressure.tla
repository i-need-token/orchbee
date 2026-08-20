------------------------ MODULE TerminalInputBackpressure ------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-003, TH-004, TH-007, TH-012, TH-019, and
\* TH-020 through
\* AcceptClientInput, AcceptHeldInput, ReleaseHeldInput, capability revision
\* update/replay/rejection, visibility transitions, attachment focus claim
\* aggregation, DetachOwnedInput, DetachIdle,
\* DetachAfterClientClose, AdmitTerminalReply, CloseClientInput, StopWriter,
\* WriteTerminalReply, and WriteClientInput. Client input and
\* terminal replies share one writer but own independent bounded admission
\* budgets. An accepted held input precharges one future repair byte; detaching
\* that attachment atomically converts only its credit into an ordered repair.
\* A capability downgrade uses the same repair transition before authority is
\* revoked; stale or unavailable revisions cannot enter the queue, an upgrade
\* creates fresh authority without replaying queued input, and ordinary client
\* backpressure cannot strand or cross-release ownership.
\* Replies receive priority within a bounded reply burst
\* (ReplyBurstRemaining): once the burst is exhausted the writer yields to a
\* waiting accepted client entry, so sustained reply work cannot starve client
\* input indefinitely while replies still make progress whenever no client
\* entry is waiting. This source-scoped queue proof is separate from
\* TerminalFrameBackpressure because neither queue participates in a frame
\* commit transaction.

CONSTANTS Attachments, MaxCapabilityRevision, MaxClientInput, MaxTerminalReply,
  MaxReplyBurst

VARIABLES clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer,
  ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
  queuedAccepted, repaired, visible, focused, focusTerminalActive

vars ==
  <<clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
    queuedAccepted, repaired, visible, focused, focusTerminalActive>>

FocusRepairCredit == IF focused = {} THEN 0 ELSE 1

Init ==
  /\ clientInputOpen = TRUE
  /\ writerOpen = TRUE
  /\ clientInputBuffer = 0
  /\ terminalReplyBuffer = 0
  /\ ReplyBurstRemaining = MaxReplyBurst
  /\ attached = Attachments
  /\ inputCapable = Attachments
  /\ capabilityRevision = [attachment \in Attachments |-> 1]
  /\ owned = {}
  /\ queuedAccepted = {}
  /\ repaired = {}
  /\ visible = {}
  /\ focused = {}
  /\ focusTerminalActive = FALSE

AcceptClientInput(attachment, revision, bytes) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in inputCapable
  /\ revision = capabilityRevision[attachment]
  /\ bytes \in 1..MaxClientInput
  /\ clientInputBuffer + Cardinality(owned) + FocusRepairCredit + bytes <=
       MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer + bytes
  /\ queuedAccepted' = queuedAccepted \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
    repaired, visible, focused, focusTerminalActive>>

AcceptHeldInput(attachment, revision) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in inputCapable
  /\ revision = capabilityRevision[attachment]
  /\ attachment \notin owned
  \* One queued input byte plus one future detach-repair credit.
  /\ clientInputBuffer + Cardinality(owned) + FocusRepairCredit + 2 <=
       MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer + 1
  /\ owned' = owned \union {attachment}
  /\ queuedAccepted' = queuedAccepted \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, repaired,
    visible, focused, focusTerminalActive>>

ReleaseHeldInput(attachment, revision) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in owned
  /\ attachment \in inputCapable
  /\ revision = capabilityRevision[attachment]
  /\ clientInputBuffer' = clientInputBuffer + 1
  /\ owned' = owned \ {attachment}
  /\ queuedAccepted' = queuedAccepted \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, repaired,
    visible, focused, focusTerminalActive>>

GainVisibility(attachment) ==
  /\ attachment \in attached
  /\ attachment \notin visible
  /\ visible' = visible \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    terminalReplyBuffer, ReplyBurstRemaining, attached, inputCapable,
    capabilityRevision, owned, queuedAccepted, repaired, focused,
    focusTerminalActive>>

LoseVisibility(attachment) ==
  /\ attachment \in visible
  /\ LET repairsHeld ==
           attachment \in owned /\ writerOpen /\ clientInputOpen
         repairsLastFocus ==
           attachment \in focused /\ focused = {attachment} /\
             writerOpen /\ clientInputOpen
     IN /\ clientInputBuffer' = clientInputBuffer +
              (IF repairsHeld THEN 1 ELSE 0) +
              (IF repairsLastFocus THEN 1 ELSE 0)
  /\ visible' = visible \ {attachment}
  /\ owned' = owned \ {attachment}
  /\ focused' = focused \ {attachment}
  /\ focusTerminalActive' = ((focused \ {attachment}) # {})
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision,
    queuedAccepted, repaired>>

AcceptFocusClaim(attachment, revision) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in inputCapable
  /\ attachment \in visible
  /\ attachment \notin focused
  /\ revision = capabilityRevision[attachment]
  /\ clientInputBuffer + Cardinality(owned) + FocusRepairCredit +
       (IF focused = {} THEN 2 ELSE 0) <= MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer +
       (IF focused = {} THEN 1 ELSE 0)
  /\ focused' = focused \union {attachment}
  /\ focusTerminalActive' = TRUE
  /\ queuedAccepted' =
       IF focused = {} THEN queuedAccepted \union {attachment} ELSE queuedAccepted
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
    repaired, visible>>

AcceptFocusRelease(attachment, revision) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in inputCapable
  /\ attachment \in focused
  /\ revision = capabilityRevision[attachment]
  /\ clientInputBuffer' = clientInputBuffer +
       (IF focused = {attachment} THEN 1 ELSE 0)
  /\ focused' = focused \ {attachment}
  /\ focusTerminalActive' = ((focused \ {attachment}) # {})
  /\ queuedAccepted' =
       IF focused = {attachment}
         THEN queuedAccepted \union {attachment}
         ELSE queuedAccepted
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
    repaired, visible>>

ReplayFocusClaim(attachment, revision, nextFocused) ==
  /\ attachment \in inputCapable
  /\ revision = capabilityRevision[attachment]
  /\ nextFocused = (attachment \in focused)
  /\ UNCHANGED vars

RejectStaleFocus(attachment, revision, nextFocused) ==
  /\ attachment \in attached
  /\ \/ revision # capabilityRevision[attachment]
     \/ attachment \notin inputCapable
     \/ nextFocused /\ attachment \notin visible
  /\ UNCHANGED vars

DowngradeInputCapability(attachment, revision) ==
  /\ attachment \in inputCapable
  /\ capabilityRevision[attachment] < MaxCapabilityRevision
  /\ revision = capabilityRevision[attachment] + 1
  /\ capabilityRevision' = [capabilityRevision EXCEPT ![attachment] = revision]
  /\ inputCapable' = inputCapable \ {attachment}
  /\ LET repairsHeld ==
           attachment \in owned /\ writerOpen /\ clientInputOpen
         repairsLastFocus ==
           attachment \in focused /\ focused = {attachment} /\
             writerOpen /\ clientInputOpen
     IN /\ clientInputBuffer' = clientInputBuffer +
              (IF repairsHeld THEN 1 ELSE 0) +
              (IF repairsLastFocus THEN 1 ELSE 0)
        /\ repaired' =
             IF repairsHeld \/ repairsLastFocus
               THEN repaired \union {attachment}
               ELSE repaired \ {attachment}
  /\ owned' = owned \ {attachment}
  /\ focused' = focused \ {attachment}
  /\ focusTerminalActive' = ((focused \ {attachment}) # {})
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, queuedAccepted, visible>>

UpgradeInputCapability(attachment, revision) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in attached
  /\ attachment \notin inputCapable
  /\ capabilityRevision[attachment] < MaxCapabilityRevision
  /\ revision = capabilityRevision[attachment] + 1
  /\ capabilityRevision' = [capabilityRevision EXCEPT ![attachment] = revision]
  /\ inputCapable' = inputCapable \union {attachment}
  /\ repaired' = repaired \ {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    terminalReplyBuffer, ReplyBurstRemaining, attached, owned, queuedAccepted,
    visible, focused, focusTerminalActive>>

ReplayInputCapabilityUpdate(attachment, revision, input) ==
  /\ attachment \in attached
  /\ revision = capabilityRevision[attachment]
  /\ input = (attachment \in inputCapable)
  /\ UNCHANGED vars

RejectConflictingCapabilityUpdate(attachment, revision, input) ==
  /\ attachment \in attached
  /\ \/ /\ revision = capabilityRevision[attachment]
          /\ input # (attachment \in inputCapable)
     \/ /\ capabilityRevision[attachment] < MaxCapabilityRevision
          /\ revision = capabilityRevision[attachment] + 1
          /\ input = (attachment \in inputCapable)
  /\ UNCHANGED vars

RejectOutOfOrderCapabilityUpdate(attachment, revision) ==
  /\ attachment \in attached
  /\ revision # capabilityRevision[attachment]
  /\ (capabilityRevision[attachment] = MaxCapabilityRevision \/
      revision # capabilityRevision[attachment] + 1)
  /\ UNCHANGED vars

RejectStaleInput(attachment, revision) ==
  /\ attachment \in attached
  /\ revision # capabilityRevision[attachment]
  /\ UNCHANGED vars

RejectUnavailableInput(attachment, revision) ==
  /\ attachment \in attached
  /\ attachment \notin inputCapable
  /\ revision = capabilityRevision[attachment]
  /\ UNCHANGED vars

DetachOwnedInput(attachment) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in owned
  \* Convert this attachment's precharged byte into its ordered repair.
  /\ attached' = attached \ {attachment}
  /\ inputCapable' = inputCapable \ {attachment}
  /\ owned' = owned \ {attachment}
  /\ visible' = visible \ {attachment}
  /\ clientInputBuffer' = clientInputBuffer + 1 +
       (IF attachment \in focused /\ focused = {attachment} THEN 1 ELSE 0)
  /\ focused' = focused \ {attachment}
  /\ focusTerminalActive' = ((focused \ {attachment}) # {})
  /\ repaired' = repaired \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, capabilityRevision, queuedAccepted>>

DetachIdle(attachment) ==
  /\ attachment \in attached
  /\ attachment \notin owned
  /\ attached' = attached \ {attachment}
  /\ inputCapable' = inputCapable \ {attachment}
  /\ visible' = visible \ {attachment}
  /\ clientInputBuffer' = clientInputBuffer +
       (IF attachment \in focused /\ focused = {attachment} /\
            writerOpen /\ clientInputOpen
         THEN 1
         ELSE 0)
  /\ focused' = focused \ {attachment}
  /\ focusTerminalActive' = ((focused \ {attachment}) # {})
  /\ repaired' =
       IF attachment \in focused /\ focused = {attachment} /\
            writerOpen /\ clientInputOpen
         THEN repaired \union {attachment}
         ELSE repaired
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, capabilityRevision, owned, queuedAccepted>>

DetachAfterClientClose(attachment) ==
  /\ ~clientInputOpen
  /\ attachment \in attached
  /\ attached' = attached \ {attachment}
  /\ inputCapable' = inputCapable \ {attachment}
  /\ owned' = owned \ {attachment}
  /\ visible' = visible \ {attachment}
  /\ focused' = focused \ {attachment}
  /\ focusTerminalActive' = ((focused \ {attachment}) # {})
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    terminalReplyBuffer, ReplyBurstRemaining, capabilityRevision,
    queuedAccepted, repaired>>

AdmitTerminalReply(bytes) ==
  /\ writerOpen
  /\ bytes \in 1..MaxTerminalReply
  /\ terminalReplyBuffer + bytes <= MaxTerminalReply
  /\ terminalReplyBuffer' = terminalReplyBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
    queuedAccepted, repaired, visible, focused, focusTerminalActive>>

CloseClientInput ==
  /\ clientInputOpen
  /\ clientInputOpen' = FALSE
  /\ UNCHANGED <<writerOpen, clientInputBuffer, terminalReplyBuffer,
    ReplyBurstRemaining, attached, inputCapable, capabilityRevision, owned,
    queuedAccepted, repaired, visible, focused, focusTerminalActive>>

StopWriter ==
  /\ writerOpen
  /\ writerOpen' = FALSE
  /\ clientInputOpen' = FALSE
  /\ clientInputBuffer' = 0
  /\ terminalReplyBuffer' = 0
  /\ ReplyBurstRemaining' = MaxReplyBurst
  /\ owned' = {}
  /\ queuedAccepted' = {}
  /\ focused' = {}
  /\ focusTerminalActive' = FALSE
  /\ UNCHANGED <<attached, inputCapable, capabilityRevision, repaired, visible>>

WriteTerminalReply ==
  /\ writerOpen
  /\ terminalReplyBuffer > 0
  /\ terminalReplyBuffer' = 0
  /\ ReplyBurstRemaining' =
      IF ReplyBurstRemaining > 0 THEN ReplyBurstRemaining - 1 ELSE 0
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    attached, inputCapable, capabilityRevision, owned, queuedAccepted, repaired,
    visible, focused, focusTerminalActive>>

WriteClientInput ==
  /\ writerOpen
  /\ clientInputBuffer > 0
  /\ (terminalReplyBuffer = 0 \/ ReplyBurstRemaining = 0)
  /\ clientInputBuffer' = 0
  /\ ReplyBurstRemaining' = MaxReplyBurst
  /\ queuedAccepted' = {}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    attached, inputCapable, capabilityRevision, owned, repaired, visible,
    focused, focusTerminalActive>>

Next ==
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision,
       bytes \in 1..MaxClientInput: AcceptClientInput(attachment, revision, bytes)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       AcceptHeldInput(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       ReleaseHeldInput(attachment, revision)
  \/ \E attachment \in Attachments: GainVisibility(attachment)
  \/ \E attachment \in Attachments: LoseVisibility(attachment)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       AcceptFocusClaim(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       AcceptFocusRelease(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision,
       nextFocused \in BOOLEAN: ReplayFocusClaim(attachment, revision, nextFocused)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision,
       nextFocused \in BOOLEAN: RejectStaleFocus(attachment, revision, nextFocused)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       DowngradeInputCapability(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       UpgradeInputCapability(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision,
       input \in BOOLEAN: ReplayInputCapabilityUpdate(attachment, revision, input)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision,
       input \in BOOLEAN: RejectConflictingCapabilityUpdate(attachment, revision, input)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       RejectOutOfOrderCapabilityUpdate(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       RejectStaleInput(attachment, revision)
  \/ \E attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
       RejectUnavailableInput(attachment, revision)
  \/ \E attachment \in Attachments: DetachOwnedInput(attachment)
  \/ \E attachment \in Attachments: DetachIdle(attachment)
  \/ \E attachment \in Attachments: DetachAfterClientClose(attachment)
  \/ \E bytes \in 1..MaxTerminalReply: AdmitTerminalReply(bytes)
  \/ CloseClientInput
  \/ StopWriter
  \/ WriteTerminalReply
  \/ WriteClientInput

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(WriteTerminalReply)
  /\ WF_vars(WriteClientInput)

TypeOK ==
  /\ clientInputOpen \in BOOLEAN
  /\ writerOpen \in BOOLEAN
  /\ clientInputBuffer \in 0..MaxClientInput
  /\ terminalReplyBuffer \in 0..MaxTerminalReply
  /\ ReplyBurstRemaining \in 0..MaxReplyBurst
  /\ attached \subseteq Attachments
  /\ inputCapable \subseteq attached
  /\ capabilityRevision \in [Attachments -> 1..MaxCapabilityRevision]
  /\ owned \subseteq Attachments
  /\ queuedAccepted \subseteq Attachments
  /\ repaired \subseteq Attachments
  /\ visible \subseteq attached
  /\ focused \subseteq inputCapable
  /\ focused \subseteq visible
  /\ focusTerminalActive \in BOOLEAN

HeldInputAndRepairCreditAreBounded ==
  /\ owned \subseteq inputCapable
  /\ clientInputBuffer + Cardinality(owned) + FocusRepairCredit <=
       MaxClientInput

InvisibleAttachmentOwnsNoFocus ==
  focused \subseteq visible

FocusAggregateMatchesTerminalState ==
  focusTerminalActive = (focused # {})

RepairDoesNotCrossAttachment ==
  repaired \intersect inputCapable = {}

DowngradedAttachmentOwnsNoHeldInput ==
  owned \subseteq inputCapable

ClosedWriterOwnsNoQueue ==
  ~writerOpen =>
    /\ ~clientInputOpen
    /\ clientInputBuffer = 0
    /\ terminalReplyBuffer = 0
    /\ owned = {}
    /\ queuedAccepted = {}
    /\ focused = {}
    /\ ~focusTerminalActive

TerminalReplyHasWritePriority ==
  (terminalReplyBuffer > 0 /\ ReplyBurstRemaining > 0) =>
    ~ENABLED WriteClientInput

ClientBackpressureCannotConsumeReplyBudget ==
  writerOpen /\ clientInputBuffer = MaxClientInput /\
    terminalReplyBuffer < MaxTerminalReply =>
      ENABLED AdmitTerminalReply(1)

ClosedClientRejectsNewInput ==
  ~clientInputOpen =>
    /\ \A attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
         ~ENABLED AcceptClientInput(attachment, revision, 1)
    /\ \A attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
         ~ENABLED AcceptHeldInput(attachment, revision)
    /\ \A attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
         ~ENABLED AcceptFocusClaim(attachment, revision)
    /\ \A attachment \in Attachments, revision \in 1..MaxCapabilityRevision:
         ~ENABLED AcceptFocusRelease(attachment, revision)

ClientInputWaitHasOnlyReplyPreemption ==
  (writerOpen /\ clientInputBuffer > 0 /\ terminalReplyBuffer = 0) ~>
    (~writerOpen \/ clientInputBuffer = 0 \/ terminalReplyBuffer > 0)

\* Bounded reply burst: a waiting accepted client entry makes progress within
\* MaxReplyBurst reply admissions instead of starving behind an endless reply
\* stream. Once ReplyBurstRemaining reaches 0 the writer yields to the waiting
\* client, so WriteClientInput is enabled and (under its weak fairness) fires.
ClientInputMakesBoundedProgress ==
  (writerOpen /\ clientInputBuffer > 0) ~>
    (~writerOpen \/ clientInputBuffer = 0)

AcceptedTerminalReplyEventuallyWrites ==
  terminalReplyBuffer > 0 ~> terminalReplyBuffer = 0

=============================================================================
