------------------- MODULE NativeCliWaitSettlement -------------------
EXTENDS Naturals, TLC

\* One-invocation settlement model of the native `terminal wait-output`
\* command: the CLI-side gate where a pending HUP/INT/QUIT/TERM flag, one
\* total deadline, and independently ordered exchange answers interleave.
\* Every awaited step's return — success or failure — is settled against a
\* fresh sample of the two flags; a sampled flag outranks the step's own
\* classification, and the last sample before emission decides the answer.
\* A flag landing after that deciding sample is unobservable and never
\* rewrites the emitted answer.
\*
\* Anchors (query.rs, pinned commit): the per-step `settle(gate)?` calls in
\* `session_preamble`, the `run.check()` + `step_deadline()` gates at
\* attach, the loop-top `run.check()` and per-exchange post-return checks
\* inside `wait_observe`, `next_sequence` consumed before `step_deadline`
\* in `wait_read_window`, the expiry/cancel-gated `step_deadline()` detach
\* decision, and the unconditional `run.check()` late sample after any
\* detach outcome. Frame decoding, UTF-8, byte matching, Server/Worker
\* internals, and the peer's byte-level protocol are covered by executable
\* tests and stay outside this model; one fixed terminal, no multi-pane.

CONSTANT MaxSeq \* issued read-rows requestSequence bound; exhaustion is a
                \* real transition, never excluded by a state constraint

Phases == {"preamble", "admitted", "attaching", "observing", "detaching",
           "failing", "done"}
PendingKinds == {"none", "attach", "poll", "feedback", "read-rows",
                 "detach"}
Answers == {"none", "ok", "err", "empty", "record", "window", "match",
            "stale", "acked"}
Outcomes == {"none", "matched", "cancelled", "timeout", "typed", "internal"}
Primaries == {"none", "result", "cancelled", "timeout", "typed",
              "typed-detach", "internal"}
Cleanups == {"absent", "confirmed", "requested", "unconfirmed"}
Skips == {"none", "cancel", "expire"}
Samples == {"ff", "cf", "fe", "ce"}
ActionNames == {
  "init", "expire", "signal", "preamble-gate", "preamble-fail",
  "issue-attach", "observe-gate", "issue-poll", "issue-feedback",
  "issue-read-rows", "issue-read-rows-exhausted", "attempt-detach",
  "answer-attach-ok", "answer-attach-err", "answer-poll-empty",
  "answer-poll-record", "answer-poll-err", "answer-feedback-ok",
  "answer-feedback-err", "answer-read-rows-window",
  "answer-read-rows-match", "answer-read-rows-stale",
  "answer-read-rows-err", "answer-detach-acked", "answer-detach-err",
  "settle", "pre-viewer-settle", "late-sample"
}

VARIABLES phase, expired, cancelled, viewerKnown, seq, obs, applied,
  feedbackOk, gateFresh, outcome, primary, cleanup, pending, answered,
  detachAcked, detachErrSeen, detachSkip, lastSample, cancelledAtDone,
  expiredAtDone, previousSeq, previousObs, settledAnswer, matchedOnce,
  emitAction, emitCount, lastAction

vars == <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
  feedbackOk, gateFresh, outcome, primary, cleanup, pending, answered,
  detachAcked, detachErrSeen, detachSkip, lastSample, cancelledAtDone,
  expiredAtDone, previousSeq, previousObs, settledAnswer, matchedOnce,
  emitAction, emitCount, lastAction>>

SampleOf(cancelFlag, expiredFlag) ==
  IF cancelFlag THEN IF expiredFlag THEN "ce" ELSE "cf"
  ELSE IF expiredFlag THEN "fe" ELSE "ff"

Record(action) ==
  /\ previousSeq' = seq
  /\ previousObs' = obs
  /\ lastAction' = action

