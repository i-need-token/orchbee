# Frontend Styling

## Styling boundaries

Orchbee-authored components use the enumerable token system and CSS Modules defined in [`docs/frontend/design-tokens.md`](design-tokens.md). Do not use Tailwind utilities for product-semantic composition, application layout, or new authored component implementations.

Install reusable primitives from shadcn/ui's Base UI registry and own the resulting open component source in `@orchbee/ui`. Do not hand-write a primitive that shadcn already provides. The checked-in `components.json` pins the Base UI Nova style so every CLI invocation uses the same component family.

Generated shadcn components retain their Tailwind CSS v4 utility classes and standard composable `className` API. The Tailwind exception is limited to `packages/ui/src/components/ui`, its generated `use-mobile.ts` hook, `packages/ui/src/lib/cn.ts`, and the `packages/ui/src/styles.css` compatibility layer. Do not copy its utilities into an authored component or introduce a second primitive implementation merely to rename generated props.

`packages/ui/src/styles.ts` is the style export's source-build entry: it imports the design-token registry before `packages/ui/src/styles.css`. The CSS file maps the generated shadcn variable vocabulary to Orchbee system tokens, retains the `@theme inline` declarations that generate utilities for checked-in shadcn source, and owns the minimal global base layer required by generated components. Neither file owns token names or concrete theme values. Orchbee's runtime registry remains standard `:root` custom properties because authored components consume variables directly and system theme mappings must remain selector-overridable.

`@orchbee/design-tokens/tailwind-compat.css` is a separate, complete mapping of the installed Tailwind default theme to Orchbee reference tokens. It exists so a future migration can preserve Tailwind's enumerable vocabulary while replacing its source of values. It is exported and compilation-tested but intentionally not imported by `@orchbee/ui`; adopting it is a separate compatibility-boundary change, not an incidental token update.

## Stylesheet imports

Applications that use shadcn components import the compatibility stylesheet once at their composition entry:

```ts
import '@orchbee/ui/styles.css';
```

An interface that does not consume `@orchbee/ui` imports the token asset directly before its own styles:

```ts
import '@orchbee/design-tokens/tokens.css';
```

`apps/storybook` imports the UI stylesheet for UI Stories. Storybook layout scaffolding must not redefine component appearance or act as an implicit theme. The Story ownership and browser-verification contract lives in [`docs/frontend/storybook.md`](storybook.md).

## Shadcn component intake

The UI package keeps a complete snapshot of the official Base UI component registry. Refresh that snapshot with:

```sh
pnpm --dir packages/ui exec shadcn add --all --overwrite
```

The CLI reads `packages/ui/components.json` and writes component source under `packages/ui/src/components/ui`. Use its `--dry-run`, `--view`, and `--diff` options before a refresh when generated changes need review.

After a refresh:

1. Keep generated implementations and Tailwind utilities intact unless repository compatibility requires a focused adaptation.
2. Restore `packages/ui/src/styles.css` as a compatibility mapping to `@orchbee/design-tokens`; never accept regenerated raw theme values as a second source of truth.
3. Review every overwrite diff and preserve TypeScript, declaration-output, and module-boundary adaptations.
4. Export every registry component explicitly through `@orchbee/ui/components`; do not expose physical source paths.
5. Test the adaptations or semantics Orchbee relies on; do not duplicate the upstream component suite component by component.
6. Run the UI package checks and the repository verification gate.

Product-semantic composition belongs in `@orchbee/workbench`, while the generated component remains the reusable primitive. A product component may pass semantic props to a shadcn primitive, but it must not use the primitive's `className` escape hatch to add Tailwind utilities.

## Verification

Run the token and UI package checks after changing tokens, generated component styles, dependencies, exports, or build configuration:

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

After a Tailwind dependency update, also regenerate the dormant complete mapping and let the style contract verify exact coverage:

```sh
pnpm generate:tailwind-token-compat
pnpm lint:styles
```

External references verified on 2026-07-17:

- [Tailwind CSS theme variables](https://tailwindcss.com/docs/theme) distinguishes top-level `@theme` declarations that generate utilities and variants from regular `:root` custom properties that do not.
- [shadcn/ui manual installation](https://ui.shadcn.com/docs/installation/manual) documents the Base UI style identifier, Tailwind imports, dependencies, aliases, and `components.json` contract.
- [shadcn/ui Base UI support](https://ui.shadcn.com/docs/changelog/2026-01-base-ui) documents the Base UI component variants and shared shadcn abstraction.
- [shadcn/ui `components.json`](https://ui.shadcn.com/docs/components-json) defines the CLI's durable generation configuration.
- [Rslib CSS support](https://rslib.rs/guide/advanced/css) documents the Tailwind CSS v4 plugin and extracted library styles.
