# Navigation Routing

## Scope

`@orchbee/web` owns the route tree and the synchronization between browser history and one
client's Workbench Client View. Reusable Workbench packages remain router-independent. Desktop,
Browser, and future shared-Web deployments use the same paths beneath different origins.

| Runtime              | Origin example           | Route owner    |
| -------------------- | ------------------------ | -------------- |
| Browser development  | `http://127.0.0.1:3000`  | `@orchbee/web` |
| Packaged Electron    | `orchbee://app`          | `@orchbee/web` |
| Shared Web, reserved | `https://192.0.2.1:8443` | `@orchbee/web` |

LAN serving, token exchange, Cookie authentication, a Settings screen, a login screen, and
operating-system delivery of external deep links are outside this contract. They must extend the
route tree through a separately accepted change before becoming current behavior.

## Route tree

| Path                                                       | Resolution                                                         |
| ---------------------------------------------------------- | ------------------------------------------------------------------ |
| `/`                                                        | Current canonical route from the reconciled Client View            |
| `/terminal`                                                | First standalone Tab, or the empty standalone terminal context     |
| `/terminal/tabs`                                           | First standalone Tab, or `/terminal` when none exists              |
| `/terminal/tabs/$tabId`                                    | Standalone terminal context with `$tabId` active                   |
| `/projects`                                                | First Project with a Workspace, then that Workspace's first Tab    |
| `/projects/$projectId`                                     | The Project's first Workspace, then that Workspace's first Tab     |
| `/projects/$projectId/workspaces`                          | The Project's first Workspace, then that Workspace's first Tab     |
| `/projects/$projectId/workspaces/$workspaceId`             | The Workspace's first Tab, or the empty Workspace terminal context |
| `/projects/$projectId/workspaces/$workspaceId/tabs`        | The Workspace's first Tab, or its empty Workspace route            |
| `/projects/$projectId/workspaces/$workspaceId/tabs/$tabId` | Named Project and Workspace with `$tabId` active                   |

An incomplete route is a navigation intent, not a second displayed state. Resolution follows
authoritative Project, Workspace, and Tab order, then replaces the current history entry with the
deepest available canonical route. `/projects` uses the first Project that currently has a
Workspace; when no such Project exists it resolves through the standalone terminal context. An
existing Project with no Workspace remains unavailable until a Project-owned screen or empty-state
contract is separately accepted.

Omitting an identifier is different from naming an unknown identifier. A structurally incomplete
route chooses its first available descendant. A route containing an unknown `$projectId`,
`$workspaceId`, or `$tabId` displays the unavailable state and never falls through to a different
entity.

Route parameters carry stable model identifiers, not mutable display names. Route generation
percent-encodes each identifier as one path segment and route resolution decodes it exactly once.
Query parameters and fragments never select a Project, Workspace, or Tab.

`/settings` and `/login` are not current routes. Adding either path requires its owning screen,
localized copy, availability behavior, and verification in the same authorized change.

## State model

The route is authoritative for the displayed location and active Tab. The Workbench Client View
remains authoritative for non-routed local state such as expanded Projects, multi-selected Tabs,
panel focus, split weights, and maximization. The shared Workbench document remains authoritative
for Project, Workspace, Tab, Panel, and terminal membership and is never mutated merely by history
navigation.

| Input                                    | Transition                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------------ |
| Initial `/`                              | Replace with the canonical route derived from the reconciled Client View             |
| Initial structurally incomplete route    | Resolve ordered first descendants and replace with their deepest canonical route     |
| Initial valid canonical route            | Apply its location and active Tab to the Client View before displaying the Workbench |
| Initial route naming a missing entity    | Display the localized route-unavailable state without changing the Client View       |
| In-app location or active-Tab navigation | Push the target route; route reconciliation then applies the target Client View      |
| Browser Back or Forward                  | Apply the observed route to the Client View without creating another history entry   |
| Current entity removed after display     | Replace with the canonical route derived from the reconciled Client View             |
| Non-navigation Client View change        | Preserve the current route and history entry                                         |

The coordinator serializes observation through a re-entrancy guard. A route application records
its target before synchronously mutating the Client View, and the resulting Client View
notification acknowledges that target instead of producing a second navigation. A later Client
View notification that no longer resolves the displayed route is a domain reconciliation and
replaces, rather than pushes, the canonical fallback.

## Resolution invariants

- A canonical Tab route resolves to exactly one terminal context and one active Tab.
- An empty-context canonical route resolves to exactly one terminal context and no active Tab.
- A Workspace route is valid only when the Project owns the Workspace and that Workspace owns the
  terminal context containing the Tab.
- A standalone route is valid only when the standalone terminal context contains the Tab.
- Partial-route resolution uses document order and never depends on mutable display names, locale,
  object enumeration, or asynchronous arrival order.
- Applying the current route twice is idempotent.
- A pushed in-app navigation creates one history entry; canonicalization and domain reconciliation
  replace the current entry.
- Back and Forward never append history and never publish a shared-document command.
- A direct unavailable route never silently opens a different Project, Workspace, or Tab.
- Runtime detection may choose a transport adapter, but it never changes route semantics.

## Runtime detection

The packaged `orchbee:` origin and an Electron user-agent token select the Desktop adapter. The
user-agent check is only adapter discovery for loopback development; it is not an authorization
boundary. Desktop capabilities still arrive exclusively through the validated sandboxed bootstrap
ports. The existing explicit Gateway development marker remains limited to the local Gateway
development path until shared-Web serving and authentication replace it.

## Ownership boundaries

| Owner                | Responsibility                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| `@orchbee/web`       | Route declarations, history, route resolution, unavailable state, and Workbench navigation port |
| `@orchbee/workbench` | Router-neutral navigation intent port and Client View transitions                               |
| `@orchbee/tab-strip` | Router-neutral Tab-selection intent port and multi-selection semantics                          |
| `@orchbee/desktop`   | `orchbee://app` resource delivery and the initial renderer URL                                  |

No reusable package imports TanStack Router, reads `window.location`, inspects the user agent, or
constructs an application URL.

## Correctness evidence

Deterministic tests cover route encoding, valid and unavailable resolution, root canonicalization,
route-to-Client-View application, Client-View fallback, push-versus-replace history intent, and
the re-entrancy guard. Integration tests cover in-app navigation and Back/Forward against the same
Workbench runtime. Desktop tests cover scheme registration order, origin and method validation,
path-containment checks, asset delivery, and navigation fallback to `index.html`.

The packaging command builds the Web renderer before Desktop, packages the renderer as a read-only
resource, and places the executable Terminal Host outside `app.asar`. Release verification must
launch the unpacked application against `orchbee://app` and reject a candidate if the custom origin
cannot load a nested canonical route. Reverting the release restores the previous renderer entry
without migrating shared documents because routing changes Client View only.