Init ==
  /\ phase = "preamble"
  /\ expired = FALSE
  /\ cancelled = FALSE
  /\ viewerKnown = FALSE
  /\ seq = 0
  /\ obs = 0
  /\ applied = FALSE
  /\ feedbackOk = FALSE
  /\ gateFresh = FALSE
  /\ outcome = "none"
  /\ primary = "none"
  /\ cleanup = "absent"
  /\ pending = "none"
  /\ answered = "none"
  /\ detachAcked = FALSE
  /\ detachErrSeen = FALSE
  /\ detachSkip = "none"
  /\ lastSample = "ff"
  /\ cancelledAtDone = FALSE
  /\ expiredAtDone = FALSE
  /\ previousSeq = 0
  /\ previousObs = 0
  /\ settledAnswer = "none"
  /\ matchedOnce = FALSE
  /\ emitAction = "none"
  /\ emitCount = 0
  /\ lastAction = "init"

\* --- Environment events: independently ordered, never sealed — a flag may
\* --- still land after the answer was emitted; only the recorded sample
\* --- and the emit-time flags say what the output saw.

Expire ==
  /\ ~expired
  /\ expired' = TRUE
  /\ Record("expire")
  /\ UNCHANGED <<phase, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, pending, answered,
       detachAcked, detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

Signal ==
  /\ ~cancelled
  /\ cancelled' = TRUE
  /\ Record("signal")
  /\ UNCHANGED <<phase, expired, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, pending, answered,
       detachAcked, detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- Preamble: the folded per-step settles of snapshot_session_before.
\* --- A flag caught there, or a step's own failure, ends the run before
\* --- any attach — with no cleanup field, because no viewer identity was
\* --- ever obtained. The caller maps the settled kind without re-sampling.

PreambleGate ==
  /\ phase = "preamble"
  /\ pending = "none" /\ answered = "none"
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ \/ /\ cancelled
        /\ phase' = "done" /\ primary' = "cancelled"
     \/ /\ ~cancelled /\ expired
        /\ phase' = "done" /\ primary' = "timeout"
     \/ /\ ~cancelled /\ ~expired
        /\ phase' = "admitted" /\ UNCHANGED primary
  /\ IF phase' = "done"
      THEN /\ cancelledAtDone' = cancelled /\ expiredAtDone' = expired
           /\ emitAction' = "preamble-gate" /\ emitCount' = emitCount + 1
      ELSE UNCHANGED <<cancelledAtDone, expiredAtDone, emitAction, emitCount>>
  /\ Record("preamble-gate")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, cleanup, pending, answered,
       detachAcked, detachErrSeen, detachSkip, settledAnswer, matchedOnce>>

PreambleFail ==
  /\ phase = "preamble"
  /\ pending = "none" /\ answered = "none"
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ phase' = "done"
  /\ primary' = IF cancelled THEN "cancelled"
                ELSE IF expired THEN "timeout" ELSE "typed"
  /\ cancelledAtDone' = cancelled /\ expiredAtDone' = expired
  /\ emitAction' = "preamble-fail" /\ emitCount' = emitCount + 1
  /\ Record("preamble-fail")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, cleanup, pending, answered,
       detachAcked, detachErrSeen, detachSkip, settledAnswer, matchedOnce>>

\* --- Attach: the admitted state models the gap where a flag may still
\* --- land between the preamble gate and the attach gate. The attach
\* --- gate's own sample (the run.check + step_deadline pair) either
\* --- submits the exchange or fails pre-submit; pre-submit failures take
\* --- the Err arm's second sample ("failing") before the answer is
\* --- written.

IssueAttach ==
  /\ phase = "admitted"
  /\ pending = "none" /\ answered = "none"
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ \/ /\ cancelled
        /\ outcome' = "cancelled" /\ phase' = "failing"
        /\ UNCHANGED <<pending, viewerKnown>>
     \/ /\ ~cancelled /\ expired
        /\ outcome' = "timeout" /\ phase' = "failing"
        /\ UNCHANGED <<pending, viewerKnown>>
     \/ /\ ~cancelled /\ ~expired
        /\ pending' = "attach" /\ phase' = "attaching"
        /\ UNCHANGED <<outcome, viewerKnown>>
  /\ Record("issue-attach")
  /\ UNCHANGED <<expired, cancelled, seq, obs, applied, feedbackOk,
       gateFresh, primary, cleanup, answered, detachAcked, detachErrSeen,
       detachSkip, cancelledAtDone, expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- The loop-top gate: run.check() before every poll. A fresh sample is
