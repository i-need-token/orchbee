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
                               +--> @orchbee/i18n <---------------- apps/web
                               |
                               +--> @orchbee/session-client --> @orchbee/session-protocol

apps/desktop --------------------> @orchbee/session-runtime --> @orchbee/session-protocol
apps/daemon ---------------------> @orchbee/session-runtime
```

Application workspaces own runtime startup, routing roots, authentication entry points, deployment or packaging configuration, and platform-specific adapters. Packages never import from an app.

## Package responsibilities

| Package                     | Runtime     | Responsibility                                                                                               | Non-goals                                                                                                           | Admission point                                                                                       |
| --------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `@orchbee/terminal`         | `browser`   | Terminal parsing, rendering, input, selection, search, and browser integration                               | React UI, application PTY processes, session transport, Electron effects, or product dialogs                        | Existing independently consumable library                                                             |
| `@orchbee/ui`               | `browser`   | Reusable shadcn components backed by Base UI                                                                 | Themes, design tokens, sessions, agents, terminal panes, routing, remote state, or platform APIs                    | Add with the first consuming product UI                                                               |
| `@orchbee/design-tokens`    | `universal` | Enumerable visual scales, semantic theme roles, and responsive thresholds                                    | React components, product interactions, application state, arbitrary component values, or effects                   | Existing consumer in the shadcn compatibility stylesheet                                              |
| `@orchbee/i18n`             | `browser`   | Orchbee locale contracts, React provider, catalog composition, and shipped product messages and translations | User locale preference, platform detection, preference persistence, or feature behavior                             | Existing consumers in `@orchbee/workbench` and `apps/web`                                             |
| `@orchbee/workbench`        | `browser`   | Shared product-semantic React interface, interaction state, and presentation model                           | Application startup, routing roots, authentication, persistence, platform imports, PTY ownership, or wire transport | Existing consumer in `apps/web`; keep private and experimental until its cross-app contract is proven |
| `@orchbee/session-protocol` | `universal` | Zod schemas and types for session commands, events, snapshots, and errors                                    | I/O, transport selection, state storage, PTY processes, or UI                                                       | Add when two trust-boundary participants consume the same wire contract                               |
| `@orchbee/session-client`   | `browser`   | Headless client synchronization, command dispatch, reconnection, and state                                   | React presentation, platform effects, transport implementation, or server process ownership                         | Add when a browser app consumes remote or cross-process session state                                 |
| `@orchbee/session-runtime`  | `node`      | Reusable PTY, process, and session lifecycle services                                                        | Electron menus or windows, browser UI, transport-specific entry points, or client state                             | Extract when Node session behavior has a concrete consumer outside its original application tree      |

The session package family is introduced by capability, not all at once. Broader orchestration, worker-node, or agent collaboration contracts receive their own named domains when their responsibilities no longer fit the session contract; do not broaden `session-*` into a catch-all.

Every added workspace must declare its exact responsibility and non-goals in `project.json` according to [`docs/PROJECTS.md`](../PROJECTS.md).

## UI ownership

`@orchbee/ui` contains only elements without Orchbee product semantics, including buttons, dialogs, menus, tooltips, and resizable layout primitives. It may own a reusable visual behavior, but it must not know what a session, agent, node, or terminal pane means.

All frontend workspaces follow the generated-component and styling ownership contract in [`docs/frontend/styling.md`](styling.md).

`@orchbee/design-tokens` owns the shared visual vocabulary independently of React and shadcn. `@orchbee/ui` consumes it only to map generated shadcn variables, while authored workbench components consume it directly through CSS Modules. The detailed tier and theme contract lives in [`docs/frontend/design-tokens.md`](design-tokens.md).

`@orchbee/workbench` owns product-semantic presentation, including the application shell, terminal panes, tab and split behavior, session and agent presentation, node status, settings surfaces, command surfaces, and the UI that fulfills application-level terminal requests.

Both packages expose reusable components together with their complete ownership trees rather than loose implementation files. A generic component and its state remain in `@orchbee/ui` only while they are independent of Orchbee product concepts; a product-semantic component and its complete ownership tree belong to `@orchbee/workbench`. The recursive ownership, Context-scoped Store, communication, and promotion contracts live in [`react.md`](react.md#recursive-component-ownership).

`@orchbee/i18n` owns the shared locale contract, React localization provider, non-React formatter, domain-namespaced product message descriptors, and shipped translations. Workbench and application workspaces consume these capabilities only through the package root; they do not own local catalogs or import the underlying formatting library. Application workspaces continue to own user locale selection, platform detection, and preference persistence, then supply the effective locale at composition time. `@orchbee/ui` remains locale-agnostic and exposes generic visible or accessibility-only text for its product-semantic consumer to localize. The complete contract lives in [`localization.md`](localization.md).

Keep a React wrapper around `@orchbee/terminal` inside the workbench terminal module until it has an independent consumer and API. Do not add React, Zustand, shadcn/ui, Base UI, TanStack Router, Electron, or Node dependencies to `@orchbee/terminal`.

## Workbench interaction model

Orchbee is terminal-first. The default Workbench surface gives focus and the remaining window area to the active terminal; it must not inherit an IDE workbench or reserve the primary surface for chat. A full-height left sidebar reserves space for management navigation. Keep it empty until projects, worktrees, agents, or another model has a concrete capability boundary; the shell must not couple its layout contract to a speculative model.

Keep secondary capabilities discoverable without making them permanent chrome. Settings use a dedicated, searchable surface entered from the account or settings area and organize related controls into named categories rather than one growing modal. Browser, changes, and similar inspection tools belong in an auxiliary right panel that is hidden by default, opened from a compact top-right control, and independently closeable. Splits and destructive terminal actions belong in a compact actions menu. Status belongs near the entity it describes: session state in a concrete session surface, runtime and connection state in the management model that owns them, and transient feedback in notifications. Do not add placeholder status chrome to the reserved sidebar.

## Product concepts

A terminal session is an authoritative running process with PTY input, output, identity, and lifecycle. A workbench layout is the client-side arrangement used to observe sessions, including tabs, splits, panes, focus, and panel dimensions. An agent or orchestration task is a collaboration entity whose identity and lifecycle must not be inferred from a terminal session even when one agent currently runs in one session.

Keep workbench layout state local to `@orchbee/workbench` until a concrete cross-device product requirement introduces a separately named synchronization contract. Do not put layout, agent collaboration, worker-node scheduling, or unrelated application state into `@orchbee/session-protocol`; introduce a narrow owning domain when those responsibilities cross a trust boundary.

## Workbench source modules

Treat `@orchbee/workbench` as the package boundary for product-semantic components. Give each reusable public product component its own source-module directory, and organize its private descendants recursively according to [`react.md`](react.md#component-directory-boundary):

```text
packages/workbench/src/
  index.ts
  workbench/
    index.ts
    Workbench.tsx
    Workbench.module.css
    contexts/
      workbench-store.ts
    components/
  terminal-pane/
    index.ts
    TerminalPane.tsx
    TerminalPane.module.css
