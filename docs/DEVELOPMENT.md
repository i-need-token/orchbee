# Development

## Authority and scope

Classify every change before editing governed artifacts. This standard governs change
authorization, staged write access, review decomposition, and convergence; it does not define
product or package behavior. Current authorities continue to govern existing facts. Proposals may
carry accepted future design, but they never become current authority until their decisions are
integrated into the owning standards, contracts, source, or tests.

| Term                 | Contract                                                                                                                                                                                         |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Current authority    | The owning standard, package reference, source contract, or test that governs an existing fact                                                                                                   |
| Semantic owner       | The workspace, package, module, or boundary responsible for a behavior, state, contract, or rule                                                                                                 |
| Decision owner       | The person or group authorized by repository ownership, assignment, or the active request to accept a change to an affected outcome, contract, boundary, or risk                                 |
| Change authorization | The accepted problem, scope, non-goals, claims, affected semantic owners, constraints, reserved decisions, evidence obligations, and exclusions that bound one change                            |
| Change tier          | The engineering treatment selected from a change's behavior, state, ownership, and consequence triggers; it determines required authorization, modeling, review, and evidence                    |
| Governed artifact    | Source, test, runtime asset, schema, migration, build or verification configuration, generated contract, or current-authority documentation that affects behavior, contracts, delivery, or proof |
| Claim                | An independently falsifiable observable rule or invariant that the change must establish or preserve                                                                                             |
| Evidence             | A check capable of judging a claim, together with its environment, bounds, assumptions, and exclusions                                                                                           |
| Reserved decision    | A choice that changes observable behavior, ownership, public boundaries, dependency direction, durable data, trust, compatibility, irreversible effects, or evidence                             |
| Delegated decision   | A reversible implementation choice already constrained by the accepted change authorization and current authorities                                                                              |
| Reversible           | Replaceable inside the same semantic owner without migration, published compatibility change, retained external effect, or durable-data consequence                                              |
| Acceptance           | An explicit decision by the required decision owner that identifies the change authorization or reserved decisions being accepted                                                                |
| Material difference  | A difference that changes an accepted claim, semantic owner, public boundary, risk, evidence obligation, or exclusion                                                                            |
| Convergence          | The comparison that maps the completed implementation and evidence back to the accepted change authorization and current authorities                                                             |

A change authorization exists only after the required decision owner explicitly accepts it.
Acceptance must identify the decision owner and the authorization or reserved decisions accepted.
Assignment, silence, an unanswered question, an existing code pattern, the easiest implementation,
or a generic approval with no identifiable decision target does not imply acceptance. A current
authority constrains a change but does not authorize changing the behavior it governs.

Resolve decision ownership from repository governance or an explicit identification in the active
request, issue, review, or proposal. Semantic ownership does not identify decision ownership. If
neither source identifies the required decision owner, ask the person directing the work or a
repository maintainer to identify that owner in the active record. Until then, the reserved decision
remains unresolved and affected governed writes stay blocked.

The active conversation, issue, review, or proposal may hold the accepted change authorization only
when a reader with no task history can recover the accepted decisions, decision owner, unresolved
questions, and exclusions from that record. Do not require a tracked authorization file when this
condition is met. Track future design under `proposals/` only when it has the durable owner and
lifecycle required by [`DOCUMENTATION.md`](DOCUMENTATION.md) and
[`REPOSITORY-HYGIENE.md`](REPOSITORY-HYGIENE.md). Task chronology, approval transcripts, and
temporary plans remain outside the repository.

Use existing authorities and active collaboration state. Do not introduce a parallel specification
framework, generated per-change directory, approval manifest, or process dependency merely to
satisfy this standard; doing so is a reserved architecture decision.

