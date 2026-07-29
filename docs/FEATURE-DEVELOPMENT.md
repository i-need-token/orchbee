# Feature Development

## Gate and authority

Apply every matching row before production integration; later rows add evidence to earlier rows.

| Behavior                                                                                        | Required record and evidence                                                                               |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Local presentation with no new domain state or transition                                       | Existing component contracts and tests                                                                     |
| Multiple states, ordering, grouping, persistence or process boundaries, async work, or gestures | The modeling record and deterministic core tests                                                           |
| Owned domain state or transitions                                                               | The modeling record extended with correctness claims, serializable transitions or traces, and traceability |
| A risk in [Correctness escalation](#correctness-escalation)                                     | Every evidence method selected by the matching risk rows; one method never substitutes for another         |

- Put public behavior in the owning package README and internal maintenance contracts beside the owning package.
- Keep the record in the owning package reference, public contract, types plus tests, or a combination of them.
- Put only reusable cross-workspace rules in `docs/`; never inventory one feature's states, upstream files, screen behavior, or test cases here.

## Modeling record

Complete one durable record with concrete domain names and types; a heading without a decision does not pass the gate.

| Field           | Required content                                                                   | Completion evidence                                                  |
| --------------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Problem         | Actor, context, trigger, desired outcome, constraints, and non-goals               | One observable problem statement                                     |
| Inputs          | Trusted and untrusted inputs plus absence semantics                                | Named input types and validation boundary                            |
| Data            | Entities, identifiers, relationships, ownership, lifecycle, and canonical form     | Named domain types                                                   |
| Outputs         | Success variants, failures, semantic events, side effects, cancellation, and no-op | Named output types                                                   |
| Invariants      | Facts that remain true before and after every operation                            | Assertions or invariant tests                                        |
| Decisions       | Independent dimensions that change the result                                      | Decision table or scenario matrix                                    |
| Model           | Extractor, normalizer, policy, builder, state machine, or workflow                 | Selected shape and principal public operation                        |
| Transitions     | States, events, guards, next state, derived state, commands, and cancellation      | Transition table for stateful behavior                               |
| Dependencies    | Time, storage, network, process, DOM, and other capabilities                       | Minimal capability interfaces                                        |
| Ownership       | Authoritative state owner and allowed writers                                      | One owner for each mutable fact                                      |
| Verification    | Core cases, integration boundaries, and real interaction paths                     | Decision-mapped deterministic tests independent of the production UI |
| Correctness     | Claims, trust boundary, refinement mappings, evidence, limits, and exclusions      | Required for owned state, transitions, or a correctness risk trigger |
| Reference scope | Version, environment, adopted behavior, observable projection, and exclusions      | Required only for compatibility or replication                       |

Another maintainer must be able to derive the public domain operation, claims, and test cases without inspecting the intended UI. For correctness-gated behavior, the same record is the correctness case and must show what is claimed, which executable artifact supports each claim, and what remains outside the claim.

## Domain contract

Define the problem in user-observable terms before naming components, Hooks, storage keys, or implementation techniques. State success, failure, cancellation, and intentional no-op behavior.

| Output kind    | Required decision                                                        |
| -------------- | ------------------------------------------------------------------------ |
| Returned value | Shape, meaning, ordering, and absence semantics                          |
| Semantic event | Name, payload, emission point, idempotency, and owner                    |
| State change   | Authoritative owner, commit point, and rollback or cancellation behavior |
| Rendered state | Meaningful visual, focus, accessibility, and reduced-motion result       |
| Side effect    | Target, timing, retry, failure handling, and observability               |
| No-op          | Rejecting guard and whether rejection is visible                         |

Choose the smallest model that represents every valid case and rejects invalid combinations.

| Shape                    | Use when                                                          | Public operation                | Side effects         |
| ------------------------ | ----------------------------------------------------------------- | ------------------------------- | -------------------- |
| Extractor or parser      | Meaning is derived from encoded input                             | `extract` or `parse`            | None                 |
| Normalizer               | External or legacy data becomes one canonical form                | `normalize`                     | None                 |
| Policy or resolver       | Explicit facts determine one decision                             | `resolve` or `decide`           | None                 |
| Builder or projector     | Canonical inputs produce one complete read model                  | `build` or `project`            | None by default      |
| State machine or reducer | Events change long-lived state through guarded transitions        | `transition` or `reduce`        | Returned as commands |
| Workflow or orchestrator | Ordered phases coordinate effects, short-circuiting, and recovery | `execute`, `load`, or `prepare` | Explicitly allowed   |

- Give durable entities stable identifiers and define relationship cardinality, ordering, uniqueness, membership, lifecycle, and referential integrity.
- Give every mutable fact one authoritative owner; separate authoritative state, reversible previews, derived values, animation projections, and presentation state.
- Use distinct variants when independent booleans could represent impossible combinations.
- Parse untrusted input and define absence at the boundary; never infer a transition from coincidental equality between optional values.
- Make required data nonoptional at the domain boundary and expose only the capabilities the operation may use.
- Return complete semantic results whose variants make invalid field combinations unrepresentable.
- Keep network, storage, time, randomness, process APIs, and DOM measurement out of deterministic domain units.
- Name domain units after the result they own, not a page, component, or framework lifecycle.

## State and workflow

Use a transition model only for long-lived state or multi-event interaction. Each transition defines its guard, next state, commands, observable output, invariant result, and cancellation path.

```ts
interface TransitionResult<State, Command> {
	readonly commands: readonly Command[];
	readonly state: State;
}

declare function transition<State, Event, Command>(
	state: State,
	event: Event,
): TransitionResult<State, Command>;

declare function observe<State, Observable>(state: State): Observable;
```

- Normalize pointer data, measurements, time, and external responses into explicit events.
- Return effects as commands and apply their results through later events.
- Define invalid, repeated, stale, out-of-order, interrupted, and cancelled events.
- Keep authoritative state unchanged during a reversible preview unless the contract explicitly owns optimistic state.
- Evaluate invariants after every accepted transition, not only after final commit.
- Make state, events, commands, and observations serializable for correctness-gated behavior, or provide a lossless canonical trace.
- Test complete transition sequences, including reversal, interruption, cancellation, rollback, and re-entry.

A workflow owns phase order, short-circuiting, retry, recovery, and effects; domain units own decisions.

| Phase     | Consumes                            | Produces                       | Effect policy            |
| --------- | ----------------------------------- | ------------------------------ | ------------------------ |
| Normalize | Untrusted or legacy input           | Canonical domain input         | Boundary validation only |
| Decide    | Canonical input and semantic intent | Decision or projection         | Deterministic            |
| Prepare   | Decision plus external capabilities | Prepared data or typed failure | Effects allowed          |
| Present   | Prepared domain result              | Public output model            | No new domain decisions  |

- Give every phase the smallest capability and typed input/output it needs.
- Document every writer if a mutable workflow context is unavoidable.
- Never convert an unexpected failure into empty success unless the output type defines degraded behavior.
- Test deterministic phases directly and the orchestrator separately for order, calls, short-circuiting, retry, recovery, and error mapping.

## Test and interaction design

Write the decision inventory, scenario matrix, and domain-core tests before production UI integration. Exercise the principal public operation rather than private helpers.

| Coverage dimension  | Required cases                                                                  |
| ------------------- | ------------------------------------------------------------------------------- |
| Equivalence classes | Every meaningfully different valid state                                        |
| Boundaries          | Empty, singleton, first, last, threshold, minimum, and maximum where applicable |
| Invalid input       | Rejected identity, relationship, transition, and stale reference                |
| Direction           | Forward and reverse when direction should not change semantics                  |
| Sequence            | Repetition, interruption, cancellation, rollback, and re-entry                  |
| Invariants          | Properties that survive every accepted operation or transition                  |
| Ownership           | Caller updates, delayed commits, and rejected optimistic assumptions            |
| Ambiguity           | Zero, one, and multiple matches plus tie-breaking                               |
| Hierarchy           | Supported depths and missing, empty, partial, and complete branches             |
| Regression          | One failing-before and passing-after case for every defect                      |

- Derive cases from independent decisions and conflict points; do not use happy-path coverage or a blind Cartesian product.
- Use domain-language test names, valid minimal fixture defaults, table-driven cases, and property assertions where appropriate.
- Keep core tests independent of rendering, browser automation, network services, and persistent storage.
- A disposable spike may answer one feasibility question; it never becomes production by default.

Model the complete gesture timeline when activation, preview, commit, cancellation, focus, geometry, or motion changes behavior.

| Dimension       | Required decision                                                    |
| --------------- | -------------------------------------------------------------------- |
| Activation      | Pointer or keyboard threshold and selection timing                   |
| Hit testing     | Eligible targets, overlap, and boundary hysteresis                   |
| Geometry        | Measurement owner, coordinate space, scrolling, and resize behavior  |
| Stacking        | Paint order during every interaction phase                           |
| Preview         | Reversible projection, insertion feedback, and membership indication |
| Focus and menus | Focus ownership, dismissal, click-through, and keyboard continuity   |
| Motion          | Displacement, interruption, reduced motion, and completion           |
| Cancellation    | Escape, lost capture, unmount, stale data, and platform cancellation |

Define priority among active, hover, focus, selected, and dragging states. Feedback must describe the result of committing at that moment; styling and animation must not hide an invalid or ambiguous transition.

Test placement, naming, and regression rules follow [`TESTING.md`](TESTING.md).

## Correctness escalation

Use every method selected by an applicable risk row.

| Risk trigger                                                                          | Required evidence                                                                                |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Owned domain state or transitions                                                     | Explicit state, events, outputs, invariants, deterministic core tests, and evidence traceability |
| Large combinations of state, ordering, hierarchy, or event sequences                  | Stateful property-based tests with shrinking and replay                                          |
| Concurrency, async completion, retries, cancellation, or independently ordered events | TLA+ safety checks and applicable progress checks                                                |
| Critical pure transformation requiring a claim beyond sampled or bounded states       | Lean preservation, determinism, or refinement theorem                                            |
| Source-compatible port or behavioral replica                                          | Pinned reference, explicit parity scope, and differential or refinement verification             |
| Gestures, timing, animation, geometry, focus, or platform integration affect behavior | Deterministic system traces in a controlled integration environment                              |

| Method                    | Claim supported                                                            | Limitation                                         |
| ------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------- |
| Example test              | A selected case produces its expected result                               | Covers only selected cases                         |
| Property test             | Invariants survive generated states and sequences                          | Samples a configured search space                  |
| TLA+                      | The abstract machine has no checked safety or progress counterexample      | Checks the model within declared bounds            |
| Lean                      | A theorem holds for all values admitted by its definitions and assumptions | Does not prove unrelated TypeScript                |
| Deterministic system test | The integrated system reproduces a controlled observable timeline          | Covers declared traces and environments            |
| Differential verification | The implementation agrees with an adopted reference projection             | Depends on reference scope and observation quality |

Evidence must form a mapped refinement chain:

`reference evidence -> owned contract -> abstract model and theorems -> deterministic core -> platform adapters -> observable system`

Each arrow needs an explicit mapping or executable comparison.

## Decision boundaries

Before changing behavior after an observed mismatch:

1. Record reference provenance, environment, canonical initial state, semantic actions, and first divergence.
2. Identify discrete modes, ownership, selection, focus, interaction phase, platform switches, and continuous inputs.
3. Define precedence when outputs compete for the same resource.
4. Partition every guard and threshold; for boundary `b`, include `b - 1`, `b`, `b + 1`, and adjacent interior values when valid.
5. Define canonical semantic state, controls, geometry, focus, overlap, and clipping for every partition and transition.
6. Make the lowest effective evidence fail, then fix the earliest owning rule.

Property tests cover the supported continuous domain by equivalence class, named examples pin boundaries, and deterministic traces confirm that adapters preserve the projection. Visibility, placement, hit testing, and accessibility for one control must derive from one semantic policy or demonstrably equivalent projections.

A focused checker is local evidence; run the full applicable gate after it passes. Treat a broader regression as a counterexample until the contract, priority, boundary, or adapter mapping explains it.

## Correctness evidence

### TLA+

- Model the smallest semantic abstraction that exposes the claimed safety or progress failure; include UI or platform details only when they change the claim.
- Define all authoritative variables, exact supported `Init` states, named semantic actions and completions, `Next`, stuttering, structural and domain invariants, applicable progress properties, and only contract-guaranteed fairness.
- Provide a checked TLC configuration with explicit constants, bounds, symmetry, state constraints, and mappings from actions and properties to owned claims.
- Consider identity, referential integrity, exclusive membership, ordering, partition, contiguity, active-item validity, preview reversibility, atomic commit, cancellation restoration, and ownership invariants.
- Never use a state constraint to exclude a reachable failure; declare exploration limits.
- Cover both directions, boundaries, empty and singleton states, maximum modeled size, relevant modes, repeated or stale actions, cancellation, reordered completions, and phase re-entry.
- Convert the shortest TLC counterexample into a deterministic regression; fix a missing contract or model before production code.
- Pin the toolchain. The merge configuration must exercise every maintained action and safety property within documented bounds; larger discovery configurations may run separately.

### Lean

- Define formal inputs, outputs, validity predicates, the transformation or transition, named theorems, minimal assumptions, and any executable oracle needed for refinement.
- Use Lean for universal claims about critical identity, membership, ordering, grouping, selection, normalization, determinism, inverse behavior, or invariant preservation.
- Do not use `sorry`, `admit`, or project-created axioms for required theorems; disclose unavoidable axioms, classical assumptions, foreign code, and parser trust.
- Bridge Lean and production code through a shared canonical schema or corpus and compare canonical observations.
- Lean is the default proof assistant for new conformance; an established alternative must provide equivalent kernel checks, trust disclosure, refinement, and merge guarantees.

### Property-based testing

- Define valid-state and stateful-command generators, model-derived preconditions, adversarial invalid or stale events, an oracle or invariant set, semantic shrinkers, coverage classifications, and stable serialization.
- Apply preservation, determinism, idempotence, inverse, symmetry, refinement, and no-op stability properties where relevant.
- Use explicit seeds, size limits, deterministic schedulers, and logged parameters; the merge gate owns a stable seed corpus.
- Reduce every discovery failure to a replayable minimal case and add it to the regression corpus.
- Report reached state classes, actions, boundaries, directions, and phases; iteration count alone is not coverage.

### Deterministic system testing

Control or record every applicable source of nondeterminism:

| Source              | Required control                                                                                      |
| ------------------- | ----------------------------------------------------------------------------------------------------- |
| Time and scheduling | Wall and monotonic clocks, timers, frames, tasks, microtasks, async completion order, and randomness  |
| Environment         | Viewport, scale, zoom, fonts, locale, theme, motion preference, platform flags, and generated IDs     |
| Interaction         | Pointer and keyboard sequence, coordinates, capture, cancellation, coalescing, focus, and animation   |
| Geometry            | Measurements, scrolling, hit-test candidates, resize observations, and snapshots                      |
| External state      | Network, storage, worker, process, operating-system responses, persisted state, and reference version |

- Record semantic checkpoints for model state, phase, commands, projection, focus, geometry, and rendered identifiers.
- Advance logical time or wait for named semantic conditions; never use arbitrary sleeps.
- Drive animation with fixed logical frames and assert start, displacement, interruption, and settlement.
- Compare canonical state and geometry before pixels; make every trace independently replayable by stable name.
- Retries cannot hide a race. Manual observation is discovery evidence until captured by an asserted deterministic trace.

### Reference refinement

A compatibility or replication claim needs a versioned parity contract.

| Field         | Required content                                                                            |
| ------------- | ------------------------------------------------------------------------------------------- |
| Provenance    | Product version, source revision, build flags, platform, theme, and runtime configuration   |
| Scope         | Supported operations, states, inputs, modes, and explicit exclusions                        |
| Projection    | Semantic state, checkpoints, geometry, accessibility, timing, and required painting details |
| Normalization | Declared nondomain noise and why removing it cannot hide a behavioral difference            |
| Authority     | Conflict rule for source, observed behavior, upstream tests, and the locally owned contract |

- Treat reference behavior as selected by revision, flags, channel, platform, experiments, and runtime configuration.
- Audit the behavioral dependency closure: policies, ownership, lifetime, constants, flags, scheduling, geometry, hit testing, animation, platform integration, accessibility, tests, and shared state.
- For each scenario, establish a clean precondition, apply the same semantic actions, capture the same checkpoints, canonicalize only declared noise, compare the full projection, and classify every mismatch.
- Source inspection and exploratory use may discover behavior; merge evidence requires a repeatable harness or versioned golden trace with recorded provenance and capture.
- Re-capture deliberately. A reference upgrade requires a dependency-closure audit, scenario rerun, golden review, and parity-scope update.

## Traceability and change discipline

Maintain one row per owned claim.

| Claim              | Contract owner           | TLA+ property         | Lean theorem          | Property test    | System trace      | Reference scenario   | Limits                 |
| ------------------ | ------------------------ | --------------------- | --------------------- | ---------------- | ----------------- | -------------------- | ---------------------- |
| Domain-named claim | Owning reference section | Name or justified N/A | Name or justified N/A | Stable test name | Stable trace name | Stable scenario name | Bounds and assumptions |

- Give every observable rule and invariant an executable witness at the lowest effective layer.
- Map every formal property and theorem to an owned claim; use stable names and explain every N/A.
- Record bounds and trust assumptions; remove stale rows with removed behavior.
- Treat coverage percentages as diagnostics, not traceability.

For every counterexample:

1. Preserve environment, seed, state, events, reference version, and observations.
2. Reduce it to the shortest failing semantic trace.
3. Find the first divergent checkpoint and violated claim or assumption.
4. Classify the missing contract, invalid assumption, incomplete generator, implementation, adapter, or harness defect.
5. Fix the earliest incorrect authority.
6. Retain the reduced case and rerun broader exploration.

Never weaken a property, exclude a reachable TLA+ state, add an axiom, retry a race, or replace a semantic regression with a screenshot to accept current behavior.

| Change kind                                     | Required updates                                                                                                 |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| State, event, guard, command, or cancellation   | Owned contract, state model, applicable TLA+ artifacts, command model, tests, and deterministic traces           |
| Pure transformation or invariant                | Owned contract, applicable Lean definitions and theorems, property oracle, and regression corpus                 |
| Platform, geometry, focus, timing, or animation | Environment contract, deterministic inputs and checkpoints, adapter tests, and parity projection when applicable |
| Reference revision or compatibility scope       | Provenance, dependency-closure audit, differential scenarios, reviewed goldens, and declared limits              |
| Defect                                          | Failing reduced counterexample, root-cause class, corrected earliest owner, and passing evidence                 |

Update the abstract model or proof before relying on changed production behavior. Do not preserve competing semantic models unless an explicit compatibility boundary owns both and differential tests compare them.

Every implementation follows the same evidence contract:

- Before production edits, retrieve the owning record, identify any reference dependency closure, list state, events, outputs, invariants, assumptions, selected evidence, and affected refinement links.
- Reproduce an existing defect as a minimal semantic trace.
- Run the smallest checker after each semantic change, then the full applicable gate; preserve and classify counterexamples before another patch.
- Review mappings from contract to formal model, formal model to deterministic core, and core observation to system or reference trace.
- Completion reports name checked claims, bounds, reference revision, executed evidence, and exclusions; file inspection, generated code, examples, or screenshots alone never establish completeness.

## Integration and implementation

Integrate the stable core through a thin shell:

- Components render prepared state and emit semantic intent.
- Hooks coordinate framework lifecycles and adapters without owning hidden state machines.
- CSS renders model-supplied presentation state and never infers domain state.
- Effects consume explicit commands and report results through explicit events.
- The shell may measure the DOM, capture pointers, schedule frames, and render pixels; it never decides membership, ordering, selection, eligibility, cancellation, or commit semantics.
- Component tests verify wiring and accessibility, browser tests verify real timelines, and visual review verifies presentation; none replaces domain evidence.

Implementation order:

1. Record the problem, non-goals, inputs, outputs, invariants, and ownership.
2. Define canonical types, decisions, model shape, and principal operation.
3. Add transitions, workflow phases, and correctness claims when applicable.
4. Build the scenario matrix and select evidence from every matching risk row.
5. Implement the deterministic core and make its evidence pass.
6. Implement and test workflows and external adapters.
7. Integrate the stable operation into the application shell.
8. Add deterministic browser, visual, formal, and differential evidence where selected.

Return to the earlier artifact whenever a later step exposes an unresolved decision.

React ownership follows [`frontend/react.md`](frontend/react.md), visual decisions follow [`frontend/visual-foundations.md`](frontend/visual-foundations.md), and cross-workspace dependencies follow [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Verification and completion

Every deterministic, bounded, pinned, and repository-provisioned evidence artifact runs in the root merge gate defined by [`VERIFICATION.md`](VERIFICATION.md). The owning package documents and exposes its real commands.

The applicable gate must fail on:

- a checked TLA+ safety or progress counterexample;
- an unchecked or admitted Lean theorem or undeclared trust assumption;
- a failed property, unreachable required generator class, or non-replayable generated failure;
- a deterministic trace mismatch or result-affecting uncontrolled timing;
- a differential mismatch inside the declared parity scope; or
- missing traceability for a changed claim.

Pin tools and configurations, cap resources, fail fast, and retain actionable counterexamples. Do not use retries. External reference capture and environment-specific exploration remain maintenance verification until their inputs are pinned and provisioned; run them before compatibility releases without making the merge gate depend on ambient external state.

Production integration is ready when:

- the modeling record defines the problem, outputs, model, ownership, invariants, dependencies, transitions, scenario matrix, and unresolved decisions;
- deterministic core tests pass without the production UI; and
- every correctness trigger has a selected evidence plan and explicit limits.

A feature is complete when:

- the owning authority describes current behavior without task history;
- core, workflow, integration, browser, accessibility, and selected correctness evidence protect their owned claims;
- every regression has a failing-before and passing-after case at the lowest effective layer;
- the traceability matrix has no unexplained gaps or stale claims;
- prototypes, duplicate state, and obsolete flags are removed;
- the root [`pnpm verify`](VERIFICATION.md#merge-gate) gate passes without retries; and
- the completion report states checked claims, bounds, reference revision, evidence, and exclusions without unbounded claims such as “fully verified” or “perfect replica.”
