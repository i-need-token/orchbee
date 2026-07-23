# Documentation

## Ownership and admission

| Content                                                                            | Authority                                                               |
| ---------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Synchronized project value, capabilities, installation, and first use              | `README.md` and `README.zh-CN.md`                                       |
| Public package capability, API, installation, and usage                            | `packages/*/README.md`                                                  |
| Current package-owned behavior, architecture, integration, and maintenance         | `packages/<package>/docs/`                                              |
| AI-agent project map, hard rules, and standard links                               | `CLAUDE.md`                                                             |
| Repository-wide standards and cross-workspace architecture                         | `docs/UPPER-KEBAB-CASE.md` or `docs/<engineering-domain>/kebab-case.md` |
| Simplified Chinese standards mirror                                                | `docs/zh-CN/<same-standard-path>`                                       |
| Workspace identity, responsibility, runtime, stability, and non-goals              | The owning workspace's `project.json`                                   |
| Product interactions, screen composition, named components, and business decisions | The owning application or package                                       |

- Prefer omission: create or retain documentation only when it has a recurring consumer, clear authority, and verification path.
- Admit content to `docs/` only when it governs repository operation, multiple workspaces, or a workspace boundary.
- Never use a repository standard to inventory one owner's behavior, source tree, screen measurements, or roadmap.
- Name a concrete path, package, command, configuration key, or rule ID only when required to apply or verify the rule.
- Give each fact or rule one authority; link to it instead of duplicating it.

## Writing contract

| Rule           | Requirement                                                                                                                                                                               |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Brevity        | If one sentence can say it, do not write two. Delete boilerplate, transitions, and summary paragraphs.                                                                                    |
| Filler         | Do not use filler openings such as “The principle is…”, “In other words…”, or “It should be noted that…”.                                                                                 |
| Representation | Prefer tables to prose and lists to paragraphs. Use prose only for necessary reasoning and code only when syntax changes meaning.                                                         |
| Scope          | Write each file for one task category and each H2 section to answer one independently retrievable question.                                                                               |
| Intent         | Express the exact intent completely and unambiguously: state the decision, scope, constraints, exceptions, prerequisites, and verification; never rely on implication or assumed context. |
| Order          | Put required actions and constraints before rationale; include rationale only when it changes a decision.                                                                                 |
| Structure      | Use one H1. Omit introductions, summaries, and tables of contents when the rules can begin immediately.                                                                                   |
| Precision      | Use exact, searchable identifiers. Write commands from the repository root and state a different working directory explicitly.                                                            |
| Current truth  | Remove history, repetition, commentary, and unverifiable guidance. Verify time-sensitive facts against primary sources and keep them with their owner.                                    |
| Enforced rules | Name the exact lint rule or compiler option and describe the compliant state in one sentence.                                                                                             |
| Examples       | Add one minimal failing and passing example only when prose could be misread; use neutral placeholders, current syntax, and no product inventory or commitment.                           |

## File and directory contract

| Location                     | Contract                                                                                                                                      |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/` root                 | Contains only cross-repository standards indexed by `CLAUDE.md`; filenames use `UPPER-KEBAB-CASE.md`.                                         |
| `docs/<engineering-domain>/` | Covers one stable discipline across workspace boundaries; filenames use `kebab-case.md`. New domains need review and lint-contract admission. |
| `docs/zh-CN/`                | Contains only the complete Simplified Chinese mirror at the same standard paths.                                                              |
| `packages/<package>/docs/`   | Contains package-owned references; filenames use `kebab-case.md`.                                                                             |

- Never store one product's or package's behavior under `docs/<product>/` or `docs/<package>/`.
- Do not create catch-all directories or files such as `misc/`, `notes/`, `general/`, `shared/`, `misc.md`, `notes.md`, or `todo.md`.
- Split files by responsibility, authority, or audience, not line count.
- Omit an index when focused filenames make a directory clear.

## Localization

- English standards under `docs/` are authoritative; every `docs/<path>` standard has exactly one `docs/zh-CN/<path>` counterpart.
- Update both languages in the same change for every addition, deletion, move, heading, example, command, link, or content edit; keep meaning, coverage, and structure equal.
- Translate all prose; preserve identifiers, paths, commands, configuration keys, rule IDs, and code semantics exactly. English resolves conflicts.
- Runtime configuration, schemas, lint diagnostics, `CLAUDE.md`, and engineering authorities link to English standards. Links inside `docs/zh-CN/` may stay in the localized mirror.
- Package references remain English and do not enter `docs/zh-CN/`; user-facing package guides may add locale-suffixed READMEs.

## Links and anchors

- Use repository-relative links and target the authoritative section when available.
- Link code, tooling, and diagnostics to English repository standards; link package behavior to its package guide or reference.
- Keep headings unique and stable. Update links, diagnostics, script inputs, and package guides with every heading or file move.
- Link an external source only while it remains authoritative.

## Verification

| Change                 | Required check                                                                                                                                        |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| New or moved content   | Classify it by the ownership table and confirm every referenced workspace exists.                                                                     |
| Rename or move         | Search for the old term, path, heading, rule ID, and script input.                                                                                    |
| Command or example     | Run it successfully before documenting it.                                                                                                            |
| Standard edit          | Update the localized counterpart in the same change.                                                                                                  |
| Diagnostic edit        | State the action, violated constraint, and repository-relative `path#anchor`.                                                                         |
| Enforced rule edit     | Update the implementation, configuration, standard, localized mirror, and rule tests together.                                                        |
| Standards verification | Run `pnpm test:lint-rules`; it checks mirror coverage, structure, inline identifiers, localized links, directory admission, and workspace references. |
| Contract edit          | Run `pnpm format:check`, `pnpm lint`, `pnpm typecheck`, and relevant tests.                                                                           |
