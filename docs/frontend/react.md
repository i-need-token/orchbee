# React

## Stack contract

- Build the product interface with React and TypeScript.
- Use TanStack Router for client-side routing.
- Use Zustand for shared client state.
- Build the component system with shadcn/ui using Base UI primitives.
- Do not introduce an alternative router, shared-state library, or UI primitive system without an explicit architecture decision.
- Author component styles and runtime theme overrides according to [`docs/frontend/styling.md`](styling.md).

## Correctness and safety

- `react/rules-of-hooks`: Call Hooks only at component or custom-Hook top level.
- `react/exhaustive-deps`: Include every reactive dependency used by a Hook callback.
- `react/jsx-key`: Give list elements stable keys from their data identity.
- `react/jsx-no-target-blank`: Protect new-tab links from opener access.
- `react/no-danger`: Render structured React content instead of injecting HTML.

The generated shadcn `ChartStyle` implementation is the sole `react/no-danger` exception. Its style text may contain only trusted, developer-authored chart configuration; never pass persisted user values or other untrusted strings into that configuration.
