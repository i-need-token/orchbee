# Module Boundaries

## Source module tree

`orchbee/module-boundaries` treats each directory in an `apps/*/src` or `packages/*/src` tree as a source module unless it is a test directory, an authored React component group, or a support directory owned by a recognized public or private component tree. The `src` directory is the root module. A `__tests__` directory belongs to its nearest source-module ancestor and does not create a module.

Every source module must have an `index.ts` boundary entry. Files in the same module may import one another directly. A descendant may also import an ancestor's implementation directly because ancestor internals are inside its lexical visibility. Every lateral dependency must import the target module's `index.ts`; a file path below a lateral boundary is private even when TypeScript can resolve it.

## Structural component groups

An authored `components/<component-name>/` directory that contains `<component-name>.tsx` is the structural form of a private component ownership boundary governed by [`docs/frontend/react.md`](frontend/react.md#component-directory-boundary), not a source module. Its entire subtree—including its `components`, `contexts`, `hooks`, `constants`, `types`, and `utils` directories—belongs to the source-module ancestor that owns the outermost matching component group.

Structural component groups do not define `index.ts` entries and are never imported across a source-module boundary. Their owner and their implementation files import one another directly as members of the same source module; dependencies on other source modules still follow the normal visibility table. Promote a private component and its complete ownership tree to an appropriate source-module boundary when another module needs it. A directory merely named `components` does not receive this treatment unless it contains the component-group shape, so the generated `packages/ui/src/components/ui` public module tree remains a normal source-module tree.

A source module organized around one public boundary component keeps only its `index.ts`, root component `.tsx`, and optional same-named `.module.css` as direct component files. Its component support directories use the same names and ownership as a private component group and remain structural parts of that source module rather than independent child modules. Non-component directories and directories outside a component ownership tree remain normal source modules and require their own `index.ts` boundaries.

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

An `index.ts` is an export manifest, not an implementation file. It may contain only explicit named re-exports with a source, including `export type`; imports, declarations, executable statements, local exports, default exports, and `export *` are forbidden.

An index may re-export its own module's implementation files and the `index.ts` of a direct child. Deeper APIs must be promoted one boundary at a time.

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

Production source must never import from `__tests__`. Tests inherit their owning module's visibility, including access to ancestor test helpers. Package-level tests in `src/__tests__` may exercise descendant modules through their `index.ts`, but never through private descendant files.

## Composition and cycles

Parent modules do not act as composition roots for their children. Put orchestration in a dedicated leaf module that consumes visible sibling boundaries, then re-export its API upward when needed.

A module implementation must not import its own `index.ts`, because that creates a barrel cycle; import the local implementation file directly.

`import/no-cycle` runs with `ignoreTypes: false`, so type-only cycles fail lint alongside runtime cycles. Break a cycle by moving shared contracts to the nearest mutually visible module, inverting the dependency behind an interface, or introducing a dedicated sibling module; do not suppress or downgrade the rule.
