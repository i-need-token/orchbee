# React

## Stack contract

- Build the product interface with React and TypeScript.
- Use TanStack Router for client-side routing.
- Use Zustand for shared client state.
- Build the component system with shadcn/ui using Base UI primitives.
- Do not introduce an alternative router, shared-state library, or UI primitive system without an explicit architecture decision.
- Author component styles and runtime theme overrides according to [`docs/frontend/styling.md`](styling.md).

## Component contract

- A product function component renders prepared display values and binds prepared semantic actions to elements or UI primitives.
- Prepare display text, variants, visibility, list items, accessibility labels, and semantic actions before rendering them. Components do not interpret those values as business rules.
- A component may receive prepared data through direct props or a feature-scoped Zustand selector Hook. Both are explicit component dependencies; neither authorizes the component to call services or controllers.
- Presentational components do not own business workflows, synchronize external resources with Effects, or translate raw DOM events into business actions.
- Give an independently named interface region its own presentational component and input contract when it has an independent data subset, state combination, or reason to change. Reuse is not required for extraction.
- When one region renders a finite set of product states or model kinds, prepare a discriminated display model outside TSX and let a focused renderer select the matching leaf component. Keep workflow dispatch outside that renderer.
- Do not split by an arbitrary line or Hook count; split at a responsibility that a maintainer can name and test independently.
- Generic UI primitives may encapsulate framework-level accessibility and visual interaction behavior. They expose product-meaningful values and callbacks instead of leaking that machinery into product components.

React still requires function components at provider and external-lifecycle boundaries. Keep those adapter components narrow:

```text
Controller or Service -> Presenter Hook or Scoped Store -> Presentational Component
```

An adapter connects React to an external lifecycle, creates or provides a scoped Store, prepares direct child inputs, and returns one product component. Application startup, cross-feature workflows, persistence, transport selection, and platform capability implementations remain outside presentational components.

## Recursive component ownership

