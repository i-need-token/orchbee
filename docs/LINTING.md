# Linting

[Verification](VERIFICATION.md#commands) lists the root commands and what each one checks.

## Enforcement boundary

| Concern             | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rule authority      | Repository standards define explicitly configured and `orchbee/*` rules; the installed oxlint version governs upstream category rules.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Docs and config     | Every rule documented as enforced must be enabled in `.oxlintrc.json`; `pnpm test:lint-rules` verifies both docs-to-config and config-to-doc coverage.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Standards structure | `pnpm test:lint-rules` enforces admitted paths under `docs/`, existing concrete `apps/<name>` and `packages/<name>` references, local Markdown links, and the [Chinese Markdown allowlist](DOCUMENTATION.md#language).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Severity            | Errors fail `pnpm lint`; routine lint output omits non-blocking warnings to keep automated feedback concise. Run `pnpm lint:code:warnings` when reviewing warnings in affected package files.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Dependencies        | `pnpm lint:dependencies` runs Knip directly with its own report, first over the complete graph and then with `--strict`. Every unused file, export or type and every dependency, resolution, duplicate-export, stale-catalog, configuration-hint or tag-hint finding fails; there is no list of accepted findings. Remove the dead code or give it a real consumer. `knip.config.mjs` changes only to describe a real entry point or project file, never to hide a finding.                                                                                                                                                                                                                                                                                                                                                                            |
| Knip scope          | Every runnable script, build configuration, test suite and public package entry must be discovered through package manifests, tool plugins or `knip.config.mjs#workspaces`. Add only missing actual entry points; redundant entries and stale configuration hints fail. Shared fixtures are project files reached through test consumers, never artificial entry points. Production-only test-helper exclusions retain complete-graph analysis; `pnpm test:dependency-scope` verifies reachability, orphan rejection and strict dependency isolation. Private packages enable `includeEntryExports`, so entry exports also need consumers; fabricated entries or test-only facades must not hide dead APIs.                                                                                                                                            |
| Type awareness      | Type-aware oxlint rules remain inactive until package review adds `oxlint-tsgolint`; lint does not detect floating promises or unsafe third-party `any` propagation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Rust                | `pnpm lint:rust` verifies the exact toolchain and locked Cargo metadata, requires shared third-party dependencies to inherit with `workspace = true`, and rejects third-party declarations without `default-features = false` directly or by inheritance, including unused workspace declarations. It then runs workspace Clippy with every target and feature, and again over default-feature library and binary targets, with enabled warnings denied. [`docs/RUST.md`](RUST.md#lint-policy-and-enforcement) defines the remaining review boundary.                                                                                                                                                                                                                                                                                                  |
| Rust modules        | `pnpm lint:rust-modules` reads each Cargo member's production module tree from `mod` declarations, with every `path` and `cfg_attr(path)` variant and without `cfg(test)` or `test-support` items, and resolves `crate`, `self`, `super`, and imported paths as text, so macro-generated references stay invisible. It reports sibling module cycles and, under a `docs/module-layers.md` contract, upward references and unassigned modules, reading each module's layer from the `// module-layer: <id>` line in its own file. A cycle edge or upward reference beyond the [Rust module record](#rust-module-record) fails `pnpm lint`, as do unreadable inputs, unresolved modules, and malformed contracts or layer lines; unassigned modules are reported without failing. [`docs/RUST.md`](RUST.md#modules-and-visibility) defines the contract. |
| Styles              | `pnpm lint:styles` enforces [`docs/frontend/design-tokens.md`](frontend/design-tokens.md): registry, generated TypeScript inventory, CSS Module, Tailwind boundary, raw values, responsive thresholds, JSX classes, and inline styles. Generated public token names must equal the CSS registry; the full Tailwind map must exactly cover the installed public theme, target registered Orchbee tokens, and preserve compile-time literal breakpoint and container values.                                                                                                                                                                                                                                                                                                                                                                             |
| Style exception     | The shadcn authoring exception for `pnpm lint:styles` is path-scoped to checked-in generated source and support files.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Browser storage     | `pnpm lint:storage` enforces [`docs/frontend/browser-storage.md`](frontend/browser-storage.md): only an application composition entry may access `localStorage` or `sessionStorage`; `orchbee:` key literals remain in their owning `*-storage.ts` module and use the versioned grammar.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Policy changes      | Never weaken a rule to pass existing code; fix the code or obtain human approval for a project-policy change.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

## Code style

Formatters own layout: `.oxfmtrc.json` for Oxfmt-managed files (tabs, single quotes, semicolons,
trailing commas), the root toolchain's `rustfmt` for Rust, and the workspace-pinned `zig fmt` for Zig
source and build files. Format changed files with their owning formatter while iterating.
`pnpm format` is for an intentional repository-wide format; a scoped change does not rewrite
unrelated files. `.gitattributes` keeps text files on LF so that checkout conversion cannot
invalidate formatting or byte-based checks; preserve its byte-sensitive exceptions.

The explicitly configured oxlint rules that no language standard owns:

- `import/no-default-export`: Use named exports except in configuration files and JavaScript lint-plugin entry points.
- `import/no-cycle`: Keep module dependencies acyclic.
- `import/no-self-import`: Do not import a module from itself.
- `import/no-duplicates`: Merge imports from the same module.
- `typescript/consistent-type-imports`: Mark type-only dependencies with `import type` or an inline `type` specifier.
- `no-var`: Declare bindings with `const` or `let`.
- `prefer-const`: Use `const` when a binding is never reassigned.
- `no-shadow`: Give nested bindings distinct names.
- `eqeqeq`: Use strict equality operators.
- `unicorn/no-nested-ternary`: Replace nested ternaries with named branches or control flow.
- `no-console`: Route runtime diagnostics through an owned logging or host boundary.
- `no-eval`, `no-implied-eval`, `no-new-func`: Do not execute code assembled from strings.

### Inline suppressions

- `orchbee/no-inline-suppressions`: Remove `oxlint-disable` and `eslint-disable` comments from production and test code; fix the code or change the project rule with human approval.

### Comments

Comments explain constraints and non-obvious decisions; names and structure explain behavior.

```ts
// ❌ Increment the retry count.
retryCount += 1;

// ✅ The bridge allows three transient failures before surfacing the host error.
retryCount += 1;
```

## Dependency graph boundaries

`knip.config.mjs` is the single graph configuration. Its callback reads CLI
`production ?? strict ?? false`, matching Knip production selection, including `--strict`.
Only tool exclusions vary by mode; entry, project, paths, plugins and private-entry checks stay
shared. Keep an exclusion only in modes where it suppresses a real tool diagnostic. A trailing
`!` on an ignored binary or dependency means production-only, not complete-only. Remove stale
exclusions instead of suppressing hints.

Knip does not discover `.mjs` configuration automatically. Every invocation must pass
`--config knip.config.mjs`, including direct CLI diagnostics and isolated fixtures.
The dependency-scope fixture imports the same callback and projects the shared graph fields;
it removes only tool exclusions unsupported by its synthetic payload.

An exact workspace configuration replaces a matching workspace glob; omitted fields use Knip's
defaults rather than inheriting the glob. Declare both `entry` and `project` when adding a private
workspace's `includeEntryExports` setting. Private packages keep this setting enabled in both the
complete and strict runs.

Mark actual production source patterns in `project` with `!`, so strict analysis records consumers
inside implementation files as well as entries. Discover Stories through the owning Storybook
configuration and plugin in the development graph; their `entry` and `project` patterns are not
production patterns. Test and Story consumers cannot keep unused production modules or private
entry APIs alive in strict mode. Generated inventories used only by tooling remain in the complete
graph instead of acquiring artificial runtime consumers.

Resolved internal type-only imports are consumers when their source files participate in the
analyzed graph. Classify runtime and development dependency declarations using actual JavaScript
and declaration consumers; a strict diagnostic alone does not establish that a dependency is only
used for types.

`pnpm test:dependency-scope` uses the current workspace graph and plugin settings, including
`includeEntryExports`. Its isolated application consumes private values and types through a
non-entry implementation, including bundled subpaths, and its Storybook host exercises configuration-based discovery. Negative
witnesses reject unconsumed private entries, test-only entry consumers in strict mode, orphan
helpers, production modules consumed only by tests or Stories, and borrowed runtime dependencies.
These fixtures verify graph policy, not the repository's current dependency results.

Knip follows workspace package entries to their built `dist` files. In an unbuilt checkout the entry
exports of every unbuilt package appear unused, so `scripts/lint-dependencies.mjs` first checks that
each buildable package under `packages/` has a `dist` folder and otherwise asks for
`pnpm build:packages`. It cannot detect a `dist` that is older than the source: rebuild after
changing a package's exports.

## Rust module record

`scripts/rust-module-graph-record.txt` lists the existing Rust module cycles and upward references,
one finding per line. `pnpm lint:rust-modules` passes those and fails on anything beyond them, so
the module graph cannot grow while the recorded findings are being removed.

| Concern          | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cycle edge       | A `cycle` line names one edge between two sibling modules of the same cycle group, per Cargo member and target. A new group, a module joining a group, and a new edge between two members of a group each add an edge and fail. An edge that a layer contract allows, from a higher layer to a lower one, is never counted: the opposite edge is the one at fault.                                                                                                              |
| Upward reference | An `upward` line gives the number of references from one module into a higher-layer module. The limit is the sum over one pair of assigned modules, so moving code between the submodules of the source passes and one more reference in the pair fails.                                                                                                                                                                                                                        |
| Rename or move   | Entries are compared by name. Recorded entries of a module that no longer exists cover the same number of unrecorded entries in the same member and target, so renaming or moving a module passes without a change to the record. Moving a cycle into another member fails: break it first, as [Module Boundaries](MODULE-BOUNDARIES.md#native-source-trees) requires.                                                                                                          |
| Removed finding  | A recorded entry that the tree no longer has never fails, and while its modules still exist it makes no room for a different finding. The check prints how many recorded lines no longer match the tree. Until the record is tightened, that same finding can come back without failing.                                                                                                                                                                                        |
| Tightening       | `pnpm lint:rust-modules -- --tighten` rewrites the record to the exact findings of a passing tree and writes nothing for a failing one. Run it in the change that removes a finding or renames a recorded module, and commit the result.                                                                                                                                                                                                                                        |
| Parallel changes | The record is sorted with one finding per line, so two changes that each remove findings delete different lines. After a conflict in the record, keep either side and run `--tighten` again. A change that adds a reference to a finding another change removed fails once it is rebased onto that change.                                                                                                                                                                      |
| New entry        | Removing the reference or restructuring is the default. An agent may add an entry only when restructuring to avoid the new edge would materially expand its task: `pnpm lint:rust-modules -- --owner-approved` records the tree as it is, the commit body states the reason, and the agent's final report lists the entry as a decision the owner may veto. A layer row added for an unassigned module can bring existing references into the count; the same rule covers them. |
| Failure output   | The failure names each new edge with its first source location, the shortest loop it closes, and how many of the other unrecorded edges leave the cycle without it, because one new reference can pull older edges into a cycle. For a grown pair it lists the source modules whose count rose, with their reference locations. `--details` removes the list bounds.                                                                                                            |

The check reads source text only, needs no network, and takes about two seconds for the whole
workspace (Windows 11 x64, Node.js 22, 2026-11-20). It covers Rust only: `import/no-cycle` and
`orchbee/module-boundaries` already fail every TypeScript import cycle and hidden import, so
TypeScript has no record.

## Generated source

| Scope                    | Contract                                                                                                                                                                                                                                                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Snapshot                 | Keep `packages/ui/src/components/ui/*.tsx` and the generated `use-mobile.ts` close to the upstream shadcn registry so CLI refreshes remain reviewable.                                                                                                                                                                   |
| Authoring exceptions     | A path-scoped override exempts explicit module-boundary return types, assertion, shadowing, equality and nested-ternary style, component directories and component/Hook export boundaries, and `use-mobile.ts` unique-symbol filename matching. The automatic JSX runtime makes `react/react-in-jsx-scope` inapplicable. |
| Active correctness rules | React Hooks, JSX keys, module boundaries, cross-workspace imports, unsafe TypeScript constructs, and inline suppression bans remain active.                                                                                                                                                                              |
| `ChartStyle`             | Its upstream-controlled style element is the only additional exception; [`docs/frontend/react.md`](frontend/react.md#correctness-and-safety) owns the constraint.                                                                                                                                                        |
| Orchbee-authored modules | Never extend generated-source overrides to them.                                                                                                                                                                                                                                                                         |
| Upstream refresh         | Review every refresh, preserve focused compatibility adaptations, and test Orchbee-owned behavior and integrations.                                                                                                                                                                                                      |

## Custom rule ownership

| Rule                                        | Authority                                             |
| ------------------------------------------- | ----------------------------------------------------- |
| `orchbee/component-directory`               | `docs/frontend/react.md#component-directory-boundary` |
| `orchbee/filename-naming-convention`        | `docs/TESTING.md#source-and-test-paths`               |
| `orchbee/module-boundaries`                 | `docs/MODULE-BOUNDARIES.md#module-boundaries`         |
| `orchbee/no-cross-workspace-source-imports` | `docs/ARCHITECTURE.md#package-boundaries`             |
| `orchbee/no-inline-suppressions`            | `docs/LINTING.md#inline-suppressions`                 |
| `orchbee/no-react-use-callback`             | `docs/frontend/react.md#callback-identity`            |
| `orchbee/component-export-boundary`         | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/hook-export-boundary`              | `docs/frontend/react.md#hook-file-boundary`           |
| `orchbee/zustand-scope`                     | `docs/frontend/react.md#zustand-and-context`          |

Every custom-rule change updates its implementation, authority, diagnostic, and regression test together.
