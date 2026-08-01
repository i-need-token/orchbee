# Frontend Design Tokens

## Ownership and consumption

`@orchbee/design-tokens` owns every enumerable visual decision shared by Orchbee interfaces. Its authoritative CSS registry is `packages/design-tokens/src/tokens.css`; `packages/design-tokens/src/design-token-names.generated.ts` is the generated TypeScript inventory behind the `designTokenNames` export. The package test and `pnpm lint:styles` reject stale generated output.

Import `@orchbee/design-tokens/tokens.css` once before authored component styles. Applications that already import `@orchbee/ui/styles.css` receive the registry through the UI package's style entry and must not import it a second time.

CSS grammar is not a design decision. Keywords such as `grid`, `auto`, `hidden`, `currentColor`, and `none`, zero values, relative percentages, grid fractions, and structural integer counts remain literal. Colors, nonzero lengths, typography values, radii, borders, shadows, opacity, layers, and motion values use registered tokens.

This document defines token architecture and enforcement. Use
[`visual-foundations.md`](visual-foundations.md) to choose the correct typography, text hierarchy,
spacing, radius, control size, icon size, border, and surface variable for a product scenario.

## Token tiers

Use the grammar `--ob-<tier>-<category>-<role>[-<variant-or-state>]` with lowercase kebab-case segments.

| Tier   | Name prefix   | Responsibility                                                            | Dependency direction     |
| ------ | ------------- | ------------------------------------------------------------------------- | ------------------------ |
| `ref`  | `--ob-ref-*`  | Concrete palette, spacing, type, radius, shadow, motion, and layer scales | Concrete CSS values only |
| `sys`  | `--ob-sys-*`  | Theme-aware interface meaning such as surface, text, action, and status   | `ref` tokens             |
| `comp` | `--ob-comp-*` | A deliberately public component styling or runtime-geometry slot          | `ref` or `sys` tokens    |

Name tokens for their stable role rather than their current value:

```css
/* ❌ Value name and unregistered local variable. */
--gray-200: #e5e5e5;

/* ✅ Registered scale and semantic role. */
--ob-ref-color-neutral-200: oklch(0.922 0 0);
--ob-sys-color-border-default: var(--ob-ref-color-neutral-200);
```

Authored component CSS consumes `--ob-sys-color-*` for every color and `--ob-sys-typography-*` for interface text roles. It may consume the approved non-color `ref` subsets in [`visual-foundations.md`](visual-foundations.md) directly when the choice is structural, such as `--ob-ref-space-4` or `--ob-ref-radius-ui-md`. Do not consume reference colors from component CSS because doing so bypasses theme semantics; do not reconstruct a published typography role from individual font references.

Add a `comp` token only when an implemented reusable component must expose a stable customization point or receive runtime geometry that cannot be enumerated, such as a user-resized panel width. Declare the token in the central registry and public inventory before using or overriding it; do not create component-local custom properties. A third-party component library's internal token inventory is not a reason to predeclare unused Orchbee component tokens.

## Theme contract

`:root` provides the fallback light system-color mapping. `[data-theme='light']` reapplies that mapping at an explicit light boundary, and `[data-theme='dark']` applies the dark mapping. Theme boundaries override system colors and `color-scheme` only; reference and component tokens remain stable. Inherited custom properties scope the selected mapping to that boundary's subtree, so component CSS never branches on the active theme.

The initial registry accepts only `light` and `dark`. An application composition boundary owns the document theme attribute. A named region owner may set `data-theme` on a wrapper only when the complete subtree intentionally uses another theme; leaf components never set it for isolated styling. Add another theme only when a concrete product consumer needs it, and override system tokens rather than reference or component tokens.

`packages/ui/src/styles.css` maps shadcn's generated variables such as `--background`, `--primary`, and `--ring` to Orchbee system tokens at `:root` and both explicit theme boundaries. Re-declaring the aliases at each boundary makes their indirect `var(--ob-sys-*)` references compute against that boundary's system-token values. Its Tailwind `dark` variant targets `[data-theme='dark']`; `.dark` is not a supported theme API. These unprefixed variables are a compatibility API for checked-in shadcn source, not an alternative token vocabulary for authored components.

