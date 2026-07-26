# @orchbee/ui

Orchbee's private package for reusable visual primitives, including shadcn/ui components backed by Base UI and the product-icon boundary.

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

The root entry currently retains the `Button` compatibility export. The explicit `@orchbee/ui/components` entry exports the complete checked-in shadcn Base UI component set and Orchbee-authored visual primitives. Components keep their standard variants, `className` composition, and Base UI props.

`Tabs` also exposes the `workspace` variant used for full-height application work areas. Compose it with `TabsBar`, a workspace `TabsList`, one `TabsItem` per trigger, and an independent one-pixel `TabsDivider` between adjacent items. Tab triggers do not draw their own separators. `TabsBar` is `35px` high, composed of a `34px` content area and a `1px` bottom border, and may contain product-owned controls beside the semantic tab list. Each item is at most `200px` wide and shrinks equally when space is constrained. Unselected tabs use the brighter base surface, hover adds a lighter surface without changing the label color, and the selected tab uses the darker selected surface. The UI package provides accessible Base UI interaction and responsive chrome; product packages remain responsible for tab data and semantic actions such as selection, creation, and closure.

## Product icons

Product code consumes `ProductIcon` through the package API and uses its Orchbee-owned name allowlist. It must not import Codicons CSS, vendor class names, font characters, or asset paths directly. The package stylesheet loads the icon font once, while `ProductIcon` keeps every glyph decorative so the owning control supplies the accessible name.

The product allowlist contains `sidebar-left`, `add`, `arrow-left`, `close`, `ellipsis`,
`more-vertical`, `pin`, `search`, `settings`, and `terminal`, backed by their corresponding
Codicons glyphs. Generated shadcn sources retain their upstream Lucide dependencies inside the
generated compatibility boundary. See the repository
[frontend styling standard](../../docs/frontend/styling.md) and
[third-party notices](../../THIRD_PARTY_NOTICES.md).

The resizable component facade exports `ResizablePanelGroupHandle` for product adapters that need
to constrain live layouts through the group imperative API.

## Refresh components

The package-level [`components.json`](components.json) pins the Base UI Nova style and directs the official CLI to `src/components/ui`.

```sh
pnpm --dir packages/ui exec shadcn add --all --overwrite
```

The repository owns a complete snapshot of the official Base UI registry rather than adding primitives on demand. Review the CLI diff before accepting a refresh because focused Orchbee compatibility adaptations may need to be reapplied. Do not hand-write a primitive that is available from shadcn.

Generated Tailwind utilities remain part of the component source. [`src/styles.css`](src/styles.css) contains the Tailwind/shadcn imports, maps shadcn's compatibility variables to `@orchbee/design-tokens`, and owns the base layer required by those utilities. It does not define the Orchbee token vocabulary. The full workflow lives in [`docs/frontend/styling.md`](../../docs/frontend/styling.md).

The stylesheet also owns the explicit Tailwind `@source './components'` registration. Keep that registration when refreshing shadcn: source-built applications compile the stylesheet from outside the UI workspace, so automatic source discovery alone cannot guarantee that generated component utilities are emitted during development.

Tailwind CSS, the shadcn CLI stylesheet, and `tw-animate-css` are build-time dependencies. Rslib compiles them into the published `styles.css`; consumers do not need those tools at runtime.

## Development

```sh
pnpm --filter @orchbee/ui... build
pnpm --filter @orchbee/ui typecheck
pnpm --filter @orchbee/ui test
pnpm --filter @orchbee/ui test:exports
```

Browse every checked-in component through the repository Storybook:

```sh
pnpm storybook
```

UI Stories live in [`stories`](stories) and are consumed by `apps/storybook`. Every component module in `src/components/ui` has one matching Story file; the Storybook application test enforces that inventory contract. Storybook is development infrastructure and is not part of this package's public exports.
