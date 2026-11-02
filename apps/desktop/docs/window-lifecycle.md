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
| `desktop.window.toggle-maximized`   | Maximize a normal window or restore a maximized window; disabled while full-screen | `CmdOrCtrl+Shift+M`                       |
| `desktop.window.toggle-full-screen` | Request entry to or exit from Electron native full-screen                          | macOS `Ctrl+Cmd+F`; other platforms `F11` |

The application menu and its accelerator call the same prepared Action path. Native traffic lights
and compositor controls remain operating-system inputs to the same BrowserWindow state; they publish
the normal Electron maximize, unmaximize, and full-screen events consumed by persistence.

Focusing an existing minimized window first restores and shows it, then focuses it. The window has
no close confirmation: closing it or quitting ends only this Client, never its Session, so the
window's place is captured at close and flushed before the process exits. A window started hidden
for automation neither restores nor records its place.

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
