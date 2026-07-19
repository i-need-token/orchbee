# @orchbee/workbench

Shared product-semantic React shell for Orchbee applications.

## Scope

The package currently owns one full-height, horizontally resizable two-panel shell. The left panel defaults to `250px` and is constrained to `200px`–`400px`; the right panel fills the remaining width. Its neutral divider is `1px` at rest and expands visually to `3px` with the same stronger gray on hover and while resizing, without changing panel geometry. Both panels are intentionally empty so product content can be introduced explicitly, one responsibility at a time. It composes the resizable primitives from `@orchbee/ui` without assigning navigation, terminal, session, or status behavior to either panel.

Workbench owns the validation and versioned storage record for its device-local panel width. The consuming application owns the platform storage implementation and injects it at composition time. Application startup, routing roots, authentication, deployment, other persistence, platform integrations, and product capabilities remain outside this package. The machine-readable scope contract lives in [`project.json`](project.json).

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
```

The consuming application selects the light or dark theme at that same boundary. Workbench uses the surrounding design-token theme for its two panel surfaces and divider.

Render the shell at the application composition boundary:

```tsx
import { Workbench, type WorkbenchLayoutStorage } from '@orchbee/workbench';

export function ProductInterface({
	layoutStorage,
}: {
	readonly layoutStorage: WorkbenchLayoutStorage;
}) {
	return <Workbench layoutStorage={layoutStorage} />;
}
```

When storage is provided, Workbench restores `orchbee:workbench:left-panel-width:v1`, validates that the JSON value is an integer within the current pixel constraints, and saves only after a completed user resize. Missing, malformed, unavailable, or out-of-range storage falls back to `250px` without blocking rendering. The repository-wide ownership and key contract lives in [`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md).

Workbench does not currently expose panel-content slots. Each product region will be added through an explicit package API when its ownership and interaction contract are defined.

## Development

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:exports
```
