# @orchbee/desktop-client-protocol

Private package entries export only symbols consumed in this repository, enforced by Knip `includeEntryExports`. Internal contracts, generated assets and independent conformance evidence retain their owners; their presence does not automatically expand the public entry. New clients add exports for admitted uses.

Strict, runtime-neutral data contracts for the Electron-specific capability boundary between Orchbee Desktop and the canonical `apps/web` renderer.

## Scope

The package owns the announcement of a document's Server connection: its ports, in the fixed `DESKTOP_SERVER_CONNECTION_PORT_ROLES` order, and the facts only Main can know about that document. The first port carries opaque Client connection bytes; the protocol inside it belongs to the Server's Client contract. Its one other message, from the page, reports how many relayed bytes the page has read, once each chunk's handling returns; Main relays output only while what is unread stays under its bound. The second is that document's Project Folder acquisition port. The third is its background-status port, which carries from the page to Main the summary of running terminals and Agent states that the Windows notification-area icon shows while the window is hidden, the page's request to show the window again, and its request to open Quit All's confirmation from a terminal that can't reconnect; Main counts nothing itself. The one message Main sends on it is Quit All's Session stop, under a stop identity Main chose, because only the page's Server connection can ask for it; the page answers each with what became of it, and Main reads the stop's progress with `operation status`. The fourth is its window-update port: Main offers the newer window version the window may switch to, or none, on connection and at each change, and answers a request it cannot carry out with waiting while a terminal keeps it out or failure when launching the newer window fails; the page asks only for the version offered, and names the place it shows, its displayed terminal or else its Workspace, by the Workbench identity schemas of `@orchbee/workbench-client-protocol`. The first document of a window that Update relaunched receives that place as `relaunchPlace`. The announcement also says whether Main keeps Quit All for that window, as `quitAllAvailable`.

Local Project Folder acquisition distinguishes existing-Folder selection, native parent selection, and explicit child creation. Creation commands carry the parent selection identity, a monotonic attempt and one bounded name, never a renderer-selected path. Known no-effect failures permit explicit correction on that parent. Successful or uncertain creation consumes it and requires an exact creation-request acknowledgement before another operation; retirement reports retained receipts only during Main lifetime. Folder acquisition returns a validated path without carrying group membership; Workbench resolves addition through the immutable resource namespace. See the [owning contract](../../apps/desktop/docs/project-folder-acquisition.md).

Outside the ports, the page posts to its own window the color its theme gives the Windows caption buttons' symbols, as `#rrggbb`, whenever that theme changes; the preload forwards only a message that parses, on `DESKTOP_WINDOW_CAPTION_IPC_CHANNEL`, and Main parses the color again before drawing the buttons with it. The same way, the page posts the site of each fault it contains or lets escape, a closed token from `DesktopWindowFaultSiteSchema` and never a message or stack, which the preload forwards on `DESKTOP_WINDOW_FAULT_IPC_CHANNEL` for Main's diagnostic log.

Settings data shapes belong to their controls in `apps/web`, and launch profiles to `@orchbee/workbench-client-protocol`; no Electron channel carries them.

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
