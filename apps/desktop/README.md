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

Main keeps one instance; starting Orchbee again switches to a newer joinable window version when
Setup or an update just placed one, and otherwise brings the window forward. On macOS the
application stays open without a window, and activating it opens one. A renderer that crashes is reloaded once,
and the new document connects again; a second crash within a minute of the last stops reloading,
and a native dialog, like VS Code's, offers Reopen or Close (Esc closes). Closing destroys the dead
page, so the window opens afresh later. Closing the window or quitting asks nothing, because nothing ends
with it: Main records the window's place and the Session's terminals keep running. From start until
quit, Desktop shows an icon in the Windows notification area, the macOS menu bar or the Linux panel,
with or without a window, whose menu counts the running terminals. On Windows and Linux, closing the
window while terminals run hides it, and Desktop keeps running until the window is reopened, Orchbee is quit from the
icon, or the last terminal ends, as
[Window lifecycle](docs/window-lifecycle.md#running-in-the-notification-area) describes; with no
terminal running, closing it quits as before. On macOS the same close hides the window, and Desktop
stays in the Dock whatever runs, as
[Running in the menu bar](docs/window-lifecycle.md#running-in-the-menu-bar) describes. Stop
Session… (Quit All in the code), in the icon's menu and the macOS application menu, is the one quit
that stops the Session, and it asks first.

## Server connection

Each document gets one `orchbee server connect` process and five transferred ports, in the order
`DESKTOP_SERVER_CONNECTION_PORT_ROLES` fixes. `ServerConnectionRelay` moves bytes between the
process and the first port without reading any, bounded both ways: the page reports each chunk
it has read, and Main pauses the process's output while 4 MiB it relayed are unread, until half is
read. A slow page then slows only its own connection, and the Server's bound on a slow Client
decides beyond that, as for any Client. A process that leaves 64 MiB of the page's input untaken
ends that connection alone. The second is the document's
[Project Folder acquisition](docs/project-folder-acquisition.md) port, served by a broker that lives
as long as the connection; the third is its background-status port, over which the page reports how
many terminals run and which Agent states need the person, and asks for the window when a
notification is clicked. Main parses each report and counts nothing itself; where Desktop keeps no
notification-area or menu-bar icon, on Linux and for a window started hidden, it closes that port. Over the same port Main asks the page for Quit
All's Session stop, which the page sends through its Server connection because Main never speaks
the Client protocol. The fourth is its window-update port, described under
[Window Update](#window-update). The fifth is its machines port, which `src/main/remote-machines/` connects to
the saved machines' process, `orchbee --desktop-machines` ([contract](../orchbee/docs/machines.md#desktop-machines-process));
the page's Settings › Machines, the sidebar's other machines and Change machine read it. Where the process answers that the host lacks saved machines, Main closes the port, so no view arrives and Settings shows no Machines category.
For each saved machine the hub reads online, the page asks for a connection to its Session, and
Main starts that machine's session relay, `orchbee --machine-session` ([contract](../orchbee/docs/machines.md#desktop-session-relay)),
reads only its first line within 30 seconds, and hands the page the relay's port under the
connection relay's bounds, or the reason there is none (`desktop-machine-session-contract.ts`). A
document holds one per machine, with no cap; closing the port or the document ends the relay, never
the remote Session. No contract carries the far
computer's home, so a remote surface starts New Workspace, with no Workspace focused, in the folder
of that Session's first Workspace, and leaves the directories its terminals report to its Server. The process owns activation, peer validation and admission, and the page
owns the protocol. A document asks for its connection once it is listening, through the preload,
under an identity the reply must repeat: a reload can hand the next document what Main meant for the
last one, and a load event cannot tell them apart. Its asking ends the previous document's
connection, and closing the process's standard input ends it.

A connection the process ends before Main or the page closed it, because its Server ended, is
waited on for `DESKTOP_SESSION_REJOIN_MILLISECONDS`, as the TUI waits. Main runs `status server`
about once a second, which starts nothing, and reloads the window once a Server serves the Session
again. A Session the automatic update's idle move ended reports `movedTo`: Main switches to the
newer window as a click on Update does when its Client joins, and otherwise reloads, which starts
the Session. The page reads Reconnecting… meanwhile and Disconnected after it; the person's
Reconnect reloads at any time, and a newer document ends the wait.

One window, connection, message, or request never ends Main. A document whose connection cannot be
made gets none, and a reload asks again; a throw or rejection that nothing else handled is
recorded as `main-fault-contained` and Main goes on, because every fault Main can meet belongs to
one of them. A throw from a notification-area or menu-bar icon, application menu, IPC or port
listener costs only that call and is recorded with its site.

The page reports the site of every fault it contains, and of every error or rejection that escapes
it, by posting it to its own window; the preload forwards a parsed site to Main, which records it
as `window-fault`. A window records at most 20 a minute, and once that minute ends one
`window-faults-dropped` record counts the rest, so a page that keeps failing cannot fill the log.
Nothing leaves the computer.

Before connecting, Main runs `orchbee server start --format json`, because whether a restore lost
anything is known only to the activating process and the connection's bytes are opaque. It announces
the result with the ports: whether the document may open a first Workspace, which only the first
document a window connects may, along with the home directory, whether a login shell is available,
the shells installed on this computer that Add profile lists, read once from the places the TUI's
local runtime reads them, and the host name. The announcement also says why that start was refused, because the connection
then ends before the page reads anything and only Main saw why: until the Session is recovered,
naming the process an earlier start left running when the CLI names one; because the system would
not create the Server, with the system's `ioKind` and `osError`; or by the Session's handshake as
the CLI reports it, with this Desktop too old, the Session too old, another environment, or a
development build of another protocol; or because the Client refused a data folder or file, naming
it and the rule it breaks, which the page words as the TUI and CLI do under this system's rules. A
version refusal carries the bundled Client's version and
the Session's, which a Session older than the handshake never states. Any other failure, including
a reason a newer CLI names, announces no reason, and so does a refusal the page cannot read, so the
page shows its general sentence. A successful start also names the Session's `sessionVersion`, which
[Session update](#session-update) compares with the window's own.

## Window Update

A window that runs from `desktop\versions\<its own version>\` offers Update in the sidebar footer
while the window pointer `desktop\active.json` names a version of higher precedence than its own,
unless that is the version a `--fallback-from=<version>` start came back from. A development
build, a flat layout, and a build's output folder never offer it. `src/main/window-update/` watches the pointer's folder, because the pointer is replaced by a rename,
reads at most 16 KiB of it, and pushes the offer, or none, to every document's window-update port
(and re-reads it directly from disk when an update check settles or a second launch starts). A
press first runs the new version's bundled Client with this window's Session arguments,
`server start --format json`; the switch goes ahead only when the start succeeds and its
`negotiatedCapabilities` include `compatibility-window`. Otherwise a press shows that the update
applies when no terminal runs, and the diagnostic log records why. A second launch only brings the
open window forward, except Setup's own launch on Windows: Setup's `[Run]` entry first writes
`desktop\setup-launch.json` naming the window version it installed, and Main switches the window to
that version, through the same start and place steps, only when the file is at most 120 seconds old,
parses, and names the offered version. Main deletes the file on every read, so a refused or failed
switch leaves nothing behind, and a first instance deletes a leftover file when it starts. The place
is asked of the focused document, or of the most recently focused one, and no answer within one
second relaunches with no place. macOS has no Setup marker, so a second launch only brings the
window forward there. A joinable version is switched to through the window entry
`desktop\<this window's file name>`: Main starts it detached, with `desktop\` as working folder and
the arguments `--relaunch-after <its process id>` followed by the place the asking document shows,
`--select-terminal=<TerminalPresentationId>` for its displayed terminal or else
`--select-workspace=<WorkspaceId>` for its Workspace, and then quits as Quit Orchbee, Keep
Terminals Running does. An entry that cannot start leaves the window running and is recorded. The
[window launcher](../orchbee-desktop-launcher/README.md) waits for the window to end and starts the
active version, whose Main strict-parses the place and gives it to its first document only as
`relaunchPlace`; nothing is stored. At start Main removes `ORCHBEE_RELAUNCH` from its environment,
so nothing it starts inherits it. When the variable named its parent and its first `server start`
answer, within 90 seconds of its start, refuses with `session-too-old`, Main exits with status 1
instead of showing the refusal, and the launcher starts the version it replaced. The Jump List task of a window that runs from a version folder names the entry, so it outlives that
version, and 15 seconds after startup Main runs `--tend-window-store` through its bundled Client
so retired window versions and flat-layout leftovers leave disk once any replaced process has
exited. The maintained design is the
[automatic-update proposal](../../proposals/platform/automatic-updates.md#window).

On macOS the application bundle is the entry. A packaged window that runs from a bundle,
`<name>.app/Contents/MacOS/<program>`, reads `desktop/active.json` under the store root that the
Client reads too (`ORCHBEE_INSTALL_DIR`, or `$HOME/.local/share/<stem>`), unless the bundle is one of
the store's own copies under `desktop/`. Each window version there is
`desktop/versions/<version>/<name>.app`, and the join check runs the Client in that bundle's
`Contents/Resources/cohort/`. A joinable version is switched to through the window launcher in that
same cohort, `<stem>-desktop-launcher`, with `--replace-application=<this window's bundle>` as its
third argument, before the place. Once the window has ended, the launcher replaces the bundle in
place, or installs into `~/Applications` when its folder cannot be written, and leaves the Session
running. Window versions reach the macOS store through `orchbee update` and install.sh's
`--desktop` in test and release builds. An application dragged from the disk image makes or advances its store on its own: while the store
has no active runtime version, or its active version is lower than the bundle's, a Session start of
its bundled Client installs that bundle's release directory (`orchbee install` with its PATH step
when no store version is active, or `orchbee install --no-path` when upgrading a lower store so an
existing `~/.local/bin` store link and `desktop/` advance without repeating the profile step), and
when joining an already-running Session it starts the same store install detached in the background
without delaying the join. The install also makes the window store `desktop/`, empty, since a copy
from the image carries no window list to store its own version with. The bundle stays the entry, the Session runs from the
store, and the window part of each later update installs the next window version there. An
install that fails leaves the start to the bundle's own cohort, as before, and is tried again at
the next start. Because `desktop/` may first appear during that start, the window follows
`desktop/active.json` from the first document served after a start that found no folder to watch.

On Linux the window store has the Windows layout: each window version is
`desktop/versions/<version>/` with its program `<stem>-desktop`, the entry `desktop/<stem>-desktop`
is a copy of the active version's window launcher, and the person's menu entry names that entry. A
window that runs from a version folder offers and switches as on Windows. A packaged window
anywhere else, such as an AppImage's mount or the Debian package's `/opt/<stem>/`, reads
`desktop/active.json` under the store root as on macOS, offers Update for a window version of higher
precedence there, and switches through the store's entry, `desktop/<stem>-desktop`, after the same
start check; it never moves to the store unasked. Its bundled Client brings its runtime version into
the store and makes an empty window store as an application dragged from the macOS disk image does,
so the window part of each later update installs window versions there.

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

### Manual self-test

Run these steps on real windows before merging a change to the Update action and its refusal and
failure notices, the relaunch, the window launcher, or the window store handover. The unit tests
do not start a real window. The release workflow runs the same steps on every release-path run as
the [real-window Update check](../../scripts/release-window-update.mjs), on a packaged Test
window and two older stand-ins copied from it; locally,
`node scripts/release-smoke-windows.mjs window-update <folder> --work <new folder>` (on macOS
`release-smoke-macos.mjs`), given the folder `pnpm desktop:package:test` writes, replaces them.
Use Test builds: they join the `desktop-test` Session, so the release Session is not touched. Keep
a terminal with a long-running command open through every step (`ping -t 127.0.0.1` on Windows,
`sleep 600` on macOS).

1. Install an older Test build and start it. Install a newer Test build without closing the older
   window: its window version lands in `desktop\versions\<newer>\` (on macOS,
   `desktop/versions/<newer>/<name>.app`), and `desktop\active.json` names it. The older window's
   sidebar footer shows Update. On Windows, clear Launch Orchbee on Setup's last page: with it ticked,
   Setup's own launch moves the older window to the newer version at once, and steps 2 to 4 cannot
   run. A Start-menu launch only focuses the older window, which keeps its footer Update. On macOS,
   install the newer build with `orchbee update` or the `--installer-window` install, not with
   `install.sh --desktop`, which quits the older window first, and not by copying from the `.dmg`,
   which needs the older app to quit.
2. Refusal: move the newer version's bundled Client aside (`<stem>.exe` in its
   `resources\cohort\` folder; on macOS `<stem>` in `Contents/Resources/cohort/`). Press Update and
   check that the waiting notice appears, the window keeps running, and the terminal is untouched.
   Move the Client back.
3. Failure: move the window entry aside (Windows: `desktop\<stem>-desktop.exe`, the file the
   relaunch starts, which is not the running copy; macOS: `<stem>-desktop-launcher` in the newer
   version's `Contents/Resources/cohort/`). Press Update and check that the failure notice appears,
   the window keeps running, and the terminal is untouched. Move the file back.
4. Success: press Update. The window quits and opens again running the newer version: About's
   version row shows it, and the terminal and its command still run.

`<stem>` is the build's artifact stem: `orchbee` in a release, `ob-test-` plus 16 hex digits in a
Test build.

## Session update

A `sessionVersion` of lower precedence than the window's own means the Session's Server runs an
older version, as after the person replaced the application while its Session kept running. Main
then moves the Session onto the window's version by itself once the document has its connection,
with nothing to confirm: it runs the bundled Client's `server restart --format json` whatever the
Session's restart tier says, because that tier reads the store's active version, which an
application copied from the disk image may still be installing.
The restart's new Server runs from the store's folder of this version once that version is active.
Meanwhile the page's notice says the Session is updating, and Update hides. The restart ends the
document's connection as any Server end does, and the window reloads once the new Server serves the
Session, as [Server connection](#server-connection) describes.

A restart the Session refuses, or one that fails, leaves it on the older version. The window-update
port then offers the move, and the page shows Update in the sidebar footer and About with a notice
that names why it waits: a configured command or plugin job runs, an older Client that cannot
reconnect by itself is connected, a Terminal Host must be replaced, or another reason. A press tries
again. A Host replacement asks first, as a window switch's does; the confirmed replacement stops the
Session through the page's connection, as Quit All does, and reloads the window, whose start runs
the Session on this version with every terminal restarted. A newer window version, when one is
offered, takes Update's place. The diagnostic log records each outcome as
`session-update-restarted`, `session-update-waiting`, `session-update-refused`, or
`session-update-host-replaced`.

A start refused with `session-too-old` runs the same restart first and starts again, so a Session
the window cannot join at all moves too where its Server can hand over its terminals. Only a
refusal that remains reaches the page: it says Orchbee could not update the Session and that
restarting the computer, or ending the Session's Orchbee processes in Activity Monitor, in Task
Manager or with `kill` on Linux, ends it with its terminals, and Try Again runs the start, and so
the restart, again.

## Window

- A hidden title bar, with the system's controls drawn over the page. On macOS the traffic lights sit
  at `(12, 12)`, and the renderer centers the sidebar toggle in the same `36px` horizontal Tab row
  at x `76`, after the reserved native-control space. On Windows and Linux the caption buttons draw
  over the page's top end, so the horizontal Tab bar, the mobile bar, and the Tab list each end in a trailing
  reserve as wide as the window leaves past `env(titlebar-area-x)` and `env(titlebar-area-width)`;
  Tabs that do not fit overflow within the remaining width, and the New Tab control stays clear of
  the caption buttons at every window width.
- On Windows and Linux, caption buttons in the page's theme. They draw with no background, so the
  page's own chrome shows beneath them, and their symbols read as the top bar's own icons: the color the page's
  theme gives `--ob-sys-color-icon-secondary`, named color themes included, blended over the bar's
  surface (`--ob-sys-color-surface-canvas` with `--ob-sys-color-surface-subtle` over it) into an
  opaque color. The page posts that color, converted to `#rrggbb`, to its own window whenever its
  theme attributes change; the preload forwards it to Main, which calls `setTitleBarOverlay`. The
  buttons therefore follow the application's theme, and the system's when the theme setting follows
  the system, without a restart. Until a document reports, the symbols keep the system's color. The
  hover fill behind minimize and maximize stays Electron's. Setting `nativeTheme.themeSource` from
  the page's theme was rejected: Electron gives the overlay the classic system button colors
  whatever that source says, so the buttons would not change, and the override would also redefine
  `prefers-color-scheme`, from which the page resolves a theme that follows the system.
- The window's title is its page's, which the taskbar and window switcher show; the page follows the
  Session's title policy as the TUI's outer title does.
- Files and folders dropped on a terminal paste their paths into it as ordinary paste, the terminal
  under the pointer taking focus first. Only the preload can read a dropped file's path
  (`webUtils.getPathForFile`), so it listens for the drop before the page does and posts the text to
  its own window, which the terminal that took the drop pastes. The text follows the platform's own
  terminal: on Windows, as Windows Terminal does, a path holding a space is double-quoted and paths
  are joined with spaces; elsewhere, as Terminal.app does, each shell-special character is escaped
  with a backslash and every path ends with a space. A served Web Client takes no file drops.
- The window's place, restored and recorded as [Window lifecycle](docs/window-lifecycle.md)
  describes.
- On Windows, the notification-area icon, from start until quit. Its image is the Orchbee mark
  with a hexagon for the most urgent Agent state, red for a failure, yellow for one needing
  attention, green for a finished turn not yet seen, and outside a release the Dev circle or Test
  square; its tooltip and its menu's first line count the running terminals and those Agents. Left
  click and double-click show and focus the window, hidden, minimized or behind others; the menu
  holds Open Orchbee, then, while terminals run, Stop Session…, which asks in a native dialog before
  it stops every terminal and quits, and last Quit Orchbee. The icons are generated
  into `resources/brand/ico/tray/`, which a Windows package carries as `resources/tray`.
- On macOS, the menu-bar icon, from start until quit, with the same menu, which a click opens. Its
  image is a template image that follows the menu bar's color: the hive ring and, outside a
  release, the Dev circle or Test square; beside it the item's title counts the Agents that need the
  person. The icons are generated into `resources/brand/png/tray/`, which a macOS package carries as
  `resources/tray`. The Dock keeps the system's own menu. The application menu ends with Stop
  Session… and then `Command+Q` Quit Orchbee, and the Window menu holds Open Orchbee while no window
  shows.
- Save Diagnostic Bundle… in the Help menu, and, since the title bar overlay hides the menu bar,
  on Windows also in the taskbar's Jump List of a packaged build and on Linux in the menu entry's
  own menu, as its one action, which the dock and application grid show. It asks
  where to save, starting in the
  Desktop folder, or in the home folder when the system cannot name the Desktop, runs
  `orchbee diagnostics create`, shows the bundle in its folder, and says in a dialog when it failed;
  one save runs at a time and cancelling the dialog writes nothing. The bundle's contents belong to
  the [CLI command](../orchbee/README.md#diagnostic-bundle).
- The menu with the window Actions and, on macOS, Secure Keyboard Entry. Every menu and item, the
  standard ones included, carries a label in the system's preferred language, as the icon's menu
  and Main's dialogs do, so none shows Electron's English default; the standard items keep their
  roles, so the system still performs them. It is installed once the window has opened, because a hidden title bar never opens the menu that would refresh what its
  accelerators may do. On macOS, Workbench owns `Command+W` as Close Terminal: the native Close
  Window item stays clickable but registers no accelerator, so a terminal-close key cannot fall
  through and close the window.

### Native window drag-region projection

- **Problem and scope:** A Desktop user must be able to move a frameless native window from every
  unoccupied point in its `36px` top chrome without losing Tabs, groups, the sidebar toggle, the
  new-Tab control, or operating-system window controls. This projection covers the sidebar top row
  in every sidebar presentation, a horizontal Tab bar, the mobile bar, the open Tab list's top band,
  and the no-Workspace row. Content below the top chrome, Panel and sidebar
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
  | Content outside the horizontal chrome                                                     | Unset          | Retains its existing non-window-drag interaction |

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
  no Desktop drag projection extends below the `36px` boundary. Showing or hiding the sidebar and switching between the wide and mobile layouts recompute the
  projection without starting, committing, or cancelling a native drag. Unsupported or absent
  horizontal chrome is an intentional no-op.
- **Dependencies and verification:** Electron's CSS app-region contract is the only platform
  capability. Synthetic and DevTools Protocol clicks bypass the operating system's hit test and pass
  even where a real pointer drags the window, so evidence comes from a real window: `WM_NCHITTEST`
  returns `HTCLIENT` at a control and `HTCAPTION` at unoccupied chrome.

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
| `ORCHBEE_DESKTOP_DEBUG_PORT`              | Optional; for automation, a loopback DevTools port, `0` for a free one    |

Main runs as development exactly when Electron's default app loaded its entry, as `pnpm desktop`
and `electron dist/main/main.cjs` both do, whatever the executable is named; Electron's
`app.isPackaged` only compares that name with `electron`, so Main never reads it. A packaged
application ships no default app, so no environment variable makes it run as development. It
ignores all but the last three variables, and a packaged release ignores those as well: it runs its
bundled cohort and serves its bundled renderer through the secure `orchbee:` scheme. A window that stays hidden produces no
frames; automation that needs pixels shows it off-screen with `showInactive`. With
`ORCHBEE_DESKTOP_DEBUG_PORT`, Chromium serves the DevTools protocol on the loopback interface; `0`
takes a free port, which Chromium writes to `DevToolsActivePort` in `<data root>/internal/desktop/`.
Any local program can then drive the window with the Session's Client authority, so set it only for
automation.

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

On macOS each profile uses its committed `.icns` from `resources/brand/icns/`, drawn on Apple's 824 px icon grid with the Dev or Test badge over the lower-right corner. The disk image background in `resources/brand/png/dmg/` comes from the same kit. After changing `resources/icon.svg` or a kit definition in the repository's `scripts/brand-assets.mjs`, run `pnpm generate:brand-assets` (requires Google Chrome); `pnpm audit:brand-assets` fails while `resources/brand/` is stale. Every Windows profile uses `resources/brand/ico/orchbee.ico`. A Linux package carries the brand's PNG icons as `resources/linux/icons/<n>x<n>.png` beside its menu entry template `resources/linux/<stem>.desktop`, whose `StartupWMClass` is the application name and whose one action passes `--save-diagnostic-bundle`, and Main gives each window the 256 px icon, which X11 window managers show.

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
Server, Worker, Terminal Host, the observation Capsule, the platform's terminal runtime, and on
macOS and Linux the window launcher that a relaunch runs ([Window Update](#window-update)). The
application carries it because the window's Session may run it: on every system the bundled
Client of a test or release package hands a Session start to the store's active version, and starts
this cohort only when the store holds no usable version
([store start](../orchbee/docs/windows-cli-configuration.md#windows-release-installation)); a
development package runs this cohort. The cohort and the Web resources stay
outside `app.asar` as `resources/cohort` and `resources/dist`.

On every system the test and release commands then make the cohort a signed release directory
before packaging it: `scripts/package-native-release.mjs` writes its `release.json` for the `stable`
channel, the workspace version (which must equal this package's version) and a build id, and
`scripts/sign-release-manifest.mjs` signs it. `--build-id <id>` sets the build id, which the release
workflow passes as `<commit>.<run number>`; otherwise it is `<commit>.local`. `--key-file <absolute
PEM>` or `--key-env <VARIABLE>`, repeatable, choose the signing keys; otherwise a test build signs
with the repository's test key and a release with the key in `ORCHBEE_RELEASE_SIGNING_KEY`.
`--cohort <absolute directory>` instead packages a signed release directory staged and signed
earlier, unchanged, after checking its stem and that its version is the workspace version; it takes
neither a build id nor keys. The release workflow uses it so that only a job of its own holds the
release key. A development cohort is never signed, so the bundled Client of a dev package cannot
install it.

On Windows the test and release commands also write `desktop-files.json` at the root of the
packaged directory once the packager returns, before Setup is compiled:
`scripts/desktop-files.mjs write` lists every other file by path, size and SHA-256 under the release
manifest's file rules and refuses links and names outside its path grammar. Setup installs that
list with the window, and the release tooling's window archive carries the same directory, so the
signed update feed binds every window file
([window archive](../../proposals/platform/automatic-updates.md#window-archive-and-file-list)).
On macOS they write it beside the signed application once signing is done, since a file at the
bundle's root would break its seal; it lists only the bundle, with its links and execute bits, and
the window archive carries the list and the bundle
([macOS window versions](../../proposals/platform/automatic-updates.md#macos-window-versions)).
On Linux they write it at the packaged directory's root as on Windows, marking each file its owner
may run as executable
([Linux window versions](../../proposals/platform/automatic-updates.md#linux-window-versions)).

On macOS the package command then signs the packaged application ad hoc
(`codesign --force --deep --sign -`) and checks it with `codesign --verify --deep --strict`: Apple
silicon runs no unsigned code, and the packager's renames break Electron's own signatures. The
[release workflow](../../docs/RELEASING.md) seals the assembled release bundle with Developer ID
before notarization. Application signing keeps the cohort sealed as resources: the package command
compares every bundled cohort file with the staged cohort by SHA-256 after signing, checks the four
programs against `cohort.json`, and checks a signed release directory against its `release.json`,
and stops on any difference.

All three package commands write beneath their profile-specific directory in `apps/desktop/out/`.
They do not perform Developer ID signing, notarization, publication, or update distribution; the
Windows installer, the macOS disk image and the Linux packages below are built from the test and
release packages.

### Windows installer

On Windows, `pnpm desktop:installer:test` and `pnpm desktop:installer:release` package their
profile as above and then compile `installer/windows/orchbee.iss` with Inno Setup into
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

1. `<store root>\desktop\` holds the window. Setup unpacks the packaged window application into its
   own temporary folder, and step 4 installs it as the window version `desktop\versions\<version>\`
   and places the window entry `desktop\<executable>`, a copy of that version's window launcher,
   which starts the active window version. A window that a release before the window self-update
   left directly in `desktop\` moves on there: its files go once that window has quit, and its
   bundled cohort once no Session runs from it, as does a `desktop.previous` an earlier Setup left.
   A build whose window updates are still switched off, which Setup asks its bundled Client first,
   writes the window application directly into `desktop\` instead: a previous version's folder
   first moves aside to `desktop.previous`, which goes once the new one is in place and comes back
   if the install fails.
2. The Start menu entry `%APPDATA%\Microsoft\Windows\Start Menu\Programs\<name>.lnk` points at
   the window entry with the profile's AppUserModelID.
3. The registration `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<registration>`
   gives Settings › Apps the name, version, publisher `Orchbee`, icon, size and the uninstaller
   `<store root>\unins000.exe`, which Setup writes with `unins000.dat`.
4. Setup runs the bundled Client, `resources\cohort\<client>.exe` below the unpacked window, as
   `install --format json` and then as `--installer-window`, with `ORCHBEE_INSTALL_DIR` set to the
   store root. The window installer helper installs the window as step 1 says whatever `install`
   answered, and a higher window version already active stays active. The store root inherits the access list of
   `%LOCALAPPDATA%\Programs`, which on some profiles lets another account in; that bundled Client
   first replaces the root's list with its private rule when the root holds only what Setup wrote
   there, and leaves `Programs` as it is. It then verifies its signed release
   directory, copies it to `versions\<version>`, activates it unless a higher version is active,
   puts the launcher at `bin\<client>.exe` and adds `bin` to the user `Path`. Setup copies nothing
   into the store itself and edits no `Path`; it only broadcasts the environment change at the end,
   so a terminal opened afterwards finds the command.

Installing a newer setup over an older one replaces it in place: one Settings entry, one Start menu
entry, the new window version active, and the new version active in the store. When window versions
are enabled, Setup stops before it changes anything while a flat layout's window runs: its
`resources\app.asar` lies beside `desktop\<executable>`, which is in use, and that window must quit
first. A running window launcher has no such file beside it and never blocks Setup: its entry moves
aside as `.remove-flat-<pid>-<n>.exe` so the new launcher takes `desktop\<executable>` at once, and
launching Orchbee at the end of Setup triggers a live window switch when a versioned window is
already running; the helper removes no file a process runs from, so a Session an earlier window
started blocks nothing.

A build whose window updates are still switched off stops instead while any program or library
below `desktop\` is loaded, and exits with code 7, saying which program to quit. When the bundled cohort's Server,
Worker or Terminal Host is among them, a Session runs from there, started by an earlier version's
window or by a start that found no usable store version: Setup asks the person to close those
terminals or choose Stop Session…, which is needed only once, because a later window starts its Session
from the store. Otherwise only the window runs from there, and Setup asks the person to quit it
while keeping its terminals running. Sessions that run from the store keep running, because
installing only adds a version directory and replaces `desktop\`; the new window joins them while
they are compatible and otherwise says they are too old.

Removal from Settings › Apps runs the uninstaller, which asks for confirmation and then, in this
order:

1. asks whether to delete the person's data too, naming settings, Sessions and saved passwords,
   with No as the default; a silent removal, or one that finds no Client left, asks nothing and
   keeps the data;
2. runs `<store root>\bin\<client>.exe --installer-uninstall`, with `--delete-data` after a
   Yes, or, when no launcher is left, a flat window's bundled Client or else a window version's,
   and waits for it from `unins000.exe` in the store root; the Client does not count that waiting
   uninstaller as Orchbee running. It ends no terminal: while one is open, or while any other
   program runs from the installation, it changes nothing, data included, and the uninstaller
   shows its reason and stops. Otherwise it removes the store's versions, `bin`, `desktop\`, the
   `Path` entry, the Agent integrations, the Start menu entry and the registration, keeps the data
   root, or after a Yes deletes it with the saved Machine passwords, and leaves `unins000.exe` and
   `unins000.dat`; its own program files go a moment after it exits;
3. removes whatever it installed that is still there and its own two files. The empty store root
   goes with whichever of the uninstaller and the Client's removal of its last files finishes
   last.

A second removal finds no Client and nothing of Orchbee's, and removes only what remains of its
own. A data root (`~/.orchbee` for release) the removal kept stays until
`orchbee uninstall --delete-data`. The uninstaller is the one the last Setup wrote, since an update
writes none: an installation updated in place since a Setup older than the question removes
without asking and keeps the data.

`orchbee uninstall` run in a terminal removes the same things and the uninstaller's two files with
them, so the store root goes too and nothing is left that Settings › Apps no longer lists. That is
a contract between the script and the Client: Setup keeps its uninstaller in the store root as
`unins000.exe` with its log `unins000.dat`, Inno Setup's names for the first uninstaller in a
folder, and the Client removes exactly these two names, only from a store root that holds its
pointer; any other file there stays and the command counts it.

For workflows, `<setup>.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG="<file>"` installs
without a window, and `<store root>\unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
/LOG="<file>"` removes and keeps the data root. Without `/LOG`, Setup still writes
`Setup Log <date> #<n>.txt` in `%TEMP%`, with the exit code and output of each Client command.
Setup exits 0 on success, 7 while Orchbee runs from the installation, 10 when the window
application was installed but `orchbee install` failed, 11 when the window could not be installed,
which it reports instead of a failed `install`, and otherwise with Inno Setup's own codes, such as
2 when cancelled before installing. The uninstaller exits 0 when the removal finished and non-zero
when it stopped.

### macOS disk image

On macOS, `pnpm desktop:installer:test` and `pnpm desktop:installer:release` package their profile
as above and then write `apps/desktop/out/<profile>/installer/<name>.dmg` with `hdiutil`: a
compressed read-only HFS+ image whose volume is named after the application and holds the signed
application beside an `Applications` link to `/Applications`. `hdiutil verify` checks the image
before the command reports it. `<platform>` is the signed `release.json`'s token, `macos-aarch64` or
`macos-x86_64`, so the command names nothing by the host's architecture.

The image opens in a fixed Finder window, which `scripts/mac-disk-image.mjs` lays out without a
Finder session: it creates the image writable, mounts it with `-nobrowse`, writes the volume's
`.DS_Store` itself, and converts the image to the compressed format. The window shows 660 × 400
points of icon view with no toolbar, sidebar, status bar or path bar; 128 px icons with the
application centred at (180, 180) and `Applications` at (480, 180); and the background
`.background.tiff`, which `tiffutil -cathidpicheck` joins from the 1x and 2x pictures in
`resources/brand/png/dmg/`. The picture carries only an arrow, since it cannot switch language,
and is light because Finder draws a window with a background picture in its light appearance, so
the black icon labels read in light and dark mode. The arrow is centred between the icons' visible
art rather than their boxes, so the picture's arrow, the layout's positions and the two icons form
one contract: change the repository's `scripts/brand-assets.mjs` and this package's
`scripts/mac-disk-image.mjs` together.

| Profile | Disk image                                       | Application        |
| ------- | ------------------------------------------------ | ------------------ |
| release | `Orchbee-<version>-<platform>.dmg`               | `Orchbee.app`      |
| test    | `ob-test-<home-digest>-<version>-<platform>.dmg` | `Orchbee Test.app` |

The release workflow's form, from a release directory staged and signed in a job of its own, is
`node apps/desktop/scripts/package.mjs --profile release --installer --cohort <signed cohort>`.
The workflow then signs the application and image again with the Developer ID, has Apple notarize
the image and staples the tickets to both (`scripts/sign-macos-release.mjs`), so a downloaded image
opens without Gatekeeper's Open Anyway steps. A locally packaged image stays ad-hoc signed and not
notarized: `spctl --assess` rejects it and a browser download meets those steps, while a download
without the quarantine attribute opens directly.
The image carries no window list beside the application, so the application's first start brings
its runtime version into the store and makes an empty window store, as the window section above
says, and updates reach it from then on.

### Linux packages

On Linux, `pnpm desktop:installer:test` and `pnpm desktop:installer:release` package their profile
as above and then write an AppImage and a Debian package into
`apps/desktop/out/<profile>/installer/` with `scripts/linux-packages.mjs`, named like the disk image
with `.AppImage` and `.deb`. Both copy the packaged application with its folders and programs 0755
and its other files 0644, since the staged cohort is private to the builder and every user of the
installed copy runs its bundled Client. The AppImage needs `mksquashfs` and an AppImage runtime
named by an absolute path in `ORCHBEE_APPIMAGE_RUNTIME`, which the release workflow builds from a
pinned AppImage/type2-runtime commit; its `AppRun` adds `--no-sandbox` only where the system
restricts unprivileged user namespaces. The Debian package needs `dpkg-deb`, installs the
application under `/opt/<stem>/` with its menu entry and icons under `/usr/share/`, and keeps
Chromium's sandbox: its `postinst` loads an AppArmor profile that lets `/opt/<stem>/<stem>-desktop`
create user namespaces when `apparmor_parser` accepts it, and makes `chrome-sandbox` setuid root
only where user namespaces are disabled outright. Neither stores its own window version; the
[Window Update](#window-update) section says how they reach the store.

### Verification

Run package checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop build
```
