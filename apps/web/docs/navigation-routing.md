# Navigation Routing

## Scope

`@orchbee/web` owns the route tree and the synchronization between router history and one
client's Workbench Client View. Reusable Workbench packages remain router-independent. Every origin
uses the same paths. A Desktop window has no address bar, so its router keeps memory history: a
reload starts at `/`, and [renderer reload focus recovery](#renderer-reload-focus-recovery) returns
it to where it was.

| Runtime              | Origin example           | Route owner    |
| -------------------- | ------------------------ | -------------- |
| Development renderer | `http://127.0.0.1:3000`  | `@orchbee/web` |
| Packaged Electron    | `orchbee://app`          | `@orchbee/web` |
| Shared Web, reserved | `https://192.0.2.1:8443` | `@orchbee/web` |

LAN serving, token exchange, Cookie authentication, a Settings screen, a login screen, and
operating-system delivery of external deep links are outside this contract. They must extend the
route tree through a separately accepted change before becoming current behavior.

## Route tree

| Path                                   | Resolution                                                                                |
| -------------------------------------- | ----------------------------------------------------------------------------------------- |
| `/`                                    | No Workspace while none exists; otherwise the canonical route from reconciled Client View |
| `/workspaces/$workspaceId`             | First Tab or empty Workspace context                                                      |
| `/workspaces/$workspaceId/tabs`        | First Tab or empty Workspace route                                                        |
| `/workspaces/$workspaceId/tabs/$tabId` | Exact Workspace and Tab, independent of grouping                                          |
| `/projects`                            | First globally ordered Workspace, including ungrouped, or `/`                             |
| `/projects/$projectId`                 | First current member Workspace and its first Tab                                          |
| `/projects/$projectId/workspaces`      | Same current-member resolution                                                            |

As in the TUI, No Workspace is a location only while no Workspace exists, and `/` is its only path. Any path outside the tree shows the localized route-unavailable state. Workspace links use stable Workspace/Tab identities. Old Project-qualified Workspace paths are outside the route tree. Group-only links still require the current Project. Missing explicit Workspace or Tab identity remains unavailable at the entered address and never selects a different entity. An empty group has no terminal route.

Incomplete routes resolve by authoritative order, then replace history with the deepest available canonical route. Parameters are stable IDs, encoded as one segment and decoded once. Query strings and fragments do not select objects. `/settings` and `/login` remain outside the current route tree.

## State model

The route is authoritative for the displayed location and active Tab. The Workbench Client View
remains authoritative for non-routed local state such as expanded Projects, multi-selected Tabs,
panel focus, and maximization. The Server's publication remains authoritative for Project,
Workspace, Tab, Panel, Split weight, and terminal membership, and history navigation never changes
it.

| Input                                    | Transition                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------------------------ |
| Initial `/`                              | Replace with the canonical route derived from the reconciled Client View             |
| Initial structurally incomplete route    | Resolve ordered first descendants and replace with their deepest canonical route     |
| Initial valid canonical route            | Apply its location and active Tab to the Client View before displaying the Workbench |
| Initial route naming a missing entity    | Display the localized route-unavailable state without changing the Client View       |
| In-app location or active-Tab navigation | Push the target route; route reconciliation then applies the target Client View      |
| Back or Forward                          | Apply the observed route to the Client View without creating another history entry   |
| Current entity removed after display     | Replace with the canonical route derived from the reconciled Client View             |
| Non-navigation Client View change        | Preserve the current route and history entry                                         |

The coordinator serializes observation through a re-entrancy guard. A route application records
its target before synchronously mutating the Client View, and the resulting Client View
notification acknowledges that target instead of producing a second navigation. A later Client
View notification that no longer resolves the displayed route is a domain reconciliation and
replaces, rather than pushes, the canonical fallback.

The navigation port exposes a read-only `isActive` predicate for the currently accepted exact target. It excludes pending navigation to another target, unavailable routes and disposal. When the existing activation Action supplies a Project creation receipt, the coordinator retains at most one exact route acceptance owned by that Action. History dispatch commits the request but leaves its completion pending until `prepare` accepts that exact location and the full receipt still matches. Other navigation, a different or unavailable prepared route, abort, disposal, or a 10-second deadline settles a bounded blocked outcome and releases the timer and abort listener. A late prepare cannot settle a retired or replaced acceptance. No navigation is polled or replayed. Desktop New Folder then rechecks canonical and current receipt plus the accepted current route before success ACK. Source cases cover delayed prepare, retirement, replacement, timeout and target removal; ordinary navigation keeps its existing dispatch semantics.

## Renderer reload focus recovery

- **Problem and inputs:** Reloading the Electron renderer replaces its in-memory Client View while
  the native window and the Server's Session survive. Without a
  client-local handoff, the active Tab falls back to its first Panel and later keyboard input can
  reach a different terminal. Trusted inputs are the runtime's latest publication and reconciled Client View;
  the optional `sessionStorage` value is untrusted and strict-parsed at the Web composition
  boundary. Missing, malformed, unavailable, or stale input means no recovery hint.
- **Data and ownership:** The Desktop composition owns one version-1
  `orchbee:web:terminal-focus-recovery:v1` record containing exactly one bounded
  `TerminalPresentationId`. Workbench remains the sole Client View and logical-focus writer, the
  route remains authoritative for location and active Tab, and Terminal Host remains authoritative
  for the process and terminal. The record is disposable with the Electron page-session lifetime
  and is neither shared nor durable application state.
- **Outputs and model:** A deterministic projector starts from the default Client View and, when
  the exact presentation still resolves, selects its stable Workspace and exact Tab, then restores that Tab's focused Panel and displayed child
  presentation. Subsequent explicit route navigation retains authority. Workbench's existing `autoFocus` projection
  moves real DOM focus only after the recovered terminal renderer is ready. A missing target is an
  intentional no-op that retains the default Client View.
- **Transitions:** Every committed Client View or terminal-topology notification resolves its one
  displayed terminal and, when it changed, replaces the session record; a view with no displayed
  terminal removes it. Renderer bootstrap reads once and supplies the projected candidate to Workbench before route
  reconciliation. Storage read, write, removal, validation, or quota failure never blocks startup
  or changes the current in-memory focus.
- **Invariants and exclusions:** At most one bounded presentation identity is stored per browsing
  context. Independent pages cannot share the hint. Recovery cannot create an identity, revive a
  removed presentation, change shared state, send terminal input, or restore maximization, Split
  weights, multi-selection, scroll, search, output, or unrelated Client View or viewport facts. Full Client
  View recovery and recovery across native-window or application restart remain excluded.
- **Verification:** Deterministic tests cover primary and child targets, stale identity, strict
  parsing, absence, storage failure, removal, and projection isolation. The Desktop system trace that
  reloaded the renderer and sent input through the restored focus retired with the Desktop Electron
  suite. TLA+ is not selected because the record has one
  synchronous writer, no asynchronous completion, and no ordering or cancellation state.

## Resolution invariants

- A canonical Tab route resolves to exactly one terminal context and one active Tab.
- An empty-context canonical route resolves to exactly one terminal context and no active Tab.
- A Workspace route is valid when the stable Workspace owns the terminal context containing the Tab; group membership does not participate.
- The no-Workspace route is valid only while no Workspace exists.
- Partial-route resolution uses published order and never depends on mutable display names, locale,
  object enumeration, or asynchronous arrival order.
- Applying the current route twice is idempotent.
- A pushed in-app navigation creates one history entry; canonicalization and domain reconciliation
  replace the current entry.
- Back and Forward never append history and never send a Server command.
- A direct unavailable route never silently opens a different Project, Workspace, or Tab.
- Runtime detection may choose a transport adapter, but it never changes route semantics.

## Runtime detection

The packaged `orchbee:` origin and an Electron user-agent token select the Desktop adapter. The
user-agent check is only adapter discovery for loopback development; it is not an authorization
boundary. Desktop capabilities still arrive exclusively through the validated sandboxed bootstrap
ports. Unsupported explicit runtime markers are rejected, and any other page renders nothing.

## Ownership boundaries

| Owner                | Responsibility                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| `@orchbee/web`       | Route declarations, history, route resolution, unavailable state, and Workbench navigation port |
| `src/workbench`      | Router-neutral navigation intent port and Client View transitions                               |
| `@orchbee/tab-strip` | Router-neutral Tab-selection intent port and multi-selection semantics                          |
| `@orchbee/desktop`   | `orchbee://app` resource delivery and the initial renderer URL                                  |

No reusable package imports TanStack Router, reads `window.location`, inspects the user agent, or
constructs an application URL.

## Correctness evidence

Deterministic tests cover route encoding, valid and unavailable resolution, root canonicalization,
route-to-Client-View application, Client-View fallback, push-versus-replace history intent, and the
re-entrancy guard. Integration tests cover in-app navigation and Back/Forward against the same
Workbench runtime. Desktop tests cover scheme registration order, origin and method
validation, path-containment checks, asset delivery, and navigation fallback to `index.html`.

The packaging command builds the Web renderer before Desktop, packages the renderer as a read-only
resource, and places the executable Terminal Host outside `app.asar`. Release verification must
launch the unpacked application against `orchbee://app` and reject a candidate if the custom origin
cannot load a nested canonical route. Reverting the release restores the previous renderer entry
with no migration because routing changes only the Client View.
