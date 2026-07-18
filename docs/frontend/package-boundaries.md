# Frontend Package Boundaries

## Workspace placement

Use `apps/*` for runtime entry points and `packages/*` for stable capabilities consumed through a library API. Do not mirror UI source categories such as components, hooks, stores, types, or utilities as workspace packages.

The repository-wide UI catalog is the development application `apps/storybook`. Story files remain owned by their frontend workspace according to [`docs/frontend/storybook.md`](storybook.md); Storybook does not change runtime package ownership.

The target dependency model is a placement contract, not a requirement to create every named workspace immediately. Add a package only in the same change as a concrete workspace consumer, and keep application-specific code in its owning app until that boundary exists.

```text
apps/desktop ---+
                 +--> @orchbee/workbench --> @orchbee/ui ----------+
apps/web --------+             |                                   |
                               +--> @orchbee/design-tokens <--------+
                               |
                               +--> @orchbee/terminal
                               |
                               +--> @orchbee/session-client --> @orchbee/session-protocol

apps/desktop --------------------> @orchbee/session-runtime --> @orchbee/session-protocol
apps/daemon ---------------------> @orchbee/session-runtime
```

Application workspaces own runtime startup, routing roots, authentication entry points, deployment or packaging configuration, and platform-specific adapters. Packages never import from an app.

## Package responsibilities

| Package                     | Runtime     | Responsibility                                                                     | Non-goals                                                                                                           | Admission point                                                                                       |
| --------------------------- | ----------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `@orchbee/terminal`         | `browser`   | Terminal parsing, rendering, input, selection, search, and browser integration     | React UI, application PTY processes, session transport, Electron effects, or product dialogs                        | Existing independently consumable library                                                             |
| `@orchbee/ui`               | `browser`   | Reusable shadcn components backed by Base UI                                       | Themes, design tokens, sessions, agents, terminal panes, routing, remote state, or platform APIs                    | Add with the first consuming product UI                                                               |
| `@orchbee/design-tokens`    | `universal` | Enumerable visual scales, semantic theme roles, and responsive thresholds          | React components, product interactions, application state, arbitrary component values, or effects                   | Existing consumer in the shadcn compatibility stylesheet                                              |
| `@orchbee/workbench`        | `browser`   | Shared product-semantic React interface, interaction state, and presentation model | Application startup, routing roots, authentication, persistence, platform imports, PTY ownership, or wire transport | Existing consumer in `apps/web`; keep private and experimental until its cross-app contract is proven |
| `@orchbee/session-protocol` | `universal` | Zod schemas and types for session commands, events, snapshots, and errors          | I/O, transport selection, state storage, PTY processes, or UI                                                       | Add when two trust-boundary participants consume the same wire contract                               |
| `@orchbee/session-client`   | `browser`   | Headless client synchronization, command dispatch, reconnection, and state         | React presentation, platform effects, transport implementation, or server process ownership                         | Add when a browser app consumes remote or cross-process session state                                 |
| `@orchbee/session-runtime`  | `node`      | Reusable PTY, process, and session lifecycle services                              | Electron menus or windows, browser UI, transport-specific entry points, or client state                             | Extract when Node session behavior has a concrete consumer outside its original application tree      |

The session package family is introduced by capability, not all at once. Broader orchestration, worker-node, or agent collaboration contracts receive their own named domains when their responsibilities no longer fit the session contract; do not broaden `session-*` into a catch-all.

Every added workspace must declare its exact responsibility and non-goals in `project.json` according to [`docs/PROJECTS.md`](../PROJECTS.md).

## UI ownership

`@orchbee/ui` contains only elements without Orchbee product semantics, including buttons, dialogs, menus, tooltips, and resizable layout primitives. It may own a reusable visual behavior, but it must not know what a session, agent, node, or terminal pane means.

All frontend workspaces follow the generated-component and styling ownership contract in [`docs/frontend/styling.md`](styling.md).

`@orchbee/design-tokens` owns the shared visual vocabulary independently of React and shadcn. `@orchbee/ui` consumes it only to map generated shadcn variables, while authored workbench components consume it directly through CSS Modules. The detailed tier and theme contract lives in [`docs/frontend/design-tokens.md`](design-tokens.md).

`@orchbee/workbench` owns product-semantic presentation, including the application shell, terminal panes, tab and split behavior, session and agent views, node status, settings views, command surfaces, and the UI that fulfills application-level terminal requests.

Keep a React wrapper around `@orchbee/terminal` inside the workbench terminal module until it has an independent consumer and API. Do not add React, Zustand, shadcn/ui, Base UI, TanStack Router, Electron, or Node dependencies to `@orchbee/terminal`.