\* --- required for the next poll issue; a flag landing between the sample
\* --- and the issue is allowed and caught at that answer's settle.

ObserveGate ==
  /\ phase = "observing"
  /\ pending = "none" /\ answered = "none"
  /\ ~applied /\ ~feedbackOk
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ \/ /\ cancelled
        /\ outcome' = "cancelled" /\ phase' = "detaching"
        /\ UNCHANGED gateFresh
     \/ /\ ~cancelled /\ expired
        /\ outcome' = "timeout" /\ phase' = "detaching"
        /\ UNCHANGED gateFresh
     \/ /\ ~cancelled /\ ~expired
        /\ gateFresh' = TRUE
        /\ UNCHANGED <<outcome, phase>>
  /\ Record("observe-gate")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, primary, cleanup, pending, answered, detachAcked,
       detachErrSeen, detachSkip, cancelledAtDone, expiredAtDone,
       settledAnswer, matchedOnce, emitAction, emitCount>>

IssuePoll ==
  /\ phase = "observing"
  /\ pending = "none" /\ answered = "none"
  /\ ~applied /\ ~feedbackOk
  /\ gateFresh
  /\ pending' = "poll"
  /\ gateFresh' = FALSE
  /\ Record("issue-poll")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, outcome, primary, cleanup, answered, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- Feedback and read-rows issues carry their own step_deadline sample:
\* --- a flag settles the step failure instead of submitting.

IssueFeedback ==
  /\ phase = "observing"
  /\ pending = "none" /\ answered = "none"
  /\ applied /\ ~feedbackOk
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ \/ /\ cancelled
        /\ outcome' = "cancelled" /\ phase' = "detaching"
        /\ UNCHANGED pending
     \/ /\ ~cancelled /\ expired
        /\ outcome' = "timeout" /\ phase' = "detaching"
        /\ UNCHANGED pending
     \/ /\ ~cancelled /\ ~expired
        /\ pending' = "feedback"
        /\ UNCHANGED <<outcome, phase>>
  /\ Record("issue-feedback")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, primary, cleanup, answered, detachAcked,
       detachErrSeen, detachSkip, cancelledAtDone, expiredAtDone,
       settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- next_sequence is consumed before step_deadline: the sequence is
\* --- spent even when a flag then turns the issue into a failure, and
\* --- exhaustion is a real internal-failure transition.
IssueReadRows ==
  /\ phase = "observing"
  /\ pending = "none" /\ answered = "none"
  /\ feedbackOk
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ IF seq = MaxSeq
      THEN /\ outcome' = "internal" /\ phase' = "detaching"
           /\ UNCHANGED <<seq, pending>>
           /\ Record("issue-read-rows-exhausted")
      ELSE /\ seq' = seq + 1
           /\ \/ /\ cancelled
                 /\ outcome' = "cancelled" /\ phase' = "detaching"
                 /\ UNCHANGED pending
              \/ /\ ~cancelled /\ expired
                 /\ outcome' = "timeout" /\ phase' = "detaching"
                 /\ UNCHANGED pending
              \/ /\ ~cancelled /\ ~expired
                 /\ pending' = "read-rows"
                 /\ UNCHANGED <<outcome, phase>>
           /\ Record("issue-read-rows")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, obs, applied,
       feedbackOk, gateFresh, primary, cleanup, answered, detachAcked,
       detachErrSeen, detachSkip, cancelledAtDone, expiredAtDone,
       settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- Detach decision: step_deadline settles the flags before submit.