```

The example shows directory shape, not a required component inventory. Add a public component directory only with its concrete interface capability; do not pre-create speculative agent, node, settings, or other components. The package root `index.ts` deliberately promotes the public product components consumed outside the package.

Within a public component directory, keep only `index.ts`, the PascalCase root component `.tsx`, and an optional same-named `.module.css` as direct files. Define and export the root component's Props type in its `.tsx`. Put a Context-scoped Store and its selector Hook together in `contexts/`; reserve `hooks/` for component behavior and `types/` for internal data structures shared by multiple files in the owned tree. Repeat the structure recursively for private child components. Put unit tests in the direct child `__tests__/` support directory; do not create `styles` or `stories` directories, place test files directly beside the component, or create empty support directories.

When another source module needs a private component, promote it with its complete ownership tree into its own public component directory and consume its `index.ts`. A public Workbench component may compose sibling public components through their source-module boundaries, but it never imports another component's private Store, Hook, support directory, or descendant. This structure follows [`docs/MODULE-BOUNDARIES.md`](../MODULE-BOUNDARIES.md#source-module-tree).

## Platform integration

Define each platform port in the product component that consumes the capability. Prefer narrow contracts such as external navigation, directory selection, native menus, and notifications over one growing host interface. The public `workbench` root component may aggregate those contracts for injection without making every descendant depend on the aggregate. `apps/desktop` implements the capabilities through Electron preload and IPC; `apps/web` implements them with browser or web-service APIs.

`@orchbee/session-client` defines the transport behavior it consumes. Applications provide an Electron IPC, WebSocket, or other transport adapter at composition time. A package must not select a transport by detecting its host environment.

Do not create a generic `bridge` or `platform` package for these adapters. Shared untrusted message shapes belong in the narrow protocol package, while runtime-specific effects remain in the application that owns that runtime.

## State ownership

| State                                                          | Owner                                                    |
| -------------------------------------------------------------- | -------------------------------------------------------- |
| Terminal buffer, selection, terminal search, and render state  | `@orchbee/terminal`                                      |
| PTY process and authoritative session lifecycle                | `@orchbee/session-runtime`                               |
| Client projection, event ordering, reconnect, and sync state   | `@orchbee/session-client`                                |
| Active tab, panel dimensions, focus, and command palette state | `@orchbee/workbench`                                     |
| Product message descriptors and shipped translations           | `@orchbee/i18n`                                          |
| User locale preference and platform locale detection           | The corresponding `apps/*`                               |
| Electron or browser capability state                           | The corresponding `apps/*`                               |
| Persistence of workbench layout or preferences                 | The consuming application through an injected capability |
| Raw cross-process or network messages                          | Parse with session schemas at the receiving boundary     |

Use Zustand for shared client UI state inside the owning browser package. Do not create workspace packages named `store`, `types`, `hooks`, `utils`, `common`, `shared`, or `helpers`; ownership is determined by the domain that gives the code meaning, not by its TypeScript shape.

## Introduction order

1. Create the consuming application together with `@orchbee/ui` and, when the product interface is shared through a library boundary, `@orchbee/workbench`.
2. Keep every Workbench and application product message in `@orchbee/i18n`, with descriptors separated by product-domain namespace and composed into one catalog per shipped locale.
3. Build the interface against an in-memory session adapter while product interaction contracts are still changing.
4. Add `@orchbee/session-protocol` and `@orchbee/session-client` when real cross-process or remote synchronization is connected.
5. Keep Node session logic in its owning application until another runtime entry has a concrete need for the same service, then extract `@orchbee/session-runtime`.

When a responsibility moves, update both affected `project.json` contracts, public exports, package READMEs, and this document in the same change. Run the package checks required by [`docs/PACKAGES.md`](../PACKAGES.md#verification-and-review) after introducing or changing a package boundary.
