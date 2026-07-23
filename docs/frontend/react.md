# React

## Stack contract

| Concern       | Required choice            |
| ------------- | -------------------------- |
| Interface     | React with TypeScript      |
| Routing       | TanStack Router            |
| Shared state  | Zustand                    |
| UI primitives | shadcn/ui with Base UI     |
| Styling       | [`styling.md`](styling.md) |

An alternative router, shared-state library, or primitive system requires an explicit architecture decision.

## Component contract

- A product component renders prepared display values and binds prepared semantic actions.
- Prepare text, variants, visibility, items, accessibility labels, and actions before TSX; components do not reinterpret them as business rules.
- Supply data through direct props or a feature-scoped Zustand selector Hook; presentational components never access external resources directly.
- Keep workflows, external-resource synchronization, and raw-event translation outside presentational components.
- Extract an independently named interface region when it has its own input subset, state combination, or reason to change; reuse and line count are irrelevant.
- Prepare a discriminated display model outside TSX when a region renders finite product states, then use a focused renderer with no workflow dispatch.
- Generic primitives may own framework accessibility and visual interaction while exposing product-meaningful values and callbacks.

When React must connect a Provider or external lifecycle, isolate that integration in a narrow component. It calls one presenter or lifecycle Hook, creates or provides scoped state when needed, prepares direct-child inputs, and returns one product component. Startup, cross-feature workflows, persistence, transport selection, and platform implementations remain outside presentational components.

## Recursive component ownership

