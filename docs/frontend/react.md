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

- Keep domain decisions and invariants with their semantic owner. Components may derive simple display values from props and local state; do not duplicate domain policy in rendering.
- Supply data through direct props or a feature-scoped Zustand selector Hook; presentational components never access external resources directly.
- Keep cross-feature workflows, persistence, transport selection, and platform implementations outside presentational components. Simple event-to-value translation may stay beside its handler.
- Extract an interface region when it has an independently changing responsibility or lifecycle. Keep cohesive private render helpers together; function count and line count do not determine ownership.
- Use a prepared discriminated display model when it keeps complex product-state policy out of rendering.
- Generic primitives may own framework accessibility and visual interaction while exposing product-meaningful values and callbacks.

Keep Provider and external-lifecycle integration narrow and explicit. A component may compose the Hooks needed for its responsibility; neither one Hook call nor one child component is a required shape.

## Recursive component ownership

- Each named interface region is rooted at one component that owns its private descendants, component-specific Hooks, scoped Store and context, transitions, styles, assets, and tests.
- Nest ownership recursively; a private descendant may own another `components/<component-name>/` tree and narrower Store.
- A component re-exported by a source module is the public root; external modules consume it only through the module `index.ts`.
- Put reusable non-product visual and accessibility primitives in `@orchbee/ui`. Put admitted
  domain-neutral React programming primitives in `@orchbee/foundation/react` according to
  [`docs/PACKAGES.md`](../PACKAGES.md#foundation-boundary); product components stay with the
  capability named by [`package-boundaries.md`](package-boundaries.md#ui-ownership).
- Place every artifact at the narrowest component subtree or source module that completely owns it; never collect owned artifacts in package-wide technical directories.
- When a private subtree gains an external consumer, promote the complete ownership unit instead of exporting one internal file.

## Component Hook boundary

- Components may call built-in React Hooks for owned local state, refs, identifiers, and presentation calculations, subject to [Callback identity](#callback-identity) and [Effects and resource lifecycles](#effects-and-resource-lifecycles).
- Extract a custom Hook for cohesive reusable behavior, complex state policy, or an external-resource lifecycle. Do not add a forwarding Hook solely to move a built-in call out of TSX.
- A presenter Hook may compose localization, Store selectors, and semantic actions when that preparation has a distinct responsibility.
- Hook call order, dependencies, state ownership, and cleanup remain required regardless of file placement.

## Callback identity

- `orchbee/no-react-use-callback`: Repository source never imports, calls, re-exports,
  aliases, or otherwise accesses React's `useCallback`; checked-in generated source has no
  exception.
- Use `useMemoizedFn` from `@orchbee/foundation/react` for event handlers, imperative adapters, and
  callbacks exposed through component or Hook contracts when callers need stable identity and the
  implementation must read the latest render values.
- Represent a consumer's need to react to semantic changes with explicit data or a version token,
  never by publishing a new function identity.
- List semantic subscription and resource-lifecycle inputs directly in Effect dependencies;
  stable callback identity must not suppress required teardown, re-registration, cancellation, or
  restart behavior.

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

- `orchbee/component-export-boundary`: An authored production component module has one named PascalCase runtime export matching its filename.
- Keep cohesive private functions, render helpers, constants, static lookup data, and related type declarations beside their component. Export related types only when they belong to its API.
- Extract a separate component or helper module for an independent responsibility or consumer, not to satisfy a function or declaration count.
- Module-level values must not introduce shared mutable feature state, resource acquisition, or side effects. Review these semantics; the export rule checks public shape, not purity.
- Extracted components use symbol-matched PascalCase `.tsx` files. A dedicated Hook uses `useHookName.ts`; non-component TSX entry modules retain responsibility-focused kebab-case names.
- Tests, Stories, and checked-in shadcn source are exempt from the component export rule.

## Component directory boundary

- `orchbee/component-directory`: A private authored component lives at `components/<component-name>/<ComponentName>.tsx` and has no `index.ts`.
- A public boundary component is directly re-exported by its source module `index.ts`; its kebab-case directory matches the component name.
- Public and private component directories may colocate owned Hooks, helpers, constants, and types beside `<ComponentName>.tsx` and its optional `<ComponentName>.module.css`. Private directories still have no `index.ts`.
- Define Props in the component `.tsx`; export and re-export that type only when it belongs to the public API.
- Do not create `<component-name>-props.ts` merely to move one component's Props out of TSX.
- Repeat the private structure recursively; a child of `Toolbar.tsx` belongs under `components/action/Action.tsx` inside the toolbar owner.

Use support directories when they improve navigation for concrete owned contents; they are not required for each file category:

| Directory     | Allowed component-owned contents                                                                                              |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `components/` | Recursive private component directories                                                                                       |
| `contexts/`   | Read-only capability Contexts, or one Context-scoped Store module with State, actions, creation, Provider, and selector Hook  |
| `hooks/`      | Behavior, presenter, action, and lifecycle Hooks; never a wrapper around the Store selector Hook                              |
| `constants/`  | JavaScript-emitting static values, `as const` enum-like objects, lookup data, and finite configuration owned by the component |
| `types/`      | Erased TypeScript interfaces, aliases, structures, and unions shared by multiple files in the subtree                         |
| `utils/`      | Framework-independent pure helpers owned only by the component                                                                |
| `__tests__/`  | Unit tests required by [`docs/TESTING.md`](../TESTING.md)                                                                     |

- Keep resources shared by sibling trees at their nearest common owner; keep cross-feature workflows, external integrations, and contracts in the owning source module.
- Do not add barrels inside private or support directories; promote the full tree when another module needs it.
- A public root component that is itself a complete capability may own direct semantic subsystem directories when each directory has several cohesive files, one domain responsibility, and no independent external consumer. Names describe the capability facet, such as `model/`, `layout/`, `dragging/`, or `accessibility/`; generic collection names such as `common/`, `shared/`, `style/`, and `misc/` are forbidden.
- Semantic subsystem directories remain structural parts of the public component source module. They have no `index.ts`, import sibling implementation files directly, and expose selected API only through the public component `index.ts`.
- A read-only capability Context carries stable immutable data or functions, such as prepared messages. It owns no evolving UI state or unrestricted mutation; use the Context-scoped Store contract when descendants share changing feature state.
- Keep a Context Store and `use<ComponentName>Store(selector)` in one module under `contexts/`; never add a forwarding `hooks/useFilterableListStore.ts`.
- Keep Props used by one component in its `.tsx`; use `types/` only for shared or independently meaningful data.
- Keep every value that survives TypeScript erasure out of `types/`; keep component-owned runtime constants beside their consumer or in `constants/` when grouping helps. Native TypeScript enums remain forbidden by `erasableSyntaxOnly`.
- Keep an optional CSS Module beside its importing `.tsx`; do not create `styles/`.
- Stories live under the owning workspace's top-level `stories/` contract in [`storybook.md`](storybook.md).
- Checked-in shadcn source is exempt.

## State ownership

| State or behavior                                                    | Owner                                                     |
| -------------------------------------------------------------------- | --------------------------------------------------------- |
| Local input, disclosure, selection, and focus                        | Owning component or cohesive Hook                         |
| Framework-only accessibility or visual interaction                   | Generic UI primitive                                      |
| State shared by one mounted feature subtree                          | Store instance scoped to that subtree                     |
| Domain transitions and invariants                                    | Pure transitions used by the owning Hook or Store         |
| Network, stream, subscription, timer, and browser-resource lifecycle | Responsibility-focused lifecycle Hook or platform adapter |
| Startup inputs needed by first render                                | Composition root before `createRoot(...).render(...)`     |

[`package-boundaries.md`](package-boundaries.md#state-ownership) remains authoritative across workspaces.

## Derived state and transitions

- Derive values during rendering, in selectors, or in cohesive presenter logic; never mirror derivable values into state with an Effect.
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

- `orchbee/hook-export-boundary`: An authored dedicated Hook module has one named Hook runtime export matching its camelCase filename; related type exports are allowed.
- Use `useKeyboardNavigation.ts` or `useKeyboardNavigation.tsx` naming.
- Keep private helpers, constants, and directly related types in the Hook file. Extract them only for an independent responsibility or consumer.
- Tests, Stories, Context Store modules, and generated `packages/ui/src/hooks/use-mobile.ts` follow their owning contracts instead.

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
