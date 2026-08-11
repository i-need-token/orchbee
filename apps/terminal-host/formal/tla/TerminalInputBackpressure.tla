------------------------ MODULE TerminalInputBackpressure ------------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-007 and TH-012 through AcceptClientInput,
\* AdmitTerminalReply, CloseClientInput, StopWriter, WriteTerminalReply, and
\* WriteClientInput. Client input and terminal replies share one writer but own
\* independent bounded admission budgets. Replies receive priority within a
\* bounded reply burst (ReplyBurstRemaining): once the burst is exhausted the
\* writer yields to a waiting accepted client entry, so sustained reply work
\* cannot starve client input indefinitely while replies still make progress
\* whenever no client entry is waiting. This source-scoped queue proof is
\* separate from TerminalFrameBackpressure because neither queue participates
\* in a frame commit transaction.

CONSTANTS MaxClientInput, MaxTerminalReply, MaxReplyBurst

VARIABLES clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer,
  ReplyBurstRemaining

vars ==
  <<clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer,
    ReplyBurstRemaining>>

Init ==
  /\ clientInputOpen = TRUE
  /\ writerOpen = TRUE
  /\ clientInputBuffer = 0
  /\ terminalReplyBuffer = 0
  /\ ReplyBurstRemaining = MaxReplyBurst

AcceptClientInput(bytes) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ bytes \in 1..MaxClientInput
  /\ clientInputBuffer + bytes <= MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer, ReplyBurstRemaining>>

AdmitTerminalReply(bytes) ==
  /\ writerOpen
  /\ bytes \in 1..MaxTerminalReply
  /\ terminalReplyBuffer + bytes <= MaxTerminalReply
  /\ terminalReplyBuffer' = terminalReplyBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer, ReplyBurstRemaining>>

CloseClientInput ==
  /\ clientInputOpen
  /\ clientInputOpen' = FALSE
  /\ UNCHANGED <<writerOpen, clientInputBuffer, terminalReplyBuffer, ReplyBurstRemaining>>

StopWriter ==
  /\ writerOpen
  /\ writerOpen' = FALSE
  /\ clientInputOpen' = FALSE
  /\ clientInputBuffer' = 0
  /\ terminalReplyBuffer' = 0
  /\ ReplyBurstRemaining' = MaxReplyBurst

WriteTerminalReply ==
  /\ writerOpen
  /\ terminalReplyBuffer > 0
  /\ terminalReplyBuffer' = 0
  /\ ReplyBurstRemaining' =
      IF ReplyBurstRemaining > 0 THEN ReplyBurstRemaining - 1 ELSE 0
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer>>

WriteClientInput ==
  /\ writerOpen
  /\ clientInputBuffer > 0
  /\ (terminalReplyBuffer = 0 \/ ReplyBurstRemaining = 0)
  /\ clientInputBuffer' = 0
  /\ ReplyBurstRemaining' = MaxReplyBurst
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer>>

Next ==
  \/ \E bytes \in 1..MaxClientInput: AcceptClientInput(bytes)
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

ClosedWriterOwnsNoQueue ==
  ~writerOpen =>
    /\ ~clientInputOpen
    /\ clientInputBuffer = 0
    /\ terminalReplyBuffer = 0

TerminalReplyHasWritePriority ==
  (terminalReplyBuffer > 0 /\ ReplyBurstRemaining > 0) =>
    ~ENABLED WriteClientInput

ClientBackpressureCannotConsumeReplyBudget ==
  writerOpen /\ clientInputBuffer = MaxClientInput /\
    terminalReplyBuffer < MaxTerminalReply =>
      ENABLED AdmitTerminalReply(1)

ClosedClientRejectsNewInput ==
  ~clientInputOpen => ~ENABLED AcceptClientInput(1)

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
