# Desktop

Sandboxed Electron window that joins an Orchbee Session as a Server Client.

## Current scope

Desktop is a Server Client, as the TUI is, on the
[native runtime topology](../../proposals/platform/native-runtime-topology.md#web-and-desktop-client-migration-plan).
The Server owns the Session's Workbench and terminals, the Worker brokers the Terminal Host, and
`apps/web` renders the window's Workbench. Electron Main owns only what a page cannot do: the native
window, its menu and Actions, the native folder picker, and the `orchbee` processes that reach the
Server. The page has no Node integration, generic IPC, filesystem capability, or Host access, and
Main never reads terminal content.

Main keeps one instance; starting Orchbee again brings the window forward. On macOS the application
stays open without a window, and activating it opens one. A renderer that crashes is reloaded, and
the new document connects again. Closing the window or quitting asks nothing, because nothing ends
with it: Main records the window's place and the Session's terminals keep running.

## Server connection

Each document gets one `orchbee server connect` process and two transferred ports, in the order
`DESKTOP_SERVER_CONNECTION_PORT_ROLES` fixes. `ServerConnectionRelay` moves bytes between the
process and the first port without reading any; the second is the document's
[Project Folder acquisition](docs/project-folder-acquisition.md) port, served by a broker that lives
as long as the connection. The process owns activation, peer validation and admission, and the page
owns the protocol. A document asks for its connection once it is listening, through the preload,
under an identity the reply must repeat: a reload can hand the next document what Main meant for the
last one, and a load event cannot tell them apart. Its asking ends the previous document's
connection, and closing the process's standard input ends it.

Before connecting, Main runs `orchbee server start --format json`, because whether a restore lost
anything is known only to the activating process and the connection's bytes are opaque. It announces
the result with the ports: whether the document may open a first Workspace, which only the first
document a window connects may, along with the home directory, whether a login shell is available,
and the host name. The announcement also says whether that start was refused until the Session is
recovered, because the connection then ends before the page reads anything and only Main saw why.

The page has the Client authority of the Session it joined. That adds nothing to what it could
already do by typing into a terminal. A link the page opens asks for a window, which Main refuses,
handing an `http`, `https`, or `mailto` address to the person's browser instead; top-level
navigation stays within the renderer's origin.

A release joins the default Session, Local joins `desktop-local`, and Dev joins
`desktop-dev-<scope-digest>`, so each distribution profile keeps its own terminals;
`ORCHBEE_DESKTOP_SESSION` names another. Outside a release, both `orchbee` commands run under the
data root's configuration file, as [Desktop data root](docs/data-root.md) describes, so a Session
they start keeps its
[shared preferences](../../crates/workbench-protocol/docs/user-preferences.md#scope-and-persistence)
apart from the installed release's. A Session already running keeps the configuration it started
with.

## Window

- A hidden title bar, with the system's controls drawn over the page. On macOS the traffic lights sit
  at `(8, 10)`, and the renderer centers the sidebar toggle in the same `36px` horizontal Tab row
  after the reserved native-control space.
- The window's place, restored and recorded as [Window lifecycle](docs/window-lifecycle.md)
  describes.
- The menu with the window Actions and, on macOS, Secure Keyboard Entry. It is installed once the
  window has opened, because a hidden title bar never opens the menu that would refresh what its
  accelerators may do. On macOS, Workbench owns `Command+W` as Close Terminal: the native Close
  Window item stays clickable but registers no accelerator, so a terminal-close key cannot fall
  through and close the window.

### Native window drag-region projection

- **Problem and scope:** A Desktop user must be able to move a frameless native window from every
  unoccupied point in its `36px` top chrome without losing Tabs, groups, the sidebar toggle, the
  new-Tab control, or operating-system window controls. This projection covers the sidebar top row
  and a horizontal Tab bar. The vertical Tab Strip, content below the top chrome, Panel and sidebar
  interactions, resize separators, Tab cross-window drag, and the native move lifecycle are non-goals.
- **Inputs, data, and ownership:** Trusted Workbench layout supplies sidebar visibility, Tab Strip
  orientation, the horizontal chrome boundary, and the rendered control geometry; `windowControls`
  supplies the operating-system control side. There is no durable drag-region state and no missing
  input is inferred. Workbench owns the shell geometry, the Desktop renderer exclusively projects
  CSS app regions, and Electron and the operating system own native-control precedence plus pointer
  activation, movement, release, and cancellation.
- **Model and output:** The renderer applies this hit-test policy after every layout projection:

  | Horizontal-chrome target                                                                | CSS app region | Observable result                                |
  | --------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------ |
  | Unoccupied sidebar top-row background                                                   | `drag`         | Starts native window movement                    |
  | Unoccupied horizontal Tab-bar background, including reserved leading and trailing space | `drag`         | Starts native window movement                    |
  | Sidebar-toggle position, Tab, group, or button geometry                                 | `no-drag`      | Retains the control's ordinary pointer behavior  |
  | Vertical Tab Strip or content outside the horizontal chrome                             | Unset          | Retains its existing non-window-drag interaction |

- **Invariants and transitions:** Native window controls and rendered Workbench controls always
  take precedence over a background drag region; no Desktop drag projection extends below the
  `36px` boundary. Showing or hiding the sidebar and switching Tab Strip orientation recompute the
  projection without starting, committing, or cancelling a native drag. Unsupported or absent
  horizontal chrome is an intentional no-op.
- **Dependencies and verification:** Electron's CSS app-region contract is the only platform
  capability. The controlled Desktop system trace that checked the sidebar-row geometry, the
  horizontal Tab-bar default, every interactive exclusion, and sidebar visibility transitions
  retired with the Desktop Electron suite, so no automated test covers the projection.

## Native Project Folder acquisition

Desktop is the only composition that injects the Workbench existing-Folder acquisition capability.
One correlated renderer request asks Main to open
`dialog.showOpenDialog(..., { properties: ['openDirectory'] })` for the owning window. Main accepts
at most one active request for that renderer, requires exactly one selected path, normalizes it to an
absolute path, and checks that it still names a readable, traversable directory. Validation follows
a symbolic link while preserving the user's selected path spelling. The renderer cannot submit a
path for validation or request directory contents.

A selected result enters the Workbench Project-create or Project-directory-add Action. Cancellation
and bounded dialog, type, existence, permission, or validation failures change no Project state.
Retiring the document or closing its connection revokes delivery, so a late native completion cannot
reach the replacement page. `New Folder…` uses a native-selected parent token and a single name,
then atomically creates one absent directory only after explicit Create. A new Project still comes
from the Server's Workspace launch; when a document ends before its new folder's Project is
confirmed, Main reports the kept folder in a message box over the window. The maintained behavior,
evidence, limits, and rollback contract live in
[`docs/project-folder-acquisition.md`](docs/project-folder-acquisition.md).

## Development

`pnpm desktop` builds shared dependencies and Web resources, runs the Web development server, and
supervises one Electron process:

- At start it builds the checkout's native cohort (`orchbee`, Server, Worker, and Terminal Host)
  and stages it once per distinct build under `target/desktop-development`, reusing an identical
  staged cohort. A Session refuses a Client of another cohort, so a development Session
  that an earlier cohort started is stopped by that cohort first, and cohorts no development Session
  names any more are removed. Native sources are not watched; restart `pnpm desktop` after changing
  them.
- A Web change reloads the renderer through the development server. A Main or preload build asks
  Electron to quit and starts it again; the Session outlives every restart, and outlives
  `pnpm desktop` itself, as it outlives a released Desktop.
- The runtime removes `PAGER` and `GIT_PAGER` from Electron's environment, which the Session's
  shells inherit through `orchbee` and where a non-interactive pager would stick.

Electron defaults to `http://127.0.0.1:3000/`; `ORCHBEE_DESKTOP_RENDERER_URL` selects another
loopback Web server. To run the built entry without the development runtime, run
`pnpm --filter @orchbee/web build` and `pnpm --filter @orchbee/desktop build`, set these, and run
`electron dist/main/main.cjs` in this directory:

| Variable                                  | Meaning                                                                   |
| ----------------------------------------- | ------------------------------------------------------------------------- |
| `ORCHBEE_DESKTOP_CLIENT_EXECUTABLE`       | A staged cohort's `orchbee`, from the same cohort as the Session's Server |
| `ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT` | This checkout, which scopes the development data root                     |
| `ORCHBEE_DESKTOP_RENDERER_ROOT`           | `apps/web/dist`, or set `ORCHBEE_DESKTOP_RENDERER_URL` for a loopback URL |
| `ORCHBEE_DESKTOP_SESSION`                 | Optional Session name                                                     |
| `ORCHBEE_DESKTOP_WINDOW_HIDDEN`           | Optional; for automation, a window never shown, restored, or recorded     |

A packaged application ignores all but the last two: it runs its bundled cohort and serves its
bundled renderer through the secure `orchbee:` scheme. A window that stays hidden produces no
frames; automation that needs pixels shows it off-screen with `showInactive`.

On macOS, the command materializes and ad-hoc signs a content-addressed `Orchbee Dev.app` beneath
`apps/desktop/out/development-runtime/`, then launches its exact executable. Its application name,
checkout-scoped bundle identifier, Dev-badged Orchbee icon, and checkout-scoped data root are
separate from the installed release application. Development diagnostics must target that exact
bundle path or identifier and never fall back to `/Applications/Orchbee.app`.

The Rsbuild configuration can be inspected without workspace build artifacts or an installed
Electron executable. Development dependencies and Electron resolution are loaded inside the
development startup hook.

### Distribution profiles

Runtime identity and descriptor parsing belong to `src/distribution/desktop-distribution-profile.ts`; packaging-only argument parsing and descriptor encoding live in `src/distribution/tooling/`. Both use the same application-name validation. These source-executed commands require Node native type stripping (enabled by default in Node 22.18 and newer). Node-executed scripts import source modules through explicit `.ts` paths under the [TypeScript module contract](../../docs/TYPESCRIPT.md#module-syntax); the no-emit Desktop typecheck enables `allowImportingTsExtensions`, and the distribution tests include a real Node import witness. `@electron/packager` is a development dependency.

Desktop owns exactly three explicit distribution profiles:

| Command                        | Application         | Bundle identifier                          | Data root                       | Icon              | Output                      |
| ------------------------------ | ------------------- | ------------------------------------------ | ------------------------------- | ----------------- | --------------------------- |
| `pnpm desktop:package:dev`     | `Orchbee Dev.app`   | `com.electron.orchbee.dev.d<scope-digest>` | `~/.orchbee-dev-<scope-digest>` | Purple Dev badge  | `apps/desktop/out/dev/`     |
| `pnpm desktop:package:local`   | `Orchbee Local.app` | `com.electron.orchbee.local`               | `~/.orchbee-local`              | Green Local badge | `apps/desktop/out/local/`   |
| `pnpm desktop:package:release` | `Orchbee.app`       | `com.electron.orchbee`                     | `~/.orchbee`                    | Orchbee mark      | `apps/desktop/out/release/` |

On macOS each profile uses its committed `.icns` from `resources/brand/icns/`, drawn on Apple's 824 px icon grid with the Dev or Local badge over the lower-right corner. After changing `resources/icon.svg`, run `pnpm generate:brand-assets` (requires Google Chrome); `pnpm audit:brand-assets` fails while `resources/brand/` is stale. Windows and Linux packages still use the Electron default icon.

`ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME` optionally overrides the default `Orchbee Dev` name
for `pnpm desktop` and `pnpm desktop:package:dev`.
`ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME` independently overrides the default `Orchbee Local` name
for `pnpm desktop:package:local`. Each value must be 1–64 characters, have no leading or trailing
whitespace, and contain no path separators, control characters, or platform-invalid filename
characters. The selected name is persisted in the corresponding distribution descriptor so the
packaged application keeps it without the shell variable. Neither override changes its profile's
bundle identifier or data root, and the `release` profile ignores both variables.

```sh
ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME="Orchbee Dev Canary" \
pnpm desktop:package:dev

ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME="Terminal" \
pnpm desktop:package:local
```

`pnpm desktop` uses the `dev` identity without creating a packaged application. Packaging requires
an explicit dev/local/release profile. Each command builds Web resources and the Main and preload
bundles, then stages the whole native cohort with `scripts/stage-local-runtime.mjs`: `orchbee`,
Server, Worker, Terminal Host, the observation Capsule, and the platform's terminal runtime. The
window's Session runs that cohort, so the application carries it. The cohort and the Web resources
stay outside `app.asar` as `resources/cohort` and `resources/dist`.

All three package commands write beneath their profile-specific directory in `apps/desktop/out/`.
They do not perform Electron code signing, notarization, installer creation, publication, or update
distribution.

### Verification

Run package checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop build
```

Unit tests cover the data root and distribution identities, the window-state record and owner,
window and Secure Keyboard Entry Actions, the menu, the renderer scheme, the preload bridge, and
the Project Folder broker and runtime. No automated test drives a native window or the Server
connection; the Desktop Electron suite that did retired with Desktop's Main-owned runtime.
