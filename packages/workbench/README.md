# @orchbee/workbench

Shared product-semantic React interface, interaction state, and presentation model for Orchbee applications.

## Scope

The package owns the application shell, management navigation, session navigation, terminal panes, tabs, splits, focus, panel layout, and other product interactions shared by browser-based Orchbee applications. Its shell is terminal-first: a full-height management sidebar is the extension point for projects, worktrees, agents, or another navigation model, while the remaining workspace belongs to the terminal container. It composes `@orchbee/ui` primitives and wraps `@orchbee/terminal` without moving React or product behavior into either lower-level package.

Application startup, routing roots, authentication, deployment, persistence, Electron or Node imports, PTY processes, transport implementations, and authoritative session lifecycles remain outside this package. The machine-readable scope contract lives in [`project.json`](project.json).

## Use

Import the UI compatibility stylesheet once before the Workbench stylesheet at the application composition boundary:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
```

The consuming application selects the light or dark theme at that same boundary. Workbench uses the surrounding design-token theme, and each terminal pane derives its foreground, background, and cursor colors from its computed Workbench surface.

Install the shared localization provider at the application composition boundary, then provide a
headless session capability owned or adapted by the consuming application:

```tsx
import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { Workbench, type WorkbenchSessionService } from '@orchbee/workbench';

declare const sessions: WorkbenchSessionService;

export function ProductInterface({ locale }: { readonly locale: OrchbeeLocale }) {
	return (
		<OrchbeeI18nProvider locale={locale}>
			<Workbench sessions={sessions} />
		</OrchbeeI18nProvider>
	);
}
```

The application owns locale preference and platform detection. Workbench formats its product
messages from `@orchbee/i18n` and expects to render beneath that provider.

`WorkbenchSessionService` is experimental while the product interaction model is being exercised against `apps/web`. It exposes a stable snapshot/subscription boundary for React and terminal channels for input, output, and resize. It does not select a transport or own remote synchronization.

## State ownership

The package owns the client-side arrangement used to observe sessions: active tabs, splits, focused panes, and panel geometry. A terminal session remains an authoritative running process outside the Workbench. If layout persistence or cross-device layout synchronization is introduced, the consuming application supplies that capability rather than turning terminal sessions into UI layout records.

## Development

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:exports
```
