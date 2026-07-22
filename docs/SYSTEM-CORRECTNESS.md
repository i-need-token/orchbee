# System Correctness

## Correctness gate

Do not claim a modeled subsystem complete until it has a reviewable correctness case and every method required by the following risk triggers has executable evidence. Passing example-based tests, matching a screenshot, translating the reference file by file, or completing a source checklist does not by itself establish correctness.

| Risk trigger                                                                                   | Required evidence                                                                                |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Any behavior that owns domain state or transitions                                             | Explicit state, events, outputs, invariants, deterministic core tests, and evidence traceability |
| Large combinations of state, ordering, hierarchy, or event sequences                           | Stateful property-based tests with shrinking and replay                                          |
| Concurrency, asynchronous completion, retries, cancellation, or independently ordered events   | A TLA+ model with checked safety properties and applicable progress properties                   |
| A pure critical transformation whose correctness must hold beyond sampled or bounded states    | Lean definitions and machine-checked preservation, determinism, or refinement theorems           |
| A source-compatible port or behavioral replica                                                 | A pinned reference, an explicit parity scope, and differential or refinement verification        |
| Behavior that depends on gestures, timing, animation, geometry, focus, or platform integration | Deterministic system traces in a controlled real integration environment                         |

Use all rows that apply. Complexity in one row does not waive another row. A source-compatible interactive state machine normally needs TLA+, property-based testing, deterministic system testing, and reference refinement; use Lean for its critical pure ordering, grouping, selection, normalization, or transition transformations when universal claims are required.

Keep future adoption or migration work under `proposals/` until it is implemented. Put the current correctness case beside the owning package contract or internal reference. This repository standard defines the reusable method and must not inventory one feature's states, upstream files, or test cases.

## Correctness case

Create one durable correctness case for every subsystem that passes the correctness gate. It is the index of the argument that the implementation satisfies its owned contract; it is not a narrative of implementation work.

| Field          | Required content                                                                                         |
| -------------- | -------------------------------------------------------------------------------------------------------- |
| Scope          | Actors, operations, observable behavior, supported environment, version, and explicit exclusions         |
| Authority      | Owned behavior contract, reference evidence, and the rule used when they disagree                        |
| Model          | Authoritative state, semantic events, transition relation, commands, and observable projection           |
| Claims         | Safety invariants, progress properties, algebraic laws, and allowed failures or no-ops                   |
| Trust boundary | Platform behavior, parsers, foreign code, measurement, and assumptions not proved locally                |
| Refinement     | How formal states and actions correspond to implementation states, events, and observations              |
| Evidence       | TLA+ properties, Lean theorems, property names, deterministic traces, and differential scenarios         |
| Limits         | Model-checking bounds, generated-case limits, unsupported reference behavior, and unverified assumptions |

The correctness case must let a reviewer answer three questions without reading the UI code: what is claimed, which machine-checkable artifact supports each claim, and what remains outside the claim.

Use precise language:

- TLA+ model checking establishes that no counterexample exists in the explored model and bounds; it does not prove the TypeScript implementation or unbounded execution.
- Lean proves theorems about the definitions and assumptions encoded in Lean; it does not prove a separately written TypeScript implementation without a refinement bridge.
- Property-based testing explores generated examples; it does not establish a universal theorem.
- Deterministic system testing establishes reproducible behavior for controlled traces and environments; it does not enumerate every environment.
- Differential verification establishes agreement on a declared observable projection; it does not establish that the reference is correct outside the adopted parity contract.

## Method selection

Select methods by the questions they can answer. Do not substitute one layer merely because it is easier to add.

| Method                    | Question answered                                                                     | Best fit                                                               | Principal limitation                                          |
| ------------------------- | ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------- |
| Example-based test        | Does this known case produce the expected result?                                     | Named regressions and boundary examples                                | Finds only cases someone selected                             |
| Property-based test       | Do invariants survive many generated states and sequences?                            | Combinatorial data and stateful commands                               | Samples a configured search space                             |
| TLA+                      | Can an abstract state machine violate safety or progress under allowed interleavings? | Workflows, interaction phases, concurrency, cancellation, and ordering | Checks the model, normally within finite bounds               |
| Lean                      | Does a theorem follow for all values admitted by the formal definitions?              | Pure transformations and invariant preservation                        | Proves the formal definition, not an unrelated implementation |
| Deterministic system test | Does the integrated system reproduce the same observable timeline?                    | Gestures, layout, scheduling, focus, animation, and adapters           | Covers declared traces and controlled environments            |
| Differential verification | Does the implementation refine the adopted reference behavior?                        | Ports, replicas, compatibility layers, and migrations                  | Depends on reference scope and observation quality            |

