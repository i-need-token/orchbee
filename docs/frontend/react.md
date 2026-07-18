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

## TSX file boundary

- `orchbee/one-function-per-tsx`: Each authored production `.tsx` file contains at most one top-level function. Move every additional component, Hook, callback factory, and helper function to its owning module.
- A function declaration, function expression, or arrow function is top-level when no other function contains it. Callbacks nested inside the file's sole component do not consume another file-level function slot.
- Put a child presentational component in its own `.tsx` file, a presenter Hook in its own `use-*.ts` file, and framework-independent helpers in a responsibility-focused `.ts` file.
- Type declarations, constants, and prepared lookup data do not consume the function slot, but they remain only when they support the component's single responsibility.
- Unit tests, Storybook stories, and the checked-in shadcn source described in [`docs/LINTING.md`](../LINTING.md#generated-source) are outside this authored-production rule.

## Component directory boundary

- `orchbee/component-directory`: An authored production component that is not directly re-exported by its owning source module's `index.ts` is a private child component and lives at `components/<component-name>/<component-name>.tsx`.
- A component directly re-exported by the owning `index.ts` is that module's boundary component and may remain beside the index. Application entry files that only mount a boundary component are not components and remain at the composition boundary.
- Repeat the grouping for deeper extracted children; a private child of `toolbar.tsx` lives at `components/action/action.tsx` inside the `toolbar` group rather than as a loose sibling.
- Keep component-owned CSS Modules and other component-owned assets in the same component group. A component imports its CSS Module from its own directory rather than reaching into an ancestor or another component group.
- A component group is a structural ownership directory, not a source-module boundary. Do not add `index.ts` barrels inside private component groups; import their implementation files directly within the owning module.
- Do not use component groups as miscellaneous feature folders. Module-level Hooks, controllers, services, and shared contracts remain in the source module that owns their responsibility.
- When a component has a meaningful optional test, put it in the group's sibling `__tests__` directory according to [`docs/TESTING.md`](../TESTING.md#required-unit-tests).
- The checked-in shadcn source described in [`docs/LINTING.md`](../LINTING.md#generated-source) is outside this authored component-directory rule.

```text
feature/
├── index.ts
├── feature.tsx
└── components/
    └── toolbar/
        ├── toolbar.tsx
        ├── toolbar.module.css
        └── components/
            └── action/
                ├── action.tsx
                └── action.module.css
```

## State ownership

| State or behavior                                                       | Owner                                                                 |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Product input, disclosure, selection, and focus state                   | A presenter Hook or a feature-scoped Store                            |
| Framework-only accessibility or visual interaction state                | The generic UI primitive that encapsulates it                         |
| UI state shared by one mounted feature subtree                          | A store instance scoped to that subtree                               |
| Domain transitions and invariants                                       | Pure transition functions behind the owning store or service          |
| Network, terminal, subscription, timer, and browser-resource lifecycles | A controller, service, or narrow platform adapter                     |
| Application startup inputs required by the first render                 | The application composition root before `createRoot(...).render(...)` |

Workspace-level ownership remains authoritative in [`docs/frontend/package-boundaries.md`](package-boundaries.md#state-ownership). Do not move state into React merely because React renders its projection.

## Derived state and transitions

- Derive values in a selector, presenter Hook, or pure presenter function before the component renders them; do not mirror derivable values into state with an Effect.
- Model multi-field invariants with semantic actions or pure transitions that receive the previous state and return the next state.
- Parse external input at its trust boundary, then pass validated domain values to transitions and prepared display values to components.

```ts
// ❌ Mirrored state can become stale.
React.useEffect(() => setIsEmpty(sessions.length === 0), [sessions]);

// ✅ Prepare the projection before rendering the component.
const viewProps = { isEmpty: sessions.length === 0 };
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

- `orchbee/one-hook-per-file`: Each authored production Hook module named `use-*.ts` or `use-*.tsx` contains exactly one top-level function and exactly one exported symbol.
- The sole top-level function is the named Hook export. Move supporting functions to responsibility-focused `.ts` modules and public contracts to type-focused modules.
- Functions nested inside the Hook, including Effect callbacks and event adapters, do not consume another file-level function slot and are not additional exports.
- Unit tests and the generated `packages/ui/src/hooks/use-mobile.ts` source described in [`docs/LINTING.md`](../LINTING.md#generated-source) are outside this authored-production rule.

## Zustand and context

- `orchbee/zustand-scope`: Create authored Zustand stores with `createStore` from `zustand/vanilla`, create each mounted feature instance inside its owner, and import `useStore` only in a `use-*.ts` or `use-*.tsx` selector Hook that supplies an explicit selector.
- Scope a Store to the narrowest feature owner that needs shared state; do not introduce a process-wide singleton for a mount-local feature.
- Put the stable `StoreApi` instance in React context. Do not put a changing state snapshot in the Provider value.
- Create the Store lazily once per Provider mount, inject complete initial state, and reconcile later external inputs through the owner that understands that relationship.
- When an external Service is authoritative for feature data, its Provider subscribes once and reconciles the latest projection into the scoped Store. Descendants select that projection from Zustand; do not expose the Service snapshot as a second React-facing subscription source.
- Expose one feature Hook such as `useWorkbenchStore(selector)`. It reads the scoped Store from context, rejects use outside the Provider, and subscribes with the caller's narrow selector.
- Consumers select only the state and semantic actions they render or invoke. Do not read the whole Store, extract reusable selector constants, or subscribe an intermediate component for a deeper descendant.
- Expose semantic actions that preserve Store invariants; do not expose unrestricted mutation to components.
- Store durable feature state and domain actions, not callbacks created by React Hooks. Pass a stable controller or capability through composition when descendants need shared behavior.
- Use separate Provider instances when the same feature mounts more than once; tests create an isolated Store through the same Provider boundary.

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
