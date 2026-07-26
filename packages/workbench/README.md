# @orchbee/workbench

Shared product-semantic React shell for Orchbee applications.

## Scope

The package owns a full-height, horizontally resizable two-panel shell. The left sidebar defaults
to `250px` and is constrained to `200px`–`400px`; the right panel fills the remaining width and
renders the host-owned terminal tab model through `@orchbee/tab-strip`. The sidebar control and
`Mod+B` shortcut hide or restore the sidebar. The control remains mounted at one fixed shell
position in both states. While the sidebar is hidden, a transparent leading position inside the
tab-strip surface reserves its footprint so the control never overlaps the first tab or paints a
separate background. The localized sidebar provides `New Terminal`, `Search`, a `Project` section
heading, and a bottom-anchored `Settings` action.

Right-clicking blank tab-strip space exposes a localized orientation action. The Workbench switches
the strip and content layout between a top horizontal strip and a left vertical strip while tabs,
groups, and the new-tab control keep their existing context menus.

The shell composes primitives from `@orchbee/ui` without owning terminal sessions, search,
settings, project, authentication, routing, or agent orchestration lifecycles. The machine-readable
scope contract lives in [`project.json`](project.json).

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application
composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';
```

Render the shell at the same boundary:

```tsx
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import type { TabModel } from '@orchbee/tab-strip';

export function ProductInterface({
	layoutStorage,
	onNewTerminal,
	onSearch,
	onSettings,
	shortcutPlatform,
	tabModel,
}: {
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly onNewTerminal: () => void;
	readonly onSearch: () => void;
	readonly onSettings: () => void;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly tabModel: TabModel;
}) {
	return (
		<Workbench
			layoutStorage={layoutStorage}
			onNewTerminal={onNewTerminal}
			onSearch={onSearch}
			onSettings={onSettings}
			shortcutPlatform={shortcutPlatform}
			tabModel={tabModel}
		/>
	);
}
```

When storage is provided, Workbench restores `orchbee:workbench:left-panel-width:v1`,
`orchbee:workbench:left-panel-visibility:v1`, and
`orchbee:workbench:tab-strip-orientation:v1`. It parses complete JSON records with feature-owned
strict Zod schemas, saves width only after a completed user resize, and saves visibility or tab-strip
orientation only after the corresponding semantic toggle action. Missing, malformed, unavailable,
or out-of-range storage falls back to a visible `250px` sidebar and a horizontal tab strip without
blocking rendering. The repository-wide ownership and key contract lives in
[`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md).

The host owns `TabModel` and terminal-session lifecycles. Workbench renders that model and forwards
the tab-strip new-tab action to `onNewTerminal`; when the callback is absent, the tab-strip retains
its local fallback behavior. A desktop host may also pass `createCrossWindowDragRuntime` to connect
the strip's pointer lifecycle to host-owned live-window discovery, compatibility checks, native
window movement, capture, and move-loop behavior. Workbench and the browser application do not
fabricate those operating-system capabilities.

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