## Runtime registry and Tailwind theme variables

The Orchbee registry uses standard custom properties under `:root` because it is a framework-independent runtime API. Standard custom properties can be overridden by theme selectors, media queries, application hosts, and non-Tailwind consumers without creating any CSS classes.

Tailwind's `@theme` directive has a different responsibility: a top-level theme-variable declaration instructs Tailwind to create matching utilities or variants. Do not publish `--ob-*` through `@theme`; doing so would expose a second utility-class authoring API, bypass CSS Modules and semantic consumption rules, and prevent selector-scoped system mappings such as `[data-theme='dark']`.

Use `@theme inline` only inside the checked Tailwind compatibility boundaries:

- `packages/ui/src/styles.css` exposes the focused shadcn vocabulary required by generated components.
- `packages/design-tokens/src/tailwind-compat.css` exposes the complete dormant migration vocabulary.

These compatibility declarations map Tailwind namespaces to Orchbee tokens; they do not own token values. New Orchbee-authored components consume `var(--ob-*)` directly and never require a generated utility class.

## Semantic coverage

The system tier is a finite cross-component language. Choose the closest existing role before proposing another; a component name does not belong in a system-token name.

| Family     | Required role matrix                                                                                                                                             |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Surface    | canvas, inactive, subtle, raised, overlay, inverse, hovered, pressed, selected, selected-hover, disabled, scrim                                                  |
| Text       | primary, heading, secondary, label, muted, placeholder, disabled, inverse, and link rest/hover/focus/pressed                                                     |
| Icon       | primary, secondary, muted, inverse, hover, disabled                                                                                                              |
| Border     | subtle, default, strong, focus, selected, disabled                                                                                                               |
| Scrollbar  | transparent track, plus thumb rest, hover, and pressed                                                                                                           |
| Action     | primary and secondary rest/hover/pressed/text, plus shared disabled surface/text/border                                                                          |
| Status     | info, success, warning, and danger; each status has rest, hover, pressed, surface, surface-hover, border, border-hover, text, text-hover, and text-pressed roles |
| Data       | five categorical series colors                                                                                                                                   |
| Typography | body sm/md/lg, label xs/sm/md/lg, heading sm/md/lg/xl, and code sm/md                                                                                            |
| Other      | link decoration rest/hover/focus, semantic motion duration/easing, focus-ring geometry, disabled/muted opacity, and raised/overlay shadow                        |

Every `--ob-sys-color-*` token has explicit `[data-theme='light']` and `[data-theme='dark']` mappings; `:root` shares the light mapping as its fallback. Package tests enforce equal inventories and ensure theme scopes contain no reference or component tokens. Status and typography matrices are also regression-tested so deleting one state cannot leave an apparently valid but incomplete registry.

Consume a typography role through the `font` shorthand so family, size, weight, and line height remain one decision:

```css
.title {
	font: var(--ob-sys-typography-heading-md);
	color: var(--ob-sys-color-text-heading);
}

.description {
	font: var(--ob-sys-typography-body-sm);
	color: var(--ob-sys-color-text-secondary);
}
```

The spacing, sizing, radius, breakpoint, motion, elevation, and layer foundations stay in the reference tier because their values are context-independent. Do not create separate system aliases named for CSS properties such as margin and padding; components select from the same finite spacing scale.

Authored transitions and animations consume `--ob-sys-motion-duration-*` and `--ob-sys-motion-easing-*`, not motion references directly. Under `prefers-reduced-motion: reduce`, the registry maps fast, normal, and slow semantic durations to the instant reference. Keyframe names and transitioned properties remain component implementation details.

## Tailwind replacement readiness

The reference tier intentionally covers the complete default theme vocabulary of the installed Tailwind CSS version, even when Orchbee-authored components do not consume most of that vocabulary yet. The current Tailwind 4.3.3 baseline contains 419 public `@theme` variables across 19 namespace families:

