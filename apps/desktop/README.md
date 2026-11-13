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
with it: Main records the window's place and the Session's terminals keep running. On Windows,
closing the window while terminals run hides it, and Desktop keeps running in the notification area
until the window is reopened, Orchbee is quit from the icon, or the last terminal ends, as
[Window lifecycle](docs/window-lifecycle.md#running-in-the-notification-area) describes; with no
terminal running, closing it quits as before. Quit All, also in the icon's menu, is the one quit that
stops the Session, and it asks first.

## Server connection

Each document gets one `orchbee server connect` process and three transferred ports, in the order
`DESKTOP_SERVER_CONNECTION_PORT_ROLES` fixes. `ServerConnectionRelay` moves bytes between the
process and the first port without reading any; the second is the document's
[Project Folder acquisition](docs/project-folder-acquisition.md) port, served by a broker that lives
as long as the connection; the third is its background-status port, over which the page reports how
many terminals run and which Agent states need the person, and asks for the window when a
notification is clicked. Main parses each report and counts nothing itself; where no
notification-area icon is kept, it closes that port. Over the same port Main asks the page for Quit
All's Session stop, which the page sends through its Server connection because Main never speaks
the Client protocol. The process owns activation, peer validation and admission, and the page
owns the protocol. A document asks for its connection once it is listening, through the preload,
under an identity the reply must repeat: a reload can hand the next document what Main meant for the
last one, and a load event cannot tell them apart. Its asking ends the previous document's
connection, and closing the process's standard input ends it.

Before connecting, Main runs `orchbee server start --format json`, because whether a restore lost
anything is known only to the activating process and the connection's bytes are opaque. It announces
the result with the ports: whether the document may open a first Workspace, which only the first
document a window connects may, along with the home directory, whether a login shell is available,
and the host name. The announcement also says why that start was refused, because the connection
then ends before the page reads anything and only Main saw why: until the Session is recovered, or
by the Session's handshake as the CLI reports it, with this Desktop too old, the Session too old,
another environment, or a development build of another protocol. A version refusal carries the
bundled Client's version and the Session's, which a Session older than the handshake never states.

The page has the Client authority of the Session it joined. That adds nothing to what it could
already do by typing into a terminal. A link the page opens asks for a window, which Main refuses,
handing an `http`, `https`, or `mailto` address to the person's browser instead; top-level
navigation stays within the renderer's origin.

A release joins the default Session, Test joins `desktop-test`, and Dev joins
`desktop-dev-<scope-digest>`, so each distribution profile keeps its own terminals;
`ORCHBEE_DESKTOP_SESSION` names another in a development or test build; a packaged release ignores
it, and `ORCHBEE_SESSION` chooses its Session as it does for the CLI. Outside a release, both `orchbee` commands run under the
data root's configuration file, as [Desktop data root](docs/data-root.md) describes, so a Session
they start keeps its [settings](../../crates/local-runtime/README.md#user-settings) apart from the
installed release's. A Session already running keeps the configuration it started with.

## Window

- A hidden title bar, with the system's controls drawn over the page. On macOS the traffic lights sit
  at `(12, 12)`, and the renderer centers the sidebar toggle in the same `36px` horizontal Tab row
  at x `76`, after the reserved native-control space. On Windows the caption buttons draw over the
  page's top end, so the horizontal Tab bar, the mobile bar, and the Tab list each end in a trailing
  reserve as wide as the window leaves past `env(titlebar-area-x)` and `env(titlebar-area-width)`;
  Tabs that do not fit overflow within the remaining width, and the New Tab control stays clear of
  the caption buttons at every window width.
- The window's title is its page's, which the taskbar and window switcher show; the page follows the
  Session's title policy as the TUI's outer title does.
- The window's place, restored and recorded as [Window lifecycle](docs/window-lifecycle.md)
  describes.
- On Windows, the notification-area icon while the window is hidden with terminals running. Its
  image is the Orchbee mark with a hexagon for the most urgent Agent state, red for a failure,
  yellow for one needing attention, green for a finished turn not yet seen, and outside a release
  the Dev circle or Test square; its tooltip and its menu's first line count the running terminals
  and those Agents. Left click and double-click reopen the window; the menu holds Reopen Orchbee,
  Quit Orchbee, Keep Terminals Running, and Quit All…, which asks in a native dialog before it stops
  every terminal and quits. The icons are generated into
  `resources/brand/ico/tray/`, which a Windows package carries as `resources/tray`.
- On Windows, Save Diagnostic Bundle… in the Help menu and in the taskbar's Jump List of a packaged
  build, since the title bar overlay hides the menu bar. It asks where to save, starting in the
  Desktop folder, or in the home folder when the system cannot name the Desktop, runs
  `orchbee diagnostics create`, shows the bundle in its folder, and says in a dialog when it failed;
  one save runs at a time and cancelling the dialog writes nothing. The bundle's contents belong to
  the [CLI command](../orchbee/README.md#windows-diagnostic-bundle).
- The menu with the window Actions and, on macOS, Secure Keyboard Entry. It is installed once the
  window has opened, because a hidden title bar never opens the menu that would refresh what its
  accelerators may do. On macOS, Workbench owns `Command+W` as Close Terminal: the native Close
  Window item stays clickable but registers no accelerator, so a terminal-close key cannot fall
  through and close the window.

### Native window drag-region projection

- **Problem and scope:** A Desktop user must be able to move a frameless native window from every
  unoccupied point in its `36px` top chrome without losing Tabs, groups, the sidebar toggle, the
  new-Tab control, or operating-system window controls. This projection covers the sidebar top row
  in every sidebar presentation, a horizontal Tab bar, the mobile bar, the open Tab list's top band,
  and the no-Workspace row. The vertical Tab Strip, content below the top chrome, Panel and sidebar
  interactions, resize separators, and the native move lifecycle are non-goals.
- **Inputs, data, and ownership:** Trusted Workbench layout supplies sidebar visibility, Tab Strip
  orientation, the horizontal chrome boundary, the rendered control geometry, the sidebar toggle's
  place in each chrome beneath it, and whether a surface covers the layout; `windowControls`
  supplies the operating-system control side. There is no durable drag-region state and no missing
  input is inferred. Workbench owns the shell geometry, the Desktop renderer exclusively projects
  CSS app regions, and Electron and the operating system own native-control precedence plus pointer
  activation, movement, release, and cancellation.
- **Model and output:** The renderer applies this hit-test policy after every layout projection:

  | Horizontal-chrome target                                                                  | CSS app region | Observable result                                |
  | ----------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------ |
  | Unoccupied sidebar top-row background, docked, overlaid, or filling the window            | `drag`         | Starts native window movement                    |
  | Unoccupied horizontal Tab-bar, mobile-bar, open Tab-list band, or no-Workspace background | `drag`         | Starts native window movement                    |
  | Reserved trailing space under the Windows caption buttons                                 | `drag`         | The native caption buttons take the pointer      |
  | Sidebar toggle, its place in the chrome beneath it, Tab, group, or button geometry        | `no-drag`      | Retains the control's ordinary pointer behavior  |
  | Overlay sidebar's scrim                                                                   | `no-drag`      | A click outside the overlay closes it            |
  | Layout covered by the overlay sidebar, the filling sidebar, or the open Tab list          | Unset          | The covering surface alone decides               |
  | Vertical Tab Strip or content outside the horizontal chrome                               | Unset          | Retains its existing non-window-drag interaction |

- **Invariants and transitions:** Native window controls and rendered Workbench controls always
  take precedence over a background drag region. Electron folds app regions in document order, a
  drag region adding its box and a no-drag region removing its own, so a drag region later in the
  document covers an earlier control whatever their stacking. Every drag region therefore precedes
  the controls over it: a control inside a drag region follows it; the sidebar toggle, which
  precedes the layout to stay first in the focus order, has a no-drag place inside each chrome that
  can lie beneath it; and the surfaces that cover the layout, which precede it for the same reason,
  leave the covered layout without any app region. The z-index never decides the projection.
  Because the blank Tab bar is a drag region, a double-click there maximizes or restores the window,
  as the operating system does, and never reaches the Tab Strip's blank-track New Tab double-click;
  no Desktop drag projection extends below the `36px` boundary. Showing or hiding the sidebar and switching Tab Strip orientation recompute the
  projection without starting, committing, or cancelling a native drag. Unsupported or absent
  horizontal chrome is an intentional no-op.
- **Dependencies and verification:** Electron's CSS app-region contract is the only platform
  capability. The controlled Desktop system trace that checked the sidebar-row geometry, the
  horizontal Tab-bar default, every interactive exclusion, and sidebar visibility transitions
  retired with the Desktop Electron suite, so no automated test covers the projection. Synthetic
  and DevTools Protocol clicks bypass the operating system's hit test and pass even where a real
  pointer drags the window, so evidence comes from a real window: `WM_NCHITTEST` returns `HTCLIENT`
  at a control and `HTCAPTION` at unoccupied chrome.

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
  with the checkout's development stem compiled in on every platform, and stages it once per
  distinct build under `target/desktop-development`, reusing an identical staged cohort. The running
  development Session stays when the new `cohort.json` names the same Server, Worker, and Terminal
  Host as the cohort that started it, because the rebuilt Client joins it by protocol fingerprint;
  otherwise that cohort stops it first, which ends its terminals. Each Session's marker beside the
  cohorts records the cohort that started it, the cohort whose Client the latest run used, and the
  Server's process ID that `session list` reported, so cleanup removes only cohorts no running
  Session can be using, and nothing while `session list` cannot answer. Native sources are not
  watched; restart `pnpm desktop` after changing them.
- A Web change reloads the renderer through the development server. A Main or preload build asks
  Electron to quit and starts it again; the Session outlives every restart, and outlives
  `pnpm desktop` itself, as it outlives a released Desktop.
- The runtime removes `PAGER` and `GIT_PAGER` from Electron's environment, which the Session's
  shells inherit through `orchbee` and where a non-interactive pager would stick.

Each checkout's development servers listen on loopback ports derived from its checkout digest, so
two checkouts run `pnpm desktop` side by side: the Web renderer on an even port of 41000–48999 and
this package's Main and preload server on the next one, both strict. `scripts/development-server-ports.mjs`
at the repository root owns the rule. Electron loads that Web port unless
`ORCHBEE_DESKTOP_RENDERER_URL` selects another loopback Web server; two checkouts landing in one
slot fail at start, and `rsbuild dev --port` serves either one elsewhere. Packaged builds run no
development server. To run the built entry without the development runtime, run
`pnpm --filter @orchbee/web build` and `pnpm --filter @orchbee/desktop build`, set these, and run
`electron dist/main/main.cjs` in this directory:

| Variable                                  | Meaning                                                                   |
| ----------------------------------------- | ------------------------------------------------------------------------- |
| `ORCHBEE_DESKTOP_CLIENT_EXECUTABLE`       | A staged cohort's `orchbee`, from the same cohort as the Session's Server |
| `ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT` | This checkout, which scopes the development data root                     |
| `ORCHBEE_DESKTOP_RENDERER_ROOT`           | `apps/web/dist`, or set `ORCHBEE_DESKTOP_RENDERER_URL` for a loopback URL |
| `ORCHBEE_DESKTOP_SESSION`                 | Optional Session name; a packaged release ignores it                      |
| `ORCHBEE_DESKTOP_WINDOW_HIDDEN`           | Optional; for automation, a window never shown, restored, or recorded     |

Main runs as development exactly when Electron's default app loaded its entry, as `pnpm desktop`
and `electron dist/main/main.cjs` both do, whatever the executable is named; Electron's
`app.isPackaged` only compares that name with `electron`, so Main never reads it. A packaged
application ships no default app, so no environment variable makes it run as development. It
ignores all but the last two variables, and a packaged release ignores those as well: it runs its
bundled cohort and serves its bundled renderer through the secure `orchbee:` scheme. A window that stays hidden produces no
frames; automation that needs pixels shows it off-screen with `showInactive`.

On Windows, the command mirrors Electron's distribution, by hard links where the volume allows,
into a content-addressed directory beneath `apps/desktop/out/development-runtime/` with the
executable renamed `ob-dev-<scope-digest>-desktop.exe`, and launches that. Task Manager then tells
the development shell apart from its own `ob-dev-<scope-digest>.exe` Client, another checkout's
shell, and an installed `orchbee-desktop.exe`.

On macOS, the command materializes and ad-hoc signs a content-addressed `Orchbee Dev.app` beneath
`apps/desktop/out/development-runtime/`, then launches its exact executable. Its application name,
checkout-scoped bundle identifier, Dev-badged Orchbee icon, and checkout-scoped data root are
separate from the installed release application. Development diagnostics must target that exact
bundle path or identifier and never fall back to `/Applications/Orchbee.app`.

The Rsbuild configuration can be inspected without workspace build artifacts or an installed
Electron executable. Development dependencies and Electron resolution are loaded inside the
development startup hook.

### Distribution profiles

Runtime identity and descriptor parsing belong to `src/distribution/desktop-distribution-profile.ts`; packaging-only argument parsing and descriptor encoding live in `src/distribution/tooling/`. The application-name rules and the default `Orchbee Dev` and `Orchbee Test` names belong to `src/distribution/desktop-application-name.ts`, which the profile and the tooling both import. These source-executed commands require Node native type stripping (enabled by default in Node 22.18 and newer). Node-executed scripts import source modules through explicit `.ts` paths under the [TypeScript module contract](../../docs/TYPESCRIPT.md#module-syntax); the no-emit Desktop typecheck enables `allowImportingTsExtensions`, and the distribution tests include a real Node import witness. `@electron/packager` is a development dependency.

Desktop owns exactly three explicit distribution profiles:

| Command                        | Application        | Bundle identifier                         | Data root                  | Icon             | Output                      |
| ------------------------------ | ------------------ | ----------------------------------------- | -------------------------- | ---------------- | --------------------------- |
| `pnpm desktop:package:dev`     | `Orchbee Dev.app`  | `com.orchbee.desktop.dev.d<scope-digest>` | `~/.ob-dev-<scope-digest>` | Purple Dev badge | `apps/desktop/out/dev/`     |
| `pnpm desktop:package:test`    | `Orchbee Test.app` | `com.orchbee.desktop.test`                | `~/.ob-test-<home-digest>` | Green Test badge | `apps/desktop/out/test/`    |
| `pnpm desktop:package:release` | `Orchbee.app`      | `com.orchbee.desktop`                     | `~/.orchbee`               | Orchbee mark     | `apps/desktop/out/release/` |

The bundle identifier is also the Windows AppUserModelID: a packaged Windows application sets it at
startup and the installer's Start menu shortcut carries it, so Windows groups the window, its
taskbar button and its notifications under that shortcut.

On macOS each profile uses its committed `.icns` from `resources/brand/icns/`, drawn on Apple's 824 px icon grid with the Dev or Test badge over the lower-right corner. After changing `resources/icon.svg`, run `pnpm generate:brand-assets` (requires Google Chrome); `pnpm audit:brand-assets` fails while `resources/brand/` is stale. Every Windows profile uses `resources/brand/ico/orchbee.ico`; Linux packages still use the Electron default icon.

Packages name their native cohort by an environment stem, so the operating system and the agent can tell the release, development, and test processes apart. The stem is `orchbee` for release, `ob-dev-<scope-digest>` for dev (one per checkout, so parallel worktrees never share names), and `ob-test-<home-digest>` for test (one per home directory). The Client, Server, Worker, and Terminal Host carry the same stem, for example `ob-dev-<scope-digest>` and `ob-dev-<scope-digest>-server`, with `.exe` on Windows. The packaged Windows Desktop executable is `<stem>-desktop.exe` (release `orchbee-desktop.exe`), so the shell never shares an image name with its own Client; the packaged output folder takes that executable's name. Display names and bundle identifiers do not change, and every data root is the same stem with a leading dot. Every cohort also compiles in the stem, which the handshake compares, so a release Client in a development or test Desktop's terminal is refused.

`ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME` optionally overrides the default `Orchbee Dev` name
for `pnpm desktop` and `pnpm desktop:package:dev`.
`ORCHBEE_DESKTOP_TEST_APPLICATION_NAME` independently overrides the default `Orchbee Test` name
for `pnpm desktop:package:test`. Each value must be 1–64 characters, have no leading or trailing
whitespace, and contain no path separators, control characters, or platform-invalid filename
characters. The selected name is persisted in the corresponding distribution descriptor so the
packaged application keeps it without the shell variable. Neither override changes its profile's
bundle identifier or data root, and the `release` profile ignores both variables.

```sh
ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME="Orchbee Dev Canary" \
pnpm desktop:package:dev

ORCHBEE_DESKTOP_TEST_APPLICATION_NAME="Terminal" \
pnpm desktop:package:test
```

`pnpm desktop` uses the `dev` identity without creating a packaged application. Packaging requires
an explicit dev/test/release profile. Each command builds Web resources and the Main and preload
bundles, then stages the whole native cohort with `scripts/stage-local-runtime.mjs`: the Client,
Server, Worker, Terminal Host, the observation Capsule, and the platform's terminal runtime. The
window's Session runs that cohort, so the application carries it. The cohort and the Web resources
stay outside `app.asar` as `resources/cohort` and `resources/dist`.

On Windows the test and release commands then make the cohort a signed release directory before
packaging it: `scripts/package-native-windows.mjs` writes its `release.json` for the `stable`
channel, the workspace version (which must equal this package's version) and a build id, and
`scripts/sign-release-manifest.mjs` signs it. `--build-id <id>` sets the build id, which the release
workflow passes as `<commit>.<run number>`; otherwise it is `<commit>.local`. `--key-file <absolute
PEM>` or `--key-env <VARIABLE>`, repeatable, choose the signing keys; otherwise a test build signs
with the repository's test key and a release with the key in `ORCHBEE_RELEASE_SIGNING_KEY`. A
development cohort is never signed, so the bundled Client of a dev package cannot install it.

All three package commands write beneath their profile-specific directory in `apps/desktop/out/`.
They do not perform Electron code signing, notarization, publication, or update distribution; the
Windows installer below is built from the test and release packages.

### Windows installer

`pnpm desktop:installer:test` and `pnpm desktop:installer:release` package their profile as above
and then compile `installer/windows/orchbee.iss` with Inno Setup into
`apps/desktop/out/<profile>/installer/`. Extra arguments after `--` reach `scripts/package.mjs`, for
example `pnpm desktop:installer:release --build-id <id> --key-env <VARIABLE>`. The compiler is
`ORCHBEE_INNO_SETUP_COMPILER`, an absolute path to `ISCC.exe`, or else `ISCC.exe` in the default
Inno Setup 6 folder. The script refuses every compiler but Inno Setup 6.7.1, the version the release
workflow installs and checks. Next to the setup file, `<setup name>.files.txt` lists every compiled
file with its SHA-256, so a release can compare it with the signed `release.json`.

| Profile | Setup file                                                 | Start menu and Settings › Apps | Store root folder       | Registration                            |
| ------- | ---------------------------------------------------------- | ------------------------------ | ----------------------- | --------------------------------------- |
| release | `Orchbee-<version>-windows-x86_64-setup.exe`               | `Orchbee`                      | `Orchbee`               | `com.orchbee.desktop_is1`               |
| test    | `ob-test-<home-digest>-<version>-windows-x86_64-setup.exe` | `ob-test-<home-digest>`        | `ob-test-<home-digest>` | `com.ob-test-<home-digest>.desktop_is1` |

The store root is `%LOCALAPPDATA%\Programs\<store root folder>`, or `ORCHBEE_INSTALL_DIR` when that
names an absolute path, as for `orchbee install`. A development checkout has no installer. A test
installer uses none of the release names, so it installs beside a release one.

Setup runs for the current user without administrator rights. It shows no welcome, folder,
component, Start menu folder or licence page: only Ready to Install, progress, and a finish page with
a Launch box, in English or Simplified Chinese by the Windows display language. The Chinese messages
are Orchbee's reviewed copy of Inno Setup's unofficial `ChineseSimplified.isl`, kept beside the
script under the Inno Setup License; the installer's own messages are in the script in both
languages.

An install changes exactly this:

1. `<store root>\desktop\` receives the packaged window application. A previous version's folder
   first moves aside to `desktop.previous`, which goes once the new one is in place and comes back if
   the install fails.
2. The Start menu entry `%APPDATA%\Microsoft\Windows\Start Menu\Programs\<name>.lnk` points at
   the window executable with the profile's AppUserModelID.
3. The registration `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<registration>`
   gives Settings › Apps the name, version, publisher `Orchbee`, icon, size and the uninstaller
   `<store root>\unins000.exe`, which Setup writes with `unins000.dat`.
4. Setup runs `<store root>\desktop\resources\cohort\<client>.exe install --format json` with
   `ORCHBEE_INSTALL_DIR` set to the store root. That bundled Client verifies its signed release
   directory, copies it to `versions\<version>`, activates it unless a higher version is active,
   puts the launcher at `bin\<client>.exe` and adds `bin` to the user `Path`. Setup copies nothing
   into the store itself and edits no `Path`; it only broadcasts the environment change at the end,
   so a terminal opened afterwards finds the command.

Installing a newer setup over an older one replaces it in place: one Settings entry, one Start menu
entry, the new window application, and the new version active in the store. Setup never ends
Orchbee. While the window, or a Session it started from the runtime in `desktop\`, still runs,
Setup stops before changing anything, asks the person to quit Orchbee and end its terminals, and
exits with code 7. Sessions the command started run from the store and keep running, because
installing only adds a version directory; the new window joins them while they are compatible and
otherwise says they are too old.

Removal from Settings › Apps runs the uninstaller, which asks once and then, in this order:

1. runs `<store root>\bin\<client>.exe --installer-uninstall-v1`, or the bundled Client when no
   launcher is left. It ends no terminal: while one is open it changes nothing, and the uninstaller
   shows its reason and stops. Otherwise it removes the store's versions, `bin`, `desktop\`, the
   `Path` entry, the Agent integrations, the Start menu entry and the registration, keeps the data
   root, and leaves `unins000.exe` and `unins000.dat`;
2. removes whatever it installed that is still there, then its own two files and the empty store
   root.

A second removal finds no Client and nothing of Orchbee's, and removes only what remains of its
own. The data root (`~/.orchbee` for release) stays until `orchbee uninstall --delete-data`.

For workflows, `<setup>.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG="<file>"` installs
without a window, and `<store root>\unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
/LOG="<file>"` removes. Without `/LOG`, Setup still writes `Setup Log <date> #<n>.txt` in `%TEMP%`,
with the exit code and output of each Client command. Setup exits 0 on success, 7 while Orchbee runs
from the installation, 10 when the window application was installed but `orchbee install` failed,
and otherwise with Inno Setup's own codes, such as 2 when cancelled before installing. The
uninstaller exits non-zero when the removal stopped.

### Verification

Run package checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop build
```

Unit tests cover the data root and distribution identities, the window-state record with its
migration and upgrade backup, the window-state owner, window and Secure Keyboard Entry Actions, the
menu, the renderer scheme, the preload bridge, the Project Folder broker and runtime, and the
background lifecycle's close, quit, Quit All, and icon invariants. No automated test drives a native window or the Server
connection; the Desktop Electron suite that did retired with Desktop's Main-owned runtime.
