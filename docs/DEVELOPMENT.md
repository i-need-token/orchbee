# Development

## Authority and scope

Start every change with this standard. It governs scoped authorization, risk classification, review,
and completion. Current authorities govern existing behavior; an accepted change updates the owning
standards, contracts, source, and evidence together. Proposals describe future design until integrated.

A clear request from an authorized decision owner authorizes the complete scoped result, including
necessary discovery, design, implementation, verification, ordinary repairs, and documentation.
An instruction to carry out an identified recommendation accepts that recommendation. Use the
conversation and existing assignment to recover this scope; do not ask for the same permission again.

Tool or model capability does not replace correctness evidence. Select modeling and evidence through
[`MODELING-AND-CORRECTNESS.md`](MODELING-AND-CORRECTNESS.md) according to the affected claims and
failure risks. It extends this lifecycle rather than creating another authorization process.

| Term                 | Contract                                                                                                                                                                          |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Current authority    | The owning standard, package reference, source contract, or test that governs an existing fact                                                                                    |
| Semantic owner       | The workspace, package, module, or boundary responsible for a behavior, state, contract, or rule                                                                                  |
| Decision owner       | The person or group authorized by repository ownership, assignment, or the active request to accept the affected outcome or risk                                                  |
| Change authorization | The accepted outcome, scope, constraints, reserved decisions, and evidence obligations bounding a complete result                                                                 |
| Change tier          | The engineering treatment selected by changed behavior, ownership, and consequences                                                                                               |
| Governed artifact    | Source, test, runtime asset, schema, migration, configuration, generated contract, or current-authority documentation affecting behavior, delivery, or evidence                   |
| Claim                | An independently falsifiable observable rule or invariant                                                                                                                         |
| Evidence             | A check capable of judging a claim, with its environment, bounds, assumptions, and exclusions                                                                                     |
| Reserved decision    | A material choice outside the accepted authorization that changes product outcomes, public boundaries, trust, durable data, compatibility, external effects, or required evidence |
| Delegated decision   | An implementation choice inside the accepted outcome, boundaries, and risk limits                                                                                                 |
| Reversible           | Replaceable without migration, published compatibility change, retained external effect, or durable-data consequence                                                              |
| Acceptance           | The decision owner's identifiable request or approval of an outcome, recommendation, or reserved decision                                                                         |
| Material difference  | A change to an accepted outcome, semantic owner, public boundary, risk, evidence obligation, or exclusion                                                                         |
| Convergence          | Comparing the completed implementation and evidence with the accepted outcome and current authorities                                                                             |

Silence, unanswered questions, and an existing code pattern do not authorize a new reserved decision.
A request to investigate authorizes investigation; it does not authorize the changes being investigated.
A broad goal does not resolve an unmentioned destructive effect or incompatible product tradeoff.

Resolve decision ownership from existing governance, assignment, and the active conversation. Ask
only when a necessary reserved decision has no identifiable authorized owner or accepted outcome.
Pause its dependent work and continue independent authorized work.

Keep scope, outstanding decisions, and evidence in the existing conversation, issue, or review.
Use enough detail for another reader to recover the accepted outcome and limits. Do not create a
tracked authorization file, approval manifest, parallel specification framework, or task ledger
solely for this process. Durable proposals follow [`DOCUMENTATION.md`](DOCUMENTATION.md) and
[`REPOSITORY-HYGIENE.md`](REPOSITORY-HYGIENE.md).

## Change classification

Classify before the first governed edit and update the classification when new facts change the risk.
Use the highest applicable tier. A tier selects engineering treatment; it does not invalidate an
existing authorization or require a new approval merely because implementation advances a stage.
File count, author confidence, and model capability do not establish risk or correctness.

| Change tier   | Trigger                                                                                                                                                   | Required treatment                                                                                                   |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Mechanical    | Behavior-preserving edit whose result is determined by current authority                                                                                  | Known scope and a relevant check                                                                                     |
| Bounded       | Local behavior or presentation inside existing ownership and contracts, with limited reversible consequences                                              | Clear requested outcome, affected behavior, and focused evidence                                                     |
| Modeled       | Interacting transitions, ordering, shared state, persistence, process boundaries, ownership redistribution, or coordination requiring explicit invariants | Scoped authorization plus the smallest useful behavior model and risk-selected evidence                              |
| Consequential | Material public-contract or compatibility change, dependency direction, migration, data loss, security, privacy, external cost, or irreversible effects   | Acceptance of the affected tradeoffs and risk, applicable model and evidence, and rollout or recovery where relevant |

