# React

## Stack contract

- Build the product interface with React and TypeScript.
- Use TanStack Router for client-side routing.
- Use Zustand for shared client state.
- Build the component system with shadcn/ui using Base UI primitives.
- Do not introduce an alternative router, shared-state library, or UI primitive system without an explicit architecture decision.
- Author component styles and runtime theme overrides according to [`docs/frontend/styling.md`](styling.md).

## Component contract

- Treat a product View as a pure projection from a prepared interface to React nodes: `ViewProps -> ReactNode`.
- Prepare display text, variants, visibility, list items, accessibility labels, and semantic callbacks before they reach the View. A View binds those values to elements and UI primitives without interpreting them.
- A View does not subscribe to a store, call a service or controller, localize messages, derive product state, own product interaction state, run Effects, or translate raw DOM events into business actions.
- Keep View dependencies explicit through props. Introduce context or a scoped store only in a React adapter when multiple descendants share one owner; product Views do not read ambient application state.
- Component size is not an architectural boundary. Extract code when another owner, lifecycle, or reusable behavior exists, not to satisfy an arbitrary line or Hook count.
- Generic UI primitives may encapsulate framework-level accessibility and visual interaction behavior. They expose product-meaningful values and callbacks instead of leaking that machinery into product Views.

React still requires a function component to call Hooks. Keep that component as a narrow adapter, distinct from the View:

```text
Controller or Store -> Presenter Hook -> ViewProps -> View
```

The adapter subscribes to React-specific sources, invokes the presenter Hook, and returns one View. It contains no product markup or business workflow. Application startup, cross-feature workflows, persistence, transport selection, and platform capability implementations remain outside both the adapter and the View.

## State ownership

| State or behavior                                                       | Owner                                                                 |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Product input, disclosure, selection, and focus state                   | A presenter Hook or a feature-scoped store                            |
| Framework-only accessibility or visual interaction state                | The generic UI primitive that encapsulates it                         |
| UI state shared by one mounted feature subtree                          | A store instance scoped to that subtree                               |
| Domain transitions and invariants                                       | Pure transition functions behind the owning store or service          |
| Network, terminal, subscription, timer, and browser-resource lifecycles | A controller, service, or narrow platform adapter                     |
| Application startup inputs required by the first render                 | The application composition root before `createRoot(...).render(...)` |

Workspace-level ownership remains authoritative in [`docs/frontend/package-boundaries.md`](package-boundaries.md#state-ownership). Do not move state into React merely because React renders its projection.

## Derived state and transitions

- Derive values in a selector, presenter Hook, or pure presenter function before constructing `ViewProps`; do not mirror derivable values into state with an Effect.
- Model multi-field invariants with semantic actions or pure transitions that receive the previous state and return the next state.
- Parse external input at its trust boundary, then pass validated domain values to transitions and prepared display values to Views.

```ts
// ❌ Mirrored state can become stale.
React.useEffect(() => setIsEmpty(sessions.length === 0), [sessions]);

// ✅ Prepare the projection before invoking the View.
const viewProps = { isEmpty: sessions.length === 0 };
```

## Effects and resource lifecycles

- Product Views do not use Effects. A presenter Hook or adapter uses an Effect only to synchronize React with an external system.
- Give each Effect one external resource owner and return the complete, idempotent cleanup for that resource.
- Keep acquisition, subscriptions, cancellation, and release together so partial initialization cannot leak resources.
- Move a lifecycle into a controller or service when it coordinates multiple subscriptions, staged asynchronous initialization, retry, ordering, or recovery. The Effect then creates or connects that owner and disposes it.
- Put cross-store synchronization in the coordinator that owns the relationship, not in a presenter Hook or View.
- Views receive semantic callbacks. Raw event normalization belongs in a generic UI primitive or adapter; multi-step workflows belong in a controller or service.

## Custom Hooks

- A presenter Hook is the React-specific adapter that prepares a complete View interface. Its result is directly consumable by a View without further localization, derivation, or event interpretation.
- Extract another custom Hook when it owns a complete React-specific lifecycle or composes reusable React behavior; do not extract a Hook solely to hide `useState`, `useMemo`, `useRef`, or `useEffect`.
- Move framework-independent calculations to pure functions and external-resource orchestration to controllers or services rather than embedding them in a larger Hook.

## Zustand and context

- Scope a Zustand store to the narrowest owner that needs shared state; do not introduce a process-wide singleton for a mount-local feature.
- Subscribe with selectors in the presenter Hook or adapter so it observes only the state needed to prepare its View interface.
- Expose semantic actions that preserve store invariants; do not expose unrestricted state mutation to adapters or Views.
- Store durable state and domain actions, not callbacks created by React Hooks. Pass a stable controller or capability through composition when descendants need shared behavior.
- Use React context for stable capabilities and scoped store instances. High-frequency mutable data belongs in the external store read through selectors.

## Testing boundaries

- Test pure transitions and selectors without React.
- Test controllers against fake ports and verify acquisition, event forwarding, cancellation, failure, and idempotent cleanup.
- Render Views directly from their exported interfaces without product providers, stores, services, or controller mocks. Cover meaningful interface variants and accessibility output.
- Test presenter Hooks against their public prepared interface, then keep adapter integration coverage narrow.
- Do not patch React private internals or duplicate production algorithms inside tests.
- Follow the repository-wide discoverability and regression requirements in [`docs/TESTING.md`](../TESTING.md).

## Correctness and safety

- `react/rules-of-hooks`: Call Hooks only at component or custom-Hook top level.
- `react/exhaustive-deps`: Include every reactive dependency used by a Hook callback.
- `react/jsx-key`: Give list elements stable keys from their data identity.
- `react/jsx-no-target-blank`: Protect new-tab links from opener access.
- `react/no-danger`: Render structured React content instead of injecting HTML.

The generated shadcn `ChartStyle` implementation is the sole `react/no-danger` exception. Its style text may contain only trusted, developer-authored chart configuration; never pass persisted user values or other untrusted strings into that configuration.
