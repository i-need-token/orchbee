# @orchbee/ui

Orchbee's private package for reusable shadcn/ui components backed by Base UI.

## Scope

This package owns reusable visual primitives and their accessibility-preserving Base UI composition. It does not own themes, design tokens, sessions, agents, terminal panes, routing, remote state, or platform-specific effects. The machine-readable scope contract lives in [`project.json`](project.json).

## Use

Import the package stylesheet once at the application entry and consume components through the package API.

```tsx
import '@orchbee/ui/styles.css';
import { Button, Dialog, DialogContent, DialogTrigger } from '@orchbee/ui/components';

export function Example() {
	return (
		<Dialog>
			<DialogTrigger render={<Button variant='outline' />}>New session</DialogTrigger>
			<DialogContent>Create a session here.</DialogContent>
		</Dialog>
	);
}
```

The root entry currently retains the `Button` compatibility export. The explicit `@orchbee/ui/components` entry exports the complete checked-in shadcn Base UI component set. Components keep their standard variants, `className` composition, and Base UI props.

## Refresh components

The package-level [`components.json`](components.json) pins the Base UI Nova style and directs the official CLI to `src/components/ui`.

```sh
pnpm --dir packages/ui exec shadcn add --all --overwrite
```

The repository owns a complete snapshot of the official Base UI registry rather than adding primitives on demand. Review the CLI diff before accepting a refresh because focused Orchbee compatibility adaptations may need to be reapplied. Do not hand-write a primitive that is available from shadcn.

Generated Tailwind utilities remain part of the component source. [`src/styles.css`](src/styles.css) contains only the imports required by Tailwind and shadcn; this package does not currently define an Orchbee theme or token layer. The full workflow lives in [`docs/frontend/styling.md`](../../docs/frontend/styling.md).

Tailwind CSS, the shadcn CLI stylesheet, and `tw-animate-css` are build-time dependencies. Rslib compiles them into the published `styles.css`; consumers do not need those tools at runtime.

## Development

```sh
pnpm --filter @orchbee/ui build
pnpm --filter @orchbee/ui typecheck
pnpm --filter @orchbee/ui test
pnpm --filter @orchbee/ui test:exports
```
