# Documentation

## Authority

| Artifact                                  | Audience                     | Responsibility                                                                                 |
| ----------------------------------------- | ---------------------------- | ---------------------------------------------------------------------------------------------- |
| `README.md`, `README.zh-CN.md`            | Users                        | Synchronized project value, capabilities, installation, and first use                          |
| Package `README.md`                       | Package consumers            | Public package capability, API, installation, and usage                                        |
| `packages/*/docs/**/*.md`                 | Package maintainers          | Current package-owned behavior, internal architecture, integration, and maintenance references |
| `CLAUDE.md`                               | AI agents                    | Short project map, hard rules, and links to authoritative standards                            |
| `docs/**/*.md`, excluding locale subtrees | Maintainers and AI agents    | Authoritative repository-wide engineering standards and cross-workspace architecture contracts |
| `docs/zh-CN/**/*.md`                      | Chinese-speaking maintainers | Complete synchronized translation of the English standards, never independent authority        |
| `proposals/**/*.md`                       | Decision participants        | Future design under review, never current authority                                            |
| `research/**/*.md`                        | Maintainers and AI agents    | Time-bounded evidence and open investigation, never authority                                  |
| Lint diagnostics                          | AI agents during execution   | Violation, corrective action, and authoritative English standard link                          |

Authority follows ownership. A repository standard may constrain every package without owning any
package's behavior. A package README or package-local reference may describe that package without
creating a rule for unrelated workspaces.

## Standard admission

A document belongs under `docs/` only when its rule or architecture decision governs repository
operation, multiple workspaces, or a boundary between workspaces. A standard states the reusable
decision and its verification; it does not inventory one package's implementation.

- Put public package API and consumer usage in the owning package's `README.md`.
- Put package-owned behavior, source architecture, external integration facts, and maintenance
  procedures in `packages/<package>/docs/`.
- Put a product interaction model, screen composition, named component behavior, and other business
  decisions with the product or package that owns them.
- Put unimplemented architecture, product behavior, package topology, and migration plans under
  `proposals/`; never present them as current standards.
- Put time-bounded evidence and unresolved findings under `research/`.
- Keep current workspace identity, responsibility, runtime, stability, and non-goals in each
  workspace's `project.json`; do not reproduce a manually maintained workspace catalog in a
  standard.

A standard may name a concrete path, package, command, configuration key, or rule ID only when that
identifier is required to apply or verify the cross-repository rule. Naming an implementation does
not admit its business behavior, source tree, or roadmap into the standard.

## Agent context contract

- Write each file for one category of agent task and each H2 section to answer one independently retrievable question.
- Make a retrieved section understandable without the surrounding conversation; name its subject, constraints, and prerequisites explicitly.
- Put required actions and constraints before rationale; include rationale only when it changes a later decision.
- Use exact, searchable identifiers where they are required to execute or verify the owning document.
- Keep overview documents shallow and link to the authority that owns the detail so an agent can load context progressively.
- Remove history, repetition, and commentary that does not change an agent's decision or execution.

## Document classes

| Class               | Location and name                         | Content                                                                   |
| ------------------- | ----------------------------------------- | ------------------------------------------------------------------------- |
| Repository standard | `docs/UPPER-KEBAB-CASE.md`                | Current repository-wide rules and cross-workspace architecture            |
| Domain standard     | `docs/<engineering-domain>/kebab-case.md` | Current cross-workspace rules for one engineering discipline              |
| Localized standard  | `docs/zh-CN/<same-standard-path>`         | Complete Simplified Chinese translation of an English standard            |
| Package guide       | `packages/*/README.md`                    | Public package capability, API, installation, and usage                   |
| Package reference   | `packages/*/docs/kebab-case.md`           | Current package-owned behavior, architecture, integration, or maintenance |
| Proposal            | `proposals/<owner>/kebab-case.md`         | Future decision with explicit status and owner, never current authority   |
| Research            | `research/<domain>/kebab-case.md`         | Time-bounded investigation, evidence, and unresolved findings             |

Only repository and domain standards belong under `docs/`. A directory name does not change a
package reference or product decision into a standard.

## Documentation localization

- Keep English standards under `docs/`, excluding locale subtrees, as the engineering authority. Runtime configuration, schemas, lint diagnostics, `CLAUDE.md`, and other repository contracts link to those English standards.
- Reserve `docs/zh-CN/` for the complete Simplified Chinese mirror. An English standard at `docs/<path>` maps exactly to `docs/zh-CN/<path>`, preserving its path and filename after `docs/`.
- Keep the standards mirror complete. Every authoritative Markdown file under `docs/` has exactly one counterpart under `docs/zh-CN/`; partial locale coverage is not allowed.
- Every addition, deletion, move, heading or section change, example, command, link, and content change under either standards tree updates its counterpart in the same change. A change is incomplete while the two versions differ in meaning, coverage, or structure.
- Translate all prose without adding, omitting, or weakening rules. Preserve exact identifiers, paths, commands, configuration keys, rule IDs, and code semantics. English resolves any conflict between the two versions.
- `pnpm test:lint-rules` verifies one-to-one standard coverage, structural markers, inline code identifiers, localized links, standard-directory admission, and concrete workspace references. Human review remains responsible for semantic translation fidelity and document classification.
- Package-local references, proposals, and research remain canonical English engineering material and do not receive files under `docs/zh-CN/`. A user-facing package guide may add its own locale-suffixed README when its audience requires one.

## Proposal lifecycle