- Treat every independently named interface region as an ownership boundary rooted at one component. That root owns its private descendant tree and every component-specific Hook, scoped Store and context, transition, style, asset, and test required by the tree.
- Nest component ownership recursively. A private descendant may own another `components/<component-name>/` tree and may introduce its own narrower Store when only that subtree shares the state.
- A component directly re-exported by a source module is the public root of its ownership tree. External modules consume it only through the owning module's `index.ts`; its descendants and Context remain private.
- Admit reusable components without product semantics to `@orchbee/ui`. Keep a product-specific component in the source module and package that own the capability it represents, according to [`docs/frontend/package-boundaries.md`](package-boundaries.md#ui-ownership).
- Place each implementation artifact at the narrowest component-owned subtree or source module that completely owns it. Do not collect component-owned Hooks, stores, types, or utilities in package-wide technical directories.
- When a private component subtree gains an external consumer, promote the complete ownership unit—root component, input contract, scoped state, Hooks, descendants, styles, and tests—to an appropriate source-module boundary. Do not export one internal file while leaving the rest of its ownership behind.

## Component Hook boundary

- `orchbee/no-react-hooks-in-tsx`: Authored production `.tsx` files do not call React's built-in Hooks directly. Move state, Effects, memoization, references, and external-store subscriptions into a responsibility-focused custom Hook.
- A narrow adapter component may call a named presenter or lifecycle Hook, then pass its prepared result to the display component. It does not add derivation or workflow steps after the Hook returns.
- A presenter Hook may compose localization, feature Store selectors, and semantic actions into one complete renderable contract. Presentational descendants consume that contract without calling services or reconstructing the workflow.
- Unit tests, Storybook stories, and the checked-in shadcn source described in [`docs/LINTING.md`](../LINTING.md#generated-source) are outside this authored-production rule.

## Component data delivery

- Use direct props when the data belongs to an immediate parent-child relationship or the component is reusable outside one feature Store.
- Use a feature-scoped Store when one feature owner supplies state or actions to multiple descendants and direct props would make intermediate components forward values they do not use.
- Do not introduce a Store merely to avoid one meaningful parent-child props contract. Props remain the default for shallow ownership.
- Intermediate layout components neither subscribe to descendant-only state nor forward it. The consuming descendant selects its own narrow state and semantic actions through the feature Hook.
- A component may combine its own props with feature Store selections when those inputs have different owners. Do not mirror props into the Store solely to make every input arrive through one channel.
- Keep a component's Context and selector Hook private to its owned subtree. A sibling or external module communicates through the component's public props and semantic actions rather than importing its Store.
- When sibling branches need one shared UI state, move ownership to their nearest common component root and let each consuming descendant select its own slice. Do not make either sibling own the other's state.
- Use the same Context-scoped Store pattern at an application or feature root only for state genuinely shared by that whole mounted tree; global reach does not justify a process-wide singleton.

## TSX file boundary

- `orchbee/one-function-per-tsx`: Each authored production `.tsx` file contains at most one top-level function. When present, that function is the sole named runtime export, is a PascalCase function component, and exactly matches the filename stem. Type-only exports such as the component's Props contract do not count as runtime exports.
- A function declaration, function expression, or arrow function is top-level when no other function contains it. Callbacks nested inside the file's sole component do not consume another file-level function slot.
- Put a child presentational component in its own symbol-matched PascalCase `.tsx` file and a presenter Hook in its own symbol-matched `useHookName.ts` file. Framework-independent helper modules follow the global source-path rule: use the exact camelCase function name for one callable runtime export, or a responsibility-focused kebab-case name when the module has no unique runtime identity.
- Type declarations, constants, and prepared lookup data do not consume the function slot, but they remain only when they support the component's single responsibility.
- Unit tests, Storybook stories, and the checked-in shadcn source described in [`docs/LINTING.md`](../LINTING.md#generated-source) are outside this authored-production rule.

## Component directory boundary

- `orchbee/component-directory`: An authored production component that is not directly re-exported by its owning source module's `index.ts` is a private child component and lives at `components/<component-name>/<ComponentName>.tsx`.
- A component directly re-exported by the owning `index.ts` is that module's public boundary component. Its source-module directory uses the component's kebab-case name while the root component uses the exact PascalCase export name. The directory keeps only `index.ts`, `<ComponentName>.tsx`, and the optional `<ComponentName>.module.css` as direct files; every supporting implementation belongs in a named subdirectory.
- A private component directory keeps only `<ComponentName>.tsx` and the optional `<ComponentName>.module.css` as direct files and does not define an `index.ts`. Its kebab-case directory name corresponds to the PascalCase component export, and its owner imports that implementation directly. The lint rule inventories direct files in both public and private component directories and rejects every other direct file.
- Define a component's Props type in that component's `.tsx` file. Export it from the same file when consumers need the type, and re-export it through the public `index.ts` when it is part of the module API. Do not create a separate `<component-name>-props.ts` merely to move the component's own signature out of TSX.
- Repeat the private component structure recursively. A private child of `Toolbar.tsx` lives at `components/action/Action.tsx` inside the `toolbar` directory rather than as a loose sibling.
- Create support directories only when they have concrete contents, using the ownership below:

| Directory     | Component-owned contents                                                                                       |
| ------------- | -------------------------------------------------------------------------------------------------------------- |
| `components/` | Private child component directories that repeat this structure recursively                                     |
| `contexts/`   | Context-scoped Store modules that keep State, actions, Provider, and the Store selector Hook together          |
| `hooks/`      | Behavior, presenter, action, and lifecycle Hooks; not the selector Hook already exported by a Store module     |
| `constants/`  | Static values and finite configuration owned only by the component                                             |
| `types/`      | Named data structures, discriminated unions, and enums shared by multiple files in the component-owned subtree |
| `utils/`      | Framework-independent pure helpers owned only by the component                                                 |

- A component directory is a structural ownership boundary, not a miscellaneous feature folder. Move resources shared by sibling component trees to their nearest common owner, and keep cross-feature controllers, services, and contracts in the source module that owns them.
- Do not add `index.ts` barrels inside private component directories or their support directories. Promote the entire component ownership tree to a public source-module boundary when another module needs it.
- Keep a Context-scoped Store's State and action types, Store creation, Context Provider, and `use<ComponentName>Store(selector)` export in one Store module under `contexts/`. The selector Hook is part of the Store's API and does not get a second wrapper in `hooks/`.
- Use `types/` only when a named data structure is shared across the component-owned tree or has an independent meaning beyond one component signature. Props used by one component remain in that component's `.tsx` file.
- Keep the component's optional CSS Module beside the `.tsx` file that imports it; do not create a `styles/` subdirectory.
- Do not place story or test files directly beside the component root. Unit tests use the direct child `__tests__/` support directory required by [`docs/TESTING.md`](../TESTING.md); Storybook ownership follows [`docs/frontend/storybook.md`](storybook.md).
- The checked-in shadcn source described in [`docs/LINTING.md`](../LINTING.md#generated-source) is outside this authored component-directory rule.

Store and selector Hook files in the following example exist only because their owning component has state shared by multiple descendants:

```text
filterable-list/
├── index.ts                                     # Re-exports the component and FilterableListProps
├── FilterableList.tsx                           # Defines the component and its Props type
├── FilterableList.module.css
├── components/
│   └── filter-bar/
│       ├── FilterBar.tsx                        # Defines FilterBarProps locally
│       ├── FilterBar.module.css
│       ├── hooks/
│       │   └── useFilterKeyboardNavigation.ts
│       └── components/
│           └── clear-filter-button/
│               ├── ClearFilterButton.tsx
│               └── ClearFilterButton.module.css
├── contexts/
│   └── filterable-list-store.ts                 # State, actions, Provider, useFilterableListStore
├── hooks/
│   └── useFilterableListFocusManagement.ts      # Component behavior, not a Store wrapper
└── types/
    └── filter-option.ts                           # Shared internal UI data structure
```

## State ownership

| State or behavior                                                     | Owner                                                                 |
| --------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Product input, disclosure, selection, and focus state                 | A presenter Hook or a feature-scoped Store                            |
| Framework-only accessibility or visual interaction state              | The generic UI primitive that encapsulates it                         |
| UI state shared by one mounted feature subtree                        | A store instance scoped to that subtree                               |
| Domain transitions and invariants                                     | Pure transition functions behind the owning store or service          |
| Network, stream, subscription, timer, and browser-resource lifecycles | A controller, service, or narrow platform adapter                     |
| Application startup inputs required by the first render               | The application composition root before `createRoot(...).render(...)` |

Workspace-level ownership remains authoritative in [`docs/frontend/package-boundaries.md`](package-boundaries.md#state-ownership). Do not move state into React merely because React renders its projection.

## Derived state and transitions

- Derive values in a selector, presenter Hook, or pure presenter function before the component renders them; do not mirror derivable values into state with an Effect.
- Model multi-field invariants with semantic actions or pure transitions that receive the previous state and return the next state.
- Parse external input at its trust boundary, then pass validated domain values to transitions and prepared display values to components.

```ts
// ❌ Mirrored state can become stale.
React.useEffect(() => setIsEmpty(items.length === 0), [items]);

// ✅ Prepare the projection before rendering the component.
const viewProps = { isEmpty: items.length === 0 };
```

## Effects and resource lifecycles

- Presentational components do not use Effects. A presenter Hook, Provider, or adapter uses an Effect only to synchronize React with an external system.
- Give each Effect one external resource owner and return the complete, idempotent cleanup for that resource.
- Keep acquisition, subscriptions, cancellation, and release together so partial initialization cannot leak resources.
- Move a lifecycle into a controller or service when it coordinates multiple subscriptions, staged asynchronous initialization, retry, ordering, or recovery. The Effect then creates or connects that owner and disposes it.
- Put cross-Store synchronization in the coordinator that owns the relationship, not in a presentational component.
- Components receive semantic actions. Raw event normalization belongs in a generic UI primitive or adapter; multi-step workflows belong in a controller or service.

## Custom Hooks

- A presenter Hook is a React-specific adapter that prepares a complete component input contract. Its result is directly renderable without further business derivation or event interpretation.
- Extract another custom Hook when it owns a complete React-specific lifecycle or composes reusable React behavior; do not extract a Hook solely to hide `useState`, `useMemo`, `useRef`, or `useEffect`.
- Move framework-independent calculations to pure functions and external-resource orchestration to controllers or services rather than embedding them in a larger Hook.

## Hook file boundary

- `orchbee/one-hook-per-file`: Each authored production Hook module is named with the exact camelCase Hook export, such as `useKeyboardNavigation.ts` or `useKeyboardNavigation.tsx`, and contains exactly one top-level function and exactly one exported symbol.
- The sole top-level function is the named Hook export and exactly matches the filename stem. Move supporting functions to focused `.ts` modules named after a sole exported function or after their shared responsibility, and move public contracts to type-focused modules.
- Functions nested inside the Hook, including Effect callbacks and event adapters, do not consume another file-level function slot and are not additional exports.
- Unit tests and the generated `packages/ui/src/hooks/use-mobile.ts` source described in [`docs/LINTING.md`](../LINTING.md#generated-source) are outside this authored-production rule.

## Zustand and context

- `orchbee/zustand-scope`: Create authored Zustand stores with `createStore` from `zustand/vanilla`, create each mounted component instance inside its owner, and import `useStore` only in the Context-scoped Store module or shared Context-Store factory that exports a selector Hook requiring an explicit selector.
- Scope a Store to the narrowest component root that needs shared descendant state; do not introduce a process-wide singleton for a mount-local component tree.
- Put the stable `StoreApi` instance in React context. Do not put a changing state snapshot in the Provider value.
- Create the Store lazily once per component-root Provider mount, inject complete initial state, and reconcile later external inputs through the owner that understands that relationship. The Provider wraps exactly the subtree that may consume the Store.
- When an external Service is authoritative for feature data, its Provider subscribes once and reconciles the latest projection into the scoped Store. Descendants select that projection from Zustand; do not expose the Service snapshot as a second React-facing subscription source.
- Export one selector Hook such as `useFilterableListStore(selector)` from the same Store module as its Provider. It reads the scoped Store from context, rejects use outside the Provider, and subscribes with the caller's narrow selector. Do not create `hooks/useFilterableListStore.ts` as a forwarding wrapper.
- Consumers select only the state and semantic actions they render or invoke. Do not read the whole Store, extract reusable selector constants, or subscribe an intermediate component for a deeper descendant.
- Expose semantic actions that preserve Store invariants; do not expose unrestricted mutation to components.
- Store durable feature state and domain actions, not callbacks created by React Hooks. Pass a stable controller or capability through composition when descendants need shared behavior.
- Use separate Provider instances when the same stateful component mounts more than once; tests create an isolated Store through the same Provider boundary.

## Testing boundaries

- Test pure transitions and selectors without React.
- Test controllers against fake ports and verify acquisition, event forwarding, cancellation, failure, and idempotent cleanup.
- Render direct-props presentational components from their exported interfaces without product providers, Stores, services, or controller mocks. Cover meaningful interface variants and accessibility output.
- Test Store-connected components with an isolated feature Provider and deterministic initial state; do not reuse a process-wide Store between tests.
- Test presenter Hooks against their public prepared interface, then keep adapter and Provider integration coverage narrow.
- Do not patch React private internals or duplicate production algorithms inside tests.
- Follow the repository-wide discoverability and regression requirements in [`docs/TESTING.md`](../TESTING.md).

## Correctness and safety

- `react/rules-of-hooks`: Call Hooks only at component or custom-Hook top level.
- `react/exhaustive-deps`: Include every reactive dependency used by a Hook callback.
- `react/jsx-key`: Give list elements stable keys from their data identity.
- `react/jsx-no-target-blank`: Protect new-tab links from opener access.
- `react/no-danger`: Render structured React content instead of injecting HTML.

The generated shadcn `ChartStyle` implementation is the sole `react/no-danger` exception. Its style text may contain only trusted, developer-authored chart configuration; never pass persisted user values or other untrusted strings into that configuration.
