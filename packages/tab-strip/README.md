# @orchbee/tab-strip

Reusable React tab-strip components backed by independently testable state, layout, and dragging
behavior. This package is a library built with Rslib; it is not a standalone site.

## Scope

The package owns tab and group transitions, selection invariants, pinned and grouped ordering,
compact-layout calculations, drag geometry, accessibility projections, and deterministic React
event integration. Product data, persistence, navigation, localization, host effects, icons, and
final presentation remain consumer responsibilities.

The current React renderer and optional stylesheet are a behavior-preserving migration surface.
They still contain reference presentation and host wiring that will be separated in a later phase.
New consumers should depend on the root engine and `@orchbee/tab-strip/react`; visual integration
must not treat the optional stylesheet as the product design system.

The unmodified reference stylesheet lives outside `src` under `reference/`: it preserves the
migration baseline and is not an Orchbee-authored token or CSS Module implementation.

## Entry points

```ts
import { TabModel, TabStripOrientation } from '@orchbee/tab-strip';
import { TabStrip } from '@orchbee/tab-strip/react';
import '@orchbee/tab-strip/styles.css';
```

- `@orchbee/tab-strip` exposes framework-independent state, layout, drag, and accessibility APIs.
- `@orchbee/tab-strip/react` exposes the React integration and the migrated reference renderer.
- `@orchbee/tab-strip/styles.css` exposes the optional migration stylesheet.

The package API intentionally uses domain names and has no browser-vendor subpath. Browser and DOM
host adapters used by the migrated React renderer remain private. Upstream source attribution and
license obligations are recorded in [`NOTICE.reference`](NOTICE.reference).

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