- Store future decisions under `proposals/<owner>/`; never cite a proposal as a current engineering authority.
- Begin each proposal with `Status`, `Owner`, and `Exit condition` metadata.
- State the proposed change, current constraint, alternatives, consequences, and validation required before acceptance.
- When accepted and implemented, promote only repository-wide rules into `docs/`; promote package behavior into the owning package README, package reference, code, or tests.
- Delete the proposal after its decision is fully represented by current authorities. Git history preserves the discussion.

## Research lifecycle

- Store investigations under `research/<domain>/`; never use research as an authoritative source without re-verifying it.
- Begin each research file with `Status`, `Last verified`, and `Exit condition` metadata.
- Promote stable conclusions into the authority that owns them: a repository standard, package guide, package reference, code, configuration, test, or tracked decision.
- Delete the research file when its exit condition is met; Git history preserves the investigation if it is needed later.
- Re-verify active research before using it after its dependencies, upstream sources, or implementation have changed.

## Directory contract

- Keep only cross-project standards indexed by `CLAUDE.md` at the `docs/` root; root standard filenames use upper kebab case.
- Use a domain directory under `docs/` only for a stable engineering discipline whose standards apply across workspace boundaries. New standard-domain directories require explicit review and lint-contract admission.
- Never use `docs/<product>/` or `docs/<package>/` as storage for one owner's behavior. Use the owning `packages/*/README.md`, `packages/*/docs/`, `proposals/`, or `research/` location.
- Reserve BCP 47 locale directories such as `docs/zh-CN/` for complete localized standard mirrors; do not use locale identifiers as product or engineering domain names.
- Group package references, proposals, and research by their owner before document type, and add a deeper subdomain before a directory becomes a mixed flat list.
- Do not use catch-all directories such as `misc/`, `notes/`, `general/`, or `shared/`.
- A focused directory does not need an index file when its filenames make the contents unambiguous.

```text
docs/
  ARCHITECTURE.md
  frontend/
    react.md
packages/
  <package>/
    README.md
    docs/
      maintenance.md
proposals/
  <owner>/
    capability-change.md
research/
  <domain>/
    compatibility-analysis.md
```

## File contract

- Split files by independent responsibility, authority, or audience, not by an arbitrary line count.
- Use a single H1 that names the subject; omit introductions, summaries, and tables of contents when the first section can start immediately.
- Order current contracts before rationale, exceptions, and verification.
- Keep public product language in project or package READMEs, package implementation guidance in package-local references, and only repository standards in `docs/`.
- Do not create generic files such as `notes.md`, `misc.md`, or `todo.md`.

## Content contract

- Give each fact or rule one authoritative section; link to it instead of duplicating it across a standard and package reference.
- State standards as reusable constraints. Do not include a package source-tree inventory, named product component catalog, current screen measurements, product roadmap, or completed-task narrative.
- Use tables for mappings, lists for independent rules, prose for necessary reasoning, and code only when syntax changes meaning.
- Write commands from the repository root and state a different working directory explicitly.
- Verify time-sensitive external facts against their primary source and keep them in the owning package reference or active research, not a repository standard.
- Remove or reclassify guidance that cannot be verified instead of presenting it as current authority.

## Rules and examples

- Lint- or compiler-enforced rules use the exact rule or option name followed by one sentence describing the compliant state.
- Rules without machine enforcement include one minimal failing and passing example when prose alone could be misread.
- Examples use neutral placeholders or generic symbols unless a real identifier is necessary to apply the rule. An example never becomes a product inventory or future commitment.
- Examples contain only the lines required to expose the distinction and remain valid for the current project stack.

```text
❌ Copy one package's current component tree into a repository standard.
✅ Document the reusable component boundary and keep the concrete tree with its owning package.
```

## Links and anchors

- Use repository-relative Markdown links and target the authoritative section when one exists.
- Point code, tooling, and diagnostics to the English repository standard that owns a cross-repository rule; point package behavior and maintenance links to the owning package guide or reference.
- Links inside the Chinese mirror may stay within `docs/zh-CN/` for localized standard navigation. Other engineering authorities never treat the localized mirror as an independent source.
- Keep headings unique and stable because Markdown anchors are part of the documentation API.
- Update every link, lint diagnostic, script input, and package guide in the same change when a heading or file moves.
- Link external sources only when the external source remains authoritative; record project decisions in the correct local authority.

## Lint-doc contract

| Component                    | Requirement                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------- |
| Standard path                | Admit only reviewed root standards and engineering-domain directories under `docs/`       |
| Concrete workspace reference | Reference an existing workspace in current standards; put future topology in `proposals/` |
| Localized mirror             | Keep file coverage, structure, inline identifiers, and links synchronized                 |
| Explicit rule ID             | Use the exact configured ID in its authoritative standard section                         |
| Custom diagnostic            | State the action, violated constraint, and repository-relative `path#anchor`              |
| Change                       | Update rule implementation, config, standards, localized mirror, and rule tests together  |

```text
❌ Invalid test file.
✅ Rename unit test "parser.test.ts" to "parser.unit.test.ts". See docs/TESTING.md#source-and-test-paths.
```

## Change checklist

- Classify the content before writing: repository standard, package guide, package reference, proposal, or research.
- Confirm that every concrete workspace named by a current standard exists; describe future topology only in a proposal.
- Search for the old term, path, heading, rule ID, and script input before renaming or moving content.
- Verify that every changed standard has a synchronized localized counterpart in the same change.
- Verify commands and examples; do not document a command that was not run successfully.
- Run formatting, lint, typecheck, and relevant tests after documentation changes that alter engineering contracts.