The methods form a refinement chain rather than independent badges:

`reference evidence -> owned contract -> abstract model and theorems -> deterministic core -> platform adapters -> observable system`

Every arrow needs an explicit mapping or executable comparison. A proof at the left side of a missing arrow does not establish behavior at the right side.

## Behavior discovery and decision boundaries

Treat each observed mismatch as evidence of a potentially missing decision rule, not immediately as an isolated implementation or styling defect. Before changing production behavior:

1. Record the scenario's reference provenance, environment, canonical initial state, semantic actions, and first divergent observation.
2. Identify the independent decision dimensions, including discrete modes, ownership, selection, focus, interaction phase, platform switches, and continuous inputs such as width or pointer position.
3. State precedence explicitly when outputs compete for the same resource. A policy such as `status indicator > primary action > decorative icon` is one ordered decision; it must not emerge accidentally from unrelated component conditions and CSS flow.
4. Derive a finite partition from every guard, source constant, and observed transition threshold. For a boundary `b`, include `b - 1`, `b`, `b + 1`, and representative interior values from adjacent partitions when the input domain permits them.
5. Define the canonical observation at each partition and transition, including semantic state, visible controls, geometry, focus, and permitted overlap or clipping.
6. Select the lowest effective evidence layer for each claim, make the new evidence fail for the intended reason, then implement the earliest owning rule.

A continuous input does not require arbitrary screenshot sampling. Its decision procedure normally induces a finite set of equivalence classes. Property tests should range across the full supported input domain and assert the class invariant; named examples should pin every boundary; deterministic system traces should traverse the classes in the real renderer and verify that no adapter, animation, or layout engine changes the projection.

Visibility, placement, hit testing, and accessibility for the same control are one policy result. Compute them from one semantic decision or from demonstrably equivalent projections. Do not let a visibility helper, flex layout, absolute positioning rule, and pointer target independently decide whether the control exists or what it displaces.

A focused checker passing is only local evidence. After it passes, run the full applicable gate. A broader regression is a counterexample to the newly inferred rule until classified otherwise: it commonly reveals an omitted higher-priority state, a boundary partition absent from the generator, or a projection that was correct only in one adapter. Refine the contract and the earliest policy owner before adjusting downstream expectations.

## Executable state-machine boundary

Represent stateful behavior as a deterministic domain transition before integrating it with a framework or platform. The boundary must expose the semantic information required for formal models, generated commands, replay traces, and system observations.

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

- Give every durable entity a stable identity and every mutable fact one authoritative owner.
- Represent impossible phase combinations with distinct state variants, not independent booleans.
- Normalize pointer data, measurements, time, external responses, and other platform input into explicit events.
- Return effects as commands. Apply their results through later events instead of performing hidden effects inside the transition.
- Keep authoritative state separate from reversible previews, animation projections, and rendered state.
- Define the result of invalid, repeated, stale, out-of-order, and interrupted events.
- Make state, events, commands, and observations serializable or provide a lossless canonical trace representation.
- Evaluate owned invariants after every accepted transition, not only after a final commit.

The integration shell may measure the DOM, capture a pointer, schedule frames, and render pixels. It must not independently decide membership, ordering, selection, eligibility, cancellation, or commit semantics.

- ❌ A component, a drag library, and CSS each keep their own partial idea of group membership and preview position.
- ✅ One transition model owns membership and preview semantics; adapters supply measurements and render its observable projection.

The state-transition and integration rules in [`FEATURE-DEVELOPMENT.md`](FEATURE-DEVELOPMENT.md) remain authoritative. This standard adds the evidence required for high-assurance behavior.

## TLA+ specification

Use TLA+ to model semantic states and actions at the smallest abstraction that can expose the relevant safety and progress failures. Do not model React components, CSS declarations, pixels, or implementation call stacks unless one of those details changes the claimed behavior.

