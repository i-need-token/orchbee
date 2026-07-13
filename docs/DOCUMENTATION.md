# Documentation

## Authority

| Artifact                       | Audience                   | Responsibility                                                  |
| ------------------------------ | -------------------------- | --------------------------------------------------------------- |
| `README.md`, `README.zh-CN.md` | Users                      | Project value, capabilities, installation, and first use        |
| Package `README.md`            | Package consumers          | Public API and package-specific usage                           |
| `CLAUDE.md`                    | AI agents                  | Short project map, hard rules, and links to authoritative specs |
| `docs/*.md`                    | Maintainers and AI agents  | Architecture, engineering rules, and operational contracts      |
| `research/*.md`                | Maintainers and AI agents  | Time-bounded evidence and open investigation, never authority   |
| Lint diagnostics               | AI agents during execution | Violation, corrective action, and authoritative docs link       |

## Agent context contract

- Write each file for one category of agent task and each H2 section to answer one independently retrievable question.
- Make a retrieved section understandable without the surrounding conversation; name its subject, constraints, and prerequisites explicitly.
- Put required actions and constraints before rationale; include rationale only when it changes a later decision.
- Use exact, searchable identifiers such as paths, commands, package names, configuration keys, and rule IDs.
- Keep overview documents shallow and link to focused detail so an agent can load context progressively.
- Remove history, repetition, and commentary that does not change an agent's decision or execution.

## Document classes

| Class             | Location and name                       | Content                                                         |
| ----------------- | --------------------------------------- | --------------------------------------------------------------- |
| Project contract  | `docs/UPPER-KEBAB-CASE.md`              | Current cross-project rules and architecture                    |
| Focused reference | `docs/<domain>/kebab-case.md`           | Current subsystem behavior, APIs, or external integration facts |
| Proposal          | `docs/proposals/<domain>/kebab-case.md` | Future design that is not yet authoritative                     |
| Research          | `research/<domain>/kebab-case.md`       | Time-bounded investigation, evidence, and unresolved findings   |
| Package guide     | `packages/*/README.md`                  | Public package API, installation, and usage                     |

Active docs describe current truth by default; proposals begin with `> Status: Draft` and are merged into authoritative docs or deleted when resolved.

## Research lifecycle

- Store investigations under `research/<domain>/`; never use research as an authoritative source without re-verifying it.
- Begin each research file with `Status`, `Last verified`, and `Exit condition` metadata.
- Promote stable conclusions into the affected authoritative docs, code, configuration, or tracked work item.
- Delete the research file when its exit condition is met; Git history preserves the investigation if it is needed later.
- Re-verify active research before using it after its dependencies, upstream sources, or implementation have changed.

## Directory contract

- Keep only cross-project contracts indexed by `CLAUDE.md` at the `docs/` root.
- Put every other document under a stable domain directory such as `docs/terminal/`, `docs/agents/`, or `docs/security/` from its first version.
- Group by product or engineering domain before document type; use `docs/proposals/<domain>/` only to separate future design from current truth.
- Add a deeper subdomain before a domain directory becomes a mixed flat list; do not wait for the directory to become crowded.
- Do not use catch-all directories such as `misc/`, `notes/`, `general/`, or `shared/`.
- A domain directory does not need an index file when its filenames make the contents unambiguous.

```text
docs/
  ARCHITECTURE.md
  CODESTYLE.md
  terminal/
    ghostty-wasm-api.md
  proposals/
    terminal/
      renderer-recovery.md
research/
  terminal/
    surface-gap-analysis.md
```

## File contract

- Split files by independent responsibility or audience, not by an arbitrary line count.
- Use a single H1 that names the subject; omit introductions, summaries, and tables of contents when the first section can start immediately.
- Order current contracts before rationale, exceptions, and verification.
- Keep public product language in READMEs and implementation guidance in `docs/`.
- Do not create generic files such as `notes.md`, `misc.md`, or `todo.md`.

## Content contract

- Give each fact or rule one authoritative section; link to it instead of duplicating it.
- Use tables for mappings, lists for independent rules, prose for necessary reasoning, and code only when syntax changes meaning.
- Write canonical documentation in English; localized counterparts use a locale suffix and keep the English source authoritative.
- Write commands from the repository root and state a different working directory explicitly.
- Verify time-sensitive external facts against their primary source; record the source and verification date when the fact can expire.
- Remove or reclassify guidance that cannot be verified instead of presenting it as current truth.

## Rules and examples

- Lint- or compiler-enforced rules use the exact rule or option name followed by one sentence describing the compliant state.
- Rules without machine enforcement include one minimal failing and passing example when prose alone could be misread.
- Examples contain only the lines required to expose the distinction and must remain valid for the current project stack.

```text
❌ Run the checks.
✅ Run `pnpm typecheck` from the repository root.
```

## Links and anchors

- Use repository-relative Markdown links and target the authoritative section when one exists.
- Keep headings unique and stable because Markdown anchors are part of the docs API.
- Update every link and lint diagnostic in the same change when a heading or file moves.
- Link external sources only when the external source remains authoritative; record project decisions locally.

## Lint-doc contract

| Component         | Requirement                                                                  |
| ----------------- | ---------------------------------------------------------------------------- |
| Explicit rule ID  | Use the exact configured ID in its authoritative docs section                |
| Custom diagnostic | State the action, violated constraint, and repository-relative `path#anchor` |
| Documentation     | Describe the compliant state without repeating the diagnostic implementation |
| Change            | Update rule implementation, config, docs, and rule tests together            |

```text
❌ Invalid test file.
✅ Rename unit test "terminal.test.ts" to "terminal.unit.test.ts". See docs/TESTING.md#source-and-test-paths.
```

## Change checklist

- Search for the old term, path, heading, and rule ID before renaming or deleting content.
- Verify commands and examples; do not document a command that was not run successfully.
- Run formatting, lint, typecheck, and relevant tests after documentation changes that alter engineering contracts.
