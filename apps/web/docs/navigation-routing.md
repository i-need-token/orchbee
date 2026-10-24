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

| Path                                   | Resolution                                                           |
| -------------------------------------- | -------------------------------------------------------------------- |
| `/`                                    | Current canonical route from reconciled Client View                  |
| `/terminal`                            | First standalone Tab or empty standalone context                     |
| `/terminal/tabs`                       | First standalone Tab or `/terminal`                                  |
| `/terminal/tabs/$tabId`                | Exact standalone Tab                                                 |
| `/workspaces/$workspaceId`             | First Tab or empty Workspace context                                 |
| `/workspaces/$workspaceId/tabs`        | First Tab or empty Workspace route                                   |
| `/workspaces/$workspaceId/tabs/$tabId` | Exact Workspace and Tab, independent of grouping                     |
| `/projects`                            | First globally ordered Workspace, including ungrouped, or standalone |
| `/projects/$projectId`                 | First current member Workspace and its first Tab                     |
| `/projects/$projectId/workspaces`      | Same current-member resolution                                       |

Workspace links use stable Workspace/Tab identities. Old Project-qualified Workspace paths are outside the route tree. Group-only links still require the current Project. Missing explicit Workspace or Tab identity remains unavailable at the entered address and never selects a different entity. An empty group has no terminal route.

Incomplete routes resolve by authoritative order, then replace history with the deepest available canonical route. Parameters are stable IDs, encoded as one segment and decoded once. Query strings and fragments do not select objects. `/settings` and `/login` remain outside the current route tree.

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

The navigation port exposes a read-only `isActive` predicate for the currently accepted exact target. It excludes pending navigation to another target, unavailable routes and disposal. When the existing activation Action supplies a Project creation receipt, the coordinator retains at most one exact route acceptance owned by that Action. History dispatch commits the request but leaves its completion pending until `prepare` accepts that exact location and the full receipt still matches. Other navigation, a different or unavailable prepared route, abort, disposal, or a 10-second deadline settles a bounded blocked outcome and releases the timer and abort listener. A late prepare cannot settle a retired or replaced acceptance. No navigation is polled or replayed. Desktop New Folder then rechecks canonical and current receipt plus the accepted current route before success ACK. Source cases cover delayed prepare, retirement, replacement, timeout and target removal; ordinary navigation keeps its existing dispatch semantics.

### First-open empty terminal contexts

- **Problem and scope:** During one application composition, the first ready visit to each distinct
  standalone or Workspace terminal context may find no Tabs. The route coordinator makes one
  terminal-creation decision per authoritative terminal-context identity. This covers a Workspace
  opened after another context was already displayed. Empty-state rendering, ordinary new-Tab
  commands, persisted restart intent, and native-window Tab assignment keep their existing owners.
- **State and transition:** `TerminalContextInitializationState` is the serializable set of context
  identities whose first ready visit has already been decided. Redirects and unavailable routes do
  not add an identity. A ready terminal route already in the set is an intentional no-op. A first
  ready route adds its identity and emits `create-terminal` exactly when that complete authoritative
  context has no Tabs; a populated context is added without a command.
- **Commit and failure:** The coordinator applies the ready route first so Workspace identity and
  default CWD are authoritative, then executes the existing runtime creation path with the current
  localized terminal title and `programmatic` source. Successful creation re-resolves the same
  request so the new Tab becomes the canonical route before rendering. Rejected creation leaves that
  canonical empty route blank and is not retried for the same context during the coordinator
  lifetime; another context remains independently eligible.
- **Concurrency and boundaries:** The decision reads the complete active terminal context rather than
  page-local render state, so presentation state cannot trigger creation. Each
  independently composed page owns its own decided-identity state and may race another page; the
  existing revisioned shared-document coordinator remains the only commit authority and converges
  competing proposals without adding a startup flag, schema field, storage key, or protocol message.
- **Verification:** Pure transition tests cover redirects, unavailable and ready routes, multiple
  context identities, empty and populated contexts, repetition, and re-entry. Coordinator tests
  cover standalone and multiple Workspace creations, current localized title input, canonical
  re-resolution, per-context failure, invalid routes, and closing a decided context's last Tab
  without replacement. Existing multi-client coordinator evidence covers proposal arbitration; no
  new TLA+ model is selected because the local transition is synchronous and has no independently
  ordered completion.

## Renderer reload focus recovery

