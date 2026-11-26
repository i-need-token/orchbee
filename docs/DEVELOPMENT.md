# Development

## Authorization

A clear request from the decision owner authorizes its complete scoped result: the discovery,
design, implementation, verification, ordinary repairs, and documentation that result needs. An
instruction to carry out an identified recommendation accepts that recommendation. Recover the scope
from the conversation and the existing assignment, and do not ask for the same permission again. A
commit, a passing check, a progress report, or the end of an internal phase is not a reason to stop.

- A request to investigate authorizes the investigation, not the changes it finds.
- Silence, an unanswered question, and an existing code pattern do not authorize a reserved
  decision.
- Choosing among implementations that satisfy the request is delegated work: choose, say why, and
  continue.

Keep scope, open decisions, and evidence in the existing conversation, issue, or review. Do not
create a tracked authorization file, approval manifest, or task ledger for them; durable proposals
follow [`DOCUMENTATION.md`](DOCUMENTATION.md).

## Decision authority

The decision owner is the person whom repository ownership, an assignment, or the active request
authorizes to accept an outcome or risk. A reserved decision needs the decision owner's acceptance
before the work that depends on it. When one appears, pause only that dependent work, continue
everything independent of it, and report the decision with a recommendation and its evidence.

| Requires acceptance when outside existing authorization                                        | Delegated within accepted boundaries                                                                |
| ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Material product outcome, scope, or user-observable tradeoff                                   | Local interaction and implementation choices satisfying the requested outcome                       |
| Public contracts, state ownership, dependency direction, new packages, or dependency admission | Algorithms, private types, helper placement, and cohesive local refactoring                         |
| Durable data, migration, compatibility, security, privacy, external effects, cost, or deletion | Fixtures, adapters, failure handling, cleanup, and ordinary repairs preserving accepted risk limits |
| Materially different architecture or behavior policy                                           | Selection among compliant implementations                                                           |
| Consequential evidence limits, trust assumptions, or weakening of mandatory checks             | Selecting focused checks and reusing valid evidence under the owning verification contract          |

- A feature point the request did not name is a material product outcome: propose it, do not add
  it.
- Lint policy is a mandatory check: `.oxlintrc.json` changes only with explicit human approval
  ([Linting](LINTING.md#enforcement-boundary)).
- A release, a message to people outside the repository, and any other action that cannot be taken
  back are external effects.
- Existing standards can constrain several valid implementations; that does not turn the choice
  among them into a new approval requirement.
- A standard may make one of its own rules act-and-report, as the
  [Rust module record](LINTING.md#rust-module-record) does; follow that standard's wording.

## Working rules

- Design a product capability for GUI and TUI together under
  [`INTERFACE-DESIGN.md`](INTERFACE-DESIGN.md). A staged delivery or an exception names its owner
  and completion condition; never treat one Client as silently complete.
- Reuse an existing composition before writing a new product interface, under
  [`frontend/experience-components.md`](frontend/experience-components.md).
- Deliver the smallest complete result. Resolve in-scope structural risks without speculative
  abstractions, extension points, packages, or unrelated work.
- Evidence follows risk. [`MODELING-AND-CORRECTNESS.md`](MODELING-AND-CORRECTNESS.md) says when a
  model, property test, or system trace is required; [`TESTING.md`](TESTING.md) decides which
  tests are admitted. Tool or model capability does not replace either.
- Preserve and investigate failures. Do not hide a gap with fallback behavior, permissive types,
  disabled checks, retries until green, or a broader implementation.
- Fix a mismatch at its earliest incorrect owner.

## Completion

1. Update the current authorities whose facts changed: code, schemas, and the documents
   [`DOCUMENTATION.md`](DOCUMENTATION.md) assigns them to.
2. Remove temporary artifacts, superseded proposals, and duplicate models whose consumers have
   finished.
3. Run the applicable root gate in [`VERIFICATION.md`](VERIFICATION.md#merge-gate): the local gate
   on the final inputs, then the pushed commit's Verify run on `main`. Reuse a valid result while
   its inputs are unchanged; a report, commit, or handoff alone is not a reason to run a check again.
4. Report the delivered outcome, what was run, what was sampled, what was not checked, and the
   unresolved risks. Never imply a review, a check, or independence that did not happen.

Completion is agreement between the authorized outcome, the current authorities, the
implementation, and the evidence. Code volume, an author's assertion, or a passing unrelated check
does not establish it.
