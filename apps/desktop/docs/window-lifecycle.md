# Desktop Window Lifecycle

## Problem and scope

Desktop terminal users need native windows, not one application window that contains every shared
Tab. Electron Main therefore owns a device-local window layout that projects the unchanged shared
Workbench document into one or more native windows. `Mod+N` creates a native window with one
default-current-directory terminal; Tab and Group menus and a cross-window drag can move existing
Tabs without restarting their terminals.

The shared Workbench document remains the authority for Projects, Workspaces, terminal contexts,
Tabs, Groups, Panels, and terminal presentation identities. Browser and mobile clients continue to
observe every shared Tab. The Desktop layout owns only native window identity, per-window Tab
assignment, activation recency, normal bounds, and maximized state. Quick Terminal, always-on-top,
native title-bar replication, workspace-per-window policy, pointer-derived insertion inside a
destination Tab Strip, and renderer or Terminal Host authority over native windows remain out of
scope.

## Inputs and canonical data

Electron Main validates all renderer messages and the recoverable `window-layout-v1.json` file
beneath Electron's product `userData` directory. The Workbench coordinator supplies a trusted,
validated shared-document snapshot. Electron supplies native window events, display work areas, and
screen-coordinate drag points.

The strict durable record is:

```ts
interface DesktopWindowLayoutV1 {
	readonly version: 1;
	readonly primaryWindowId: string;
	readonly nextWindowSequence: number;
	readonly activationOrder: readonly string[];
	readonly windows: readonly {
		readonly windowId: string;
		readonly ordinal: number;
		readonly assignments: readonly {
			readonly terminalContextId: string;
			readonly tabIds: readonly string[];
		}[];
		readonly bounds: {
			readonly x: number;
			readonly y: number;
			readonly width: number;
			readonly height: number;
		};
		readonly maximized: boolean;
	}[];
}
```

Window IDs and ordinals are stable for the record's lifetime and are generated only by Main.
`activationOrder` contains each logical window exactly once, least recent first. Assignment arrays
are canonicalized in shared-document Tab order; they do not create a second shared ordering model.
A valid layout contains at least one logical window even while macOS has no open native window.

Coordinates, dimensions, sequence values, ordinals, collection sizes, and strings are bounded.
Unknown terminal contexts and Tabs are removed during reconciliation. Every current shared Tab is
then assigned exactly once: an unassigned Tab goes to the most recently active logical window, or
to the primary window when no recent target resolves. Duplicate assignment, an unknown primary
window, a partial activation order, malformed input, an unreadable file, or an obsolete version
invalidates the complete record and safely collapses all current Tabs into one new primary layout.
Recovery never mutates the shared document or deletes a terminal. A valid legacy single-window
bounds record may seed the primary rectangle but never supplies identity or assignment state.

## Outputs and effects

The deterministic layout transition returns a canonical next state, a semantic result, and ordered
commands. Results distinguish `completed`, `unchanged`, `blocked`, and `failed`; stale Tab, Group,
window, renderer-generation, or drag-session identities never fall through to another target.
Commands may persist a proposed record, create or close one native window, publish a renderer
projection, focus a window, or provision one shared terminal Tab.

Main serializes layout intents. A user-requested reassignment is prepared and atomically persisted
before it becomes the published in-memory assignment. Persistence or native-window preparation
failure leaves the source assignment unchanged. Document reconciliation is failure-contained: a
new or previously unassigned shared Tab is published in the most recently active window even when
the recoverable preference write fails, so no current Tab becomes invisible in every Desktop
window. A later successful write or process restart converges the record.

Creating a new window first provisions one default-current-directory Tab in the shared document.
That committed Tab initially follows normal unassigned-Tab reconciliation. Main then prepares a
hidden native window and a durable reassignment. If native creation or assignment persistence
fails, the Tab remains in the source or primary window and no terminal is deleted. The window is
published and shown only after the assignment commits.

Moving an individual Tab changes only its device-local assignment; its shared Group membership,
Panel layout, terminal presentation identity, and PTY remain unchanged. Moving a Group resolves the
current shared Group membership and atomically reassigns all of its Tabs, including fragments in
other windows. A destination projects assigned Tabs in shared-document order. A drop does not infer
an insertion index from OS z-order or pointer geometry.

## Actions and interaction projection

Main owns the non-automatable `desktop.window.create` Action. The application File menu and its
`Mod+N` accelerator invoke the same prepared Action path. Existing maximize and full-screen Actions
target the currently active native window:

