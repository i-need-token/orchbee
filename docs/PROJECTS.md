# Project Contracts

## Purpose

| Concern          | Contract                                                                                                                                     |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Location         | Every direct `apps/*` or `packages/*` workspace keeps `project.json` beside `package.json`.                                                  |
| Purpose          | Give an agent enough local context to place a change without loading the repository.                                                         |
| Application rule | Placement and lifecycle follow [`docs/APPLICATIONS.md`](APPLICATIONS.md).                                                                    |
| Package rule     | Package contracts follow [`docs/PACKAGES.md`](PACKAGES.md).                                                                                  |
| Content          | Keep only facts not reliably derived from the path or `package.json`; identity, scripts, dependencies, and exports remain in `package.json`. |
| Schema           | `schemas/project-contract.schema.json` provides editor validation.                                                                           |
| Validation       | `pnpm lint:projects` parses the contract with Zod and rejects missing, stale, or invalid metadata.                                           |

## Required fields

| Field            | Contract                                                       |
| ---------------- | -------------------------------------------------------------- |
| `domain`         | Stable kebab-case product or engineering domain.               |
| `runtime`        | One or more of `browser`, `electron`, `node`, or `universal`.  |
| `stability`      | `experimental`, `stable`, or `deprecated`.                     |
| `responsibility` | One positive statement of the capability this workspace owns.  |
| `nonGoals`       | Explicit neighboring responsibilities that do not belong here. |

Responsibility and non-goals are architectural placement rules. Update them only when ownership genuinely moves; do not broaden them merely to justify an implementation.

## Change rules

- Add `project.json` in the same change that creates a workspace.
- Change `runtime` when emitted code or public types gain a new platform requirement.
- Change `stability` deliberately; promotion to `stable` requires reviewed API and behavioral contracts.
- Move a responsibility by updating both the source and destination contracts so ownership remains unambiguous.
- Keep public entry points in `package.json#exports` and verification commands in `package.json#scripts`.
- Do not add numerical quality budgets as merge blockers without measured evidence that the metric predicts defects with an acceptably low false-positive rate.

## Validation

`pnpm lint:projects` verifies every direct workspace contract and keeps the JSON Schema synchronized with its Zod source. For `packages/*`, it also verifies the `@orchbee/<directory>` name and the standard library scripts required by [`docs/PACKAGES.md`](PACKAGES.md#required-package-shape).

The validator deliberately does not score prose, count source lines, cap dependency quantities, or duplicate export maps. Dependency direction and source visibility remain enforced by the architecture lint rules.