| Capability group | Tailwind namespaces                                                                  | Variables |
| ---------------- | ------------------------------------------------------------------------------------ | --------: |
| Color            | `color`                                                                              |       288 |
| Typography       | `font`, `font-weight`, `text`, `tracking`, `leading`                                 |        54 |
| Layout           | `spacing`, `breakpoint`, `container`, `radius`, `perspective`, `aspect`, `max-width` |        35 |
| Effects          | `shadow`, `inset-shadow`, `drop-shadow`, `blur`                                      |        27 |
| Motion/defaults  | `ease`, `animate`, `default`                                                         |        15 |
| Total            |                                                                                      |       419 |

`@orchbee/design-tokens/tailwind-compat.css` enumerates that installed theme one for one. It is generated from Tailwind's packaged `theme.css`, maps each public theme variable to an Orchbee reference token, and is exported as migration infrastructure. `@orchbee/ui/styles.css` does not import this file: shadcn keeps its focused compatibility map until a deliberate migration switches that boundary.

The coverage target is Tailwind's public `@theme` contract, not its internal `--tw-*` runtime state. Variables such as `--tw-shadow`, `--tw-ring-color`, and `--tw-translate-x` are implementation details emitted by utilities; they are not enumerable design decisions and disappear with the utility that owns them.

Breakpoints and container sizes are the only mapping exception. CSS custom properties are invalid in media and container query conditions, so `--breakpoint-*` and `--container-*` must remain literal inside the Tailwind compatibility file. Matching `--ob-ref-breakpoint-*` and `--ob-ref-container-*` tokens still make the vocabulary enumerable. `pnpm lint:styles` verifies that all three sources—the installed Tailwind literal, the compatibility literal, and the Orchbee reference token—are identical.

Full color palettes exist in the reference tier for replacement completeness. They do not expand the authored-component contract: component colors must still use semantic `--ob-sys-color-*` tokens.

The same restriction applies to spacing, radius, typography reference, control-size, and icon-size
tokens. Registry presence proves that a value is enumerable; it does not make that value an
approved product choice. The authored subsets and their scenario rules live in
[`visual-foundations.md`](visual-foundations.md).

## Authored component consumption

- Follow [`styling.md`](styling.md) for CSS Modules, `className`, Tailwind, JSX spread, inline-style, and generated-source boundaries.
- Consume `--ob-sys-color-*` for colors and `--ob-sys-typography-*` for complete typography roles.
- Consume only the non-color reference subsets approved by [`visual-foundations.md`](visual-foundations.md).
- Use registered `--ob-comp-*` properties only for public customization or runtime geometry.
- Never use an unregistered custom property or a fallback for a registered token.

## Responsive thresholds

CSS custom properties cannot supply values to `@media` or `@container` conditions. `responsiveBreakpoints` from `@orchbee/design-tokens` is the finite exception and currently defines `sm`, `md`, `lg`, `xl`, and `2xl` thresholds. The TypeScript map is generated from the matching `--ob-ref-breakpoint-*` declarations, so CSS, TypeScript, and Tailwind compatibility values have one authority. Use the corresponding exported value as the literal CSS threshold; `pnpm lint:styles` rejects other `px` or `rem` query values.

Do not use a breakpoint merely because a device class exists. Prefer intrinsic layout, flex/grid constraints, and container queries; add a new threshold only when a concrete layout transition cannot use the existing vocabulary.

## Token changes

Before adding or changing a token:

1. Identify the recurring cross-component role and confirm that an existing token does not express it.
2. Select the lowest valid tier and a role-based name.
3. Add the CSS declaration, add every required theme mapping for a system color, and run `pnpm generate:design-token-inventory`; never edit the generated TypeScript inventory by hand.
4. Update the consumer and any affected Story in the same change.
5. Run `pnpm lint:styles`, `pnpm --filter @orchbee/design-tokens test`, and the root `pnpm verify` gate.

After changing the Tailwind version, run `pnpm generate:tailwind-token-compat`. If Tailwind added a theme variable, the generator rejects an unknown namespace or `pnpm lint:styles` reports its missing Orchbee target token. Add that reference token deliberately, regenerate the compatibility file, update the 419-variable baseline documented and tested here, and run the full verification gate. Do not hand-edit generated compatibility output.

Do not add aliases solely to preserve an unshipped name. Rename an experimental token and all consumers atomically. Once a token becomes part of a stable published package, follow the package compatibility rules in [`docs/PACKAGES.md`](../PACKAGES.md#versioning).
