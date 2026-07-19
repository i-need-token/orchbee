# Feature Development

## Modeling gate

Do not begin production integration for a feature until its model answers the following questions in a reviewable form. The required artifact may be a proposal, an owning package reference, a public package contract, types plus tests, or a combination of them; do not create a one-off task document that has no durable owner.

| Question                      | Required answer                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------- |
| What problem is being solved? | Actor, context, desired outcome, constraints, and explicit non-goals            |
| What are the outputs?         | Observable results, semantic events, failures, cancellation, and no-op behavior |
| What is the data structure?   | Entities, identifiers, relationships, ownership, lifecycle, and invariants      |
| How does state move?          | States, events, guards, transitions, derived state, and side effects            |
| Can the core be tested first? | A scenario matrix and deterministic tests that do not require the production UI |

The modeling gate is required when behavior coordinates multiple states, owns ordering or grouping, crosses a persistence or process boundary, includes asynchronous work, or interprets a multi-step user gesture. A local presentational change may keep its model in existing component contracts and tests when it introduces no new domain state or transition.

- Put future behavior under `proposals/` until its decisions are accepted.
- Put current public behavior in the owning package README and current internal maintenance contracts in an owning package reference.
- Put only reusable cross-workspace rules in `docs/`; a repository standard must not inventory one feature's states or screen behavior.

## Modeling record

Before production implementation, create one compact modeling record in the authority that will own the decision. Complete it with concrete domain names and types; a heading with no decision does not pass the gate.

| Field        | Required content                                                          | Completion evidence                       |
| ------------ | ------------------------------------------------------------------------- | ----------------------------------------- |
| Problem      | Actor, trigger, desired outcome, constraints, and non-goals               | One observable problem statement          |
| Inputs       | Trusted and untrusted inputs plus absence semantics                       | Named input types and validation boundary |
| Outputs      | Success variants, failures, events, side effects, cancellation, and no-op | Named output types                        |
| Invariants   | Facts that must remain true before and after every operation              | Assertions or invariant tests             |
| Decisions    | Independent dimensions that change the result                             | A decision table or scenario matrix       |
| Model shape  | Extractor, normalizer, policy, builder, state machine, or workflow        | One selected shape with a reason          |
| Dependencies | Time, storage, network, process, DOM, and other capabilities              | Minimal capability interface              |
| Ownership    | Authoritative state owner and allowed writers                             | One owner for each mutable fact           |
| Verification | Core cases, integration boundaries, and real interaction paths            | Test list mapped to decisions             |

The record is complete only when another maintainer can derive the public domain operation and its test cases without inspecting the intended UI implementation.

## Problem contract

Define the problem in user-observable terms before naming components, Hooks, storage keys, or implementation techniques.

- Identify who encounters the problem and in which context.
- State the outcome that changes for that actor.
- Record constraints that materially restrict a valid solution.
- State non-goals so adjacent behavior does not enter the feature implicitly.
- Define how success, failure, cancellation, and intentional no-op behavior are observed.

- ❌ Build a draggable list component that looks like the reference.
- ✅ Let a user reorder owned items while preserving selection, membership, and cancellation invariants.

## Output contract

List every externally observable output before choosing an internal representation.

| Output kind    | Required decision                                                          |
| -------------- | -------------------------------------------------------------------------- |
| Returned value | Shape, meaning, ordering, and absence semantics                            |
| Semantic event | Name, payload, emission point, idempotency, and owner                      |
| State change   | Authoritative owner, commit point, and rollback or cancellation behavior   |
| Rendered state | Meaningful visual, focus, accessibility, and reduced-motion result         |
| Side effect    | Target system, timing, retry behavior, failure handling, and observability |
| No-op          | Guard that rejects work and whether the rejection is visible               |

Prefer semantic outputs such as an item identifier and target position over DOM coordinates or component-local flags. A caller-owned model remains authoritative until the owning contract explicitly transfers that responsibility.

## Domain model shape

Choose the smallest model shape that matches the problem. Do not introduce a state machine when the domain is a deterministic transformation, and do not hide a multi-phase workflow inside a builder.

| Shape                    | Use when                                                          | Public operation                | Side effects         |
| ------------------------ | ----------------------------------------------------------------- | ------------------------------- | -------------------- |
| Extractor or parser      | Meaning must be derived from an encoded input                     | `extract` or `parse`            | None                 |
| Normalizer               | External or legacy data must become one canonical domain form     | `normalize`                     | None                 |
| Policy or resolver       | One decision must be made from explicit facts                     | `resolve` or `decide`           | None                 |
| Builder or projector     | Several canonical inputs produce one complete read model          | `build` or `project`            | None by default      |
| State machine or reducer | Events change a long-lived state through guarded transitions      | `transition` or `reduce`        | Returned as commands |
| Workflow or orchestrator | Ordered phases coordinate effects, short-circuiting, and recovery | `execute`, `load`, or `prepare` | Explicitly allowed   |

