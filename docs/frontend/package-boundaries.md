# Frontend Package Boundaries

## Workspace placement

Use `apps/*` for independently runnable frontend entry points and `packages/*` for stable
capabilities consumed through a library API. Do not mirror source categories such as components,
Hooks, stores, types, or utilities as workspace packages.

Keep application-specific presentation and behavior in the owning application until another
workspace has a concrete need for the same capability. Extract a package together with its first
real consumer; a possible future consumer or architecture diagram is not an admission reason.

Applications own runtime startup, routing roots, authentication entry points, deployment and
packaging configuration, and platform composition. Packages never import from an application.

The repository-wide visual catalog remains the development application `apps/storybook`. Story
ownership follows [`storybook.md`](storybook.md) and does not change runtime package ownership.

## Capability admission

A frontend package owns one named capability with an independently reviewable contract. It has a
concrete consumer, can be tested without an application source import, and does not rely on a
speculative sibling package.

- Name the capability that callers consume rather than its current framework, transport, or source
  category.
- Declare the package's exact responsibility, non-goals, runtime, and stability in `project.json`
  according to [`docs/PROJECTS.md`](../PROJECTS.md).
- Treat the distributed `project.json` files as the current workspace registry. Do not maintain a
  package catalog or future topology in this standard.
- Put proposed packages, dependency changes, and migration order under `proposals/` until they are
  implemented.
- Follow package naming, exports, builds, dependency, and verification rules in
  [`docs/PACKAGES.md`](../PACKAGES.md).

Do not create workspace packages named after technical buckets such as `components`, `hooks`,
`store`, `types`, `utils`, `common`, `shared`, or `helpers`. Ownership comes from the capability
that gives the code meaning, not its TypeScript shape.

## UI ownership

A generic UI primitive encapsulates framework-level accessibility and visual interaction without
knowing the product entity or workflow that consumes it. A product-semantic component understands
an application concept, prepared business state, or semantic action and therefore stays with the
product capability that owns that meaning.

- Keep a product-semantic component in its owning application while it has one application owner.
- Extract shared product presentation only when multiple applications consume the same semantic
  contract; name the package for that product capability rather than for UI in general.
- Move a component together with its complete ownership tree: Props, scoped state, Hooks, private
  descendants, styles, assets, and tests. Do not export an internal implementation fragment.
- Keep reusable primitives free of product messages, domain state, routing, remote data, and
  platform effects. Consumers provide product-visible and accessibility-only text.
- Keep framework-independent engines free of React, application state, product dialogs, and host
  effects. A React wrapper remains with the semantic consumer until it has its own independent API
  and consumer.

Recursive component ownership and component-directory structure follow [`react.md`](react.md).
Styling ownership follows [`styling.md`](styling.md), and product-message ownership follows
[`localization.md`](localization.md).

## Dependency direction

Applications consume packages only through declared package exports. A package consumes another
package only through that package's public capability; it never imports another workspace's source
or build tree.

Keep dependency direction aligned with semantic authority:

- Product presentation may depend on generic UI, localization, design vocabulary, and headless
  capabilities.
- Generic UI and design vocabulary never depend on product presentation.
- A headless capability never depends on the React presentation that renders it.
- A protocol or validation boundary never depends on transport, persistence, process lifecycle, or
  UI.
- Runtime-specific implementations remain in the application or runtime package that owns their
  lifecycle.

When two packages would need each other's implementation, move the shared contract to the narrowest
independently meaningful capability or invert one dependency behind an owned port. Do not create a
catch-all package to hide a cycle.

## Platform integration

Define a platform port in the capability that consumes it and let the composition application
provide the implementation. Prefer narrow contracts such as navigation, file selection,
notifications, persistence, or transport over one growing host interface.

A package does not select an implementation by detecting its host environment. Browser, desktop,
server, and test adapters remain with the runtime that owns their effects. Shared untrusted message
shapes belong at a narrow validation or protocol boundary, while transport-specific behavior stays
with the transport owner.

Do not create generic `bridge` or `platform` packages merely to collect unrelated adapters. Extract
an adapter package only when multiple concrete runtimes consume the same independently named
integration contract.

## State ownership

| State or behavior                                    | Owner                                                                   |
| ---------------------------------------------------- | ----------------------------------------------------------------------- |
| Local visual and interaction state                   | The narrowest mounted component subtree that shares it                  |
| Product presentation state and semantic UI actions   | The owning product application or product-capability package            |
| Headless capability state and invariants             | The package that owns that capability                                   |
| Authoritative process, server, or resource lifecycle | The runtime or service that creates and disposes it                     |
| User preference and host-environment selection       | The composing application                                               |
| Product message descriptors and shipped translations | The repository localization capability                                  |
| Raw process, storage, IPC, or network data           | Parse at the first trusted boundary before assigning internal ownership |

Use Zustand for shared client UI state inside the browser workspace that owns the state. Scope each
Store to the narrowest mounted ownership tree that needs it; package extraction is not a substitute
for correct Store scope.

## Boundary changes

When responsibility moves between workspaces, update the affected `project.json` contracts, public
exports, package READMEs, package-local references, tests, and consumers in the same change. Remove
the old path and authority rather than documenting both as valid.

Do not record an implementation sequence or uncreated workspace as current architecture. Put that
work under `proposals/`, then update this standard only if the completed change introduces a reusable
cross-workspace rule.

Run the package checks required by
[`docs/PACKAGES.md`](../PACKAGES.md#verification-and-review) after introducing or changing a package
boundary.