| Action ID                           | Effect                                                                             | Menu accelerator                          |
| ----------------------------------- | ---------------------------------------------------------------------------------- | ----------------------------------------- |
| `desktop.window.create`             | Create one native window with one default-current-directory terminal               | `CmdOrCtrl+N`                             |
| `desktop.window.toggle-maximized`   | Maximize a normal window or restore a maximized window; disabled while full-screen | `CmdOrCtrl+Shift+M`                       |
| `desktop.window.toggle-full-screen` | Request entry to or exit from Electron native full-screen                          | macOS `Ctrl+Cmd+F`; other platforms `F11` |

The Tab Strip extends its existing reviewed context-menu composition with Move Tab and Move Group
submenus for each eligible existing window plus a new-window target. The renderer sends only stable
identities; Main revalidates current document membership and layout assignment immediately before
commit. Selecting the current window is an intentional no-op. An empty source window closes after
a successful move.

Cross-window drag owns only a reversible source preview until release. Main hit-tests the
screen-coordinate point against current native bounds and activation order. Releasing over another
window requests the same reassignment operation as the menu; releasing outside every eligible
window requests a prepared new-window move. Escape, lost capture, renderer retirement, stale source,
stale target, and failed preparation restore the source preview and commit no assignment.

## Window lifecycle and geometry

Closing a non-last native window prevents the immediate native close, atomically rehomes all of its
Tabs to the most recently active remaining window, persists the layout without the source, and then
closes the source shell. A move that empties one or more source windows follows the same post-commit
close path. These closes never enter application-wide quiescing or stop a terminal.

The last logical window always enters the existing quit and terminal-confirmation gate before its
shell can close. A cancelled or unavailable decision retains both state and shell on every platform.
On macOS, application activation opens the retained primary logical window if the operating system
has left the application running without a native shell. A second-instance request focuses the most
recently active native window.

Only `BrowserWindow.getNormalBounds()` enters a window record. Maximize records restorable normal
bounds plus `maximized: true`. Full-screen is transient and is never restored across process start.
Each restored rectangle is projected through current display work areas: select the greatest
intersection, otherwise use the primary work area, shrink oversize dimensions, and clamp both axes
so the complete window is visible. Geometry captures share the serialized atomic persistence queue
with assignment transitions.

## State model and transitions

`DesktopWindowLayout` is the sole state owner. It has a committed canonical layout, at most one
prepared transition, zero or more native window attachments keyed by logical window ID, and a
monotonic renderer generation per attachment.

| Event                                          | Guard                                            | Commit and commands                                                             | Failure or no-op                                                             |
| ---------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `Load(document, storedLayout)`                 | Application composition starts                   | Normalize, attach/open the primary and persist recovery when needed             | Invalid storage collapses to one primary                                     |
| `ReconcileDocument(document)`                  | Canonical document revision changes              | Drop stale IDs, assign missing Tabs to the recent target, publish, then persist | Store failure retains the safe in-memory projection                          |
| `FocusWindow(windowId)`                        | Window is current and attached                   | Move ID to the end of activation order and enqueue persistence                  | Stale ID is unchanged                                                        |
| `RequestNewWindow(sourceId)`                   | Source generation and active context are current | Command shared default-CWD Tab provisioning                                     | Invalid source is blocked                                                    |
| `TerminalProvisioned(tabId)`                   | Matching request is current                      | Prepare hidden window and reassignment                                          | Stale completion is unchanged                                                |
| `NativeWindowPrepared(windowId)`               | Matching preparation is current                  | Persist proposed layout                                                         | Preparation failure retains the reconciled source assignment                 |
| `AssignmentPersisted(requestId)`               | Matching proposal is current                     | Publish layout, attach/show destination, close newly empty sources              | Stale completion is unchanged                                                |
| `MoveTabs(tabIds, destination)`                | Every Tab and destination are current            | Persist one atomic reassignment proposal                                        | Same target is unchanged; validation or persistence failure preserves source |
| `MoveGroup(groupId, destination)`              | Group still resolves in the shared document      | Resolve all members and use `MoveTabs`                                          | Missing or empty Group is unchanged                                          |
| `CompleteDrag(sessionId, point)`               | Source generation and drag session are current   | Resolve existing/new destination and use `MoveTabs`                             | Cancellation or stale state restores preview                                 |
| `CloseWindow(windowId)`                        | Source is not the last logical window            | Persist atomic rehome/removal, publish, then close shell                        | Persistence failure keeps the window open                                    |
| `CloseLastWindow(windowId)`                    | Source is the last logical window                | Enter the platform application-lifetime path                                    | Cancelled quit retains state and shell                                       |
| `CaptureGeometry(windowId, bounds, maximized)` | Attached window is current and not full-screen   | Normalize and enqueue ordered persistence                                       | Invalid or stale capture is unchanged                                        |
| `RetireRenderer(windowId, generation)`         | Generation matches                               | Revoke only that attachment's brokers and drag sessions                         | Other windows remain attached                                                |

