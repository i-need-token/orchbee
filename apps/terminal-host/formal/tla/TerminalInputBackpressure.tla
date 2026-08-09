------------------------ MODULE TerminalInputBackpressure ------------------------
EXTENDS Naturals, TLC

\* Refines runtime-model claims TH-007 and TH-012 through AcceptClientInput,
\* AdmitTerminalReply, CloseClientInput, StopWriter, WriteTerminalReply, and
\* WriteClientInput. Client input and terminal replies share one writer but own
\* independent bounded admission budgets. Replies receive priority at each
\* slice; therefore a waiting client write either progresses or is preempted by
\* actual higher-priority reply work. This source-scoped queue proof is separate
\* from TerminalFrameBackpressure because neither queue participates in a frame
\* commit transaction.

CONSTANTS MaxClientInput, MaxTerminalReply

VARIABLES clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer

vars ==
  <<clientInputOpen, writerOpen, clientInputBuffer, terminalReplyBuffer>>

Init ==
  /\ clientInputOpen = TRUE
  /\ writerOpen = TRUE
  /\ clientInputBuffer = 0
  /\ terminalReplyBuffer = 0

AcceptClientInput(bytes) ==
  /\ writerOpen
  /\ clientInputOpen
  /\ bytes \in 1..MaxClientInput
  /\ clientInputBuffer + bytes <= MaxClientInput
  /\ clientInputBuffer' = clientInputBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, terminalReplyBuffer>>

AdmitTerminalReply(bytes) ==
  /\ writerOpen
  /\ bytes \in 1..MaxTerminalReply
  /\ terminalReplyBuffer + bytes <= MaxTerminalReply
  /\ terminalReplyBuffer' = terminalReplyBuffer + bytes
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer>>

CloseClientInput ==
  /\ clientInputOpen
  /\ clientInputOpen' = FALSE
  /\ UNCHANGED <<writerOpen, clientInputBuffer, terminalReplyBuffer>>

StopWriter ==
  /\ writerOpen
  /\ writerOpen' = FALSE
  /\ clientInputOpen' = FALSE
  /\ clientInputBuffer' = 0
  /\ terminalReplyBuffer' = 0

WriteTerminalReply ==
  /\ writerOpen
  /\ terminalReplyBuffer > 0
  /\ terminalReplyBuffer' = 0
  /\ UNCHANGED <<clientInputOpen, writerOpen, clientInputBuffer>>

WriteClientInput ==
  /\ writerOpen
  /\ clientInputBuffer > 0
  /\ terminalReplyBuffer = 0
  /\ clientInputBuffer' = 0
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

ClosedWriterOwnsNoQueue ==
  ~writerOpen =>
    /\ ~clientInputOpen
    /\ clientInputBuffer = 0
    /\ terminalReplyBuffer = 0

TerminalReplyHasWritePriority ==
  terminalReplyBuffer > 0 => ~ENABLED WriteClientInput

ClientBackpressureCannotConsumeReplyBudget ==
  writerOpen /\ clientInputBuffer = MaxClientInput /\
    terminalReplyBuffer < MaxTerminalReply =>
      ENABLED AdmitTerminalReply(1)

ClosedClientRejectsNewInput ==
  ~clientInputOpen => ~ENABLED AcceptClientInput(1)

ClientInputWaitHasOnlyReplyPreemption ==
  (writerOpen /\ clientInputBuffer > 0 /\ terminalReplyBuffer = 0) ~>
    (~writerOpen \/ clientInputBuffer = 0 \/ terminalReplyBuffer > 0)

AcceptedTerminalReplyEventuallyWrites ==
  terminalReplyBuffer > 0 ~> terminalReplyBuffer = 0

=============================================================================
