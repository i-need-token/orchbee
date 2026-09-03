---------------------- MODULE LocalClientSubmission ----------------------
EXTENDS Naturals, TLC

\* One-invocation refinement of LocalClient's post-admission OBCS workflow.
\* WriteBegins is the first syscall that may hand request bytes to the kernel.
\* Timer actions model monotonic-deadline ordering, not elapsed wall time.
\* The already implemented OBLC connect and handshake timeout is a separate
\* boundary; endpoint selection, activation, retry, and presentation are absent.

Phases == {
  "ready", "encoding", "write-ready", "write-begun", "awaiting-result",
  "completed", "definite-failure", "retired"
}

ResultClasses == {
  "none", "matching-known", "matching-server-unknown", "wrong-kind",
  "wrong-id", "malformed"
}

Outcomes == {
  "none", "definite-failure", "result-known", "result-server-unknown",
  "transport-unknown"
}

Actions == {
  "init", "begin-submission", "encoding-fails", "encoding-succeeds",
  "pre-write-deadline", "pre-write-io-fails", "write-begins",
  "write-completes", "write-fails", "write-deadline-expires",
  "result-deadline-expires", "matching-known-result-arrives",
  "matching-unknown-result-arrives", "wrong-kind-result-arrives",
  "wrong-id-result-arrives", "malformed-result-arrives", "read-fails",
  "accept-matching-result", "retire-timed-out-result", "retire-wrong-kind",
  "retire-wrong-id", "retire-malformed-result",
  "reject-send-after-retirement"
}

VARIABLES phase, resultClass, deadlineExpired, sendCount, outcome,
  rejectedAfterRetirement, previousPhase, previousSendCount, lastAction

vars == <<phase, resultClass, deadlineExpired, sendCount, outcome,
  rejectedAfterRetirement, previousPhase, previousSendCount, lastAction>>

Init ==
  /\ phase = "ready"
  /\ resultClass = "none"
  /\ deadlineExpired = FALSE
  /\ sendCount = 0
  /\ outcome = "none"
  /\ rejectedAfterRetirement = FALSE
  /\ previousPhase = phase
  /\ previousSendCount = sendCount
  /\ lastAction = "init"

Record(action) ==
  /\ previousPhase' = phase
  /\ previousSendCount' = sendCount
  /\ lastAction' = action

BeginSubmission ==
  /\ phase = "ready"
  /\ phase' = "encoding"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("begin-submission")

EncodingFails ==
  /\ phase = "encoding"
  /\ phase' = "definite-failure"
  /\ outcome' = "definite-failure"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("encoding-fails")

EncodingSucceeds ==
  /\ phase = "encoding"
  /\ phase' = "write-ready"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("encoding-succeeds")

PreWriteDeadline ==
  /\ phase = "write-ready"
  /\ phase' = "definite-failure"
  /\ deadlineExpired' = TRUE
  /\ outcome' = "definite-failure"
  /\ UNCHANGED <<resultClass, sendCount, rejectedAfterRetirement>>
  /\ Record("pre-write-deadline")

PreWriteIoFails ==
  /\ phase = "write-ready"
  /\ phase' = "definite-failure"
  /\ outcome' = "definite-failure"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("pre-write-io-fails")

WriteBegins ==
  /\ phase = "write-ready"
  /\ phase' = "write-begun"
  /\ sendCount' = sendCount + 1
  /\ UNCHANGED <<resultClass, deadlineExpired, outcome,
       rejectedAfterRetirement>>
  /\ Record("write-begins")

WriteCompletes ==
  /\ phase = "write-begun"
  /\ phase' = "awaiting-result"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("write-completes")

WriteFails ==
  /\ phase = "write-begun"
  /\ phase' = "retired"
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("write-fails")

WriteDeadlineExpires ==
  /\ phase = "write-begun"
  /\ phase' = "retired"
  /\ deadlineExpired' = TRUE
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, sendCount, rejectedAfterRetirement>>
  /\ Record("write-deadline-expires")

ResultDeadlineExpires ==
  /\ phase = "awaiting-result"
  /\ ~deadlineExpired
  /\ deadlineExpired' = TRUE
  /\ UNCHANGED <<phase, resultClass, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("result-deadline-expires")

MatchingKnownResultArrives ==
  /\ phase = "awaiting-result"
  /\ resultClass = "none"
  /\ resultClass' = "matching-known"
  /\ UNCHANGED <<phase, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("matching-known-result-arrives")

MatchingUnknownResultArrives ==
  /\ phase = "awaiting-result"
  /\ resultClass = "none"
  /\ resultClass' = "matching-server-unknown"
  /\ UNCHANGED <<phase, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("matching-unknown-result-arrives")