A defect follows the risk of its remedy. Simple local async completion or cancellation can remain
Bounded when ownership, ordering policy, and effects are already established; async syntax alone
does not require a formal model. Competing transitions or newly designed recovery require Modeled
treatment, and consequential failure risks retain the highest tier.

A private refactor that preserves responsibility and contracts stays Bounded or Mechanical.
Moving files, updating supporting projections, or touching several workspaces does not by itself
change the tier; changing their ownership, interaction policy, or public contract does.

## Change authorization

Recover accepted decisions from the request and existing authorities before asking for more detail.
Record only fields that affect the outcome, risk, coordination, or acceptance. For Bounded work,
the request plus a short statement of the approach and checks can be sufficient.

| Field              | Applicable content                                                                       |
| ------------------ | ---------------------------------------------------------------------------------------- |
| Problem            | Actor, trigger, observable outcome, and why current behavior is insufficient             |
| Scope              | Included behavior, affected owners, and non-goals                                        |
| Claims             | Relevant invariants, failure, cancellation, no-op, and compatibility behavior            |
| Boundaries         | State owners, public contracts, dependencies, external capabilities, and allowed writers |
| Decision authority | Accepted tradeoffs, genuinely unresolved reserved decisions, and delegated choices       |
| Evidence           | Claim-mapped checks, assumptions, bounds, environments, and exclusions                   |
| Delivery           | Integration order, migration, rollout, recovery, and rollback where applicable           |
| Unknowns           | Questions that can change the accepted result or risk, and how to resolve them           |

Multiple compliant implementation options are normal delegated work. Choose and explain the
appropriate option without pausing. Present alternatives when the choice introduces a material
product, architecture, compatibility, or risk decision not covered by the authorization.

## Governed-write stages

Stages describe the information and work needed for delivery, not mandatory separate turns or
approval checkpoints. Combine or revisit them within the accepted scope. A clear request may already
supply the outcome, acceptance, and constraints needed to proceed after classification.

| Stage              | Work                                                                        | Governed writes                                                     |
| ------------------ | --------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Classify           | Locate authorities, scope, ownership, and risks                             | After applicable scope and decisions are established                |
| Discover           | Inspect behavior, reproduce defects, and resolve implementation unknowns    | Within accepted scope                                               |
| Specify and design | Define missing claims, ownership, alternatives, and evidence                | Within accepted scope; unresolved reserved decisions remain blocked |
| Accept             | Resolve a new reserved decision with its authorized owner                   | Only work independent of that unresolved decision                   |
| Slice              | Arrange independent outcomes and real dependencies when decomposition helps | Within accepted scope                                               |
| Implement          | Complete the accepted behavior, evidence, adapters, and ordinary repairs    | Within accepted scope                                               |
| Converge           | Review the result, update authorities, verify final inputs, and clean up    | Within accepted scope                                               |

Establish the behavior and invariants before integrating dependent production behavior. Types and
tests may carry the modeling record and evolve with the implementation; a separate document or
preliminary documentation-only slice is not required. Keep unaccepted design drafts out of current
authority. A disposable experiment may be integrated after its result is accepted, reviewed, and
verified under the same production requirements; it need not be rewritten solely for its origin.

When discovery exposes a new reserved decision, pause only the dependent writes or effects.
Continue independent authorized work. Ordinary debugging, test repair, and local design corrections
inside accepted claims do not restart authorization. Do not backfill approval for an unauthorized
effect or conceal a changed outcome as implementation detail.

## Decision authority

Reserve outcome and risk choices; delegate the implementation needed to deliver accepted choices.

| Requires acceptance when outside existing authorization                                        | Delegated within accepted boundaries                                                                |
| ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Material product outcome, scope, or user-observable tradeoff                                   | Local interaction and implementation choices satisfying the requested outcome                       |
| Public contracts, state ownership, dependency direction, new packages, or dependency admission | Algorithms, private types, helper placement, and cohesive local refactoring                         |
| Durable data, migration, compatibility, security, privacy, external effects, cost, or deletion | Fixtures, adapters, failure handling, cleanup, and ordinary repairs preserving accepted risk limits |
| Materially different architecture or behavior policy                                           | Selection among compliant implementations                                                           |
| Consequential evidence limits, trust assumptions, or weakening of mandatory checks             | Selecting focused checks and reusing valid evidence under the owning verification contract          |

An implementer may recommend a reserved decision and provide evidence, but must obtain its required
acceptance before dependent work. Existing standards can constrain several valid implementations;
that does not turn the choice among them into a new approval requirement.

