# Modeling and Correctness

## When evidence is required

Select the smallest evidence that can judge the changed claims. Use of async work, cancellation,
gestures, or several files does not by itself require a stronger method; a real ordering, ownership,
or durability risk does, however local the change looks.

| The change has                                                                                     | It carries                                                                                           |
| -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Local presentation without new domain state or transitions                                         | The existing component contract and tests admitted under [`TESTING.md`](TESTING.md)                  |
| Local completion, cancellation, or retry under one established owner with bounded effects          | Types and focused tests for stale results, cancellation, failure, and cleanup                        |
| Owned domain state, interacting transitions, persistence, or process coordination                  | Explicit state, events, outputs, and invariants in types, with deterministic tests of the core       |
| Combinations or sequences that named examples cannot bound                                         | Property-based or model-based exploration with shrinking and a replayable failure                    |
| A cross-process protocol that passes the [TLA+ admission bar](#tla)                                | A TLA+ model with safety and applicable progress checks, and evidence tying it to the implementation |
| A critical pure transformation that needs a claim over all inputs                                  | A Lean theorem                                                                                       |
| A source-compatible port or behavioral replica                                                     | A pinned reference, a stated parity scope, and differential checks inside that scope                 |
| Timing, geometry, focus, gestures, or platform behavior that cannot be judged at the core boundary | Controlled adapter or system traces at the lowest faithful boundary                                  |

- Types, transitions, and behavior tests are the model. Add prose only for a decision, assumption,
  or boundary they do not explain, under [package documents](DOCUMENTATION.md#package-documents);
  never restate them in a model document, a per-change record, or a claim-to-evidence table.
- Existing evidence obligations and registered models stay in force. Retiring a model, weakening a
  property, or replacing a mandatory method needs the decision owner's acceptance; cost alone does
  not waive evidence.
- A passing model or theorem says nothing about the implementation without a refinement mapping or
  an executable comparison. The model, the implementation, and the physical platform are separate
  claims.

| Method                    | Supports                                                             | Limit                                       |
| ------------------------- | -------------------------------------------------------------------- | ------------------------------------------- |
| Example test              | One selected case                                                    | Only the selected cases                     |
| Property test             | Invariants over generated states and sequences                       | The configured search space                 |
| TLA+                      | No checked safety or progress counterexample in the abstract machine | The model within its declared bounds        |
| Lean                      | A theorem for all values its definitions and assumptions admit       | Nothing about unrelated production code     |
| Deterministic system test | A controlled observable timeline of the integrated system            | The declared traces and environments        |
| Differential verification | Agreement with an adopted reference projection                       | The reference scope and observation quality |

## Domain contract

Define the behavior in user-observable terms before naming components, Hooks, storage keys, or
techniques: success, failure, cancellation, and the intentional no-op.

- Give every mutable fact one authoritative owner. Keep authoritative state, reversible previews,
  derived values, and presentation state apart.
- Use distinct variants where independent booleans could represent an impossible combination, and
  return complete results whose variants make invalid field combinations unrepresentable.
- Parse untrusted input and define absence at the boundary. Never infer a transition from
  coincidental equality between optional values.
- Keep network, storage, time, randomness, process APIs, and DOM measurement out of deterministic
  domain units. A transition returns its effects as commands and takes their results as later
  events.
- For long-lived state, define what an invalid, repeated, stale, out-of-order, interrupted, or
  cancelled event does, and hold the invariants after every accepted transition, not only after the
  final commit.
- A UI shell renders prepared state and emits intent. It may measure, capture pointers, and schedule
  frames; it never decides membership, ordering, selection, eligibility, cancellation, or commit.

## Commit and publication boundaries

- Name the authoritative commit point and classify every fallible action as before or after it.
  Complete fallible allocation, encoding, validation, and capacity checks before commit when the
  contract permits it.
- A failure before commit leaves authoritative state unchanged or runs a rollback owned by the same
  boundary. A failure after commit preserves and reports the committed outcome, then follows an
  explicit idempotency, retry, compensation, or terminal-failure policy.
- Never mutate one owner and then do untracked fallible work before another required mutation or
  effect. Put the work under one owner and commit boundary, or model the partial outcome and its
  recovery.
- When several capabilities cannot commit atomically, model the intermediate and ambiguous
  outcomes; do not claim atomicity the implementation cannot provide.
- Never turn an unexpected failure into an empty success unless the output type defines that
  degraded result.

## TLA+

A TLA+ model is a design aid for a protocol, not a regression net: implementation tests carry
regressions. Admit a new model only when all three hold, and when in doubt, do not add it:

- independently ordered processes share a handover, commit, recovery, or ownership protocol;
- a wrong order would end every terminal, lose user data, or leave two owners of one resource; and
- the harmful orders are too many for named implementation tests to cover.

Async calls, an explicit state machine, several UI states, a Client-local workflow, or a long fixed
workflow do not qualify. Extend the model of the same authority boundary before adding another.

- State the falsifiable claim, the mapping from actions to source, the finite bounds, and the
  assumptions in the module. Fairness comes from the contract.
- Keep competing identities, stale completions, failure cuts, and progress risks in the
  abstraction. Never hide a reachable failure with a state constraint or a weakened assertion.
- Require reachable action coverage and a mutation case for each motivating fault. Turn a real
  counterexample into a deterministic implementation regression at the owning boundary.
- Register each model once in `scripts/tla/models.json`; every configuration finishes within its
  20-second deadline.
- Update a model only when the protocol it abstracts changes; ordinary implementation edits leave it
  alone. A passing unchanged model says nothing about changed code.
- TLA+ runs only when a model or its tooling changes
  ([TLA+ models](VERIFICATION.md#tla-models)); no default gate and no CI job runs it.
- Retire a model by deleting its module, configuration, registration, mutation cases, and the
  passages that cite it together, naming the implementation tests that keep its behavior and
  stating the lost exhaustive assurance.

## Other methods

- **Lean.** No `sorry`, `admit`, or project-created axiom in a required theorem; disclose
  unavoidable axioms, classical assumptions, and foreign code. Tie the proof to production code
  through a shared canonical schema or corpus.
- **Property tests.** Use explicit seeds, size limits, and deterministic schedulers. Reduce every
  discovered failure to a replayable minimal case and keep it in the regression corpus. Iteration
  count is not coverage.
- **System traces.** Control or record time, scheduling, environment, interaction, geometry, and
  external state. Advance logical time or wait for a named condition; never sleep for an arbitrary
  duration. A retry cannot hide a race, and a manual observation is discovery until an asserted
  deterministic trace captures it.
- **Reference parity.** Record the reference's version, revision, flags, and platform, the
  supported scope with its exclusions, and the noise removed with the reason it cannot hide a
  difference. Capture again deliberately when the reference changes.

## Counterexamples

Preserve the environment, seed, state, events, and observations; reduce to the shortest failing
trace; find the first divergent step and the violated claim or assumption; fix the earliest
incorrect authority; keep the reduced case and run the broader exploration again.

Never weaken a property, exclude a reachable state, add an axiom, retry a race, or replace a
semantic regression with a screenshot to accept current behavior. Remove a model, property, or
theorem together with the behavior it described.

## Gate

The root gate in [`VERIFICATION.md`](VERIFICATION.md) runs the default implementation evidence; a
changed TLA+ model runs as [TLA+ models](VERIFICATION.md#tla-models) describes. The applicable
command fails, without retries, on:

- a checked TLA+ safety or progress counterexample;
- an unchecked or admitted Lean theorem, or an undeclared trust assumption;
- a failed property, or a generated failure that cannot be replayed;
- a deterministic trace mismatch, or timing that affects the result and is not controlled;
- a differential mismatch inside the declared parity scope; or
- a registered model whose module or configuration no longer exists.

A completion report names the checked claims, their bounds, and the exclusions. It makes no
unbounded claim such as "fully verified" or "perfect replica".