\* --- Cancel makes the exchange an unconfirmed skip; expiry makes it a
\* --- requested (never submitted) cleanup; a submitted exchange resolves
\* --- for real. The later late sample is a separate step.
AttemptDetach ==
  /\ phase = "detaching"
  /\ pending = "none" /\ answered = "none"
  /\ cleanup = "absent" /\ detachSkip = "none" /\ ~detachAcked
  /\ ~detachErrSeen
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ \/ /\ cancelled
        /\ cleanup' = "unconfirmed" /\ detachSkip' = "cancel"
        /\ UNCHANGED pending
     \/ /\ ~cancelled /\ expired
        /\ cleanup' = "requested" /\ detachSkip' = "expire"
        /\ UNCHANGED pending
     \/ /\ ~cancelled /\ ~expired
        /\ pending' = "detach"
        /\ UNCHANGED <<cleanup, detachSkip>>
  /\ Record("attempt-detach")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, answered, detachAcked,
       detachErrSeen, cancelledAtDone, expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- Answers: the peer/environment resolves each pending exchange. The
\* --- delivered facts are recorded at delivery; Settle consumes them.
\* --- A submitted detach can only ack or fail — an in-flight timeout is
\* --- transport loss, never the no-submit "requested" kind.

AnswerAttachOk ==
  /\ pending = "attach"
  /\ pending' = "none" /\ answered' = "ok"
  /\ Record("answer-attach-ok")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerAttachErr ==
  /\ pending = "attach"
  /\ pending' = "none" /\ answered' = "err"
  /\ Record("answer-attach-err")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerPollEmpty ==
  /\ pending = "poll"
  /\ pending' = "none" /\ answered' = "empty"
  /\ Record("answer-poll-empty")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerPollRecord ==
  /\ pending = "poll"
  /\ pending' = "none" /\ answered' = "record"
  /\ Record("answer-poll-record")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerPollErr ==
  /\ pending = "poll"
  /\ pending' = "none" /\ answered' = "err"
  /\ Record("answer-poll-err")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerFeedbackOk ==
  /\ pending = "feedback"
  /\ pending' = "none" /\ answered' = "ok"
  /\ Record("answer-feedback-ok")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerFeedbackErr ==
  /\ pending = "feedback"
  /\ pending' = "none" /\ answered' = "err"
  /\ Record("answer-feedback-err")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerReadRowsWindow ==
  /\ pending = "read-rows"
  /\ pending' = "none" /\ answered' = "window"
  /\ Record("answer-read-rows-window")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerReadRowsMatch ==
  /\ pending = "read-rows"
  /\ pending' = "none" /\ answered' = "match"
  /\ Record("answer-read-rows-match")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerReadRowsStale ==
  /\ pending = "read-rows"
  /\ pending' = "none" /\ answered' = "stale"
  /\ Record("answer-read-rows-stale")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerReadRowsErr ==
  /\ pending = "read-rows"
  /\ pending' = "none" /\ answered' = "err"
  /\ Record("answer-read-rows-err")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachErrSeen, detachSkip, lastSample, cancelledAtDone,
       expiredAtDone, settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerDetachAcked ==
  /\ pending = "detach"
  /\ pending' = "none" /\ answered' = "acked"
  /\ detachAcked' = TRUE
  /\ Record("answer-detach-acked")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachErrSeen,
       detachSkip, lastSample, cancelledAtDone, expiredAtDone,
       settledAnswer, matchedOnce, emitAction, emitCount>>

AnswerDetachErr ==
  /\ pending = "detach"
  /\ pending' = "none" /\ answered' = "err"
  /\ detachErrSeen' = TRUE
  /\ Record("answer-detach-err")
  /\ UNCHANGED <<phase, expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, primary, cleanup, detachAcked,
       detachSkip, lastSample, cancelledAtDone, expiredAtDone,
       settledAnswer, matchedOnce, emitAction, emitCount>>