## Workbench interaction model

Orchbee is terminal-first. The default Workbench surface gives focus and the remaining window area to the active terminal; it must not inherit an IDE workbench or reserve the primary surface for chat. A full-height left sidebar owns management navigation. Its eventual contents may include projects, worktrees, agents, or sessions, but the shell must not couple its layout contract to one of those models before that model has a concrete capability boundary.

Keep secondary capabilities discoverable without making them permanent chrome. Settings use a dedicated, searchable surface entered from the account or settings area and organize related controls into named categories rather than one growing modal. Browser, changes, and similar inspection tools belong in an auxiliary right panel that is hidden by default, opened from a compact top-right control, and independently closeable. Splits and destructive terminal actions belong in a compact actions menu. Status belongs near the entity it describes: session state in session navigation, runtime and connection state in the sidebar footer, and transient feedback in notifications.

## Product concepts

A terminal session is an authoritative running process with PTY input, output, identity, and lifecycle. A workbench layout is the client-side arrangement used to observe sessions, including tabs, splits, panes, focus, and panel dimensions. An agent or orchestration task is a collaboration entity whose identity and lifecycle must not be inferred from a terminal session even when one agent currently runs in one session.

Keep workbench layout state local to `@orchbee/workbench` until a concrete cross-device product requirement introduces a separately named synchronization contract. Do not put layout, agent collaboration, worker-node scheduling, or unrelated application state into `@orchbee/session-protocol`; introduce a narrow owning domain when those responsibilities cross a trust boundary.

## Workbench source modules

Organize workbench source by product ownership rather than technical artifact type:

```text
packages/workbench/src/
  agents/
  commands/
  composition/
  nodes/
  sessions/
  settings/
  shell/
  terminals/
  index.ts
```

Each directory is a source module with an `index.ts` boundary. Put cross-feature assembly in the leaf `composition` module, which consumes sibling entry points and is then promoted through the root `index.ts`. Do not put an `App.tsx` in the root module that imports private descendant files. This structure follows [`docs/MODULE-BOUNDARIES.md`](../MODULE-BOUNDARIES.md#composition-and-cycles).

Keep feature-specific components, hooks, state, types, and utilities inside the feature that owns them. Promote a capability through the feature's `index.ts` only when another visible module needs it.

## Platform integration

Define each platform port in the feature that consumes the capability. Prefer narrow contracts such as external navigation, directory selection, native menus, and notifications over one growing host interface. The workbench composition module may aggregate those contracts for injection without making every feature depend on the aggregate. `apps/desktop` implements the capabilities through Electron preload and IPC; `apps/web` implements them with browser or web-service APIs.

`@orchbee/session-client` defines the transport behavior it consumes. Applications provide an Electron IPC, WebSocket, or other transport adapter at composition time. A package must not select a transport by detecting its host environment.

Do not create a generic `bridge` or `platform` package for these adapters. Shared untrusted message shapes belong in the narrow protocol package, while runtime-specific effects remain in the application that owns that runtime.

## State ownership

| State                                                          | Owner                                                    |
| -------------------------------------------------------------- | -------------------------------------------------------- |
| Terminal buffer, selection, terminal search, and render state  | `@orchbee/terminal`                                      |
| PTY process and authoritative session lifecycle                | `@orchbee/session-runtime`                               |
| Client projection, event ordering, reconnect, and sync state   | `@orchbee/session-client`                                |
| Active tab, panel dimensions, focus, and command palette state | `@orchbee/workbench`                                     |
| Electron or browser capability state                           | The corresponding `apps/*`                               |
| Persistence of workbench layout or preferences                 | The consuming application through an injected capability |
| Raw cross-process or network messages                          | Parse with session schemas at the receiving boundary     |

Use Zustand for shared client UI state inside the owning browser package. Do not create workspace packages named `store`, `types`, `hooks`, `utils`, `common`, `shared`, or `helpers`; ownership is determined by the domain that gives the code meaning, not by its TypeScript shape.

## Introduction order

1. Create the consuming application together with `@orchbee/ui` and, when the product interface is shared through a library boundary, `@orchbee/workbench`.
2. Build the interface against an in-memory session adapter while product interaction contracts are still changing.
3. Add `@orchbee/session-protocol` and `@orchbee/session-client` when real cross-process or remote synchronization is connected.
4. Keep Node session logic in its owning application until another runtime entry has a concrete need for the same service, then extract `@orchbee/session-runtime`.

When a responsibility moves, update both affected `project.json` contracts, public exports, package READMEs, and this document in the same change. Run the package checks required by [`docs/PACKAGES.md`](../PACKAGES.md#verification-and-review) after introducing or changing a package boundary.