Each maintained specification must contain:

- variables for every authoritative fact needed by the claims;
- an `Init` predicate that admits exactly the supported initial states;
- named actions for semantic events and effect completions;
- a `Next` relation and stuttering-complete temporal specification;
- type and structural invariants plus domain safety invariants;
- applicable progress or liveness properties;
- fairness assumptions only where the owned environment contract guarantees them;
- a checked TLC configuration with explicit constants, bounds, symmetry choices, and state constraints; and
- a mapping from each formal action and property to the correctness case.

At minimum, consider invariants for identity uniqueness, referential integrity, exclusive membership, ordering or partition preservation, contiguity, active-item validity, preview reversibility, commit atomicity, cancellation restoration, and non-overlapping ownership. Use domain names in the maintained specification.

Never use a state constraint to remove a reachable bad state. State constraints may bound exploration but must be listed as limits; safety invariants must detect invalid reachable states. A model that completes only after excluding the failure being investigated is not evidence.

Model checking must cover more than the happy path:

- both directions and every meaningful boundary position;
- empty, singleton, maximum modeled, collapsed, expanded, selected, and unselected forms where applicable;
- repeated, stale, reordered, interrupted, and cancelled actions;
- effect completion before, during, and after cancellation when effects race with ownership; and
- histories long enough to leave and re-enter every interaction phase.

When TLC finds a counterexample, preserve its shortest semantic action sequence in the defect record and convert it into a deterministic implementation trace. If the trace is legal but absent from the owned contract, fix the contract and model before production code. If it violates a stated invariant, retain it as a regression at the lowest executable layer.

Pin the TLA+ toolchain and make the merge configuration reproducible. Larger discovery configurations may run outside the merge gate, but the merge gate must check a representative bounded model that exercises every action and every maintained safety property. Document both configurations and their limits.

## Lean proof

Use Lean for pure definitions and claims that must hold for all values admitted by those definitions. Prefer it for transformations whose failure would corrupt identity, membership, ordering, grouping, selection, normalization, or state-machine preservation.

Each maintained proof unit must provide:

- formal input, output, and validity predicates;
- a definition of the transformation or abstract transition being reasoned about;
- named theorems for the claims in the correctness case;
- the minimum explicit assumptions needed by each theorem; and
- executable examples or a serialized oracle when they are needed to connect Lean semantics to the production implementation.

Typical theorem classes include:

- output identities are a permutation or declared subset of input identities;
- uniqueness, partition, membership, and contiguity are preserved;
- a valid active or selected identity remains valid under every accepted operation;
- normalization is idempotent;
- inverse or cancellation operations restore the prior abstract state;
- the transition is deterministic; and
- a concrete pure operation refines an abstract relation.

Do not use `sorry`, `admit`, or project-created axioms to make a required theorem pass. Any unavoidable axiom, classical assumption, foreign-function boundary, or parser trust must be named in the correctness case and reviewed as part of the trusted computing base. Generated proof code and proof automation are acceptable only when the Lean kernel checks the resulting theorem without admitted obligations.

Lean does not automatically prove TypeScript. Bridge the two deliberately. Prefer a shared canonical input and output schema, execute the Lean definition or an extracted oracle over a maintained corpus, execute the production deterministic core over the same corpus, and compare canonical observations. Property-based tests may generate the corpus, but failures must shrink and replay independently of the generator.

Lean is the default deductive proof assistant for new conformance under this standard. A package with an established Coq or other proof-assistant codebase may keep it only when it provides the same kernel-checked claims, trust-boundary disclosure, refinement bridge, and merge-gate guarantees.

## Property-based testing

Use property-based testing to search the implementation state space, especially where examples cannot cover combinations of order, grouping, hierarchy, selection, phase, direction, and history.

For a stateful subsystem, define:

- a generator for valid initial states;
- command generators whose preconditions derive from the current generated model state;
- explicit adversarial generators for rejected, stale, repeated, and boundary events;
- a simple oracle, formal oracle, metamorphic relation, or invariant set;
- semantic shrinkers that preserve the preconditions needed to reproduce the failure;
- coverage classifications for the independent decision dimensions; and
- a stable serialization for seeds, generated states, command sequences, and observations.

