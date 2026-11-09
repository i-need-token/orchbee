# Frontend Styling

## Ownership boundaries

| Boundary                        | Contract                                                                                                                                                                                                  |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Orchbee-authored components     | Use CSS Modules and the tokens selected under [`design-tokens.md`](design-tokens.md) and [`visual-foundations.md`](visual-foundations.md).                                                                |
| Generated shadcn source         | Lives under `packages/ui/src/components/ui`, retains Tailwind CSS v4 utilities and composable `className`, and remains a compatibility boundary.                                                          |
| shadcn configuration            | `packages/ui/components.json` pins the Base UI Nova registry style for every CLI run.                                                                                                                     |
| UI style entry                  | `packages/ui/src/styles.ts` imports the token registry before `packages/ui/src/styles.css`.                                                                                                               |
| UI compatibility stylesheet     | Maps shadcn variables to Orchbee tokens, keeps required `@theme inline` declarations, registers `./components` with `@source`, and owns the minimal base layer, including native scrollbar normalization. |
| Complete Tailwind compatibility | `@orchbee/design-tokens/tailwind-compat.css` maps the installed public Tailwind theme but remains unimported until a deliberate migration.                                                                |

The generated exception also covers `packages/ui/src/hooks/use-mobile.ts`, `packages/ui/src/lib/cn.ts`, and `packages/ui/src/styles.css`. Do not copy its utilities into authored components or create a duplicate primitive to rename generated props.

Install an official shadcn primitive instead of hand-writing an equivalent; product composition stays outside the generated primitive.

Standard `:root` custom properties remain the runtime registry because theme selectors, media queries, hosts, and non-Tailwind consumers can override them. Compatibility files map values but never own Orchbee token names or theme values.

## Authored style contract

Use one CSS Module per authored component and import it from JSX.

```tsx
import styles from './FilterPanel.module.css';

export function FilterPanel() {
	return <section className={styles.root}>...</section>;
}
```

Outside the generated shadcn boundary, do not use:

- Tailwind imports, directives, utility strings, or `tailwind-merge`;
- literal or template-string `className` values instead of CSS Module references;
- a general-purpose `className` prop on an authored component;
- JSX attribute spreads except an inline object with statically named properties and no styling escape hatch;
- raw visual values covered by the token registry;
- unregistered custom properties or `var()` fallbacks for registered tokens; or
- inline standard CSS properties.

Inline style objects may contain only registered `--ob-comp-*` keys for measured or user-controlled runtime geometry. Product-semantic composition and application layout never use Tailwind utilities.

## Pointer cursors

At rest every Orchbee control shows the default arrow, including rows, headings, panel titles, tabs, and buttons. A held drag of a Tab, Panel, sidebar row, or any other item shows `grabbing` on every element from the moment it starts until its release or cancellation, whatever the pointer crosses: its owner calls `holdDragCursor` from `@orchbee/ui/components`, and the package stylesheet's document rule overrides every element's own cursor. Surfaces never add their own `grab` or `grabbing` rules. Text inputs and terminal text keep the text cursor, resize splitters keep their resize cursors, and `pointer` belongs only to links.

## Product icons

- Product controls consume `ProductIcon` from `@orchbee/ui/components` through the Orchbee icon-name allowlist.
- Product code never imports vendor stylesheets, class names, font characters, or physical icon assets.
- `@orchbee/ui` maps names to Codicons and loads the provider stylesheet through its style entry.
- `ProductIcon` is decorative, inherits `currentColor`, and defaults to the `14px` `icon-sm` visual-foundation size; the owning control supplies its accessible name and semantic color. A non-default size requires a documented component variant and explicit component-level requirement.
- Generated shadcn source may retain Lucide imports only inside its compatibility boundary.

An icon change updates the allowlist, mapping, focused test, affected package documentation, and third-party notice when provider or license changes. Verify font loading, optical weight, and alignment in a browser.

## Stylesheet imports

| Consumer                          | Single composition-entry import               |
| --------------------------------- | --------------------------------------------- |
| Application using `@orchbee/ui`   | `import '@orchbee/ui/styles.css';`            |
| Interface not using `@orchbee/ui` | `import '@orchbee/design-tokens/tokens.css';` |
| `apps/storybook` UI Stories       | The public `@orchbee/ui` stylesheet           |

Do not import the token registry twice. Storybook scaffolding never restyles components or acts as an implicit theme; follow [`storybook.md`](storybook.md).

## Shadcn component intake

Use the UI package's `component:add` script to refresh explicit admitted component names. Admission follows [experience components](experience-components.md); a Story alone is insufficient. Do not import the complete official registry.

The CLI reads `packages/ui/components.json` and writes to `packages/ui/src/components/ui`; use `--dry-run`, `--view`, and `--diff` when reviewing generated changes.

After refresh:

1. Keep generated implementations and utilities unless a focused repository adaptation is required.
2. Restore the `packages/ui/src/styles.css` mapping to `@orchbee/design-tokens`, including `@source './components'`; reject regenerated raw theme values.
3. Review every overwrite and preserve TypeScript, declaration-output, and module-boundary adaptations.
4. Export only components and slots with admitted consumers through `@orchbee/ui/components`; preserve the narrowed entry and never expose physical source paths.
5. Test repository adaptations and relied-on semantics without duplicating the upstream suite.
6. Run the UI package checks and root verification gate.

Product composition stays with its application or product-capability package. An authored product component may pass semantic props to a primitive but may not add Tailwind utilities through `className`.

## Verification

After changing tokens, generated styles, dependencies, exports, or build configuration, run:

```sh
pnpm lint:styles
pnpm --filter @orchbee/design-tokens... build
pnpm --filter @orchbee/design-tokens typecheck
pnpm --filter @orchbee/design-tokens test
pnpm --filter @orchbee/design-tokens test:exports
pnpm --filter @orchbee/ui... build
pnpm --filter @orchbee/ui typecheck
pnpm --filter @orchbee/ui test
pnpm --filter @orchbee/ui test:exports
```

After a Tailwind dependency update, also run:

```sh
pnpm generate:tailwind-token-compat
pnpm lint:styles
```

Primary references: [Tailwind theme variables](https://tailwindcss.com/docs/theme), [shadcn manual installation](https://ui.shadcn.com/docs/installation/manual), [Base UI support](https://ui.shadcn.com/docs/changelog/2026-01-base-ui), [`components.json`](https://ui.shadcn.com/docs/components-json), [Rslib CSS](https://rslib.rs/guide/advanced/css), and [Codicons](https://github.com/microsoft/vscode-codicons).