\* --- Settle: consume one delivered answer, sample the flags at this
\* --- point, and route. The sampled flags outrank the answer's own class;
\* --- pre-viewer exits carry no cleanup field, post-viewer exits always
\* --- route through the shared cleanup path.
Settle ==
  /\ answered /= "none"
  /\ pending = "none"
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ settledAnswer' = answered
  /\ answered' = "none"
  /\ Record("settle")
  /\ UNCHANGED <<expired, cancelled, pending, detachAcked, detachErrSeen,
       detachSkip>>
  /\ \/ /\ phase = "detaching"
        /\ answered \in {"acked", "err"}
        /\ \* A detach answer only writes the cleanup fact; the late
           \* sample runs unconditionally after it, as a separate step.
           cleanup' = IF answered = "acked" THEN "confirmed"
                      ELSE "unconfirmed"
        /\ UNCHANGED <<phase, outcome, primary, viewerKnown, seq, obs,
             applied, feedbackOk, gateFresh, matchedOnce, emitAction, emitCount,
             cancelledAtDone, expiredAtDone>>
     \/ /\ phase = "attaching"
        /\ answered = "ok"
        /\ \* A named viewer now exists: every later exit — including a
           \* flag sampled at this very settle — takes the shared cleanup
           \* path, never a pre-viewer drop.
           viewerKnown' = TRUE
        /\ UNCHANGED <<primary, cleanup, seq, obs, applied, feedbackOk,
             gateFresh, matchedOnce, emitAction, emitCount,
             cancelledAtDone, expiredAtDone>>
        /\ \/ /\ cancelled
              /\ outcome' = "cancelled" /\ phase' = "detaching"
           \/ /\ ~cancelled /\ expired
              /\ outcome' = "timeout" /\ phase' = "detaching"
           \/ /\ ~cancelled /\ ~expired
              /\ UNCHANGED outcome /\ phase' = "observing"
     \/ /\ phase = "attaching"
        /\ answered = "err"
        /\ phase' = "done"
        /\ primary' = IF cancelled THEN "cancelled"
                      ELSE IF expired THEN "timeout" ELSE "typed"
        /\ cancelledAtDone' = cancelled /\ expiredAtDone' = expired
        /\ emitAction' = "settle" /\ emitCount' = emitCount + 1
        /\ UNCHANGED <<outcome, cleanup, viewerKnown, seq, obs, applied,
             feedbackOk, gateFresh, matchedOnce>>
     \/ /\ phase = "observing"
        /\ cancelled
        /\ outcome' = "cancelled" /\ phase' = "detaching"
        /\ UNCHANGED <<primary, cleanup, viewerKnown, seq, obs, applied,
             feedbackOk, gateFresh, matchedOnce, emitAction, emitCount,
             cancelledAtDone, expiredAtDone>>
     \/ /\ phase = "observing"
        /\ ~cancelled /\ expired
        /\ outcome' = "timeout" /\ phase' = "detaching"
        /\ UNCHANGED <<primary, cleanup, viewerKnown, seq, obs, applied,
             feedbackOk, gateFresh, matchedOnce, emitAction, emitCount,
             cancelledAtDone, expiredAtDone>>
     \/ /\ phase = "observing"
        /\ ~cancelled /\ ~expired
        /\ UNCHANGED <<primary, cleanup, viewerKnown, seq, emitAction, emitCount,
             cancelledAtDone, expiredAtDone>>
        /\ \/ /\ answered = "empty"
              /\ UNCHANGED <<phase, outcome, obs, applied, feedbackOk,
                   gateFresh, matchedOnce>>
           \/ /\ answered = "record"
              /\ applied' = TRUE
              /\ UNCHANGED <<phase, outcome, obs, feedbackOk, gateFresh,
                   matchedOnce>>
           \/ /\ answered = "ok"
              /\ feedbackOk' = TRUE
              /\ UNCHANGED <<phase, outcome, obs, applied, gateFresh,
                   matchedOnce>>
           \/ /\ answered = "window"
              /\ obs' = IF obs < 2 THEN obs + 1 ELSE obs
              /\ applied' = FALSE /\ feedbackOk' = FALSE
              /\ UNCHANGED <<phase, outcome, gateFresh, matchedOnce>>
           \/ /\ answered = "match"
              /\ obs' = IF obs < 2 THEN obs + 1 ELSE obs
              /\ outcome' = "matched" /\ phase' = "detaching"
              /\ matchedOnce' = TRUE
              /\ UNCHANGED <<applied, feedbackOk, gateFresh>>
           \/ /\ answered = "stale"
              /\ applied' = FALSE /\ feedbackOk' = FALSE
              /\ UNCHANGED <<phase, outcome, obs, gateFresh, matchedOnce>>
           \/ /\ answered = "err"
              /\ outcome' = "typed" /\ phase' = "detaching"
              /\ applied' = FALSE /\ feedbackOk' = FALSE
              /\ UNCHANGED <<obs, gateFresh, matchedOnce>>

