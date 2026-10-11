# @orchbee/design-tokens

Enumerable design decisions shared by Orchbee CSS interfaces.

Common GUI/TUI semantics and theme correspondence follow
[`docs/INTERFACE-DESIGN.md`](../../docs/INTERFACE-DESIGN.md). This package implements CSS tokens;
native terminal themes implement their own mapping of the common roles.

## Use

Import the CSS registry once through the application stylesheet or a compatibility layer:

```css
@import '@orchbee/design-tokens/tokens.css';

.panel {
	padding: var(--ob-ref-space-4);
	color: var(--ob-sys-color-text-primary);
	background: var(--ob-sys-color-surface-raised);
}
```

JavaScript consumers that need a CSS variable reference can use the typed inventory:

```ts
import { designToken, responsiveBreakpoints } from '@orchbee/design-tokens';

const foreground = designToken('--ob-sys-color-text-primary');
const compactViewport = responsiveBreakpoints.sm;
```

The `designTokenNames` export is the complete public CSS custom-property inventory. [`src/design-token-names.generated.ts`](src/design-token-names.generated.ts) generates that inventory and `responsiveBreakpoints` from [`src/tokens.css`](src/tokens.css); package tests and the repository style check reject stale output.

## Tailwind compatibility

The package also exports `@orchbee/design-tokens/tailwind-compat.css`. This generated, dormant asset enumerates all 419 public `@theme` variables from the installed Tailwind CSS 4.3.3 theme and maps them to Orchbee reference tokens. It is migration infrastructure, not a second authoring API, and `@orchbee/ui` does not import it today.

Consumers adopting the full compatibility layer must provide both the runtime token registry and the Tailwind build-time theme map:

```css
@import 'tailwindcss';
@import '@orchbee/design-tokens/tokens.css';
@import '@orchbee/design-tokens/tailwind-compat.css';
```

Tailwind's internal `--tw-*` utility state is intentionally outside the design-token inventory. Breakpoint and container mappings use checked literal values because CSS custom properties cannot appear in query conditions.

The runtime registry deliberately uses standard custom properties. Tailwind's `@theme` directive is a compile-time API that creates utilities and variants, so it belongs only in the Tailwind and shadcn compatibility boundary. Publishing `--ob-*` directly through `@theme` would create a second utility-class authoring API and would prevent system mappings from being scoped under selectors such as `[data-theme='dark']`.

## Themes

Reference tokens (`--ob-ref-*`) are stable visual scales. System tokens (`--ob-sys-*`) give those values interface meaning and change under explicit `[data-theme='light']` and `[data-theme='dark']` boundaries; `:root` supplies the light fallback. Components consume system color tokens so theme changes do not alter component CSS.

System colors enumerate surface, content, border, action, status, and interaction states. Every system color has explicit light and dark mappings. Semantic typography tokens combine family, size, weight, and line height for body, label, heading, and code roles:

```css
.title {
	font: var(--ob-sys-typography-heading-md);
	color: var(--ob-sys-color-text-heading);
}
```

Authored motion uses the semantic duration and easing tokens. The registry maps non-instant semantic durations to instant when the user requests reduced motion.

CSS custom properties cannot be evaluated in media-query conditions. Use only the exported `responsiveBreakpoints` values as literal `@media` or `@container` thresholds; the repository style check enforces that finite exception.

The authoring, naming, and component-token rules are defined in [`docs/frontend/design-tokens.md`](../../docs/frontend/design-tokens.md).

## Development

```sh
pnpm --filter @orchbee/design-tokens build
pnpm --filter @orchbee/design-tokens typecheck
pnpm --filter @orchbee/design-tokens test
pnpm --filter @orchbee/design-tokens test:exports
pnpm generate:design-token-inventory
pnpm generate:tailwind-token-compat
```