Name the domain unit after the result it owns, not the page, framework lifecycle, or current component that happens to call it.

## Data model

Model the smallest domain that can express every valid case and reject invalid combinations.

- Give durable entities stable identifiers and define relationship cardinality.
- Name the single authoritative owner of each mutable fact.
- Separate authoritative state, transient projections, derived values, and presentation state.
- State ordering, uniqueness, membership, lifecycle, and referential-integrity invariants.
- Define boundary parsing and absence semantics explicitly; do not use coincidental equality between two optional values as a state transition.
- Prefer discriminated unions when independent booleans could represent impossible combinations.

```ts
// ❌ "idle with a group" and "dragging without a group" are representable.
interface DragState {
	readonly isDragging: boolean;
	readonly groupId?: string;
}

// ✅ Each state carries exactly the data that it requires.
type DragState =
	{ readonly kind: 'idle' } | { readonly groupId: string; readonly kind: 'dragging-group' };
```

If the model cannot explain which owner may change a value or which combinations are valid, implementation must not compensate with component-local state or CSS selectors.

## Domain unit contract

Each domain unit exposes a narrow, typed public surface whose constructor or entry function establishes all prerequisites needed by the main operation.

```ts
interface Item {
	readonly id: string;
}

interface CanonicalInput {
	readonly items: readonly Item[];
}

interface Selection {
	readonly itemId: string | undefined;
}

interface BuildDependencies {
	readonly resolvePath: (itemId: string) => string;
}

type BuildResult =
	| { readonly kind: 'empty' }
	| { readonly items: readonly Item[]; readonly kind: 'ready'; readonly selectedId: string };

declare class ItemProjectionBuilder {
	constructor(input: CanonicalInput, dependencies: BuildDependencies);
	build(selection: Selection): BuildResult;
}
```

- Make required data nonoptional at the domain-unit boundary; reject a missing prerequisite once instead of branching on it throughout the operation.
- Accept a dedicated capability interface. `Pick<Context, ...>` is acceptable at an integration boundary only when it exposes the exact methods the unit may use.
- Keep the principal public operation small and explicit. Secondary public queries require their own output contract.
- Use private helpers to name independent domain decisions, not merely to shorten a long method.
- Return a complete domain result whose variants make invalid field combinations unrepresentable.
- Normalize remote, persisted, legacy, and framework-owned types before they enter builders, policies, or state machines.
- Do not import network or storage services into a deterministic extractor, normalizer, policy, or builder.

## State transition model

Use a state transition model only when the feature owns long-lived state or a multi-event interaction. Represent behavior as events applied to explicit states. For each transition, record its guard, next state, semantic output, side effects, and cancellation path.

| Current state   | Event           | Guard                          | Next state             | Output                      |
| --------------- | --------------- | ------------------------------ | ---------------------- | --------------------------- |
| Idle            | Start           | Target is eligible             | Armed                  | None                        |
| Armed           | Cross threshold | Gesture remains valid          | Active                 | Activation event            |
| Active          | Move            | Candidate satisfies invariants | Active with projection | Preview only                |
| Active          | Commit          | Projection remains valid       | Idle                   | Semantic move               |
| Armed or active | Cancel          | Always                         | Idle                   | Restore authoritative state |

- Keep the authoritative model unchanged while a reversible preview is active unless the contract explicitly defines optimistic ownership.
- Derive visual state from the current model and interaction phase; do not use DOM presence or CSS as the source of business truth.
- Specify repeated, out-of-order, interrupted, and stale events when the environment can produce them.
- Test transition sequences, not only isolated final states.

## Domain core

Implement domain decisions before connecting them to the production interface. Prefer a pure function for extraction, normalization, and policy decisions; use a class when construction establishes prerequisites, preserves a domain cache, or groups several operations over the same canonical input. Deterministic domain units must return the same result for the same validated inputs.

```ts
interface TransitionResult<State, Command> {
	readonly commands: readonly Command[];
	readonly state: State;
}

declare function transition<State, Event, Command>(
	state: State,
	event: Event,
): TransitionResult<State, Command>;
```