Required property families include those that apply:

| Property family | Example claim                                                                                      |
| --------------- | -------------------------------------------------------------------------------------------------- |
| Preservation    | Every accepted transition preserves identity, membership, ordering, and active-state invariants    |
| Determinism     | Equal canonical state and event produce equal canonical state, commands, and observations          |
| Metamorphic     | Normalizing twice is equivalent to normalizing once                                                |
| Inverse         | Commit followed by its defined inverse, or preview followed by cancel, restores the abstract state |
| Symmetry        | Reversing direction or mirrored geometry changes only the declared directional projection          |
| Refinement      | Production output agrees with the abstract model, Lean oracle, or pinned reference projection      |
| No-op stability | Rejected or stale input leaves authoritative state unchanged and emits only allowed diagnostics    |

Randomness must never make the merge gate nondeterministic. Use explicit seeds, fixed size limits, deterministic schedulers, and logged generation parameters. The merge suite owns a stable seed corpus. Broader rotating or exhaustive fuzzing may run as discovery, but every failure must be reduced to a minimal serialized case and added to the stable regression corpus before it is considered resolved.

A property suite does not pass merely because it ran many cases. It must report whether generators reached each required state class, action, boundary, direction, and interaction phase. A generator that almost never creates the hard state provides weak evidence regardless of its iteration count.

## Deterministic system testing

Use deterministic system tests to execute real integration code while controlling every input that can change the observable timeline. The goal is exact replay, not merely low flakiness.

Control or record every applicable source of nondeterminism:

- wall and monotonic time, timers, animation frames, and task or microtask scheduling;
- randomness, generated identifiers, process order, and asynchronous completion order;
- viewport, device scale, zoom, fonts, locale, theme, reduced motion, and platform flags;
- pointer or keyboard event sequence, coordinates, capture, cancellation, and coalescing;
- layout measurements, scroll offsets, hit-test candidates, resize observations, and geometry snapshots;
- network, storage, worker, process, and operating-system responses; and
- initial persisted state and external reference version.

A deterministic interaction trace must record semantic checkpoints, not only input events or final pixels. Checkpoints should include the authoritative model, interaction phase, pending commands, observable projection, focus owner, relevant geometry, and rendered-state identifiers at activation, boundary crossings, preview changes, commit, cancellation, and motion completion.

- Advance a logical clock or wait for a named semantic condition; do not use arbitrary wall-clock sleeps.
- Drive animation with fixed logical frames and assert its semantic start, intermediate displacement rules, interruption behavior, and settled result.
- Compare canonical geometry and state before relying on screenshots. Pixel comparisons are secondary evidence for painting and styling.
- Make every trace runnable alone by stable name and serializable input.
- Treat retries that hide an unchanged race as a failure, not as a stabilization mechanism.

Headed exploratory observation remains useful for discovering behavior and validating the controlled harness. Manual observation is not merge evidence until the same scenario can be replayed with captured inputs and assertions.

## Reference refinement

When compatibility or replication is the goal, define a versioned parity contract. “Matches the reference” or “fully cloned” is not an acceptable unbounded claim.

The parity contract must identify:

- the upstream product version, source revision, build flags, operating system, theme, and other relevant environment facts;
- the supported operations, states, input modalities, visual modes, and explicit exclusions;
- the observable projection to compare, including semantic state, timing checkpoints, geometry, accessibility, and required painting details;
- permitted normalization, such as unstable process identifiers, and why it cannot hide a behavioral difference; and
- the authority rule for conflicts between source inspection, observed product behavior, upstream tests, and the locally owned contract.

Treat a reference source tree as a parameterized family of behaviors, not as one unconditional oracle. Record the revision, feature switches, build channel, platform branches, experiments, and runtime configuration that select the adopted behavior. An upstream test establishes evidence for the branch it configures; it does not establish that a separately installed binary selected that branch. When source, upstream tests, and observed binaries disagree, first classify the variant instead of blending the evidence or silently choosing the easiest implementation.

Audit the behavioral dependency closure, not a list of similarly named files. The closure includes called policies, ownership and lifetime rules, constants, feature flags, scheduler behavior, geometry and hit-testing code, animation models, platform theme integration, accessibility behavior, upstream tests, and state shared outside the apparent component. File-by-file translation without this closure is source resemblance, not behavioral refinement.

