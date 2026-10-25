# Linting

## Commands

| Command                                        | Responsibility                                                                                                                                    |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm verify`                                  | Run the complete merge gate defined by `docs/VERIFICATION.md`                                                                                     |
| `pnpm lint`                                    | Run the complete required repository lint pipeline                                                                                                |
| `pnpm lint:code`                               | Run blocking oxlint diagnostics with agent-oriented output                                                                                        |
| `pnpm lint:code:warnings`                      | Audit blocking diagnostics and non-blocking oxlint warnings                                                                                       |
| `pnpm lint:rust`                               | Validate the pinned Cargo workspace and run locked Clippy work when members exist                                                                 |
| `pnpm lint:documentation`                      | Validate every repository Markdown pair's path, receipt, and structure                                                                            |
| `pnpm lint:styles`                             | Validate the token registry and authored frontend styling contract                                                                                |
| `pnpm lint:storage`                            | Validate browser-storage access, ownership, key shape, and versioning                                                                             |
| `pnpm lint:dependencies`                       | Run complete and strict Knip analysis, then assess explicitly retained development consumers                                                      |
| `pnpm lint:projects`                           | Validate every schema-backed workspace project contract                                                                                           |
| `pnpm lint:experience`                         | Validate experience-component schemas, ownership, paths, and relations                                                                            |
| `pnpm lint:fix`                                | Apply safe oxlint fixes; remaining diagnostics require code changes                                                                               |
| `pnpm typecheck`                               | Run TypeScript diagnostics that syntax-aware lint does not replace                                                                                |
| `pnpm test:lint-rules`                         | Test the `orchbee/*` rules and English/Chinese standards contract                                                                                 |
| `pnpm test:dependency-scope`                   | Test complete and strict Knip graph coverage with isolated positive and negative fixtures                                                         |
| `pnpm test:rust-runner`                        | Test Rust target coverage, failure propagation, dependency inheritance/defaults, and authored unsafe/import policies with isolated Cargo fixtures |
| `pnpm test:documentation-pairing`              | Run negative tests for documentation pairing checks and confirmation                                                                              |
| `pnpm test:ghostty-source`                     | Test exact-pin and clean-worktree admission for Ghostty provenance tools                                                                          |
| `pnpm test:style-contract`                     | Run regression tests for the frontend style-contract checker                                                                                      |
| `pnpm test:storage-contract`                   | Run regression tests for the browser-storage contract checker                                                                                     |
| `pnpm test:experience-contract`                | Run regression tests for the experience catalog and lookup contract                                                                               |
| `pnpm docs:confirm-pairing -- <markdown-path>` | Update only explicitly named receipts after semantic review                                                                                       |

## Enforcement boundary

| Concern             | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rule authority      | Repository standards define explicitly configured and `orchbee/*` rules; the installed oxlint version governs upstream category rules.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Docs and config     | Every rule documented as enforced must be enabled in `.oxlintrc.json`; `pnpm test:lint-rules` verifies both docs-to-config and config-to-doc coverage.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Standards structure | `pnpm test:lint-rules` enforces one-to-one `docs/zh-CN/` coverage, structure, inline identifiers, localized links, admitted paths under `docs/`, and existing concrete `apps/<name>` and `packages/<name>` references.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Markdown content    | `pnpm lint:documentation` discovers tracked and unignored candidate `.md` files, enforces the `docs/zh-CN/` mirror or same-directory `.zh-CN.md` naming contract, and compares every English/Chinese pair with its SHA-256 receipt in `scripts/documentation-pairing.json`. After semantic review, `pnpm docs:confirm-pairing -- <markdown-path>` updates only named pairs; receipts detect unconfirmed content edits but cannot judge translation fidelity.                                                                                                                                                                                                                                          |
| Severity            | Errors fail `pnpm lint`; routine lint output omits non-blocking warnings to keep automated feedback concise. Run `pnpm lint:code:warnings` when reviewing warnings in affected package files.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Dependencies        | The dependency command preserves complete and strict Knip analysis and raw results. Unregistered unused files, exports or types and every dependency, resolution, duplicate-export or stale-catalog error fail. Exact development retentions follow [Retained development consumers](#retained-development-consumers).                                                                                                                                                                                                                                                                                                                                                                                |
| Knip scope          | Every runnable script, build configuration, test suite and public package entry must be discovered through package manifests, tool plugins or `knip.json#workspaces`. Add only missing actual entry points; redundant entries and stale configuration hints fail. Shared fixtures are project files reached through test consumers, never artificial entry points. Production-only test-helper exclusions retain complete-graph analysis; `pnpm test:dependency-scope` verifies reachability, orphan rejection and strict dependency isolation. Private packages enable `includeEntryExports`, so entry exports also need consumers; fabricated entries or test-only facades must not hide dead APIs. |
| Type awareness      | Type-aware oxlint rules remain inactive until package review adds `oxlint-tsgolint`; lint does not detect floating promises or unsafe third-party `any` propagation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Rust                | `pnpm lint:rust` verifies the exact toolchain and locked Cargo metadata, requires shared third-party dependencies to inherit with `workspace = true`, and rejects third-party declarations without `default-features = false` directly or by inheritance, including unused workspace declarations. It then runs workspace Clippy with every target and feature and enabled warnings denied. [`docs/RUST.md`](RUST.md#lint-policy-and-enforcement) defines the remaining review boundary.                                                                                                                                                                                                              |
| Styles              | `pnpm lint:styles` enforces [`docs/frontend/design-tokens.md`](frontend/design-tokens.md): registry, generated TypeScript inventory, CSS Module, Tailwind boundary, raw values, responsive thresholds, JSX classes, and inline styles. Generated public token names must equal the CSS registry; the full Tailwind map must exactly cover the installed public theme, target registered Orchbee tokens, and preserve compile-time literal breakpoint and container values.                                                                                                                                                                                                                            |
| Style exception     | The shadcn authoring exception for `pnpm lint:styles` is path-scoped to checked-in generated source and support files.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Browser storage     | `pnpm lint:storage` enforces [`docs/frontend/browser-storage.md`](frontend/browser-storage.md): only an application composition entry may access `localStorage` or `sessionStorage`; `orchbee:` key literals remain in their owning `*-storage.ts` module and use the versioned grammar.                                                                                                                                                                                                                                                                                                                                                                                                              |
| Experience catalog  | `pnpm lint:experience` enforces [`docs/frontend/experience-components.md`](frontend/experience-components.md): schema freshness, package-local ownership, existing owner-scoped paths, named Story exports and generated IDs, unique component IDs, and resolvable composition and deprecation relations.                                                                                                                                                                                                                                                                                                                                                                                             |
| Policy changes      | Never weaken a rule to pass existing code; fix the code or obtain human approval for a project-policy change.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

## Dependency graph boundaries

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

## Retained development consumers

`pnpm lint:dependencies` invokes `node scripts/dependency-retentions.mjs`. It runs the installed
Knip CLI over the complete graph and then with `--strict`, using `--reporter json` and a second
evidence reporter configured by `scripts/dependency-retentions-reporter.config.mjs`. These reporting
options do not change the analysis scope or private-entry flags.
The evidence envelope includes the actual report categories, counters, configuration and tag hints;
Knip's built-in JSON reporter alone does not include those hints.

The complete run must finish with raw exit zero and no diagnostics. A nonzero complete run fails
before strict execution. Strict raw exit zero is accepted only with no retention records. Strict
raw exit one may receive a separate aggregate acceptance only when every diagnostic exactly equals
an approved package-owned `(kind, file, name)` record and every record still has its diagnostic.
Only individual `files`, `exports` and `types` records are eligible. Dependencies, unresolved
imports, other categories, unknown fields, duplicates, hints, changed configuration, parse errors,
process errors, signals and timeouts fail. Counts, globs, ignores and artificial entries never grant
acceptance. Raw stdout, stderr and exit/signal/error results remain separate from the aggregate result
in the printed external log directory, including when the aggregate accepts a raw strict exit one.

Each package owns its admitted purpose, real development consumers, next integration owner and
removal conditions in `dependency-retentions.json` and its README. Consumer file presence is
insufficient: reviewed full source fingerprints bind actual render/call/assertion use, import/export
links and discovery/resolution configuration. The gate checks those fingerprints before and after
analysis; missing or changed inputs fail. Refreshing a digest requires an explicit reviewed change
with renewed semantic/graph evidence, never an automatic baseline update. These checks preserve an
already reviewed use; they do not establish new semantics or replace build, typecheck, Story,
behavior and export verification. Existing dependency-scope negative witnesses remain mandatory.

Add a real production consumer or withdraw a purpose and remove its records in the same change.
A vanished diagnostic, including strict exit zero with remaining records, fails as stale retention.
Remove an exhausted record/file and its explicit gate registration. Retentions do not authorize a
new public entry, dependency or product migration. The invoking task owns the raw logs and removes
them after the last diagnosis, review or recovery consumer finishes.

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
| `orchbee/no-inline-suppressions`            | `docs/CODESTYLE.md#inline-suppressions`               |
| `orchbee/no-react-use-callback`             | `docs/frontend/react.md#callback-identity`            |
| `orchbee/component-export-boundary`         | `docs/frontend/react.md#tsx-file-boundary`            |
| `orchbee/hook-export-boundary`              | `docs/frontend/react.md#hook-file-boundary`           |
| `orchbee/zustand-scope`                     | `docs/frontend/react.md#zustand-and-context`          |

Every custom-rule change updates its implementation, authority, diagnostic, and regression test together.