WrongKindResultArrives ==
  /\ phase = "awaiting-result"
  /\ resultClass = "none"
  /\ resultClass' = "wrong-kind"
  /\ UNCHANGED <<phase, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("wrong-kind-result-arrives")

WrongIdResultArrives ==
  /\ phase = "awaiting-result"
  /\ resultClass = "none"
  /\ resultClass' = "wrong-id"
  /\ UNCHANGED <<phase, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("wrong-id-result-arrives")

MalformedResultArrives ==
  /\ phase = "awaiting-result"
  /\ resultClass = "none"
  /\ resultClass' = "malformed"
  /\ UNCHANGED <<phase, deadlineExpired, sendCount, outcome,
       rejectedAfterRetirement>>
  /\ Record("malformed-result-arrives")

ReadFails ==
  /\ phase = "awaiting-result"
  /\ resultClass = "none"
  /\ phase' = "retired"
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("read-fails")

AcceptMatchingResult ==
  /\ phase = "awaiting-result"
  /\ resultClass \in {"matching-known", "matching-server-unknown"}
  /\ ~deadlineExpired
  /\ phase' = "completed"
  /\ outcome' = IF resultClass = "matching-known"
       THEN "result-known" ELSE "result-server-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("accept-matching-result")

RetireTimedOutResult ==
  /\ phase = "awaiting-result"
  /\ deadlineExpired
  /\ phase' = "retired"
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("retire-timed-out-result")

RetireWrongKind ==
  /\ phase = "awaiting-result"
  /\ resultClass = "wrong-kind"
  /\ ~deadlineExpired
  /\ phase' = "retired"
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("retire-wrong-kind")

RetireWrongId ==
  /\ phase = "awaiting-result"
  /\ resultClass = "wrong-id"
  /\ ~deadlineExpired
  /\ phase' = "retired"
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("retire-wrong-id")

RetireMalformedResult ==
  /\ phase = "awaiting-result"
  /\ resultClass = "malformed"
  /\ ~deadlineExpired
  /\ phase' = "retired"
  /\ outcome' = "transport-unknown"
  /\ UNCHANGED <<resultClass, deadlineExpired, sendCount,
       rejectedAfterRetirement>>
  /\ Record("retire-malformed-result")

RejectSendAfterRetirement ==
  /\ phase = "retired"
  /\ ~rejectedAfterRetirement
  /\ rejectedAfterRetirement' = TRUE
  /\ UNCHANGED <<phase, resultClass, deadlineExpired, sendCount, outcome>>
  /\ Record("reject-send-after-retirement")

Next ==
  \/ BeginSubmission
  \/ EncodingFails
  \/ EncodingSucceeds
  \/ PreWriteDeadline
  \/ PreWriteIoFails
  \/ WriteBegins
  \/ WriteCompletes
  \/ WriteFails
  \/ WriteDeadlineExpires
  \/ ResultDeadlineExpires
  \/ MatchingKnownResultArrives
  \/ MatchingUnknownResultArrives
  \/ WrongKindResultArrives
  \/ WrongIdResultArrives
  \/ MalformedResultArrives
  \/ ReadFails
  \/ AcceptMatchingResult
  \/ RetireTimedOutResult
  \/ RetireWrongKind
  \/ RetireWrongId
  \/ RetireMalformedResult
  \/ RejectSendAfterRetirement

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ phase \in Phases
  /\ resultClass \in ResultClasses
  /\ deadlineExpired \in BOOLEAN
  /\ sendCount \in 0..1
  /\ outcome \in Outcomes
  /\ rejectedAfterRetirement \in BOOLEAN
  /\ previousPhase \in Phases
  /\ previousSendCount \in 0..1
  /\ lastAction \in Actions

AtMostOneHandoff == sendCount <= 1

DefiniteFailurePrecedesHandoff ==
  outcome = "definite-failure" => sendCount = 0

UnknownRequiresHandoffAndRetirement ==
  outcome = "transport-unknown" =>
    /\ sendCount = 1
    /\ phase = "retired"

AcceptedResultIsExactAndBeforeDeadline ==
  outcome \in {"result-known", "result-server-unknown"} =>
    /\ phase = "completed"
    /\ resultClass \in {"matching-known", "matching-server-unknown"}
    /\ ~deadlineExpired
    /\ sendCount = 1

TimeoutCannotProduceAcceptedResult ==
  deadlineExpired => outcome \notin {"result-known", "result-server-unknown"}

WriteStateTracksPossibleHandoff ==
  phase \in {"write-begun", "awaiting-result", "completed", "retired"} =>
    sendCount = 1

NoSendAfterRetirement ==
  previousPhase = "retired" => sendCount = previousSendCount

RejectedRetiredSubmissionPreservesRetirement ==
  rejectedAfterRetirement =>
    /\ phase = "retired"
    /\ outcome = "transport-unknown"
    /\ sendCount = 1

=============================================================================