- Each named interface region is rooted at one component that owns its private descendants, component-specific Hooks, scoped Store and context, transitions, styles, assets, and tests.
- Nest ownership recursively; a private descendant may own another `components/<component-name>/` tree and narrower Store.
- A component re-exported by a source module is the public root; external modules consume it only through the module `index.ts`.
- Put reusable non-product primitives in `@orchbee/ui`; product components stay with the capability named by [`package-boundaries.md`](package-boundaries.md#ui-ownership).
- Place every artifact at the narrowest component subtree or source module that completely owns it; never collect owned artifacts in package-wide technical directories.
- When a private subtree gains an external consumer, promote the complete ownership unit instead of exporting one internal file.

## Component Hook boundary

- `orchbee/no-react-hooks-in-tsx`: Authored production `.tsx` files never call built-in React Hooks directly.
- Move state, Effects, memoization, refs, and external-store subscriptions into a responsibility-focused custom Hook.
- A narrow adapter may call one presenter or lifecycle Hook and pass its prepared result without further derivation or workflow.
- A presenter Hook may compose localization, Store selectors, and semantic actions into one renderable contract.
- Tests, Stories, and checked-in shadcn source under [`docs/LINTING.md`](../LINTING.md#generated-source) are exempt.

## Component data delivery

| Situation                                    | Delivery                                                                  |
| -------------------------------------------- | ------------------------------------------------------------------------- |
| Immediate parent-child data                  | Direct props                                                              |
| Reusable component outside one feature Store | Direct props                                                              |
| One owner shared by several descendants      | Context-scoped feature Store with narrow descendant selectors             |
| Sibling branches sharing UI state            | Store owned by their nearest common component root                        |
| Component inputs with different owners       | Combine props and Store selections without mirroring props into the Store |

Props remain the default for shallow ownership. Intermediate layout components neither forward unused values nor subscribe for descendants. Context, Store, and selector Hooks stay private to their subtree; external modules communicate through public props and semantic actions.

## TSX file boundary

- `orchbee/one-function-per-tsx`: Each authored production `.tsx` file has at most one top-level function.
- That function is the sole named runtime export, is a PascalCase component, and exactly matches the filename stem.
- Nested callbacks, type-only exports, constants, and lookup data do not consume the function slot but must support the component's single responsibility.
- Put a child component in a symbol-matched PascalCase `.tsx` file and a presenter Hook in a symbol-matched `useHookName.ts` file.
- Framework-independent helpers use the exact camelCase function name for one callable export or a responsibility-focused kebab-case filename when no runtime symbol owns the module.
- Tests, Stories, and checked-in shadcn source are exempt.

## Component directory boundary

- `orchbee/component-directory`: A private authored component lives at `components/<component-name>/<ComponentName>.tsx` and has no `index.ts`.
- A public boundary component is directly re-exported by its source module `index.ts`; its kebab-case source-module directory contains only `index.ts`, `<ComponentName>.tsx`, and optional `<ComponentName>.module.css` as direct files.
- A private component directory contains only `<ComponentName>.tsx` and optional `<ComponentName>.module.css` as direct files.
- Define Props in the component `.tsx`; export and re-export that type only when it belongs to the public API.
- Do not create `<component-name>-props.ts` merely to move one component's Props out of TSX.
- Repeat the private structure recursively; a child of `Toolbar.tsx` belongs under `components/action/Action.tsx` inside the toolbar owner.

Create a support directory only with concrete owned contents:

| Directory     | Allowed component-owned contents                                                                 |
| ------------- | ------------------------------------------------------------------------------------------------ |
| `components/` | Recursive private component directories                                                          |
| `contexts/`   | One Context-scoped Store module containing State, actions, creation, Provider, and selector Hook |
| `hooks/`      | Behavior, presenter, action, and lifecycle Hooks; never a wrapper around the Store selector Hook |
| `constants/`  | Static values and finite configuration owned only by the component                               |
| `types/`      | Named structures, unions, and enums shared by multiple files in the subtree                      |
| `utils/`      | Framework-independent pure helpers owned only by the component                                   |
| `__tests__/`  | Unit tests required by [`docs/TESTING.md`](../TESTING.md)                                        |

- Keep resources shared by sibling trees at their nearest common owner; keep cross-feature workflows, external integrations, and contracts in the owning source module.
- Do not add barrels inside private or support directories; promote the full tree when another module needs it.
- Keep a Context Store and `use<ComponentName>Store(selector)` in one module under `contexts/`; never add a forwarding `hooks/useFilterableListStore.ts`.
- Keep Props used by one component in its `.tsx`; use `types/` only for shared or independently meaningful data.
- Keep an optional CSS Module beside its importing `.tsx`; do not create `styles/`.
- Stories live under the owning workspace's top-level `stories/` contract in [`storybook.md`](storybook.md).
- Checked-in shadcn source is exempt.

## State ownership

| State or behavior                                                    | Owner                                                     |
| -------------------------------------------------------------------- | --------------------------------------------------------- |
| Product input, disclosure, selection, and focus                      | Presenter Hook or feature-scoped Store                    |
| Framework-only accessibility or visual interaction                   | Generic UI primitive                                      |
| State shared by one mounted feature subtree                          | Store instance scoped to that subtree                     |
| Domain transitions and invariants                                    | Pure transitions used by the owning Hook or Store         |
| Network, stream, subscription, timer, and browser-resource lifecycle | Responsibility-focused lifecycle Hook or platform adapter |
| Startup inputs needed by first render                                | Composition root before `createRoot(...).render(...)`     |

[`package-boundaries.md`](package-boundaries.md#state-ownership) remains authoritative across workspaces.

## Derived state and transitions

- Derive values in selectors, presenter Hooks, or pure presenter functions; never mirror derivable values into state with an Effect.
- Preserve multi-field invariants through semantic actions or pure previous-state transitions.
- Parse external input at its trust boundary, then pass validated domain values to transitions and prepared values to components.

## Effects and resource lifecycles

- Presentational components use no Effects.
- A presenter Hook, Provider, or adapter uses one Effect only to synchronize with one external resource and returns complete idempotent cleanup.
- Keep acquisition, subscription, cancellation, and release together.
- Use responsibility-focused lifecycle Hooks for subscriptions, staged initialization, retry, ordering, or recovery; each Effect only connects and disposes one external resource.
- Put cross-Store synchronization in the coordinator that owns the relationship.
- Components receive semantic actions; primitives or adapters normalize raw events, and the owning feature Hook or Store action coordinates the workflow.

## Custom Hooks

- A presenter Hook returns a complete renderable component contract with no remaining business derivation or event interpretation.
- Extract a Hook only when it owns a complete React lifecycle or reusable React behavior, never merely to hide `useState`, `useMemo`, `useRef`, or `useEffect`.
- Put framework-independent calculations in pure functions and external-resource synchronization in lifecycle Hooks.

## Hook file boundary

- `orchbee/one-hook-per-file`: Each authored Hook module is named for its exact camelCase export, contains one top-level function, and exports only that Hook.
- Use `useKeyboardNavigation.ts` or `useKeyboardNavigation.tsx` naming; the function matches the filename stem.
- Move helpers to focused `.ts` modules and public contracts to type-focused modules.
- Nested Effect callbacks and adapters do not consume a function slot.
- Tests and generated `packages/ui/src/hooks/use-mobile.ts` are exempt.

## Zustand and context

- `orchbee/zustand-scope`: Create authored Stores with `createStore` from `zustand/vanilla`; import `useStore` only in the Context Store module or shared factory.
- Create one Store lazily per owning Provider mount, inject complete initial state, keep its stable `StoreApi` in context, and wrap exactly the consuming subtree.
- Scope the Store to the narrowest mounted owner; repeated component mounts use separate Providers.
- Reconcile later external inputs through their relationship owner.
- When external data is authoritative outside React, the owning Provider subscribes once and reconciles one projection into the Store; descendants have no second React-facing subscription source.
- Export `use<ComponentName>Store(selector)` beside its Provider, reject use outside it, and require a narrow selector.
- Consumers select only rendered state and invoked semantic actions; never read the whole Store, publish reusable selector constants, or subscribe an intermediate component for a descendant.
- Expose invariant-preserving actions rather than unrestricted mutation.
- Store feature state and domain actions, not callbacks created by Hooks; provide stable external capabilities through composition.

## Testing boundaries

- Test pure transitions and selectors without React.
- Test lifecycle Hooks and platform adapters with injected fakes for acquisition, event forwarding, cancellation, failures, and idempotent cleanup.
- Render direct-props components without product Providers, Stores, or mocks for external integrations.
- Test Store-connected components with isolated Providers and deterministic initial state.
- Test presenter Hooks through their prepared public interface; keep adapter and Provider integration narrow.
- Never patch React internals or duplicate production algorithms in tests.
- Follow [`docs/TESTING.md`](../TESTING.md).

## Correctness and safety

| Rule                        | Required state                                       |
| --------------------------- | ---------------------------------------------------- |
| `react/rules-of-hooks`      | Hooks run only at component or custom-Hook top level |
| `react/exhaustive-deps`     | Hook callbacks declare every reactive dependency     |
| `react/jsx-key`             | List keys come from stable data identity             |
| `react/jsx-no-target-blank` | New-tab links cannot access the opener               |
| `react/no-danger`           | Structured React content replaces injected HTML      |

The generated shadcn `ChartStyle` is the sole `react/no-danger` exception; its style text contains only trusted developer-authored chart configuration, never persisted or otherwise untrusted values.
