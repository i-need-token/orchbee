# Desktop Window Lifecycle

## Problem and scope

Desktop now owns the minimum durable window lifecycle expected by a terminal user: maximize/restore,
native full-screen toggling, safe normal bounds restoration, and one semantic Action path for menu
selection and menu accelerators.

This contract is limited to the current single local Desktop window. Multi-window identity, moving
tabs between windows, Quick Terminal, always-on-top, native title-bar replication, and renderer or
Host authority over window state remain out of scope.

## Ownership and boundaries

Electron Main is the only owner of native window state and effects. The renderer, the Server's
Workbench, Terminal Host, and projected terminals neither persist nor decide window geometry. The window-state file is a recoverable device-local preference beneath Electron's
product `userData` directory; it is not terminal recovery or shared product state.

The strict version-1 record is:

```ts
interface DesktopWindowStateV1 {
	readonly version: 1;
	readonly bounds: {
		readonly x: number;
		readonly y: number;
		readonly width: number;
		readonly height: number;
	};
	readonly maximized: boolean;
}
```

Coordinates and dimensions are bounded integers. Missing, malformed, unreadable, or obsolete input
falls back to a `1024 × 720` launch. A valid record is projected through the current Electron display
topology: choose the work area with the greatest intersection, otherwise use the primary work area,
shrink oversize dimensions, and clamp both axes so the complete window is visible. This projection
also handles negative display coordinates and a removed monitor.

Only `BrowserWindow.getNormalBounds()` enters the record. Maximize records the restorable normal
bounds plus `maximized: true`. Full-screen is transient and is never restored across process start.
Geometry and maximize events enqueue atomic file replacements in observation order. Window close,
same-process reopen, and application shutdown wait behind the same queue; write failure is a
recoverable preference failure and never blocks terminal or application-lifetime behavior.

## Actions and platform projection

The Main-owned Action runtime registers two stable, non-automatable intents:

| Action ID                           | Effect                                                                             | Menu accelerator                          |
| ----------------------------------- | ---------------------------------------------------------------------------------- | ----------------------------------------- |
| `desktop.window.toggle-maximized`   | Maximize a normal window or restore a maximized window; disabled while full-screen | None                                      |
| `desktop.window.toggle-full-screen` | Request entry to or exit from Electron native full-screen                          | macOS `Ctrl+Cmd+F`; other platforms `F11` |

The application menu and the full-screen accelerator call the same prepared Action path. Maximize
has no key of its own; macOS keeps its menu row, and the window controls and a double-click on the
blank Tab bar maximize everywhere. Native traffic lights
and compositor controls remain operating-system inputs to the same BrowserWindow state; they publish
the normal Electron maximize, unmaximize, and full-screen events consumed by persistence.

Focusing an existing minimized or hidden window first restores and shows it, then focuses it. The
window has no close confirmation: closing it or quitting ends only this Client, never its Session,
so the window's place is captured at close and flushed before the process exits. A window started
hidden for automation neither restores nor records its place.

## Running in the notification area