## Reviewable decomposition

Use a complete observable outcome as the default work unit. One owner may carry it through discovery,
design, implementation, tests, documentation, and repair. Split only for an independent result,
a genuine dependency, separate write ownership, or a review boundary that improves delivery.

Each slice identifies its outcome, owner, dependencies, and necessary evidence in existing task
state. Keep unrelated decisions out of scope and leave a verified or explicitly staged result.
Proceed directly to the next authorized slice; a commit, check, progress report, or internal phase
change does not require a new instruction.

Distinguish a staged foundation from an integrated user outcome. Before extending a foundation,
identify its actual consumer and next observable integration result. Do not create speculative
infrastructure or a second progress ledger. A task graph is useful for real dependencies; a small
cohesive task needs no artificial decomposition.

When structure changes, apply [Semantic module design](MODULE-BOUNDARIES.md#semantic-module-design).
Execution slices do not define source-module boundaries.

Require a fresh read-only design challenge before implementing a new consequential decision.
For Modeled work, add a pre-implementation challenge when a concrete unresolved ownership,
interleaving, or refinement risk would make later correction costly; otherwise review the complete
candidate before convergence. Review accepted decisions again only when relevant facts change.

Give a challenge reviewer the scope, authorities, proposed claims, and evidence plan, without the
author's hidden reasoning. Reviewers report findings rather than granting reserved choices.
Use a separate context when available; otherwise disclose the limitation and perform an explicit
adversarial read without calling it independent evidence.

## Implementation discipline

- Design product capabilities for GUI and TUI together under
  [`INTERFACE-DESIGN.md`](INTERFACE-DESIGN.md). Establish common semantics and adapter mappings
  before implementing either projection; record an explicitly scoped exception or staged delivery
  with its owner and completion condition rather than silently treating one Client as complete.
- Retrieve the authorities and source needed for the active outcome; reuse unchanged material.
  Summaries guide retrieval and do not replace required instruction reads or semantic review of
  changed documentation pairs.
- Before composing an admitted product interface, follow the discovery and reuse requirements in
  [`frontend/experience-components.md`](frontend/experience-components.md). Existing authorization
  may cover the visible outcome; new reserved product decisions still require acceptance.
- Establish domain decisions independently of framework integration when selected by
  [`MODELING-AND-CORRECTNESS.md`](MODELING-AND-CORRECTNESS.md), and preserve
  [`frontend/react.md`](frontend/react.md) ownership boundaries.
- Deliver the smallest complete result. Resolve in-scope structural risks without speculative
  abstractions, extension points, packages, or unrelated work.
- Run focused checks at useful feedback boundaries. Reuse valid results while relevant inputs
  remain unchanged; a report, commit, or handoff alone is not a reason to rerun checks.
- Preserve and investigate failures. Do not hide scope or evidence gaps with fallback behavior,
  permissive types, disabled checks, retries until green, or broader implementation.

Tests written with implementation support only their stated claims and oracles. Passing does not
make them independent confirmation. Add the property, model, system, or differential evidence
selected for the actual risks.

## Review

Review the complete candidate or an independently deliverable slice before convergence. Concentrate
on changed claims, ownership, public boundaries, consequential effects, evidence gaps, and remaining
implementation risks. Scale the detail to the result; do not repeat unchanged design reviews.

State what was reviewed, checked mechanically, sampled, or excluded. Never imply line-by-line review
or independence that did not occur. Resolve findings in the same task, with additional review or
checks only for affected risks. Keep decisions and results in the existing conversation, issue, or
review rather than creating a tracked process artifact.

## Convergence and completion

Compare the result with the accepted outcome and current authorities. For each affected claim,
identify its implementation and meaningful evidence; keep detailed traceability only where the
modeling contract requires it. Fix a mismatch at its earliest incorrect owner.

Before completion:

1. Resolve deviations and new reserved decisions affecting the delivered result.
2. Update current authorities whose facts changed; reuse types and tests where they already own the fact.
3. Retire proposals, research, duplicate models, and temporary artifacts whose consumers have finished.
4. Run focused evidence and the applicable root gate in [`VERIFICATION.md`](VERIFICATION.md) on final
   inputs. If a required gate omits evidence assigned to it, report the verification-contract defect
   and resolve it before claiming completion.
5. Report the delivered outcome, meaningful checks, limits, and unresolved risks.

Completion requires agreement between the authorized outcome, current authorities, implementation,
and evidence. Code volume, an author assertion, or a passing unrelated check does not establish it.
