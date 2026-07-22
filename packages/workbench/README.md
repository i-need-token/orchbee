# @orchbee/workbench

Shared product-semantic React shell for Orchbee applications.

## Scope

The package owns a full-height, horizontally resizable two-panel shell. The left sidebar defaults
to `250px` and is constrained to `200px`–`400px`; the right panel fills the remaining width and
is currently reserved for future product composition. The root-level control and `Mod+B` shortcut
hide or restore the sidebar. The localized sidebar provides `New Terminal`, `Search`, a `Project`
section heading, and a bottom-anchored `Settings` action.

The shell composes primitives from `@orchbee/ui` without owning terminal sessions, search,
settings, project, authentication, routing, or agent orchestration lifecycles. The machine-readable
scope contract lives in [`project.json`](project.json).

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application
composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
```

Render the shell at the same boundary:

```tsx
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';

export function ProductInterface({
	layoutStorage,
	onNewTerminal,
	onSearch,
	onSettings,
	shortcutPlatform,
}: {
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly onNewTerminal: () => void;
	readonly onSearch: () => void;
	readonly onSettings: () => void;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
}) {
	return (
		<Workbench
			layoutStorage={layoutStorage}
			onNewTerminal={onNewTerminal}
			onSearch={onSearch}
			onSettings={onSettings}
			shortcutPlatform={shortcutPlatform}
		/>
	);
}
```

When storage is provided, Workbench restores `orchbee:workbench:left-panel-width:v1` and
`orchbee:workbench:left-panel-visibility:v1`. It validates complete JSON records, saves width only
after a completed user resize, and saves visibility only after the button or shortcut invokes the
semantic toggle action. Missing, malformed, unavailable, or out-of-range storage falls back to a
visible `250px` sidebar without blocking rendering. The repository-wide ownership and key contract
lives in [`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md).

Workbench owns `Mod+B` for sidebar visibility, `Mod+N` for `onNewTerminal`, `Mod+K` for `onSearch`,
and `Mod+,` for `onSettings`. Apple platforms display `⌘B`, `⌘N`, `⌘K`, and `⌘,`; standard
platforms display `Ctrl B`, `Ctrl N`, `Ctrl K`, and `Ctrl+,`. Missing callbacks remain visible for
layout continuity and are excluded from global shortcut handling.

## Development

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:exports
```