\* --- Pre-viewer second sample: the attach Err arm re-checks the gate
\* --- before returning, so a flag landing after the first sample upgrades
\* --- the stored failure — cancel first, then expiry, else the recorded
\* --- class.
PreViewerSettle ==
  /\ phase = "failing"
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ primary' = IF cancelled THEN "cancelled"
                ELSE IF expired THEN "timeout" ELSE outcome
  /\ cancelledAtDone' = cancelled /\ expiredAtDone' = expired
  /\ emitAction' = "pre-viewer-settle" /\ emitCount' = emitCount + 1
  /\ phase' = "done"
  /\ Record("pre-viewer-settle")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, cleanup, pending, answered,
       detachAcked, detachErrSeen, detachSkip, settledAnswer, matchedOnce>>

\* --- Late sample: unconditional after the cleanup fact is fixed, whatever
\* --- the detach outcome was. The flags sampled here decide the primary
\* --- over every earlier classification; the cleanup fact itself is never
\* --- rewritten.
LateSample ==
  /\ phase = "detaching"
  /\ pending = "none" /\ answered = "none"
  /\ cleanup /= "absent"
  /\ primary = "none"
  /\ lastSample' = SampleOf(cancelled, expired)
  /\ primary' =
       IF cancelled THEN "cancelled"
       ELSE IF expired THEN "timeout"
       ELSE IF outcome = "matched"
            THEN IF cleanup = "confirmed" THEN "result"
                 ELSE "typed-detach"
            ELSE outcome
  /\ cancelledAtDone' = cancelled /\ expiredAtDone' = expired
  /\ emitAction' = "late-sample" /\ emitCount' = emitCount + 1
  /\ phase' = "done"
  /\ Record("late-sample")
  /\ UNCHANGED <<expired, cancelled, viewerKnown, seq, obs, applied,
       feedbackOk, gateFresh, outcome, cleanup, pending, answered,
       detachAcked, detachErrSeen, detachSkip, settledAnswer, matchedOnce>>

Next ==
  \/ Expire \/ Signal
  \/ PreambleGate \/ PreambleFail \/ IssueAttach
  \/ ObserveGate \/ IssuePoll \/ IssueFeedback \/ IssueReadRows
  \/ AttemptDetach
  \/ AnswerAttachOk \/ AnswerAttachErr
  \/ AnswerPollEmpty \/ AnswerPollRecord \/ AnswerPollErr
  \/ AnswerFeedbackOk \/ AnswerFeedbackErr
  \/ AnswerReadRowsWindow \/ AnswerReadRowsMatch \/ AnswerReadRowsStale
  \/ AnswerReadRowsErr
  \/ AnswerDetachAcked \/ AnswerDetachErr
  \/ Settle \/ PreViewerSettle \/ LateSample

