# Frontend Styling

## Current stack

Install reusable primitives from shadcn/ui's Base UI registry and own the resulting open component source in `@orchbee/ui`. Do not hand-write a primitive that shadcn already provides. The checked-in `components.json` pins the Base UI Nova style so every CLI invocation uses the same component family.

Generated components retain their Tailwind CSS v4 utility classes and standard composable `className` API. Do not introduce a second primitive implementation or wrapper layer merely to rename generated props.

`packages/ui/src/styles.css` owns the shared shadcn semantic color tokens, light/dark values, Tailwind `@theme` mappings, and the minimal global base layer required by the components. It is not an Orchbee product theme, token registry, `ThemeRoot`, or CSS Module enforcement layer. Product-specific theming remains a separate design-system concern.

Applications import the component stylesheet explicitly at their composition entry:

```ts
import '@orchbee/ui/styles.css';
```

`apps/storybook` imports the same public stylesheet for UI Stories. Storybook layout scaffolding must not redefine component appearance or act as an implicit theme. The Story ownership and browser-verification contract lives in [`docs/frontend/storybook.md`](storybook.md).

## Shadcn component intake

The UI package keeps a complete snapshot of the official Base UI component registry. Refresh that snapshot with:

```sh
pnpm --dir packages/ui exec shadcn add --all --overwrite
```

The CLI reads `packages/ui/components.json` and writes component source under `packages/ui/src/components/ui`. Use its `--dry-run`, `--view`, and `--diff` options before a refresh when generated changes need review.

After a refresh:

1. Keep generated implementations and Tailwind utilities intact unless repository compatibility requires a focused adaptation.
2. Review every overwrite diff and preserve TypeScript, declaration-output, and module-boundary adaptations.
3. Export every registry component explicitly through `@orchbee/ui/components`; do not expose physical source paths.
4. Test the adaptations or semantics Orchbee relies on; do not duplicate the upstream component suite component by component.
5. Run the UI package checks and the repository verification gate.

Product-semantic composition belongs in `@orchbee/workbench`, while the generated component remains the reusable primitive.

## Verification

Run the UI package checks after changing its source, dependencies, exports, or build configuration:

```sh
pnpm --filter @orchbee/ui build
pnpm --filter @orchbee/ui typecheck
pnpm --filter @orchbee/ui test
pnpm --filter @orchbee/ui test:exports
```

External references verified on 2026-07-17:

- [shadcn/ui manual installation](https://ui.shadcn.com/docs/installation/manual) documents the Base UI style identifier, Tailwind imports, dependencies, aliases, and `components.json` contract.
- [shadcn/ui Base UI support](https://ui.shadcn.com/docs/changelog/2026-01-base-ui) documents the Base UI component variants and shared shadcn abstraction.
- [shadcn/ui `components.json`](https://ui.shadcn.com/docs/components-json) defines the CLI's durable generation configuration.
- [Rslib CSS support](https://rslib.rs/guide/advanced/css) documents the Tailwind CSS v4 plugin and extracted library styles.
