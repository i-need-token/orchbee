# @orchbee/tab-strip

Reusable React tab-strip components backed by independently testable state, layout, and dragging
behavior. This package is a library built with Rslib; it is not a standalone site.

## Scope

The package owns tab and group transitions, selection invariants, pinned and grouped ordering,
compact-layout calculations, drag geometry, accessibility projections, and deterministic React
event integration. Product data, persistence, navigation, localization, host effects, icons, and
final presentation remain consumer responsibilities.

The current React renderer is a behavior-preserving migration surface. Its presentation is owned
by `src/TabStrip.module.css` and consumes the repository design-token registry. Host wiring that
still lives in the renderer will be separated in a later phase.

## Entry points

```ts
import { TabModel, TabStripOrientation } from '@orchbee/tab-strip';
import { TabStrip } from '@orchbee/tab-strip/react';
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
```

- `@orchbee/tab-strip` exposes framework-independent state, layout, drag, and accessibility APIs.
- `@orchbee/tab-strip/react` exposes the React integration and renderer.
- `@orchbee/ui/styles.css` provides the shared primitive styles and design-token registry used by
  the context menu.
- `@orchbee/tab-strip/styles.css` provides the component's CSS Module output for built consumers.

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
