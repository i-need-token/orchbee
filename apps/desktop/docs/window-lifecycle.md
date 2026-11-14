# Desktop Window Lifecycle

## Problem and scope

The local Desktop client owns one Electron window but previously launched it at a fixed size and
relied on incidental operating-system controls. Desktop now owns the minimum durable window
lifecycle expected by a terminal user: maximize/restore, native full-screen toggling, safe normal
bounds restoration, and one semantic Action path for menu selection and menu accelerators.

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

On Windows a close while terminals run hides the window instead of closing it, so Desktop and its
Client connection keep running with the window's document intact. The page reports a summary over
its background-status port: running terminals, which exclude ended, failed, and unstarted ones, and
the Agents that are working, failed, need attention, or finished a turn not yet seen. Main keeps the
latest summary across a document's replacement and counts nothing itself. A Session that ends
reports none.

`desktop-background-lifecycle.ts` holds the model: the window as `absent`, `shown`, or `hidden`; the
application as running, `quitting`, or `session-ending`; Quit All as `idle`, `confirming`,
`stopping`, or `failed` with its reason; the latest summary; and whether the one-time notice has
been shown. `transitionDesktopBackground` applies close, show, closed, summary, quit, session-end,
notice, and Quit All events and returns `hide-window`, `quit`, `show-notice`, `record-notice`,
`confirm-quit-all`, `stop-session`, and `report-quit-all-failure` commands; `projectDesktopTray`
derives the icon.

- A close while the window shows with terminals running becomes a hide. Without a running
  terminal, the close proceeds and Desktop quits, as it always has.
- A close while the window is hidden cannot come from the window's own controls; it comes from
  another program, such as a graceful `taskkill` or an installer. It proceeds: Desktop quits and
  leaves the Session running, as Quit Orchbee, Keep Terminals Running does.
- No close is turned into a hide once the application is quitting, which every quit passes through
  in `before-quit`, the development runtime's restart included, or once the window has reported
  `query-session-end` or `session-end`. Desktop never delays logoff or shutdown.
- While the application runs, the icon exists exactly when the window is hidden and either
  terminals run or a Quit All is stopping or reporting its failure; it is created when the window
  hides and removed when it shows, so a hidden window always has an icon and a shown one never does.
- When the last terminal ends while the window is hidden, Desktop quits, unless a Quit All is
  stopping the Session, which ends every terminal before it completes, or has failed.
- The first hide shows a notice from the icon, which Windows withholds during Do Not Disturb. Only a
  notice Windows reports as shown is recorded in `internal/state/desktop-tray.json`, so a withheld
  one is shown at a later hide. Each distribution profile records its own.
- Clicking or double-clicking the icon or its notice, Reopen Orchbee, starting Orchbee again, and a
  notification clicked while the window is hidden all show and focus the window; the page asks Main
  for it, because a page cannot show a window Main hid. Quit Orchbee, Keep Terminals Running quits
  Desktop and leaves the Session running.
- Quit All… asks every time, in a native dialog that needs no window, with Cancel focused and on
  Esc; its body counts the terminals that stop and the Agents working in them. Cancel changes
  nothing. Only a confirmation stops the Session: Main chooses a stop identity and asks the page
  over the background-status port, the page sends `workbench.session-stop` once through its Server
  connection and answers, and Main then reads `server status` every half second. Desktop quits
  only once a stopped receipt proves the stop, which also ends every Client, an attached TUI
  included. A stop that was refused, never sent, failed, or not proven within 75 seconds shows the
  window with the reason, naming the stop identity when a stop was recorded, and nothing quits.
  While the stop runs, the menu holds only its status line and Reopen Orchbee.
- Only a release icon has a fixed GUID, so Windows remembers whether it is shown in the taskbar.
  Test and Dev run from changing paths, to which Windows ties an unsigned executable's GUID.
- A window started hidden for automation, macOS, and Linux keep no icon and close as before.

Deterministic tests of the model cover the close decision, closes never intercepted while ending,
the quit when the last terminal ends, Quit All stopping only after a confirmation, a cancel that
changes nothing, a failure that returns to the shown window, and the icon invariant over every
reachable state. The tray adapter, the notice, the dialogs, the stop request and its status reads,
and the native events have no automated evidence.

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

Pure state tests cover ordered bounds capture, transient full-screen exclusion, display selection and clamp boundaries. File tests cover missing/valid/invalid/unreadable state, atomic replacement, serialized writes and failure. Owner tests cover safe projection of the loaded record, ordered geometry and maximize capture on each platform's event set, listener removal, and continuation after a failed save. Native maximize, full-screen, restart, and off-screen behavior has no automated evidence: the manual native-window system test retired with the Desktop Electron suite. No separate TLA workflow proof is claimed.

## Evidence limits

Electron and the operating system remain trusted for display enumeration, `getNormalBounds()`,
maximize events, and native full-screen transitions. Every platform is covered by deterministic
model and adapter tests only; Wayland compositors may constrain absolute positioning even though
Orchbee always supplies a safe requested rectangle.

On Windows, a hidden window has been checked in a real process for the title-bar overlay,
the enabled window Actions, the second start, and the absent record. A visible window's restore and record
were not exercised there; they share the owner and store above.
