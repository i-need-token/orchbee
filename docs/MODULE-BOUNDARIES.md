# Module Boundaries

## Semantic module design

Apply this section to every authored language. Start with the accepted domain contract and modeling
record in [`MODELING-AND-CORRECTNESS.md`](MODELING-AND-CORRECTNESS.md); this section owns only the
mapping from those semantic responsibilities to source modules. Change authorization and task
slicing remain in [`DEVELOPMENT.md`](DEVELOPMENT.md), cross-workspace direction remains in
[`ARCHITECTURE.md`](ARCHITECTURE.md), and language-specific visibility remains in the sections below
and the owning language standard.

| Module role                 | Owned responsibility                                                                                  | Excluded responsibility                                                     |
| --------------------------- | ----------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Deterministic domain module | One cohesive domain contract, its canonical types, principal operation, result, and invariant set     | Workflow order, framework lifecycle, and external mechanisms                |
| State owner or aggregate    | One cohesive set of mutable facts, allowed transitions, invariants, and authoritative commit boundary | Unrelated state, another owner’s writes, and adapter mechanics              |
| Workflow                    | Phase order, short-circuiting, retry, recovery, and coordination through typed owner operations       | Reimplementing domain decisions or mutating another module’s internal state |
| Adapter                     | Translation between one external capability and canonical domain inputs, outputs, events, or commands | Domain policy and independent authoritative state                           |
| Composition root            | Construction, dependency wiring, and owned runtime lifetime                                           | Domain decisions and hidden state machines                                  |

- Admit a module only when its responsibility fits one sentence and its typed inputs, outputs,
  owner, allowed dependencies, affected claims, and evidence can be named.
- Split a responsibility when it contains independently changing contracts, owners, invariant
  sets, workflow phases, or adapter capabilities. Keep behavior together when one owner and commit
  boundary must preserve its invariants; line count, file count, framework category, and test
  convenience never determine the boundary.
- Put reusable code with the nearest semantic owner. Create a shared domain module only for a
  stable named contract with real consumers; coincidental duplication and catch-all utility
  groupings do not establish ownership.
- Communicate through typed operations, results, events, and commands. Do not expose internal state
  or mutation solely to make an extraction compile.
- Follow authority in the dependency direction: workflows and adapters depend on visible domain
  operations, while domain and state-owner modules never depend on their orchestrator, framework
  shell, or composition root.
- Reject an extraction that creates a cycle, a broad internal public API, callbacks into its former
  parent, or coordinated writes outside one owner. Return to the domain model and correct ownership
  or the commit boundary instead.

## Source module tree

`orchbee/module-boundaries` enforces:

| Concern                | Contract                                                                                                                                                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source module          | Every directory containing TypeScript source under `apps/*/src` or `packages/*/src`, except test directories, authored React component groups, and support directories owned by recognized public or private component trees.                     |
| Root module            | The `src` directory of a TypeScript workspace.                                                                                                                                                                                                    |
| Test directory         | `__tests__` belongs to its nearest source-module ancestor and does not create a module.                                                                                                                                                           |
| Boundary entry         | Every TypeScript source module has `index.ts`, except the root of a private package exporting only explicit subpaths or assets with no root export or `main`/`module`/`types` fields; see [Packages](PACKAGES.md#public-api-and-package-exports). |
| Same module            | Files import one another directly.                                                                                                                                                                                                                |
| Descendant to ancestor | A descendant may import an ancestor implementation directly because it is inside lexical visibility.                                                                                                                                              |
| Lateral dependency     | TypeScript imports target `index.ts`; paths below a lateral boundary remain private even when TypeScript resolves them.                                                                                                                           |

## Native source trees

`orchbee/module-boundaries` governs TypeScript and TSX imports only. A native application declares
its executable and importable module roots through its pinned native build graph and never adds a
placeholder `index.ts`. Before a native workspace grows beyond one source module, its owner must
define the language-specific visibility and lateral-import contract in an accepted owner-local
authority; relative-path reachability alone does not make another native subtree public. Rust
crate and module visibility follows [`docs/RUST.md`](RUST.md#modules-and-visibility); Zig visibility
follows [`docs/ZIG.md`](ZIG.md#naming-and-modules).

An application-internal Rust crate is a compilation boundary within its owning application,
admitted under [Rust](RUST.md#placement-and-workspace-ownership). Its library root explicitly
re-exports only the contracts consumed by the application or sibling internal crates. Moving a
module does not authorize wider state access, a callback into the application, or a dependency
cycle; preserve the semantic owner and break cycles before extraction.

## Structural component groups

| Concern                         | Contract                                                                                                                                                                                                              |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Recognition                     | An authored `components/<component-name>/` containing `<ComponentName>.tsx` is a private component ownership boundary under [`docs/frontend/react.md`](frontend/react.md#component-directory-boundary), not a module. |
| Ownership                       | Its full subtree, including recursive components, support directories, and declared semantic subsystems, belongs to the source-module ancestor owning the outermost matching component group.                         |
| Entry and imports               | It has no `index.ts` and is never imported across a source-module boundary; owner and implementation files import one another directly as members of the same source module.                                          |
| External dependencies           | Dependencies on other source modules follow the normal visibility table.                                                                                                                                              |
| Promotion                       | When another module needs a private component, promote the component and its complete ownership tree to an appropriate source-module boundary.                                                                        |
| Directory-name exception        | A directory named `components` is structural only when it matches the component-group shape; `packages/ui/src/components/ui` remains a normal public module tree.                                                     |
| Public boundary component files | Keep only `index.ts`, the root component `.tsx`, and optional same-named `.module.css` as direct component files.                                                                                                     |
| Public component support        | Support directories and cohesive semantic subsystems allowed by the React component contract remain structural parts of the source module and have no `index.ts`.                                                     |
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