- Keep time, randomness, storage, network, process APIs, and DOM measurement behind explicit capability boundaries.
- Keep ordering, membership, eligibility, conflict, and cancellation rules out of components and event handlers.
- Return semantic results; adapters translate them into framework callbacks and external effects.
- Treat a rule that can only be explained through current markup or selector order as evidence that the model is incomplete.

## Workflow contract

A workflow owns sequencing and effects; its phases delegate decisions to domain units. Record the phase order before implementing the orchestrator.

| Phase     | Consumes                            | Produces                       | Effect policy            |
| --------- | ----------------------------------- | ------------------------------ | ------------------------ |
| Normalize | Untrusted or legacy input           | Canonical domain input         | Boundary validation only |
| Decide    | Canonical input and semantic intent | Decision or projection         | Deterministic            |
| Prepare   | Decision plus external capabilities | Prepared data or typed failure | Effects allowed          |
| Present   | Prepared domain result              | Public output model            | No new domain decisions  |

- The orchestrator is the only owner of phase order, short-circuiting, retry, and recovery.
- Each phase declares the smallest capabilities it reads or writes; do not pass a full application store or request context by convenience.
- Prefer phase-specific input and output types. If a mutable workflow context is necessary, document every writer, give readers a narrow capability view, and fail at the phase boundary when required data is absent.
- Never catch an unexpected failure and fabricate an empty success value unless the output type explicitly models degraded behavior.
- Test deterministic phases directly and test the orchestrator separately for ordering, effect calls, short-circuiting, and error mapping.

## Test design gate

Write the decision inventory, scenario matrix, and domain-core tests before production UI integration. The core test suite must pass independently of rendering, browser automation, network services, and persistent storage.

| Coverage dimension  | Required cases                                                                  |
| ------------------- | ------------------------------------------------------------------------------- |
| Equivalence classes | One representative for each meaningfully different valid state                  |
| Boundaries          | First, last, empty, singleton, threshold, maximum, and minimum where applicable |
| Invalid input       | Rejected identity, relationship, transition, and stale reference                |
| Direction           | Forward and reverse operations when direction should not change semantics       |
| Sequence            | Repetition, interruption, cancellation, rollback, and re-entry                  |
| Invariants          | Properties that remain true across every accepted transition                    |
| Ownership           | Caller-controlled updates, delayed commits, and rejected optimistic assumptions |
| Ambiguity           | Zero, one, and multiple matches plus the tie-breaking rule                      |
| Hierarchy           | Every supported depth and missing, empty, partial, and complete branches        |
| Regression          | One case that fails before every bug fix and passes after it                    |

- Derive the matrix from independent decision dimensions before writing cases. Cover meaningful combinations and conflict points; do not rely on happy-path line coverage or blindly generate an irrelevant Cartesian product.
- Name cases in domain language so the suite reads as a decision tree: input condition, selected rule, and expected outcome.
- Exercise the principal public operation and assert its complete typed output. Do not test private helpers as a substitute for the domain contract.
- Give fixture builders valid minimal defaults, then override only the decision dimension under test.
- Test missing prerequisites and invalid boundary data at the point where the domain unit promises to reject them.
- Prefer table-driven tests for compact matrices and property-style assertions for ordering, uniqueness, and membership invariants.
- A disposable interaction spike may precede the core only to answer an unresolved feasibility question; it must not silently become production implementation.

Test placement, naming, and regression requirements follow [`docs/TESTING.md`](TESTING.md).

## Interaction modeling

An interactive feature models the full gesture timeline, not only the state after release. At minimum, distinguish idle, armed, active, preview, commit, and cancel phases when those phases produce different behavior.

| Dimension       | Required decision                                                     |
| --------------- | --------------------------------------------------------------------- |
| Activation      | Pointer or keyboard threshold and the moment selection changes        |
| Hit testing     | Eligible targets, overlap rules, and boundary hysteresis              |
| Geometry        | Measurement owner, coordinate space, scrolling, and resizing behavior |
| Stacking        | Which surface paints above another during every interaction phase     |
| Preview         | Reversible projection, insertion feedback, and membership indication  |
| Focus and menus | Focus ownership, dismissal, click-through, and keyboard continuity    |
| Motion          | Displacement, interruption, reduced motion, and completion behavior   |
| Cancellation    | Pointer cancellation, escape, lost capture, unmount, and stale data   |

- Test both directions whenever the operation is expected to be symmetric.
- Keep preview and commit distinct so moving across a candidate does not mutate authoritative data.
- Define active, hover, focus, selected, and dragging priority when their visual states overlap.
- Make feedback describe the result that would occur if the user committed at that moment.
- Do not use animation or styling to hide an ambiguous transition or invalid state.