For each parity scenario:

1. Establish the reference precondition from a clean, recorded state.
2. Apply the same semantic action sequence to the reference and the implementation.
3. Capture observations at the same semantic checkpoints rather than approximately equal wall-clock moments.
4. Canonicalize only declared nondomain noise.
5. Compare the full adopted projection and preserve both traces on mismatch.
6. Classify the mismatch as a contract gap, model gap, implementation defect, harness defect, environment difference, or explicitly unsupported behavior.

Source inspection and exploratory computer use may discover scenarios and explain mismatches. Stable differential evidence must come from a repeatable harness or a versioned golden trace whose provenance and capture procedure are recorded. Re-capture reference traces deliberately; never silently update them to accept a new implementation result.

An upstream upgrade is a semantic change. Re-audit the affected dependency closure, re-run the scenario inventory, review every changed golden trace, and update the parity scope before claiming compatibility with the new version.

## Evidence traceability

Maintain one traceability matrix in the correctness case. Each row represents one owned claim, not one file or one implementation function.

| Claim              | Contract owner           | TLA+ property         | Lean theorem          | Property test    | System trace      | Reference scenario   | Limits                 |
| ------------------ | ------------------------ | --------------------- | --------------------- | ---------------- | ----------------- | -------------------- | ---------------------- |
| Domain-named claim | Owning reference section | Name or justified N/A | Name or justified N/A | Stable test name | Stable trace name | Stable scenario name | Bounds and assumptions |

- Every observable rule and invariant must have at least one executable witness at the lowest effective layer.
- Every maintained formal property or theorem must map back to an owned contract claim.
- Mark a method N/A only with a reason based on the method-selection rules; blank cells are unresolved work.
- Use stable names so a failed checker identifies the affected claim without reconstructing history from a task or commit.
- Record model bounds and trust assumptions next to the claim they limit.
- Remove traceability rows when behavior is deliberately removed; do not retain stale evidence to inflate apparent coverage.

Coverage percentages are supporting diagnostics, not traceability. Line, branch, screenshot, or generated-case counts cannot show that every owned rule has evidence.

## Counterexample discipline

Treat every discovered failure as evidence that one link in the correctness case is missing or false.

1. Preserve the exact environment, seed, state, event sequence, reference version, and observations.
2. Reduce the failure to the shortest semantic trace that still violates the claim.
3. Identify the first divergent checkpoint and the first violated invariant, theorem assumption, or refinement mapping.
4. Classify the root cause as a missing contract rule, invalid formal assumption, incomplete generator, implementation violation, adapter violation, or harness defect.
5. Fix the earliest incorrect authority in the refinement chain.
6. Add the reduced case to the stable regression corpus and re-run broader exploration.

If the smallest relevant checker passes but the full gate fails, preserve both results. The pair is evidence that the local claim was too narrow, the inferred rule was too broad, or the refinement mapping omitted a competing state. Do not update the broader test merely because it predates the change; identify which authority owns the conflict and make the priority or boundary explicit.

Do not only add a screenshot assertion for a state-machine defect, weaken a property to accept current output, exclude a reachable TLA+ state, add a Lean axiom, or retry a racing system test. Those actions remove evidence instead of resolving the contradiction.

If repeated defects cross state combinations, directions, interaction phases, or ownership boundaries, stop local patching and revise the model or refinement boundary. A sequence of narrow visual fixes is evidence of a missing semantic state, invariant, or projection rule.

## AI-assisted implementation

AI agents may inspect sources, propose models, generate tests and proofs, and implement code. Mechanical checkers and reviewable evidence decide conformance.

Before editing production code for a correctness-gated subsystem, an agent must:

- retrieve the current owning contract and correctness case;
- identify the behavioral dependency closure for any adopted reference behavior;
- list authoritative state, events, outputs, invariants, and unresolved assumptions;
- apply the method-selection table and name the required artifacts;
- reproduce an existing defect as a minimal semantic trace when the work begins from a failure; and
- state which claim and refinement link each intended change affects.

During implementation, update the abstract model or proof definition before relying on a new implementation behavior. Run the smallest relevant checker after each semantic change, then the full applicable gate. When a checker returns a counterexample, preserve and classify it before asking the agent for another patch.

