------------------------ MODULE TerminalInputBackpressure ------------------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-003, TH-004, TH-007, and TH-012 through
\* AcceptClientInput, AcceptHeldInput, ReleaseHeldInput, DetachOwnedInput,
\* DetachIdle, DetachAfterClientClose, AdmitTerminalReply, CloseClientInput,
\* StopWriter, WriteTerminalReply, and WriteClientInput. Client input and
\* terminal replies share one writer but own independent bounded admission
\* budgets. An accepted held input precharges one future repair byte; detaching
\* that attachment atomically converts only its credit into an ordered repair,
\* so ordinary client backpressure cannot strand or cross-release ownership.
\* Replies receive priority within a bounded reply burst
\* (ReplyBurstRemaining): once the burst is exhausted the writer yields to a
\* waiting accepted client entry, so sustained reply work cannot starve client
\* input indefinitely while replies still make progress whenever no client
\* entry is waiting. This source-scoped queue proof is separate from
\* TerminalFrameBackpressure because neither queue participates in a frame
\* commit transaction.

CONSTANTS Attachments, MaxClientInput, MaxTerminalReply, MaxReplyBurst

VARIABLES clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer,
  ReplyBurstRemaining, attached, owned, repaired

vars ==
  <<clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer,
    ReplyBurstRemaining, attached, owned, repaired>>

Init ==
  /\ clientInputOpen = TRUE
  /\ writerOpen = TRUE
  /\ clientInputBuffer = 0
  /\ terminalReplyBuffer = 0
  /\ ReplyBurstRemaining = MaxReplyBurst
  /\ attached = Attachments
  /\ owned = {}
  /\ repaired = {}

AcceptClientInput(bytes) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ bytes \in 1..MaxClientInput
  /\ clientInputBuffer + Cardinality(owned) + bytes <= MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, owned, repaired>>

AcceptHeldInput(attachment) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in attached
  /\ attachment \notin owned
  \* One queued input byte plus one future detach-repair credit.
  /\ clientInputBuffer + Cardinality(owned) + 2 <= MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer + 1
  /\ owned' = owned \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, repaired>>

ReleaseHeldInput(attachment) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in owned
  /\ clientInputBuffer' = clientInputBuffer + 1
  /\ owned' = owned \ {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining, attached, repaired>>

DetachOwnedInput(attachment) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ attachment \in owned
  \* Convert this attachment's precharged byte into its ordered repair.
  /\ clientInputBuffer' = clientInputBuffer + 1
  /\ attached' = attached \ {attachment}
  /\ owned' = owned \ {attachment}
  /\ repaired' = repaired \union {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    ReplyBurstRemaining>>

DetachIdle(attachment) ==
  /\ attachment \in attached
  /\ attachment \notin owned
  /\ attached' = attached \ {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    terminalReplyBuffer, ReplyBurstRemaining, owned, repaired>>

DetachAfterClientClose(attachment) ==
  /\ ~clientInputOpen
  /\ attachment \in attached
  /\ attached' = attached \ {attachment}
  /\ owned' = owned \ {attachment}
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    terminalReplyBuffer, ReplyBurstRemaining, repaired>>

AdmitTerminalReply(bytes) ==
  /\ writerOpen
  /\ bytes \in 1..MaxTerminalReply
  /\ terminalReplyBuffer + bytes <= MaxTerminalReply
  /\ terminalReplyBuffer' = terminalReplyBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    ReplyBurstRemaining, attached, owned, repaired>>

CloseClientInput ==
  /\ clientInputOpen
  /\ clientInputOpen' = FALSE
  /\ UNCHANGED <<writerOpen, clientInputBuffer, terminalReplyBuffer,
    ReplyBurstRemaining, attached, owned, repaired>>

StopWriter ==
  /\ writerOpen
  /\ writerOpen' = FALSE
  /\ clientInputOpen' = FALSE
  /\ clientInputBuffer' = 0
  /\ terminalReplyBuffer' = 0
  /\ ReplyBurstRemaining' = MaxReplyBurst
  /\ owned' = {}
  /\ UNCHANGED <<attached, repaired>>

WriteTerminalReply ==
  /\ writerOpen
  /\ terminalReplyBuffer > 0
  /\ terminalReplyBuffer' = 0
  /\ ReplyBurstRemaining' =
      IF ReplyBurstRemaining > 0 THEN ReplyBurstRemaining - 1 ELSE 0
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer,
    attached, owned, repaired>>

WriteClientInput ==
  /\ writerOpen
  /\ clientInputBuffer > 0
  /\ (terminalReplyBuffer = 0 \/ ReplyBurstRemaining = 0)
  /\ clientInputBuffer' = 0
  /\ ReplyBurstRemaining' = MaxReplyBurst
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer,
    attached, owned, repaired>>

Next ==
  \/ \E bytes \in 1..MaxClientInput: AcceptClientInput(bytes)
  \/ \E attachment \in Attachments: AcceptHeldInput(attachment)
  \/ \E attachment \in Attachments: ReleaseHeldInput(attachment)
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
  /\ owned \subseteq Attachments
  /\ repaired \subseteq Attachments

HeldInputAndRepairCreditAreBounded ==
  /\ owned \subseteq attached
  /\ clientInputBuffer + Cardinality(owned) <= MaxClientInput

RepairDoesNotCrossAttachment ==
  repaired \intersect attached = {}

ClosedWriterOwnsNoQueue ==
  ~writerOpen =>
    /\ ~clientInputOpen
    /\ clientInputBuffer = 0
    /\ terminalReplyBuffer = 0
    /\ owned = {}

TerminalReplyHasWritePriority ==
  (terminalReplyBuffer > 0 /\ ReplyBurstRemaining > 0) =>
    ~ENABLED WriteClientInput

ClientBackpressureCannotConsumeReplyBudget ==
  writerOpen /\ clientInputBuffer = MaxClientInput /\
    terminalReplyBuffer < MaxTerminalReply =>
      ENABLED AdmitTerminalReply(1)

ClosedClientRejectsNewInput ==
  ~clientInputOpen =>
    /\ ~ENABLED AcceptClientInput(1)
    /\ \A attachment \in Attachments: ~ENABLED AcceptHeldInput(attachment)

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
