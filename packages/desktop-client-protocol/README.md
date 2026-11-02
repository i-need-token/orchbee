# @orchbee/desktop-client-protocol

Private package entries export only symbols consumed in this repository, enforced by Knip `includeEntryExports`. Internal contracts, generated assets and independent conformance evidence retain their owners; their presence does not automatically expand the public entry. New clients add exports for admitted uses.

Strict, runtime-neutral data contracts for the Electron-specific capability boundary between Orchbee Desktop and the canonical `apps/web` renderer.

## Scope

The package owns the announcement of a document's Server connection: its ports, in the fixed `DESKTOP_SERVER_CONNECTION_PORT_ROLES` order, and the facts only Main can know about that document. The first port carries opaque Client connection bytes; the protocol inside it belongs to the Server's Client contract. The second is that document's Project Folder acquisition port, the only capability Main serves beside the connection.

Local Project Folder acquisition distinguishes existing-Folder selection, native parent selection, and explicit child creation. Creation commands carry the parent selection identity, a monotonic attempt and one bounded name, never a renderer-selected path. Known no-effect failures permit explicit correction on that parent. Successful or uncertain creation consumes it and requires an exact creation-request acknowledgement before another operation; retirement reports retained receipts only during Main lifetime. Folder acquisition returns a validated path without carrying group membership; Workbench resolves addition through the immutable resource namespace. See the [owning contract](../../apps/desktop/docs/project-folder-acquisition.md).

The Agent integration, terminal default, and terminal launch-profile contracts supply the data shapes and event vocabulary of the Web settings controls, which the Server path fills from Server operations. No Electron channel carries their commands, and no boundary parses their events; the terminal snapshot and launch-profile identity schemas stay exported as `@internal` fixture parsers for Web Stories and tests. They expose no executable path, packaged-resource path, raw Agent output, or generic process or filesystem capability.

It contains no Electron imports, UI, broker, coordinator, transport lifecycle, authentication, terminal projection, PTY, or VT behavior. The contract grants only the scoped capability represented by each port.

## Use

Applications import schemas and inferred types from the package root. Every process, IPC, Window, or MessagePort boundary parses untrusted values before passing them to an internal owner.

```ts
import { DesktopServerConnectionWindowMessageSchema } from '@orchbee/desktop-client-protocol';

const message = DesktopServerConnectionWindowMessageSchema.safeParse(untrustedMessage);
```

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/desktop-client-protocol build
pnpm --filter @orchbee/desktop-client-protocol typecheck
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop-client-protocol test:exports
```
