# Repository Hygiene

## Tracked-file admission

Add a file to version control only when its durable repository value exceeds its ongoing maintenance cost. Before creating or retaining a file, answer all of these questions:

1. **Future consumer**: Which runtime, build, test, maintainer, or user will read or execute it after the current change is complete?
2. **Authoritative responsibility**: What current behavior, contract, configuration, or reusable capability does it own that is not already owned elsewhere?
3. **Recurring use**: Under what repeatable workflow will it be used again? A description of work already performed is not recurring use.
4. **Lifecycle**: What causes it to be updated or deleted, and what verification prevents it from becoming stale?
5. **Repository fit**: Why is a tracked file the correct delivery mechanism instead of an existing source file, test, authoritative document, issue, pull-request description, Git history, or temporary workspace?

If any answer is missing, keep the artifact outside the repository or delete it before handoff. Uncertainty defaults to not tracking the file until a durable consumer is identified.

## Repository value

Files normally justify their place when they provide at least one of these durable values:

- Production behavior or assets required at runtime.
- Build, test, release, migration, or maintenance automation used by a repeatable workflow.
- Regression coverage for an observable invariant.
- Configuration or schemas consumed by repository tooling.
- Current authoritative documentation needed to operate, extend, or consume the project.
- Active research with unresolved decisions that follows the lifecycle in [`docs/DOCUMENTATION.md`](DOCUMENTATION.md#research-lifecycle).

The following reasons are insufficient by themselves:

- Recording what an agent or maintainer did during one task.
- Preserving investigation evidence after its conclusions have been incorporated.
- Making a one-time manual check convenient.
- Keeping output because it took significant effort to produce.
- Providing context that already exists in Git history, a pull request, or an authoritative source.

## Working artifacts

Do not track task-local artifacts, including:

- One-off audit, migration, inspection, or data-extraction scripts.
- Scratch notes, copied upstream files, patches, command transcripts, and temporary fixtures.
- Point-in-time reports, coverage snapshots, screenshots, logs, benchmark output, or historical work summaries.
- Local paths, environment-specific launchers, editor state, caches, and build output.
- Generated files without a committed consumer, generator, and stale-state check.

Create disposable artifacts in an operating-system temporary directory when possible. If a tool must place them inside the repository, remove them before handoff and do not add a package command or documentation entry merely to legitimize a temporary file.

Promote the result, not the work trail: add the regression test, update the current contract, or implement the reusable tool, then delete the intermediate artifact.

## Durable scripts and generated files

A new script is durable only when it has a named repeatable workflow, a stable entry point, clear input and output ownership, documented prerequisites, and verification proportional to its risk. A script used only to complete or validate the current change remains temporary even if it is polished.

A generated file is tracked only when a runtime, build, release, or external consumer requires the materialized output. Commit its deterministic generator and a check that fails when tracked output is stale. Otherwise generate it locally or in CI and leave it untracked.

Static evidence manifests and provenance reports require an ongoing conformance or compliance workflow. They must define who updates them, what source they are authoritative for, and how completeness is verified. A snapshot of an investigation is research or task history, not a permanent contract.

## Documentation admission

Documentation describes current truth, an active proposal, or unresolved research. Do not add documents whose primary purpose is to narrate a completed task, list files changed, preserve review findings, or record a point-in-time audit.

When an investigation finishes:

- Move stable behavior and decisions into the existing authoritative document.
- Express discovered invariants as tests when they are mechanically verifiable.
- Leave task chronology in the pull request and Git history.
- Delete resolved research and intermediate evidence.

## Pre-handoff review

Before presenting a change as complete:

1. Run `git status --short` and review every new and untracked path individually.
2. Apply the tracked-file admission questions to each added file, including scripts, documents, fixtures, and generated output.
3. Remove task-local artifacts and every command, link, configuration entry, or documentation statement that refers to them.
4. Confirm that remaining new files are placed under the owner identified by [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md) or [`docs/DOCUMENTATION.md`](DOCUMENTATION.md).
5. Run `git diff --check` and the verification required by [`docs/VERIFICATION.md`](VERIFICATION.md).

Reviewers should request removal when a new file has no concrete future consumer or lifecycle, even when the file is technically correct.