\* Fairness: time must pass, every issued exchange is eventually answered,
\* and progress actions cannot stall forever. The signal itself is not
\* fair — a run may never be cancelled and still must terminate.
Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Expire)
  /\ WF_vars(PreambleGate) /\ WF_vars(PreambleFail) /\ WF_vars(IssueAttach)
  /\ WF_vars(ObserveGate) /\ WF_vars(IssuePoll) /\ WF_vars(IssueFeedback)
  /\ WF_vars(IssueReadRows) /\ WF_vars(AttemptDetach)
  /\ WF_vars(Settle) /\ WF_vars(PreViewerSettle) /\ WF_vars(LateSample)
  /\ WF_vars(AnswerAttachOk) /\ WF_vars(AnswerAttachErr)
  /\ WF_vars(AnswerPollEmpty) /\ WF_vars(AnswerPollRecord)
  /\ WF_vars(AnswerPollErr)
  /\ WF_vars(AnswerFeedbackOk) /\ WF_vars(AnswerFeedbackErr)
  /\ WF_vars(AnswerReadRowsWindow) /\ WF_vars(AnswerReadRowsMatch)
  /\ WF_vars(AnswerReadRowsStale) /\ WF_vars(AnswerReadRowsErr)
  /\ WF_vars(AnswerDetachAcked) /\ WF_vars(AnswerDetachErr)

\* --- Safety invariants ---------------------------------------------------

TypeOK ==
  /\ phase \in Phases
  /\ expired \in BOOLEAN
  /\ cancelled \in BOOLEAN
  /\ viewerKnown \in BOOLEAN
  /\ seq \in 0..MaxSeq
  /\ obs \in 0..2
  /\ applied \in BOOLEAN
  /\ feedbackOk \in BOOLEAN
  /\ gateFresh \in BOOLEAN
  /\ outcome \in Outcomes
  /\ primary \in Primaries
  /\ cleanup \in Cleanups
  /\ pending \in PendingKinds
  /\ answered \in Answers
  /\ detachAcked \in BOOLEAN
  /\ detachErrSeen \in BOOLEAN
  /\ detachSkip \in Skips
  /\ lastSample \in Samples
  /\ cancelledAtDone \in BOOLEAN
  /\ expiredAtDone \in BOOLEAN
  /\ previousSeq \in 0..MaxSeq
  /\ previousObs \in 0..2
  /\ settledAnswer \in Answers
  /\ matchedOnce \in BOOLEAN
  /\ emitAction \in ActionNames \cup {"none"}
  /\ emitCount \in 0..2
  /\ lastAction \in ActionNames

\* The command emits an answer exactly on the transition into done, and
\* only the terminal-writing actions can enter it — one answer per run.
\* emitCount counts the answer-writing actions themselves (never derived
\* from entering done, which would hide a repeated emit), so a second
\* emission — even an identical one — is a real violation, not a stutter.
\* emitAction is frozen at emission so later flag flips cannot blur who
\* wrote the answer.
OutputWrittenOnce ==
  /\ emitCount <= 1
  /\ primary /= "none" <=> phase = "done"
  /\ phase = "done" =>
       emitAction \in {"preamble-gate", "preamble-fail", "settle",
                        "pre-viewer-settle", "late-sample"}

\* cancelled/timeout primaries only ever trace to a real recorded sample —
\* never fabricated from a step's own classification.
NoFabricatedTerminalAnswers ==
  /\ primary = "cancelled" => lastSample \in {"cf", "ce"}
  /\ primary = "timeout" => lastSample \in {"fe", "ce"}

\* Settle precedence, keyed on the recorded sample and the emit-time flags
\* — not on a sealed environment: the deciding sample's flags win over
\* every step classification; a flag landing after it is unobservable and
\* legal beside any primary.
LateSampleDecides ==
  phase = "done" =>
    /\ lastSample \in {"cf", "ce"} => primary = "cancelled"
    /\ lastSample = "fe" => primary = "timeout"
    /\ lastSample = "ff" =>
         primary \in {"result", "typed", "typed-detach", "internal"}
    /\ cancelledAtDone => primary = "cancelled"
    /\ ~cancelledAtDone /\ expiredAtDone => primary = "timeout"

\* Cleanup honesty: each value has exactly its real source, and a fact
\* once consumed is never rewritten — the delivered-but-unsettled window
\* (answered still holding the exchange class) is the only legal gap.
CleanupHonest ==
  /\ cleanup = "confirmed" => detachAcked
  /\ detachAcked /\ answered = "none" => cleanup = "confirmed"
  /\ detachErrSeen /\ answered = "none" => cleanup = "unconfirmed"
  /\ cleanup = "requested" => detachSkip = "expire"
  /\ cleanup = "unconfirmed" => detachErrSeen \/ detachSkip = "cancel"
  /\ phase = "done" => (cleanup = "absent" <=> ~viewerKnown)

