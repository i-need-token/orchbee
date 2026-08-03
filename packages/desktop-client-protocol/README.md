# @orchbee/desktop-client-protocol

Strict, runtime-neutral data contracts for the sandboxed capability boundary between Orchbee
Desktop and the canonical `apps/web` renderer.

## Scope

The package owns the validated messages for one atomic Desktop renderer bootstrap and its three
independent ports: shared Workbench client session, terminal projection control, and application
lifetime control. It also owns the projected attachment stream exchanged through a successfully
opened projection.

It contains no Electron imports, UI, broker, coordinator, transport lifecycle, network Gateway,
authentication, PTY, or VT behavior. Workbench and TerminalPresentation semantics remain with
`@orchbee/workbench`; Terminal Host wire semantics remain with `@orchbee/terminal-protocol`.

## Use

Applications import schemas and inferred types from the package root. Every process, IPC, Window,
or MessagePort boundary parses untrusted values before passing them to an internal owner.

```ts
import {
	DesktopClientSessionEventSchema,
	DesktopRendererBootstrapSchema,
} from '@orchbee/desktop-client-protocol';

const bootstrap = DesktopRendererBootstrapSchema.parse(untrustedBootstrap);
const event = DesktopClientSessionEventSchema.parse(untrustedEvent);
```

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/desktop-client-protocol build
pnpm --filter @orchbee/desktop-client-protocol typecheck
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop-client-protocol test:exports
```
