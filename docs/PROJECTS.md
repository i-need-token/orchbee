# Project Contracts

## Purpose

| Concern          | Contract                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Location         | Every direct `apps/*`, `packages/*`, or `crates/*` workspace and every `apps/*/crates/*` internal Rust crate keeps `project.json` beside its workspace manifest.                |
| Purpose          | Give an agent enough local context to place a change without loading the repository.                                                                                            |
| Application rule | Placement and lifecycle follow [`docs/APPLICATIONS.md`](APPLICATIONS.md).                                                                                                       |
| Package rule     | JavaScript and TypeScript package contracts follow [`docs/PACKAGES.md`](PACKAGES.md); Rust crate contracts follow [`docs/RUST.md`](RUST.md).                                    |
| Content          | Keep only facts not reliably derived from the path or owning manifest; identity, commands, dependencies, exports, targets, and implementation language remain in that manifest. |
| Schema           | `schemas/project-contract.schema.json` provides editor validation.                                                                                                              |
| Validation       | `pnpm lint:projects` parses the contract with Zod and rejects missing, stale, or invalid metadata.                                                                              |

## Required fields

| Field            | Contract                                                                |
| ---------------- | ----------------------------------------------------------------------- |
| `domain`         | Stable kebab-case product or engineering domain.                        |
| `runtime`        | One or more of `browser`, `electron`, `native`, `node`, or `universal`. |
| `stability`      | `experimental`, `stable`, or `deprecated`.                              |
| `responsibility` | One positive statement of the capability this workspace owns.           |
| `nonGoals`       | Explicit neighboring responsibilities that do not belong here.          |

Responsibility and non-goals are architectural placement rules. Update them only when ownership genuinely moves; do not broaden them merely to justify an implementation.

An application-internal crate names its owning application in `responsibility` and excludes
independent application lifecycle and use by other applications in `nonGoals`. Its path and Cargo
manifest remain the authorities for application membership and dependencies; no duplicate owner
field is needed.

## Change rules

- Add `project.json` in the same change that creates a workspace.
- Change `runtime` when emitted code or public types gain a new platform requirement.
- Change `stability` deliberately; promotion to `stable` requires reviewed API and behavioral contracts.
- Move a responsibility by updating both the source and destination contracts so ownership remains unambiguous.
- Keep npm public entry points and commands in `package.json`; keep Rust targets and dependencies
  in `Cargo.toml`. Root Cargo orchestration owns Rust verification commands.
- Do not add numerical quality budgets as merge blockers without measured evidence that the metric predicts defects with an acceptably low false-positive rate.

## Validation

`pnpm lint:projects` verifies every direct application, package, shared crate, and application-internal crate contract and keeps the
JSON Schema synchronized with its Zod source. For `packages/*`, it also verifies the
`@orchbee/<directory>` name and the standard library scripts required by
[`docs/PACKAGES.md`](PACKAGES.md#required-package-shape). For `crates/*` and `apps/*/crates/*`, it requires a Cargo
manifest; the root Rust gate validates workspace membership, the toolchain, and Cargo work.

The validator deliberately does not score prose, count source lines, cap dependency quantities, or duplicate export maps. Dependency direction and source visibility remain enforced by the architecture lint rules.
