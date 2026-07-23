# Module Boundaries

## Source module tree

`orchbee/module-boundaries` enforces:

| Concern                | Contract                                                                                                                                                                                         |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Source module          | Every directory under `apps/*/src` or `packages/*/src`, except test directories, authored React component groups, and support directories owned by recognized public or private component trees. |
| Root module            | The `src` directory.                                                                                                                                                                             |
| Test directory         | `__tests__` belongs to its nearest source-module ancestor and does not create a module.                                                                                                          |
| Boundary entry         | Every source module has `index.ts`.                                                                                                                                                              |
| Same module            | Files import one another directly.                                                                                                                                                               |
| Descendant to ancestor | A descendant may import an ancestor implementation directly because it is inside lexical visibility.                                                                                             |
| Lateral dependency     | Import the target `index.ts`; paths below a lateral boundary remain private even when TypeScript resolves them.                                                                                  |

## Structural component groups

| Concern                         | Contract                                                                                                                                                                                                              |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Recognition                     | An authored `components/<component-name>/` containing `<ComponentName>.tsx` is a private component ownership boundary under [`docs/frontend/react.md`](frontend/react.md#component-directory-boundary), not a module. |
| Ownership                       | Its full subtree, including `components`, `contexts`, `hooks`, `constants`, `types`, and `utils`, belongs to the source-module ancestor owning the outermost matching component group.                                |
| Entry and imports               | It has no `index.ts` and is never imported across a source-module boundary; owner and implementation files import one another directly as members of the same source module.                                          |
| External dependencies           | Dependencies on other source modules follow the normal visibility table.                                                                                                                                              |
| Promotion                       | When another module needs a private component, promote the component and its complete ownership tree to an appropriate source-module boundary.                                                                        |
| Directory-name exception        | A directory named `components` is structural only when it matches the component-group shape; `packages/ui/src/components/ui` remains a normal public module tree.                                                     |
| Public boundary component files | Keep only `index.ts`, the root component `.tsx`, and optional same-named `.module.css` as direct component files.                                                                                                     |
| Public component support        | Support directories use private-group names and ownership and remain structural parts of the source module.                                                                                                           |
| Other directories               | Non-component directories and directories outside a component ownership tree are source modules and require `index.ts`.                                                                                               |

## Module visibility

Source-module visibility follows lexical tree scope:

| Source module | Target module                          | Allowed | Required target     |
| ------------- | -------------------------------------- | ------- | ------------------- |
| `B/B1`        | `B/B1` itself                          | Yes     | Implementation file |
| `B/B1`        | `B` or another ancestor                | Yes     | Implementation file |
| `B/B1`        | `A`, a sibling of an ancestor          | Yes     | `index.ts`          |
| `B/B1`        | `B/B2`, a sibling at the current level | Yes     | `index.ts`          |
| `B/B1`        | `A/A1`, a different subtree            | No      | Hidden              |
| `B`           | `B/B1`, a child or descendant          | No      | Hidden              |
| `A/A1`        | `B/B1`, a cousin subtree               | No      | Hidden              |

The rule applies equally to runtime imports, type-only imports, dynamic imports, and re-exports.

## Boundary entry points

- `index.ts` is an export manifest, not an implementation file.
- It contains only explicit named re-exports with a source, including `export type`.
- Imports, declarations, executable statements, local exports, default exports, and `export *` are forbidden.
- It may re-export its own module's implementation files and a direct child's `index.ts`; promote deeper APIs one boundary at a time.

```text
src/
├── a/
│   ├── index.ts
│   └── a1/
│       └── index.ts
└── b/
    ├── index.ts
    └── b1/
        ├── index.ts
        └── consumer.ts
```

```ts
// src/a/index.ts: explicitly promote selected A1 API through A.
export { createA1 } from './a1';
export type { A1Options } from './a1';

// src/b/b1/consumer.ts
import { createA1 } from '../../a'; // Allowed through A's boundary.
import { createA1 as hiddenA1 } from '../../a/a1'; // Forbidden: A1 is hidden from B1.
```

The second import remains forbidden even though `src/a/a1/index.ts` exists. Its API becomes visible to `B/B1` only through the explicit exports of `src/a/index.ts`. TypeScript verifies that the requested symbol is actually exported.

## Tests

- Production source never imports from `__tests__`.
- Tests inherit their owning module's visibility, including ancestor test helpers.
- Package-level tests in `src/__tests__` may exercise descendants through `index.ts`, never private descendant files.

## Composition and cycles

- Parent modules are not composition roots for their children; put orchestration in a dedicated leaf consuming visible sibling boundaries, then re-export upward when needed.
- A module implementation never imports its own `index.ts`; import the local implementation file directly to avoid a barrel cycle.
- `import/no-cycle` runs with `ignoreTypes: false`; type-only and runtime cycles fail.
- Break cycles by moving shared contracts to the nearest mutually visible module, inverting the dependency behind an interface, or introducing a dedicated sibling module; never suppress or downgrade the rule.
