# Project Contracts

## Purpose

Every direct workspace under `apps/*` or `packages/*` contains `project.json` beside `package.json`. The project contract gives a fresh agent the local context needed to place a change without loading the entire repository.

Application placement and lifecycle follow [`docs/APPLICATIONS.md`](APPLICATIONS.md). Package contracts follow [`docs/PACKAGES.md`](PACKAGES.md).

Keep only information that cannot be derived reliably from the path or `package.json`. Package identity, scripts, dependencies, and public exports remain authoritative in `package.json`; do not duplicate them in `project.json`.

`schemas/project-contract.schema.json` provides editor validation. `pnpm lint:projects` parses the same contract with Zod and rejects missing, stale, or invalid metadata.

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
