# @orchbee/ui

Orchbee's private package for reusable visual primitives, including shadcn/ui components backed by Base UI and the product-icon boundary.

## Scope

This package owns reusable visual primitives and their accessibility-preserving Base UI composition. It does not own themes, design tokens, sessions, agents, terminal panes, routing, remote state, or platform-specific effects. The machine-readable scope contract lives in [`project.json`](project.json).

## Use

Import the package stylesheet once at the application entry and consume components through the package API.

```tsx
import '@orchbee/ui/styles.css';
import { useState } from 'react';
import { Button, Dialog, DialogContent } from '@orchbee/ui/components';

export function Example() {
	const [open, setOpen] = useState(false);
	return (
		<>
			<Button variant='outline' onClick={() => setOpen(true)}>
				New session
			</Button>
			<Dialog open={open} onOpenChange={setOpen}>
				<DialogContent>Create a session here.</DialogContent>
			</Dialog>
		</>
	);
}
```

The explicit `@orchbee/ui/components` entry exports consumed shadcn Base UI and Orchbee visual primitives; `@orchbee/ui/styles.css` supplies their styles. These are the only public entries; the package has no root entry. Components retain the relied-on variants, `className` composition and Base UI props. New components and slots require an admitted use rather than speculative inventory.

`Tabs` also exposes the `workspace` variant used for full-height application work areas. Compose it with `TabsBar`, a workspace `TabsList`, one `TabsItem` per trigger, and an independent one-pixel `TabsDivider` between adjacent items. Tab triggers do not draw their own separators. `TabsBar` is `35px` high, composed of a `34px` content area and a `1px` bottom border, and may contain product-owned controls beside the semantic tab list. Each item is at most `200px` wide and shrinks equally when space is constrained. Unselected tabs use the brighter base surface, hover adds a lighter surface without changing the label color, and the selected tab uses the darker selected surface. The UI package provides accessible Base UI interaction and responsive chrome; product packages remain responsible for tab data and semantic actions such as selection, creation, and closure.

Dropdown, context, and submenu surfaces use the shared `250px` menu width and shrink only when the available viewport is narrower. Product consumers keep the primitive default instead of assigning one-off menu widths. Their content, trigger, and interaction contract follows the repository [frontend menu standard](../../docs/frontend/menus.md).

Portaled floating content keeps its generated default layer outside a modal. Modal consumers use the supported `positionerClassName` override with a CSS Module class mapped to `--ob-ref-layer-modal`, so the portal stays above its owning dialog without a one-off popup `z-index`.

Default Dialog and Alert Dialog surfaces use the shared `480px` width, constrained by the viewport, and shadow without an outer outline. Their action footers share the content surface and rely on spacing instead of a divider or tinted footer band.

## Product icons

Product code consumes `ProductIcon` through the package API and uses its Orchbee-owned name allowlist. It must not import Codicons CSS, vendor class names, font characters, or asset paths directly. The package stylesheet loads the icon font once, while `ProductIcon` keeps every glyph decorative so the owning control supplies the accessible name. `ProductIcon` and generated primitive icon defaults use the shared `14px` small-icon size; a non-default glyph size is an explicit `ProductIcon` component variant rather than a consumer override. Compact section-heading actions use the documented `13px` section-action variant. Leading Dropdown and Context icons use the `16px` menu-leading variant while trailing disclosure arrows remain `14px`.

The product allowlist contains `sidebar-left`, `add`, `arrow-left`, `close`, `ellipsis`, `more-vertical`, `pin`, `search`, `settings`, and `terminal`, backed by their corresponding Codicons glyphs. Generated shadcn sources retain their upstream Lucide dependencies inside the generated compatibility boundary. See the repository [frontend styling standard](../../docs/frontend/styling.md) and [third-party notices](../../THIRD_PARTY_NOTICES.md).

The resizable component facade exports `ResizablePanelGroupHandle` for product adapters that need to constrain live layouts through the group imperative API.

## Refresh components

The package-level [`components.json`](components.json) pins the Base UI Nova style and directs the official CLI to `src/components/ui`.

Use the package's `component:add` script only for checked-in components or explicit names admitted under the [experience-component rules](../../docs/frontend/experience-components.md). Do not import the entire registry; a Story alone does not establish product admission. Review CLI changes, preserve Orchbee adaptations and narrowed exports, and do not restore unused slots. Do not hand-write primitives available from shadcn.

The snapshot uses `@orchbee/foundation/react` where generated event callbacks need the repository's stable-identity and latest-render-value contract. Preserve that focused adaptation when refreshing the affected registry components unless upstream provides the same contract.

The snapshot also maps dropdown and context menu geometry to the shared Orchbee menu-width token. It also omits the upstream tooltip arrow so every shared Tooltip surface keeps the Orchbee visual contract, and normalizes generated primitive icon defaults to `14px`. Preserve these focused adaptations when refreshing those primitives.

Generated Tailwind utilities remain part of the component source. [`src/styles.css`](src/styles.css) contains the Tailwind/shadcn imports, maps shadcn's compatibility variables to `@orchbee/design-tokens`, and owns the base layer required by those utilities, including the application-wide CSS-native scrollbar normalization. It does not define the Orchbee token vocabulary. The full workflow lives in [`docs/frontend/styling.md`](../../docs/frontend/styling.md).

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
