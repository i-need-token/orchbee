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

The root has no JavaScript API; products consume CSS custom properties. [`src/design-token-names.generated.ts`](src/design-token-names.generated.ts) generates the internal `designTokenNames` and `responsiveBreakpoints` inventories from [`src/tokens.css`](src/tokens.css); package tests and repository style checks use them to reject stale output.

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

The sans and code stacks include explicit Chinese system-font fallbacks, avoiding serif browser defaults for Chinese glyphs. Display roles use semibold type with a compact 1.125 line-height at 48, 60 and 72 CSS pixels; application heading roles retain their existing scale.

Authored motion uses the semantic duration and easing tokens. The registry maps non-instant semantic durations to instant when the user requests reduced motion.

CSS custom properties cannot be evaluated in media-query conditions. Use only the generated `responsiveBreakpoints` inventory values as literal `@media` or `@container` thresholds; the repository style check enforces that finite exception.

The `xs` breakpoint at 24rem distinguishes narrow phone layouts where display headings and primary actions need a more compact arrangement.

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

Agent attention, working, idle, completed, failed and unread are separate roles. Attention and working use `--ob-sys-color-status-attention` and `--ob-sys-color-status-working`; idle, completed and failed use `--ob-sys-color-status-agent-idle`, `--ob-sys-color-status-agent-done` and `--ob-sys-color-status-agent-error`. Unread keeps `--ob-sys-color-status-unread` blue independently of status. Unknown retains the secondary icon color. Consumers preserve these foregrounds on selected surfaces.

The registry owns the shared dark Agent palette below: neutral Working plus GrokNight outcome colors. Native One Dark status roles project the same values; light and terminal-native themes retain their contrast-aware adaptations. Working uses `--ob-sys-motion-duration-agent-breath` and `--ob-sys-motion-easing-agent-breath` for a two-second white-to-gray breathing solid circle; light themes use dark foreground for contrast. Attention alternates full and half intensity over `--ob-sys-motion-duration-agent-cycle` (one second). Reduced-motion consumers keep a static frame.

| State     | Dark color |
| --------- | ---------- |
| Working   | `#E5E5E5`  |
| Attention | `#E0AF68`  |
| Completed | `#9ECE6A`  |
| Failed    | `#F7768E`  |
| Idle      | `#585858`  |
