# Frontend Package Boundaries

## Workspace placement

| Concern                | Rule                                                                                                                                                  |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Runnable frontend      | Place independently runnable entry points in `apps/*`; applications own startup, routing roots, authentication, deployment, and platform composition. |
| Shared capability      | Place a stable, independently testable library API with a concrete consumer in `packages/*`; packages never import applications.                      |
| Application-owned code | Keep presentation and behavior in its application until another workspace needs the same semantic capability.                                         |
| Extraction             | Move the capability with its first real consumer; a possible consumer or architecture diagram does not justify a package.                             |
| Storybook              | `apps/storybook` is the repository visual catalog; Story ownership follows [`storybook.md`](storybook.md) without changing runtime ownership.         |

Do not create packages for source categories such as components, Hooks, stores, types, or utilities.

## Capability admission

A frontend package must:

- own one named capability with a reviewable contract and concrete consumer;
- run its tests without importing application source or a speculative sibling package;
- declare responsibility, non-goals, runtime, and stability in `project.json` under [`docs/PROJECTS.md`](../PROJECTS.md);
- use distributed `project.json` files as the workspace registry instead of duplicating a package catalog here; and
- follow [`docs/PACKAGES.md`](../PACKAGES.md) for naming, exports, builds, dependencies, compatibility, and verification.

Name a package after the capability callers consume, never a framework, transport, or technical bucket. Forbidden bucket names include `components`, `hooks`, `store`, `types`, `utils`, `common`, `shared`, and `helpers`.

## UI ownership

| Unit                         | Owner and boundary                                                                                                                               |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Generic UI primitive         | `@orchbee/ui`; owns framework accessibility and visual interaction without product entities, messages, routing, remote data, or platform effects |
| Product-semantic component   | The application or product-capability package that owns its business concept, prepared state, and semantic actions                               |
| Shared product presentation  | A named product-capability package only after multiple applications consume the same semantic contract                                           |
| Framework-independent engine | A headless capability with no React, product dialog, application state, or host effect                                                           |
| Component ownership unit     | Root component, Props, scoped state, Hooks, private descendants, styles, assets, and tests                                                       |

- Move a complete ownership unit; never export an internal fragment while leaving its state, styles, or tests behind.
- Keep a React wrapper with its semantic consumer until the wrapper has an independent API and consumer.
- Generic primitives receive product-visible and accessibility text from consumers.
- Recursive component ownership follows [`react.md`](react.md), styling follows [`styling.md`](styling.md), and product messages follow [`localization.md`](localization.md).

## Dependency direction

Applications and packages consume other workspaces only through declared exports; they never import another workspace's `src/*` or `dist/*`.

| From                         | Allowed dependency direction                                           |
| ---------------------------- | ---------------------------------------------------------------------- |
| Product presentation         | Generic UI, localization, design vocabulary, and headless capabilities |
| Generic UI or design tokens  | Never product presentation                                             |
| Headless capability          | Never its React presentation                                           |
| Protocol or validation layer | Never transport, persistence, process lifecycle, or UI                 |
| Runtime implementation       | Remains in the application or runtime package that owns its lifecycle  |

Break a cycle by moving the shared contract to the narrowest independently meaningful capability or by inverting one dependency behind an owned port; never hide a cycle in a catch-all package.

## Platform integration

- Define a narrow platform port—such as navigation, file selection, notifications, persistence, or transport—in the consuming capability.
- Let the composition application provide browser, desktop, server, and test adapters.
- Parse untrusted messages at a protocol or validation boundary; keep transport behavior with the transport owner.
- Do not select an adapter through host detection inside a package.
- Do not create generic `bridge` or `platform` packages for unrelated adapters; extract one only when multiple runtimes consume the same named integration contract.

## State ownership

| State or behavior                                    | Owner                                                          |
| ---------------------------------------------------- | -------------------------------------------------------------- |
| Local visual and interaction state                   | The narrowest mounted component subtree that shares it         |
| Product presentation state and semantic UI actions   | The owning application or product-capability package           |
| Headless capability state and invariants             | The package that owns the capability                           |
| Authoritative process, server, or resource lifecycle | The runtime or service that creates and disposes it            |
| User preference and host selection                   | The composition application                                    |
| Product message descriptors and translations         | `@orchbee/i18n`                                                |
| Raw process, storage, IPC, or network data           | Parsed at the first trusted boundary before internal ownership |

Use Zustand only for shared client UI state inside the browser workspace that owns it; scope each Store to the narrowest mounted ownership tree.

## Boundary changes

Update affected `project.json` contracts, exports, READMEs, package references, tests, and consumers in the same change; remove the old path and authority. Do not describe an uncreated workspace or implementation sequence as current architecture.

Run the package checks in [`docs/PACKAGES.md`](../PACKAGES.md#verification-and-review) after changing a package boundary.
