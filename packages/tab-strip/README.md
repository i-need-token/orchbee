# @orchbee/tab-strip

Complete reusable React tab-strip capability. The package combines the public component, its
domain model, layout rules, accessibility projections, drag interactions, and recursively owned
private component tree behind one package entry. It is an Rslib library, not a standalone site.

## Scope

The package owns tab and group transitions, selection invariants, pinned and grouped ordering,
horizontal and vertical layout, drag geometry, accessibility projections, hover cards, menus, and
deterministic React event integration. Consumers own application composition, terminal sessions,
navigation, persistence, product localization, theme token values, and platform or cross-window
host implementations.

## Entry points

```ts
import { TabModel, TabStrip, TabStripOrientation, type TabStripMessages } from '@orchbee/tab-strip';
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
```

- `@orchbee/tab-strip` exposes the component and the selected contracts needed to create and host it.
- `@orchbee/ui/styles.css` provides the shared primitive styles and design-token registry used by
  the context menu.
- `@orchbee/tab-strip/styles.css` provides the component's CSS Module output for built consumers.

Product hosts may provide the complete `TabStripMessages` contract to localize visible controls,
menus, accessibility names, and live-region announcements. The built-in English messages remain a
reference fallback for package tests and non-product fixtures; Orchbee product surfaces supply the
contract from `@orchbee/i18n`.

The package API intentionally has no React or browser-vendor subpath. Browser and DOM adapters
remain private; selected host contracts are promoted through the root only when a consumer must
provide them. Upstream source attribution and license obligations are recorded in
[`NOTICE.reference`](NOTICE.reference).

## Source ownership

```text
src/
├── index.ts
├── styles.ts
└── tab-strip/
    ├── index.ts
    ├── TabStrip.tsx
    ├── components/
    │   └── tab-strip-view/
    │       ├── TabStripView.tsx
    │       └── components/
    │           ├── horizontal-contents/
    │           ├── vertical-contents/
    │           ├── tab-button/
    │           ├── group-header/
    │           ├── drag-ghost/
    │           ├── hover-card/
    │           └── tab-strip-context-menu/
    ├── model/
    ├── layout/
    ├── dragging/
    ├── accessibility/
    ├── contexts/
    ├── hooks/
    ├── constants/
    ├── types/
    └── utils/
```

`TabStrip.tsx` is the narrow public adapter. `TabStripView` composes the prepared horizontal,
vertical, overlay, and menu regions. Private components repeat the same ownership structure
recursively and have no barrels. The four semantic subsystems are cohesive facets of this complete
component capability, not separately consumable source modules; selected public contracts are
re-exported only through `tab-strip/index.ts` and the package root.

## Correctness

The package carries unit and property tests, a TLA+ model for horizontal group drag, a Lean proof
for contiguous-block movement, and deterministic browser traces. See the
[`horizontal group-drag contract`](docs/horizontal-group-drag.md) and
[`formal/README.md`](formal/README.md).

```sh
pnpm --filter @orchbee/tab-strip typecheck
pnpm --filter @orchbee/tab-strip test
pnpm --filter @orchbee/tab-strip verify:correctness
pnpm --filter @orchbee/tab-strip test:exports
```