An agent must not claim completeness because it inspected every visible file, translated every class, passed current examples, or matched selected screenshots. Completion reports must name the checked claims, bounds, reference revision, executed evidence, and exclusions. Unknown behavior remains explicit scope debt; it is not silently counted as compatible.

Independent review is most valuable at the abstraction boundaries: owned contract to TLA+ or Lean, formal model to deterministic core, and core observation to real system or reference trace. Reviewing generated code without reviewing these mappings cannot establish system correctness.

## Change contract

Classify a semantic change before implementation and update every affected owner in the same change.

| Change kind                                                  | Required updates                                                                                                             |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| New or changed state, event, guard, or cancellation rule     | Owned contract, state model, applicable TLA+ actions and properties, generated command model, and deterministic traces       |
| New or changed pure transformation or invariant              | Owned contract, Lean definitions and theorems when applicable, property oracle, and regression corpus                        |
| New platform, geometry, focus, timing, or animation behavior | Environment contract, deterministic trace inputs and checkpoints, adapter tests, and reference projection when parity-scoped |
| Reference revision or compatibility scope                    | Pinned provenance, dependency-closure audit, differential scenarios, reviewed golden changes, and declared limits            |
| Defect fix                                                   | Failing-before reduced counterexample, root-cause classification, corrected earliest owner, and passing-after evidence       |

Do not change production behavior and postpone its specification, proofs, generators, or traces. Do not preserve two competing semantic models as a transition convenience unless an explicit compatibility boundary owns both and differential tests compare them.

When a contract change invalidates a proof or model property, review whether the claim truly changed. Weakening the claim only to make current code pass requires the same product and architecture approval as any other behavior change.

## Verification gate

Every maintained correctness artifact that is deterministic, bounded, pinned, and provisioned by the repository must run in the root merge gate defined by [`VERIFICATION.md`](VERIFICATION.md). The owning package must document and expose the real commands; this standard does not invent commands that the repository does not execute.

The applicable merge gate must fail on:

- a TLA+ invariant or progress-property counterexample in the checked configuration;
- an unchecked Lean theorem, an admitted proof, or an undeclared trust assumption;
- a failed property, unreachable required generator class, or non-replayable generated failure;
- a deterministic system-trace mismatch or use of uncontrolled timing that affects the result;
- a differential mismatch inside the declared parity scope; or
- missing traceability for a changed owned claim.

Keep the gate reproducible and fail-fast. Do not make a flaky command pass through retries. Pin checker versions and model configurations, cap resource use explicitly, and retain actionable counterexamples as artifacts.

Reference-product capture, remote-service comparison, and environment-specific exploratory suites remain maintenance verification when the repository cannot provision them deterministically. They must run before a compatibility release, but they must not make the ordinary merge gate depend on ambient external state. Promote them into the merge gate only after their reference, environment, and inputs are pinned and provisioned.

The testing conventions and regression requirements in [`TESTING.md`](TESTING.md) apply to every executable test introduced by this standard.

## Definition of done

A correctness-gated subsystem is complete only when:

- its current owned contract and correctness case define the scope, observable projection, authority, invariants, assumptions, and exclusions;
- stateful behavior has one deterministic semantic transition boundary and a serializable observation model;
- every applicable method-selection trigger has evidence, or a reviewed N/A reason appears in traceability;
- every adopted discrete mode and continuous threshold has an explicit decision partition, applicable precedence rule, and executable boundary evidence;
- TLA+ checks every maintained property under documented bounds and converts discovered counterexamples into replay traces;
- Lean checks every maintained theorem without admitted obligations and its semantics are connected to production code by a tested refinement bridge;
- property-based tests cover required state classes and preserve replayable minimal failures;
- deterministic system tests control the relevant environment and assert semantic checkpoints across complete interaction timelines;
- parity claims identify a pinned reference and pass the full declared differential projection;
- every known defect has been reduced, classified, and retained at the lowest effective regression layer;
- the traceability matrix contains no unexplained gaps or stale claims;
- the root merge gate passes without retries; and
- the completion report states the checked bounds and remaining exclusions without using unbounded claims such as “fully verified” or “perfect replica.”