\* The original observe outcome survives the detach exchange: without a
\* late flag the emitted primary is the observe's own class, never the
\* detach's — and a match's typed-detach provenance stays distinct.
ObserveOutcomeSurvives ==
  phase = "done" /\ lastSample = "ff" =>
    /\ outcome = "typed" => primary = "typed"
    /\ outcome = "internal" => primary = "internal"
    /\ outcome = "matched" => primary \in {"result", "typed-detach"}

\* A result is only ever the honest one: a matched observe whose detach
\* was confirmed. Anything less is a typed-detach, never a fake success.
ResultRequiresConfirmedDetach ==
  primary = "result" => outcome = "matched" /\ cleanup = "confirmed"

\* A match fact once observed is never erased — a late cancel or timeout
\* keeps matchObserved true beside its own primary.
MatchFactPersists ==
  matchedOnce => outcome = "matched"

\* requestSequence is strictly increasing, in both directions: a growth
\* only ever comes from an issue-read-rows step of exactly +1, and every
\* issue-read-rows really consumes a fresh number — a repeated issue that
\* kept the old sequence would forge a stale-replay on the wire. The
\* exhaustion arm records its own action name and is untouched by this.
SeqStrictIncrease ==
  /\ seq >= previousSeq
  /\ seq > previousSeq =>
       /\ lastAction = "issue-read-rows"
       /\ seq = previousSeq + 1
  /\ lastAction = "issue-read-rows" => seq = previousSeq + 1

\* The observation counter only ever grows on a consumed consistent-window
\* or match answer — stale or failed reads never count.
ObservedConsistentOnly ==
  /\ obs >= previousObs
  /\ obs > previousObs =>
       /\ lastAction = "settle"
       /\ settledAnswer \in {"window", "match"}

\* --- Liveness ------------------------------------------------------------

Termination == <>(phase = "done")
MatchedSettles == (outcome = "matched") ~> (phase = "done")

\* --- Reachability probes --------------------------------------------------
\* Each probe is a deliberately false invariant asserted only by a separate
\* evidence configuration: its violation trace is the witness that the
\* control case is genuinely reachable. The shipped .cfg never lists them.

\* Happy path: match + confirmed detach + no flag at emission.
ProbeNoResult ==
  ~(phase = "done" /\ primary = "result" /\ cleanup = "confirmed")

\* NC3: typed detach refusal with no signal/expiry pending at emission.
ProbeNoTypedDetachUnflagged ==
  ~(phase = "done" /\ primary = "typed-detach" /\ cleanup = "unconfirmed"
    /\ ~cancelledAtDone /\ ~expiredAtDone)

\* Expiry skips the detach submit, then a late signal still wins the
\* primary — the legal {cancelled, requested} split-sample outcome.
ProbeNoCancelledRequested ==
  ~(phase = "done" /\ primary = "cancelled" /\ cleanup = "requested")

\* NC1: cancel lands after a confirmed detach; cleanup stays confirmed.
ProbeNoCancelledConfirmed ==
  ~(phase = "done" /\ primary = "cancelled" /\ cleanup = "confirmed")

\* Sequence exhaustion is a real internal-failure transition.
ProbeNoInternal ==
  ~(phase = "done" /\ primary = "internal")

\* A second read issue is reachable (stale answers re-issue the sequence).
ProbeNoSecondSequence ==
  ~(seq = 2)

\* An observe-stage typed failure survives a failing detach exchange.
ProbeNoObservedTypedSurvivor ==
  ~(phase = "done" /\ primary = "typed" /\ outcome = "typed"
    /\ cleanup = "unconfirmed")

\* A pre-viewer cancel exits with no cleanup field at all.
ProbeNoPreViewerCancel ==
  ~(phase = "done" /\ primary = "cancelled" /\ cleanup = "absent")

=============================================================================