Prepared transition IDs make delayed, duplicate, and reordered effect completions harmless. One
renderer can write the shared document only through the existing multi-client coordinator; the
layout owner never becomes a second Workbench document writer. Renderer projection filters the full
canonical runtime by assignment, and a window with no Tabs in its active terminal context renders
an empty Workbench panel surface rather than another window's active terminal.

## Invariants

- Every current shared Tab belongs to exactly one logical Desktop window after reconciliation.
- Every assignment references a current terminal context and Tab; the primary and activation order
  reference every logical window exactly once.
- A failed or stale move, close, drag, persistence, or native preparation cannot remove the source
  assignment or delete a terminal.
- Tab and Group movement never changes shared Group membership, Panel topology, presentation
  identity, terminal binding, Host session, or PTY identity.
- Closing a non-last window rehomes its complete assignment and never enters application-wide
  quiescing; only the last-window path can request application exit.
- Renderer retirement revokes only the matching window generation's brokers, document attachment,
  and drag sessions.
- No durable record contains full-screen state, and every durable rectangle originates from normal
  bounds and restores fully inside one current display work area.
- Browser and mobile clients continue to receive the complete shared document without Desktop
  assignment fields.

## Correctness and verification

The deterministic normalizer and reducer are production-independent. Example tests cover missing,
valid, duplicate, stale, corrupt, unreadable, legacy-bounds, removed-display, and write-failure
cases. A seeded stateful generator exercises create, focus, move, whole-Group move, reconcile,
close, geometry, cancellation, duplicate completion, and reordered completion traces; it records
the seed and shrinks failures to a replayable command sequence.

`DesktopWindowLayout` checks exclusive assignment, rehome-before-close, failure preservation,
stale-completion no-op behavior, and progress from a prepared operation to one terminal result in a
bounded two-window, three-Tab model. Main/renderer protocol tests validate untrusted messages,
generation retirement, and exact-window routing. Hidden-window Electron system traces cover native
creation, menu and accelerator equivalence, assignment projection, non-last close, restart restore,
off-screen correction, and stable terminal attachment identity.

The focus-stealing native-window trace remains manual-only under the repository's explicit approval
gate. Automated evidence does not claim OS focus, z-order, global pointer delivery, Wayland absolute
positioning, or native behavior on an operating-system family that has not run the same trace.
Electron remains trusted for display enumeration, normal bounds, native events, and screen
coordinate reporting.

| Claim                                | Owner          | TLA+                                                    | Property or core test                                                | Integration or system trace                                | Limits                                                           |
| ------------------------------------ | -------------- | ------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------- |
| `AssignmentsPartitionCurrentTabs`    | This reference | `AssignmentsPartitionCurrentTabs`                       | `preserves an exclusive complete assignment across generated traces` | `projects each Tab into exactly one hidden native window`  | Bounded TLA+ identities; sampled seeded traces                   |
| `FailedPreparationPreservesSource`   | This reference | `FailedPreparationPreservesSource`                      | `keeps the source on every rejected or failed move`                  | `keeps a terminal attached when new-window creation fails` | Atomic file replacement and Electron failure adapter are trusted |
| `NonLastCloseRehomesBeforeEffect`    | This reference | `NonLastCloseRehomesBeforeEffect`                       | `rehomes every source Tab before emitting close`                     | `closes one hidden window without application quiescing`   | OS shell completion is adapter evidence                          |
| `GroupMovePreservesSharedMembership` | This reference | N/A: membership is immutable input to the layout model  | `moves all generated Group members without editing the document`     | `moves a fragmented Group through one protocol request`    | Group semantics are owned by Workbench                           |
| `RendererRetirementIsWindowScoped`   | This reference | `RendererRetirementIsWindowScoped`                      | `ignores stale generation completions`                               | `retires one renderer while another remains attached`      | Process crash delivery is trusted                                |
| `RestoredBoundsAreVisible`           | This reference | Existing `DesktopLifecycle.WindowFullScreenIsTransient` | Existing display projection tests                                    | `restores each hidden window inside a work area`           | Current display topology only                                    |

## Reference scope, rollout, and recovery

Reference discovery is pinned to Kitty source revision
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`. Orchbee adopts the semantic availability of a native
new-window command (`new_os_window`), a current-working-directory variant, and moving a Tab to an
existing or new OS window (`detach_tab`). Orchbee does not claim Kitty-compatible configuration,
window placement, focus, animation, drag hit testing, or timing. This owner contract overrides the
reference when the two differ.

Rollout uses the new recoverable layout file without changing shared-document version 1 or Terminal
Host persistence. A prior Desktop version ignores the new file and opens one window. Corruption,
missing assignments, or explicit rollback collapses all current Tabs into one primary Desktop
window; removing the local layout file loses only window placement and assignment preferences.
Shared Workbench and PTY recovery data remain usable and are never deleted by layout recovery.
