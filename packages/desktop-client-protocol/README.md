# @orchbee/desktop-client-protocol

Strict, runtime-neutral data contracts for the Electron-specific capability boundary between Orchbee Desktop and the canonical `apps/web` renderer.

## Scope

The package owns one atomic Desktop renderer bootstrap, the fixed ordering and security attestation for its four independent capability ports, and Electron application-lifetime control. The bootstrap embeds the transport-neutral Workbench client-session bootstrap owned by `@orchbee/workbench-client-protocol`.

Application lifetime distinguishes user quit, update restart, and development-only Terminal Host restart. User quit may cancel, preserve, or stop. Update restart automatically preserves. Development Host restart is an explicit non-interactive mode that durably stops current sessions and stages fresh provisions before Main reports readiness; it is not an updater alias. Commands carry only a request identity, mode, aggregate counts, the exact shared-document revision, progress, bounded typed failures, and the Workbench presentation identities needed to resume an in-progress development Host restart after renderer replacement. They never expose Host, binding, terminal, or filesystem authority.

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