## Reference and novel behavior

- Treat an existing product as evidence, not as the feature specification. Record the observed scenario, infer the rule, and decide explicitly whether the product adopts or rejects it.
- Compare interaction timelines and state changes, not screenshots alone.
- When no mature reference exists, create a disposable prototype and review the problem, outputs, model, transition table, and scenario matrix before production integration.
- Record unresolved behavior as a decision that blocks implementation; do not let incidental prototype behavior become the contract.
- Re-run the same controlled scenarios against the reference, prototype, and integrated feature when behavioral parity is a goal.

- ❌ Copy visible details first and discover the underlying rules through regressions.
- ✅ Establish an owned behavior contract, validate difficult transitions in a prototype, then implement the stable core.

## Integration boundary

Integrate the stable core through a thin shell that translates platform input into semantic events and renders the resulting model.

- Components render prepared state and emit semantic intent; they do not recreate domain decisions.
- Hooks coordinate framework lifecycles and adapters; they do not become an implicit state machine spread across Effects.
- CSS represents presentation state supplied by the model; selectors do not infer domain state from missing values or sibling order.
- External effects consume explicit commands after a transition and report results through explicit events.
- Component tests verify wiring and accessibility, browser tests verify real gesture timelines, and visual review verifies presentation; none replaces domain-core coverage.

React ownership follows [`docs/frontend/react.md`](frontend/react.md), visual decisions follow [`docs/frontend/visual-foundations.md`](frontend/visual-foundations.md), and cross-workspace dependencies follow [`docs/ARCHITECTURE.md`](ARCHITECTURE.md).

## Change discipline

- Classify every defect as a missing model rule, a missing scenario, or an implementation violation before editing production code.
- Add the missing invariant and a failing core or interaction test before applying the production fix.
- Treat a fix that changes behavior outside its target scenario as a stop signal: restore a coherent model and test matrix instead of adding another conditional flag.
- Stop integration when the authoritative owner, transition guard, cancellation behavior, or expected output is unclear.
- Remove obsolete projections and flags after a model change; parallel representations of the same fact are not a migration strategy unless an explicit boundary requires them.
- Revisit the abstraction when repeated defects cross state combinations, directions, or interaction phases; repeated local patches are evidence that the current boundary is wrong.

- ❌ Add one more component boolean because the current screenshot is wrong.
- ✅ Identify the missing state or invariant, extend the core and scenario matrix, then update the projection.

## Implementation sequence

Use this order for every modeled feature:

1. Write the problem, non-goals, inputs, and observable outputs.
2. Define canonical input, output, error, identifier, and invariant types.
3. Choose the domain model shape and name its principal public operation.
4. Inventory independent decisions and build the scenario matrix.
5. Implement the deterministic domain units and make their tests pass.
6. Implement and test the workflow or external adapters when effects are required.
7. Integrate the stable public operation into the application or component shell.
8. Add browser and visual verification for behavior that exists only in a real interaction environment.

If a later step reveals an unresolved earlier decision, return to that artifact and update its tests before continuing. Do not encode the answer only in the integration layer.

## Definition of ready

Production integration may begin only when:

- the problem, actor, outcome, constraints, and non-goals are explicit;
- observable outputs, failures, cancellation, and no-op behavior are defined;
- the selected model shape and principal public operation match the problem;
- the data model names ownership, lifecycle, absence semantics, and invariants;
- required dependencies are expressed as minimal capabilities and workflow phases are ordered;
- when the feature is stateful, the transition model covers guards, previews, commits, side effects, and cancellation;
- the scenario matrix covers equivalence classes, boundaries, ambiguity, hierarchy, sequences, and symmetry;
- the deterministic core and its tests pass without the production UI; and
- unresolved reference or prototype behavior has been converted into explicit decisions.

## Definition of done

A feature is complete only when:

- the owning current authority describes the implemented contract without task history;
- domain-core tests protect every owned rule, decision dimension, and invariant;
- workflow tests protect phase order, effect boundaries, short-circuiting, and failure mapping;
- integration tests protect framework boundaries and accessibility;
- browser-level tests protect meaningful multi-step gestures and cancellation paths;
- every discovered regression has a failing-before, passing-after test at the lowest effective layer;
- temporary prototype behavior, duplicate state, and obsolete flags have been removed; and
- the root [`pnpm verify`](VERIFICATION.md#merge-gate) gate passes.
