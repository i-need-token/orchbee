# Frontend Styling

## Stack contract

Build accessible component behavior with Base UI and use shadcn/ui as a reviewed open-code component recipe. Own the resulting React and CSS source in `@orchbee/ui`; Tailwind CSS is not part of the Orchbee runtime or authoring stack.

Use CSS Modules for component and product styles. Use registered CSS custom properties for every design-bearing value so an application or user theme can change presentation at runtime without recompiling component code.

Applications import each library stylesheet explicitly at their composition entry:

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/workbench/styles.css';
```

## Token authority

`packages/ui/src/theme/theme.css` is the authoritative registry and default-value source for all `--ob-*` theme properties. Component styles consume those properties but never declare new custom properties or local fallback values.

Add a token only when its meaning is reusable across components or is a deliberate product-layout customization point. Name tokens by semantic responsibility, not by their current literal value. A component-specific token stays exceptional and names the owning component explicitly.

```css
/* Incorrect: private value and local fallback bypass the theme contract. */
.panel {
	padding: var(--panel-gap, 12px);
}

/* Correct: the registered semantic token owns the value. */
.panel {
	padding: var(--ob-space-3);
}
```

The token catalog is an experimental public contract while `@orchbee/ui` is experimental. Review renames and removals as consumer-facing changes, and update component styles, runtime theme adapters, package documentation, and tests together.

## CSS Modules

Name component stylesheets `*.module.css` and keep selectors local. `:global`, `!important`, and custom-property declarations are forbidden in a CSS Module. Do not expose `className` or `style` from design-system components as a general styling escape hatch; callers compose layout around a component or request a reviewed variant.

Global source CSS is restricted to architecture-owned boundaries:

- `packages/ui/src/theme/theme.css` declares theme properties and theme-root normalization.
- `apps/main/src/composition/document.css` establishes the document viewport required by the application root.

Add another global stylesheet only after documenting the runtime boundary that cannot be expressed through a theme root or CSS Module.

## Authored value policy

Colors, spacing, dimensions, typography, radii, borders, shadows, opacity, layers, transforms, and motion values must reference a registered `--ob-*` token. Raw colors, CSS dimensions, and durations are rejected in component styles.

Finite structural CSS values may remain literal when they describe behavior rather than visual choice, including `display: grid`, `overflow: hidden`, `position: relative`, `minmax(0, 1fr)`, `inline-size: 100%`, and zero-value resets. A structural declaration must still use a token for any embedded design dimension.

```css
/* Incorrect: the grid structure owns a hidden design dimension. */
.shell {
	grid-template-columns: 72px minmax(0, 1fr);
}

/* Correct: structure remains local and the dimension is customizable. */
.shell {
	grid-template-columns: var(--ob-layout-activity-width) minmax(0, 1fr);
}
```

Inline styles may be introduced only through a typed runtime API that sets a reviewed custom property for genuinely dynamic geometry or user theme data. Do not place raw design values in JSX.

## Shadcn component intake

Do not run `shadcn add` directly against canonical package source. The generated component assumes its configured styling system and may overwrite owned code. Use the Base UI variant as an upstream recipe, then adapt it deliberately:

1. Review the current shadcn component composition and the corresponding Base UI behavior and state attributes.
2. Keep the accessible semantics, keyboard behavior, state model, and recognizable shadcn component API where they fit Orchbee.
3. Remove Tailwind utilities and translate presentation into an owned CSS Module.
4. Replace every design-bearing value with a registered `--ob-*` token.
5. Narrow styling escape hatches and expose reviewed variants instead of arbitrary classes.
6. Add unit coverage for variants, state attributes, semantics, and owned behavior.
7. Run `pnpm lint:styles` and the package verification commands.

A future Orchbee shadcn registry may distribute these owned implementations if another repository becomes a concrete consumer. Do not add registry infrastructure before that consumer exists.

## Runtime customization

Application defaults come from `ThemeRoot`. Built-in color schemes and future user themes override the same registered properties at that root. Components must react to those changes through normal CSS inheritance and must not branch on a theme name in React.

Persisted user theme data is untrusted input. When a settings boundary is introduced, validate token names against the registered allowlist and validate values by token category before applying them. Components never read persisted theme data directly.

## Verification

`pnpm lint:styles` scans source CSS, resolves the token registry, and rejects unknown tokens, local fallbacks, raw design literals, module-defined properties, global escapes, and unapproved global stylesheets. `pnpm test:styles` covers the checker behavior. Both commands run through `pnpm verify`.

External references verified on 2026-07-17:

- [Base UI styling](https://base-ui.com/react/overview/about) confirms that Base UI is unstyled and supports CSS Modules.
- [shadcn/ui Base UI support](https://ui.shadcn.com/docs/changelog/2026-01-base-ui) documents the Base UI component variants and shared shadcn abstraction.
- [Rslib CSS support](https://rslib.rs/guide/advanced/css) documents built-in CSS Modules processing and extracted library styles.
