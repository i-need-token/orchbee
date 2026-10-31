# Documentation

## Ownership and admission

| Content                                                                            | Authority                                                               |
| ---------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Synchronized project value, capabilities, installation, and first use              | `README.md` and `README.zh-CN.md`                                       |
| Public package capability, API, installation, and usage                            | `packages/*/README.md`                                                  |
| Current package-owned behavior, architecture, integration, and maintenance         | Markdown under `packages/<package>/docs/`                               |
| AI-agent project map, hard rules, and standard links                               | `CLAUDE.md`                                                             |
| Repository-wide standards and cross-workspace architecture                         | `docs/UPPER-KEBAB-CASE.md` or `docs/<engineering-domain>/kebab-case.md` |
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
| Representation | Use the smallest clear form: prose for a compact rule, lists for independent items, and tables for actual comparisons or mappings. Use code only when syntax changes meaning.             |
| Scope          | Write each file for one task category and each H2 section to answer one independently retrievable question.                                                                               |
| Intent         | Express the exact intent completely and unambiguously: state the decision, scope, constraints, exceptions, prerequisites, and verification; never rely on implication or assumed context. |
| Order          | Put required actions and constraints before rationale; include rationale only when it changes a decision.                                                                                 |
| Structure      | Use one H1. Omit introductions, summaries, and tables of contents when the rules can begin immediately.                                                                                   |
| Precision      | Use exact, searchable identifiers. Write commands from the repository root and state a different working directory explicitly.                                                            |
| Current truth  | Remove history, repetition, commentary, and unverifiable guidance. Verify time-sensitive facts against primary sources and keep them with their owner.                                    |
| Enforced rules | Name the exact enforcing lint rule or compiler option; distinguish review requirements from automated checks.                                                                             |
| Examples       | Add one minimal failing and passing example only when prose could be misread; use neutral placeholders, current syntax, and no product inventory or commitment.                           |

## File and directory contract

| Location                     | Contract                                                                                                                                                                   |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/` root                 | Contains only cross-repository standards indexed by `CLAUDE.md`; filenames use `UPPER-KEBAB-CASE.md`.                                                                      |
| `docs/<engineering-domain>/` | Covers one stable discipline across workspace boundaries; filenames use `kebab-case.md`. New domains need review and lint-contract admission.                              |
| `packages/<package>/docs/`   | Contains package-owned references; filenames use `kebab-case.md`.                                                                                                          |
| `<owner>/docs/experience/`   | Contains schema-backed package-local experience Manifests under [`frontend/experience-components.md`](frontend/experience-components.md); filenames use `kebab-case.json`. |

- Never store one product's or package's behavior under `docs/<product>/` or `docs/<package>/`.
- Do not create catch-all directories or files such as `misc/`, `notes/`, `general/`, `shared/`, `misc.md`, `notes.md`, or `todo.md`.
- Split files by responsibility, authority, or audience, not line count.
- Omit an index when focused filenames make a directory clear.

## Language

- Repository Markdown is written in English and uses an unsuffixed `.md` filename. Do not add Chinese translations of other documents; `pnpm test:lint-rules` rejects Chinese Markdown outside the two files below.
- `README.zh-CN.md` is the only translated document. Update it with `README.md` in the same change, keeping meaning and coverage equal. English resolves conflicts.
- `apps/orchbee/docs/cli-agent-skill.zh-CN.md` is product copy that `orchbee --skill` prints under `zh-CN`. Update it with its English source in the same change.

## Links and anchors

- Use repository-relative links and target the authoritative section when available.
- Link code, tooling, and diagnostics to English repository standards; link package behavior to its package guide or reference.
- Keep headings unique and stable. Update links, diagnostics, script inputs, and package guides with every heading or file move.
- Link an external source only while it remains authoritative.

## Verification

| Change                | Required check                                                                                                                                                                                                                                           |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| New or moved content  | Classify it by the ownership table and confirm every referenced workspace exists.                                                                                                                                                                        |
| Rename or move        | Search for the old term, path, heading, rule ID, and script input.                                                                                                                                                                                       |
| Command or example    | Run it successfully before documenting it.                                                                                                                                                                                                               |
| Root README edit      | Update `README.md` and `README.zh-CN.md` in the same change.                                                                                                                                                                                             |
| Diagnostic edit       | State the action, violated constraint, and repository-relative `path#anchor`.                                                                                                                                                                            |
| Enforced rule edit    | Update the implementation, configuration, standard, and rule tests together.                                                                                                                                                                             |
| Markdown verification | Run `pnpm test:lint-rules`; it checks local inline-link paths and Markdown heading anchors across repository Markdown and the Chinese Markdown allowlist under [Language](#language). Fenced and inline code examples are excluded from link resolution. |
| Contract edit         | Run `pnpm format:check`, `pnpm lint`, `pnpm typecheck`, and relevant tests.                                                                                                                                                                              |
