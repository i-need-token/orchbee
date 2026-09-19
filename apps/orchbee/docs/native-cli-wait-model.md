# `NativeCliWaitSettlement` — `wait-output` settlement model

`formal/tla/NativeCliWaitSettlement.tla` is a bounded TLA+ model of one `terminal wait-output`
invocation's client-side settlement. It exists because the command composes cancellation,
an asynchronous deadline, and independently ordered transport outcomes — the test suite
samples representative interleavings, while the model enumerates all of them inside its
bounds. The modeled source is `src/cli/query.rs` at the pinned base commit recorded in the
model header.

## Scope

The model covers one invocation on one terminal:

- the preamble gate (`session_preamble` settles the gate before the socket cohort is
  admitted, so a sampled flag can end the command with no attach at all);
- the `terminal.viewer` attach exchange, where an `Ok` answer establishes a named viewer
  and every later exit goes through the shared cleanup path, while an `Err` answer may end
  the invocation with no cleanup obligation;
- the observe loop's per-step flag gates before `Poll`, `FrameFeedback`, and `ReadRows`;
- bounded `read-rows` sequencing: the sequence number is consumed before the step deadline
  check, stale answers force a retry with a fresh number, and exhaustion is a real internal
  failure — never a reset or wraparound;
- the detach decision (`step_deadline` samples cancel before expiry; a sampled flag may
  skip submission while recording the real cleanup provenance);
- the unconditional late sample after detach, which may change the primary classification
  but never rewrites an already emitted cleanup fact.

Environment flags (`cancelled`, `expired`) may land between any two steps and may still
arrive after emission — the model records emit-time evidence (`emitAction`,
`cancelledAtDone`, `expiredAtDone`, `matchedOnce`) precisely so that late, unobservable
environment steps cannot falsify a finished answer.

## Source mapping

| Model element | Runtime anchor in `src/cli/query.rs` |
|---|---|
| `PreambleGate` / `PreambleFail` | `session_preamble` returns after `run.check()` settles the gate; a sampled flag ends the invocation with `cleanup = "absent"` and no attach |
| `IssueAttach` / `AnswerAttachOk` / `AnswerAttachErr` | `attach_viewer` after `run.check()` + `step_deadline`; `Ok` yields a named viewer, `Err` reaches `PreViewerSettle` |
| `ObserveGate` / `IssuePoll` | the loop-top `run.check()` in `wait_observe` before each poll |
| `IssueFeedback` / `IssueReadRows` | `step_deadline` before each exchange; `wait_read_window` consumes `next_sequence` first |
| `Settle` | the post-exchange `run.check()` that lets a sampled flag outrank the exchange's own classification |
| `AttemptDetach` | the pre-submit `step_deadline`: `cancel` skips with `unconfirmed`, `expiry` skips with `requested` |
| `LateSample` | the unconditional `run.check()` after detach that decides the final primary while preserving the cleanup fact |
| `matchedOnce` | set only when a consistent window passes the gate and the matcher accepts it — never on a raw reply |

## Properties

Safety invariants: `TypeOK`, `OutputWrittenOnce`, `NoFabricatedTerminalAnswers`,
`LateSampleDecides`, `CleanupHonest`, `ObserveOutcomeSurvives`,
`ResultRequiresConfirmedDetach`, `MatchFactPersists`, `SeqStrictIncrease`, and
`ObservedConsistentOnly`. Temporal properties: `Termination` and `MatchedSettles` under
weak fairness on the command's own actions; the signal flag itself is not required to be
fair. Bounds: `MaxSeq = 2`, observation count saturates, `CHECK_DEADLOCK FALSE`.

## Evidence

The authoritative run uses the pinned `tla2tools` jar, one worker, a 512 MiB heap, and the
shared `heavy.lock`, with each run's log and exit code kept in separate batch evidence
files. The positive run passes all invariants and both temporal properties with nonzero
coverage on every action. Two out-of-band mutants must keep producing genuine
counterexamples: skipping the late sample on detach error violates `LateSampleDecides`,
and letting a late flag erase a confirmed cleanup violates `CleanupHonest`. Reachability
probes (`ProbeNo*`) witness the happy path, unflagged typed detach, cancelled with
`requested` or `confirmed` cleanup, sequence exhaustion, a second sequence number, a typed
outcome surviving a detach error, and pre-viewer cancellation with absent cleanup.

## Boundaries

The model does not cover Server or Worker internals, wire codecs, the matcher algorithm,
wall-clock timing, multiple invocations, or any TUI surface; it claims nothing about HERDR
parity. It models the settlement contract only — which classification is emitted, what
cleanup state is reported, and which sampled facts are allowed to decide them.