- **Problem and inputs:** Reloading the Electron renderer replaces its in-memory Client View while
  the native window, canonical shared document, and Terminal Host sessions survive. Without a
  client-local handoff, the active Tab falls back to its first Panel and later keyboard input can
  reach a different terminal. Trusted inputs are the reconciled shared document and Client View;
  the optional `sessionStorage` value is untrusted and strict-parsed at the Web composition
  boundary. Missing, malformed, unavailable, or stale input means no recovery hint.
- **Data and ownership:** The Desktop Web composition owns one version-1
  `orchbee:web:terminal-focus-recovery:v1` record containing exactly one bounded
  `TerminalPresentationId`. Workbench remains the sole Client View and logical-focus writer, the
  route remains authoritative for location and active Tab, and Terminal Host remains authoritative
  for the process and terminal. The record is disposable with the Electron page-session lifetime
  and is neither shared nor durable application state.
- **Outputs and model:** A deterministic projector starts from the default Client View and, when
  the exact presentation still resolves, selects its stable Workspace (or standalone) and exact Tab, then restores that Tab's focused Panel and displayed child
  presentation. Subsequent explicit route navigation retains authority. Workbench's existing `autoFocus` projection
  moves real DOM focus only after the recovered terminal renderer is ready. A missing target is an
  intentional no-op that retains the default Client View.
- **Transitions:** Every committed shared-document or Client View notification resolves its one
  displayed terminal and replaces the session record; a view with no displayed terminal removes
  it. Renderer bootstrap reads once and supplies the projected candidate to Workbench before route
  reconciliation. Storage read, write, removal, validation, or quota failure never blocks startup
  or changes the current in-memory focus.
- **Invariants and exclusions:** At most one bounded presentation identity is stored per browsing
  context. Independent pages cannot share the hint. Recovery cannot create an identity, revive a
  removed presentation, change shared state, send terminal input, or restore maximization, Split
  weights, multi-selection, scroll, search, output, or unrelated Client View or viewport facts. Full Client
  View recovery and recovery across native-window or application restart remain excluded.
- **Verification:** Deterministic tests cover primary and child targets, stale identity, strict
  parsing, absence, storage failure, removal, and projection isolation. The Desktop system trace
  focuses a non-first split terminal, reloads the renderer, and sends input through the naturally
  restored DOM focus to that same presentation. TLA+ is not selected because the record has one
  synchronous writer, no asynchronous completion, and no ordering or cancellation state.

## Resolution invariants

- A canonical Tab route resolves to exactly one terminal context and one active Tab.
- An empty-context canonical route resolves to exactly one terminal context and no active Tab.
- A Workspace route is valid when the stable Workspace owns the terminal context containing the Tab; group membership does not participate.
- A standalone route is valid only when the standalone terminal context contains the Tab.
- Partial-route resolution uses document order and never depends on mutable display names, locale,
  object enumeration, or asynchronous arrival order.
- Applying the current route twice is idempotent.
- First-open empty-context creation is attempted at most once for each terminal-context identity per
  route-coordinator lifetime, and closing its last Tab after that decision never triggers another
  attempt.
- A pushed in-app navigation creates one history entry; canonicalization and domain reconciliation
  replace the current entry.
- Back and Forward never append history and never publish a shared-document command.
- A direct unavailable route never silently opens a different Project, Workspace, or Tab.
- Runtime detection may choose a transport adapter, but it never changes route semantics.

## Runtime detection

The packaged `orchbee:` origin and an Electron user-agent token select the Desktop adapter. The
user-agent check is only adapter discovery for loopback development; it is not an authorization
boundary. Desktop capabilities still arrive exclusively through the validated sandboxed bootstrap
ports. Unsupported explicit runtime markers are rejected.

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
per-context first-open empty-context transitions, route-to-Client-View application, Client-View
fallback, push-versus-replace history intent, and the re-entrancy guard. Integration tests cover
first-open terminal creation, post-close blank retention, in-app navigation, and Back/Forward against
the same Workbench runtime. Desktop tests cover scheme registration order, origin and method
validation, path-containment checks, asset delivery, and navigation fallback to `index.html`.

The packaging command builds the Web renderer before Desktop, packages the renderer as a read-only
resource, and places the executable Terminal Host outside `app.asar`. Release verification must
launch the unpacked application against `orchbee://app` and reject a candidate if the custom origin
cannot load a nested canonical route. Reverting the release restores the previous renderer entry
without migrating shared documents because routing changes Client View only.