On Windows and macOS, Desktop shows an icon from the moment it starts until it quits, whether its
window shows, is hidden, or has not opened yet: in the notification area on Windows, and in the menu
bar on macOS ([Running in the menu bar](#running-in-the-menu-bar)), so long-running terminals and
Agents stay one click away.

On Windows a close while terminals run hides the window instead of closing it, so Desktop and its
Client connection keep running with the window's document intact. The page reports a summary over
its background-status port: running terminals, which exclude ended, failed, and unstarted ones, and
the Agents that are working, failed, need attention, or finished a turn not yet seen. Main keeps the
latest summary across a document's replacement and counts nothing itself. A Session that ends
reports none.

`desktop-background-lifecycle.ts` holds the model: the window as `absent`, `shown`, or `hidden`; the
application as running, `quitting`, or `session-ending`; Quit All as `idle`, `confirming`,
`stopping`, or `failed` with its reason; the latest summary; whether the one-time notice has been
shown; and the platform's release, what ends a hidden window that nothing is left to reopen for
(`quit` on Windows, `close-window` on macOS). `transitionDesktopBackground` applies close, show,
closed, summary, quit, session-end, notice, and Quit All events and returns `hide-window`,
`close-window`, `quit`, `show-notice`, `record-notice`, `confirm-quit-all`, `stop-session`, and
`report-quit-all-failure` commands; `projectDesktopTray` derives the icon and
`projectDesktopBackgroundMenu` its menu, which the macOS application menu's background items follow.
`DesktopBackgroundRuntime` runs the model for both platforms behind one surface port,
`desktop-background-surface.ts`, which `DesktopBackgroundTray` implements as the notification-area
icon on Windows and the menu-bar icon on macOS.

- A close while the window shows with terminals running becomes a hide. Without a running
  terminal, the close proceeds and Desktop quits, as it always has.
- A close while the window is hidden cannot come from the window's own controls; it comes from
  another program, such as a graceful `taskkill` or an installer. It proceeds: Desktop quits and
  leaves the Session running, as Quit Orchbee, Keep Terminals Running does.
- No close is turned into a hide once the application is quitting, which every quit passes through
  in `before-quit`, the development runtime's restart included, or once the window has reported
  `query-session-end` or `session-end`. Desktop never delays logoff or shutdown.
- The icon is created when the background runtime starts, before the first window opens, and is
  removed only once the application is quitting. The end of the person's Windows session keeps it,
  so Desktop stays reachable when another program cancels the logoff. A hidden window therefore
  always has an icon to reopen it from.
- The icon's image marks the most urgent Agent state the page reports, whether or not the window
  shows, and no state while Quit All stops the Session. Its tooltip names the application and, while
  a page reports, repeats the menu's status line.
- The menu's first, inert line counts the running terminals and the Agents that failed, need
  attention, or finished a turn not yet seen, or says that Quit All is stopping them; it is absent
  while no window's page reports, as before the first document does. Open Orchbee follows, always,
  as Docker Desktop's and 1Password's icon menus keep their open entry. Quit Orchbee, Keep Terminals
  Running and Quit All… follow; Quit All is offered only while a page reports running terminals,
  the application is not ending, and no Quit All is under way, and an item that is not offered is
  left out rather than disabled.
- When the last terminal ends while the window is hidden, Desktop quits, unless a Quit All is
  stopping the Session, which ends every terminal before it completes, or has failed.
- The first hide shows a notice from the icon, which Windows withholds during Do Not Disturb. Only a
  notice Windows reports as shown is recorded in `internal/state/desktop-tray.json`, so a withheld
  one is shown at a later hide. Each distribution profile records its own.
- Clicking or double-clicking the icon or its notice, Open Orchbee, starting Orchbee again, and a
  notification clicked while the window is hidden all show and focus the window: a hidden window
  shows, a minimized one is restored, and a shown one comes forward. The page asks Main for it,
  because a page cannot show a window Main hid. Quit Orchbee, Keep Terminals Running quits Desktop
  and leaves the Session running. A switch through Update quits the same way once the window entry
  has started ([Window Update](../README.md#window-update)).
- Quit All… asks every time, in a native dialog that needs no window, with Cancel focused and on
  Esc; its body counts the terminals that stop and the Agents working in them. Cancel changes
  nothing. Only a confirmation stops the Session: Main chooses a stop identity and asks the page
  over the background-status port, the page sends `workbench.session-stop` once through its Server
  connection and answers, and Main then reads `operation status` every half second. Desktop quits
  only once a stopped receipt proves the stop, which also ends every Client, an attached TUI
  included. A stop that was refused, never sent, failed, or not proven within 75 seconds shows the
  window with the reason in plain words, records the reason in the diagnostic log, and nothing
  quits.
  Once the Server gave up reconnecting a lost Worker's terminals, each terminal's Quit All… button
  opens the same confirmation: the page asks for it over the background-status port, and the
  bootstrap's `quitAllAvailable` offers the button only to a window whose Main keeps Quit All.
  While the stop runs, the menu holds only its status line and Open Orchbee.
- Only a release icon has a fixed GUID, so Windows remembers whether it is shown in the taskbar and
  macOS keeps the place the person dragged it to in the menu bar. Test and Dev run from changing
  paths, to which Windows ties an unsigned executable's GUID.
- A window started hidden for automation and Linux keep no icon and close as before; macOS follows
  [Running in the menu bar](#running-in-the-menu-bar).

## Running in the menu bar

On macOS the same model runs with the `close-window` release, so Desktop follows the Dock
convention of Terminal, iTerm2, and Ghostty: the application keeps running after its window closes,
whether or not terminals run, and never quits by itself.

- A close while the window shows with terminals running hides the window, keeping its document and
  Client connection, as on Windows. A full-screen window leaves full screen first and hides once it
  has, so no empty Space is left behind. Without a running terminal the close proceeds and the
  window closes, as before.
- When the last terminal ends while the window is hidden, or a Quit All failure is dismissed with
  none running, Main closes the hidden window and Desktop stays in the Dock.
- The menu-bar icon shows from start until quit, as the Windows icon does, with the same menu.
  Clicking it opens that menu, as menu-bar icons do, rather than the window. Its image is a
  template image, which macOS draws in the menu bar's own color: the hive ring, a hexagon
  in its lower-right corner while an Agent failed, needs attention, or finished a turn not yet seen,
  and outside a release the environment's circle or square in its upper-left corner. The status
  line names which states; the fixed status colors stay with the Windows icon and the window.
- The Dock keeps what macOS gives every application: clicking its icon reopens the window, and its
  menu is the system's, with the window list, Options, Hide, and Quit, which keeps the terminals
  running. Orchbee adds no Dock menu items, because they would repeat the menu-bar icon's menu.
- The application menu names `Command+Q` Quit Orchbee, Keep Terminals Running; it stays the native
  quit and asks nothing, because nothing ends with it. Quit All… follows it and is enabled as in the
  icon's menu. The Window menu holds Open Orchbee while no window shows.
- Clicking the Dock icon, starting Orchbee again, Open Orchbee, and a notification clicked while
  the window is hidden show and focus the window. Open Orchbee also makes Orchbee the active
  application, because a menu-bar menu leaves the application that was active before it.
- Quit All asks, stops the Session, reports a failure, and quits exactly as on Windows.
- The first hide shows a notification, the macOS counterpart of the icon's notice, that names the
  running terminals and says how to reopen Orchbee from the Dock and reach Quit All from the
  menu-bar icon. Only a notification macOS reports as shown is recorded, in the same
  `internal/state/desktop-tray.json`; one that is never delivered is shown at a later hide, and
  clicking it reopens the window.

## State model and invariants

The durable state is `normal bounds + maximized`. Runtime observation additionally distinguishes
normal, maximized, full-screen, and unavailable window states. Inputs are load, geometry completion,
maximize, unmaximize, full-screen entry/exit, close, reopen, and application shutdown.

The owner preserves these invariants:

- no durable record contains full-screen state;
- every durable rectangle originates from normal bounds, never the maximized or full-screen frame;
- every restored rectangle is fully contained by one current display work area;
- queued writes preserve capture order and a completed flush observes the last successful capture;
- unavailable or full-screen-incompatible Actions produce no hidden effect;
- window state and Actions never mutate a Workbench document, terminal binding, Host session, or
  projected attachment.

## Evidence limits

Electron and the operating system remain trusted for display enumeration, `getNormalBounds()`,
maximize events, and native full-screen transitions. Every platform is covered by deterministic
model and adapter tests only; Wayland compositors may constrain absolute positioning even though
Orchbee always supplies a safe requested rectangle.

On Windows, a hidden window has been checked in a real process for the title-bar overlay,
the enabled window Actions, the second start, and the absent record. A visible window's restore and record
were not exercised there; they share the owner and store above.

A real development Desktop showed its icon while the window was shown and kept it after a close hid
the window, on Windows, where the shell reported the notification-area icon, and on macOS, where
the window server listed the menu-bar item. The icon's menu and notices were not exercised there.
