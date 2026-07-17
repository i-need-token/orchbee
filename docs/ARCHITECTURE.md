# Architecture

## Dependency direction

```text
apps/* -> packages/* -> external dependencies
```

- Apps are runtime entry points and never dependencies of packages or other apps.
- Packages may depend on other packages only through their declared package exports.
- Code shared by multiple apps belongs in a package.

## Package boundaries

- Package naming, manifests, public exports, builds, dependencies, and versioning follow [`docs/PACKAGES.md`](PACKAGES.md).
- Frontend product UI, platform adapters, and session-facing libraries follow [`docs/frontend/package-boundaries.md`](frontend/package-boundaries.md).
- `orchbee/no-cross-workspace-source-imports`: Import another workspace package through its public package exports; never cross into another `apps/*/src` or `packages/*/src` tree.
- Relative imports inside one app or package follow [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md).
- Each package defines its public API through `package.json#exports`; `src/index.ts` implements the root export when present.

## Source module boundaries

Source trees use hierarchical visibility and explicit `index.ts` entry points. The authoritative visibility, re-export, test, composition, and cycle rules live in [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md) and are enforced by `orchbee/module-boundaries` plus `import/no-cycle`.

## Project registry

Every workspace declares its domain, responsibility, non-goals, runtime, and stability in a schema-backed `project.json`. [`docs/PROJECTS.md`](PROJECTS.md) is the authoritative contract; `pnpm lint:projects` verifies the registry without growing this overview into a manually maintained project catalog.

## Data boundaries

Parse untrusted process, network, storage, IPC, and user-input data at its first trusted boundary; pass validated types internally.

```ts
// ❌
const message = rawMessage as AgentMessage;

// ✅
const message = AgentMessageSchema.parse(rawMessage);
```
