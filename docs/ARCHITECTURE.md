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
- `orchbee/no-cross-workspace-source-imports`: Import another workspace package through its public package exports; never cross into another `apps/*/src` or `packages/*/src` tree.
- Relative imports may access internal modules only inside the current app or package.
- Each package defines its public API through `package.json#exports`; `src/index.ts` implements the root export when present.

## Current modules

| Module              | Responsibility                                                              | Public boundary                  |
| ------------------- | --------------------------------------------------------------------------- | -------------------------------- |
| `@orchbee/terminal` | Terminal parsing, rendering, input, selection, and browser host integration | `packages/terminal/src/index.ts` |

## Data boundaries

Parse untrusted process, network, storage, IPC, and user-input data at its first trusted boundary; pass validated types internally.

```ts
// ❌
const message = rawMessage as AgentMessage;

// ✅
const message = AgentMessageSchema.parse(rawMessage);
```