How to select a change tier is defined in [Change classification](#change-classification), write
permissions in [Governed-write stages](#governed-write-stages), review timing in
[Reviewable decomposition](#reviewable-decomposition) and [Review](#review), and completion in
[Convergence and completion](#convergence-and-completion).

## Change classification

Start with the highest change tier supported by current knowledge during read-only discovery and
finalize it before the first governed edit. Raise the change tier whenever discovery or
implementation reveals another trigger. The highest tier controls the authorization threshold; all
applicable modeling and evidence obligations remain cumulative. Apparent simplicity, author
confidence, and change size never lower the tier.

| Change tier   | Trigger                                                                                                                                                                                           | Required authorization before governed edits                                                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mechanical    | An exact, behavior-preserving edit whose result is already determined by current authority                                                                                                        | Confirmed scope, affected current authority, and verification                                                                                              |
| Bounded       | Observable behavior changes inside one existing owner, plus authority-determined projections into supporting owners, without a new public boundary, long-lived state model, or consequential risk | Accepted compact change authorization with outcome, non-goals, claims, semantic owner, decision owner, and verification                                    |
| Modeled       | Multiple states, ordering, hierarchy, async work, cancellation, gestures, persistence, process boundaries, structural ownership changes, or cross-owner coordination                              | Accepted change authorization plus the applicable modeling record and evidence selected by [`FEATURE-DEVELOPMENT.md`](FEATURE-DEVELOPMENT.md)              |
| Consequential | Public or cross-workspace contracts, dependency direction, migration, data loss, security, privacy, external cost or side effects, compatibility, or irreversible risk                            | Explicit decision-owner acceptance of alternatives, consequences, evidence limits, rollout, and rollback or recovery, plus every applicable model or check |

A defect does not become Mechanical merely because the expected result is obvious. Classify it
according to the behavior, ownership, state, and risk changed by the remedy. A behavior-preserving
refactor becomes Modeled when it redistributes responsibility or creates a new boundary. A
presentation change that only projects already-authoritative state remains Bounded unless it
introduces a new policy, transition, writer, or observable state meaning. Routine edits across
supporting owners remain Bounded only when current authorities independently determine every
projection and no new interaction, shared state, ordering, policy, or contract must be designed.

The Mechanical tier is available only when current authority determines both the exact result and
its verification without author judgment. Otherwise use Bounded or a higher tier and obtain the
corresponding acceptance.

## Change authorization

Use concrete domain names and omit fields that cannot affect the classified change. A Modeled or
Consequential authorization must make every applicable field explicit.

| Field              | Required decision                                                                                                |
| ------------------ | ---------------------------------------------------------------------------------------------------------------- |
| Problem            | Actor, context, trigger, observable outcome, and why current behavior is insufficient                            |
| Scope              | Included behavior, affected semantic owners, and exact non-goals                                                 |
| Claims             | Observable rules, invariants, failure, cancellation, no-op, and compatibility behavior                           |
| Boundaries         | Authoritative state owners, public contracts, dependency direction, external capabilities, and allowed writers   |
| Decision authority | Required decision owners, reserved decisions they accepted, and delegated decisions left to implementation       |
| Evidence           | Claim-mapped checks, trust assumptions, bounds, environments, and exclusions                                     |
| Delivery           | Independent slices, integration order, migration, rollout, recovery, and rollback when applicable                |
| Unknowns           | Unresolved questions, feasibility risks, and the experiment or decision needed before the affected governed edit |

Acceptance covers only the stated authorization. When several materially different outcomes remain
valid, present the options, consequences, and recommendation instead of choosing silently.

## Governed-write stages

Write permission follows the current stage rather than the implementer's ability to produce code.

| Stage              | Allowed work                                                                                                                                       | Governed writes  |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- |
| Classify           | Locate current authorities, classify the change, identify semantic and decision owners, and identify risk triggers                                 | Forbidden        |
| Discover           | Inspect behavior, dependencies, history, references, tests, and unknowns; reproduce defects and run read-only checks                               | Forbidden        |
| Specify and design | Draft or revise the change authorization, proposal, unpublished modeling record, alternatives, and evidence plan                                   | Forbidden        |
| Accept             | Resolve reserved decisions and record acceptance from the required decision owner                                                                  | Forbidden        |
| Slice              | Build a dependency-ordered task graph whose units have one semantic outcome and independent evidence                                               | Forbidden        |
| Implement          | Edit only the accepted slice, add its lowest-layer evidence first when applicable, and run focused checks after each semantic change               | Allowed in scope |
| Converge           | Compare claims, design, tasks, implementation, and evidence; update current authorities; remove temporary artifacts; run the applicable merge gate | Allowed in scope |

Mechanical work may move from Classify directly to Implement after its scope and verification are
confirmed. Every other tier requires accepted change authorization. During Specify and design,
keep draft authorizations and modeling records in active collaboration state or a disposable
workspace; a proposal may be tracked only when repository documentation rules admit it. Do not
publish a draft into current authority before acceptance. For modeled work, persist the accepted
modeling record in its owning durable location as the first Implement slice, before editing
production behavior that depends on it. Research spikes remain disposable and outside governed
paths unless a later accepted authorization deliberately reimplements their result.

Governed writing stops immediately when implementation exposes a new reserved decision, an
incorrect owner, an unsupported claim, or a materially different risk. Return to the earliest stage
that owns the issue, revise the authority there, obtain the required acceptance, then resume with a
new slice. Never backfill a plan to legitimize code already written.

## Decision authority

Review attention stays on semantic leverage; implementation and evidence work stay inside accepted
boundaries.

| Reserved decisions                                                                                                           | Delegated implementation work                                                                          |
| ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Product outcome, scope, non-goals, and user-observable tradeoffs                                                             | Repository discovery, current-state reconstruction, and ambiguity detection                            |
| State ownership, public contracts, cross-owner composition, dependency direction, new packages, and new dependencies         | Private naming, local types, helper placement, and implementation already determined by current rules  |
| Durable data, migration, compatibility, security, privacy, external effects, cost, deletion, recovery, rollout, and rollback | Deterministic implementation, fixtures, and claim-mapped tests inside accepted boundaries              |
| Selection among materially different architectures or behavior policies                                                      | Reversible refactoring steps that preserve accepted behavior and ownership                             |
| Claims, trust assumptions, evidence limits, and accepted exclusions for consequential work                                   | Running checks, reducing counterexamples, and reporting evidence without expanding the supported claim |

An implementer may recommend a reserved decision and provide evidence for it; the recommendation
does not become authority until the required decision owner accepts it. Ambiguity is not delegation.
Existing standards decide a question only when they admit one compliant outcome.

## Reviewable decomposition

Each implementation slice must:

- deliver one domain claim, transition group, contract boundary, adapter capability, or
  behavior-preserving structural step;
- name its inputs, output, owner, dependencies, affected claims, and completion evidence;
- keep unrelated reserved decisions out of scope;
- be independently reviewable and leave the repository in a verified or explicitly staged state;
- stop before the next slice instead of opportunistically completing adjacent work.

Split work by semantic responsibility and feedback boundary, not by line count, file count, or
framework category. Do not use tasks such as “implement the feature,” “refactor the Hook,” or “fix
the tests.” A task graph is execution state and remains in the active tool, issue, or review unless
it has an independent durable consumer.

Modeled and consequential work requires a fresh read-only challenge pass after Slice and before
Implement. The review checks the change authorization, model, task graph, and evidence for
ambiguity, contradiction, missing claims, ownership leaks, unsupported assumptions, and tasks that
introduce new decisions. The reviewer reports findings; it does not silently rewrite authority or
approve decision-owner-reserved choices.

Give the challenge reviewer only the accepted authorities and review artifacts, not the author's
hidden reasoning or an editable implementation context. When a separate context is unavailable,
disclose that limitation and perform an explicit adversarial read; never label it independent
evidence.

## Implementation discipline

- Retrieve only the authorities, model, source, and evidence required by the active slice.
- Implement the deterministic domain operation before framework integration when
  [`FEATURE-DEVELOPMENT.md`](FEATURE-DEVELOPMENT.md) selects a model-first approach.
- Keep components and Hooks inside the ownership boundaries in
  [`frontend/react.md`](frontend/react.md); production UI never becomes the first or only behavior
  model.
- Prefer the smallest complete change that satisfies the active claims; do not add speculative
  abstractions, extension points, packages, or compatibility behavior.
- Run the smallest relevant checker after each semantic change and preserve reduced failures before
  continuing.
- Report discovered scope, authority, and evidence gaps immediately. Do not hide them with fallback
  behavior, permissive types, disabled checks, retries, or broader implementation.

Tests authored with their implementation are evidence only to the strength of their owned claims
and oracles. They do not provide independent confirmation merely because they pass. For high-risk
logic, use the additional property, model, system, or differential evidence selected by
[`FEATURE-DEVELOPMENT.md`](FEATURE-DEVELOPMENT.md).

## Review

Present review material in this order:

1. reserved decisions and unresolved questions;
2. observable claims, invariants, state transitions, and ownership;
3. public boundaries, dependency changes, migration, rollout, and rollback;
4. claim-to-evidence mapping and known limits;
5. implementation hotspots where trust, concurrency, irreversible effects, or non-obvious
   refinement remains;
6. mechanical implementation covered by the accepted contract and executable evidence.

Never imply that implementation was reviewed line by line. State what was reviewed, what was
verified mechanically, what was sampled, and what remains excluded. If one review unit cannot be
understood without reconstructing several unrelated owners, return to design and split the
responsibility before adding more code.

Run implementation review before final Converge or before merging each independently deliverable
slice. Record acceptance, challenge findings, and completion reporting in the active conversation,
issue, or review; do not create a tracked process artifact solely for that record.

## Convergence and completion

Convergence is a read-first comparison, not permission to expand the implementation. For every
accepted claim, identify its owner, implementation, executable witness, result, bounds, and
exclusions. Classify every mismatch as an authority, specification, design, task, implementation,
adapter, or evidence defect and fix the earliest incorrect owner.

Before completion:

1. resolve or obtain acceptance for every deviation and new reserved decision;
2. update the owning current authority when its governed fact changed or the authority was
   incomplete; otherwise report that no authority update was required;
3. satisfy the exit condition of each proposal or research artifact and delete artifacts whose
   durable responsibility has ended;
4. remove temporary plans, scripts, reports, fixtures, flags, and duplicate models;
5. run focused evidence and the applicable root gate in [`VERIFICATION.md`](VERIFICATION.md); if
   a current authority assigns selected evidence to a required gate or CI check that does not
   actually run it, report a verification-contract defect and block completion instead of weakening
   the evidence plan;
6. report checked claims, decisions, evidence, bounds, exclusions, and remaining risks.

Completion means the current authorities, implementation, and evidence agree inside an explicitly
bounded claim. Passing lint, code volume, file inspection, screenshots, or an author assertion
alone never establishes convergence.
