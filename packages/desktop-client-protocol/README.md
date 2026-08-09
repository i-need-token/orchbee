# @orchbee/desktop-client-protocol

Strict, runtime-neutral data contracts for the Electron-specific capability boundary between Orchbee Desktop and the canonical `apps/web` renderer.

## Scope

The package owns one atomic Desktop renderer bootstrap, the fixed ordering and security attestation for its three independent capability ports, and Electron application-lifetime control. The bootstrap embeds the transport-neutral Workbench client-session bootstrap owned by `@orchbee/workbench-client-protocol`.

It contains no Electron imports, UI, broker, coordinator, transport lifecycle, network Gateway, authentication, terminal projection, PTY, or VT behavior. Shared Workbench page-client contracts remain with `@orchbee/workbench-client-protocol`; Workbench semantics remain with `@orchbee/workbench`.

## Use

Applications import schemas and inferred types from the package root. Every process, IPC, Window, or MessagePort boundary parses untrusted values before passing them to an internal owner.

```ts
import { DesktopRendererBootstrapSchema } from '@orchbee/desktop-client-protocol';

const bootstrap = DesktopRendererBootstrapSchema.parse(untrustedBootstrap);
```

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/desktop-client-protocol build
pnpm --filter @orchbee/desktop-client-protocol typecheck
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop-client-protocol test:exports
```
