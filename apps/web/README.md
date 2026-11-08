# @orchbee/web

Renderer entry for the Orchbee Workbench, which a Desktop window runs as a Server Client.

## Server Client path

Desktop runs this application as a Server Client; [Native runtime topology](../../proposals/platform/native-runtime-topology.md#web-and-desktop-client-migration-plan) holds the migration steps that remain. `workbench-client/server-connection/` holds the transport-neutral connection: `ServerConnection` multiplexes requests and subscriptions over a `ServerCarrier`, one lane per terminal, and `openServerTerminalProjection` presents one Server terminal through the existing `ProjectedTerminalTransport`, whose only addition is the optional `takeControl`. A connection whose control was taken over becomes read-only without a failure once its next input or resize is refused; the Server sends no notice before then. The surface then shows View only with a Take control button, and the terminal context menu offers Take control among its panel rows. Both run one Action over `takeControl`, which nothing repeats on its own: a grant resizes the terminal to the pane and returns the keyboard to it, and a refused or unknown outcome is reported for the person to repeat. Poll stays the only way to take a record: the Server's `ViewerRecordReady` hint polls that terminal at once, a lost hint recovers at a 250 ms fallback, and the 8 ms deadline in the [Workbench client runtime](../orchbee/docs/workbench-client-runtime.md) applies before the connection's first hint and while the terminal is owed a full frame. Each terminal keeps its own Poll in flight, so unlike the TUI's single-Poll scheduler it needs no focus order: a focused terminal waits behind at most one Poll per sibling. Selection stays with the Host: gestures and commands answer with its `SelectionChanged` or its own failure, so the renderer retires a stale gesture, and `ServerSelectionCopy` assembles an export's polled chunks, acknowledging each, into the bytes the page writes through its Clipboard API. A program's bell and OSC 52 clipboard write request arrive through Poll like any record, and `ServerTerminalEffects` hands them to the renderer, which shows the bell and writes the clipboard through that API. Its decision then only answers the request, so the Server hears nothing of it, and a request that arrives while one waits is dropped. A Desktop window opens a link itself, as the TUI does, so a link never reaches the projection: the Server path offers no file links, and an address goes to the person's browser through Main. Clear and reset go to the Server in order with the terminal's input, as its transient `clear` and `reset` controls, and each reaches the Host through an attachment of its own rather than the controller's. Revealing a find match that lies in history reads the viewport at that absolute row, which goes to the Server as `read-viewport-rows` with an absolute origin; the Host serves it only at the revision it names. A projection in control reports the window's colors as the Server's `host-colors`, as the TUI reports its outer terminal's, once after each grant and again when they change. The window reports its colors under every terminal display theme, resolved as the Worker resolves a launch: the selected light or dark theme, with the system's appearance where the application theme is System. The Worker applies them to each terminal it controls, so a program's color queries answer with what the window paints.

`workbench-client/server-workbench/` puts the Workbench on that connection. `ServerWorkbenchSession` observes one Session's published snapshot and submits commands one at a time, and `projectServerSnapshot` turns each snapshot into the publication the React Workbench's stores hold, so every shared fact comes from a publication and rendering creates no terminal. A Presentation whose launch the Server recorded as failed, or one bound to no terminal, never reads as starting, because nothing here launches or retries it. Its surface says that creation failed, naming the program when the launch named one, or that no terminal is available, and offers New Tab and its Panel's Close in place of the terminal; it takes the keyboard where the terminal would have, so a key typed as it appears runs nothing. A surface opens whichever terminal its Presentation is bound to in the latest snapshot. It takes control only when no other Client holds it and never takes it over for a layout change; a surface being replaced releases its projection first, and the opens of one terminal run one at a time, so a Client never meets its own control. A released projection still sends the input it accepted before its lane closes, for at most half a second, and the next open of that terminal waits for it, so a key typed as the layout changes reaches the terminal ahead of the keys after it. A surface that only views a terminal cannot change its size, so its first painted frame makes it ready whatever that size is, where a surface in control waits for a frame that fits. A terminal's Host colors are those the Worker set at launch from the Session's preferences, and the Server has no theme transaction, so no renderer joins a theme presentation. As in the TUI, the window instead paints every terminal's default-coded cells in the selected theme's colors, through the renderer's client-only `defaultColorOverride`. It resolves them as the Worker resolves a launch, with the system's appearance in place of an outer terminal, so a theme change recolors open terminals at once while their programs keep querying the Host's colors. Selection and search keep the renderer's dark overlay colors. Tabs and terminals are named by the TUI's rule in [native Tab labels](../orchbee/docs/native-tab-labels.md): a console's default title, which is the path of its launched program, names that program, such as `PowerShell` or `Command Prompt`, and keeps a command that cmd runs, while other titles show as written once one elevation prefix and one activity mark are dropped. A terminal that shows no title takes the program the Server launched for its binding, else the program of its failed launch, else its Tab's place, such as `Tab 2`. A name a person chose still comes first. A language change projects the snapshot again and names the latest titles again, so every name changes at once. Unlike the TUI, a terminal that sets no title but reports its directory is named by that directory, a step the shared Workbench takes. `ServerUserPreferencesClient` reads and saves the Session's shared preferences. As in the TUI, a State stream of the settings read then carries every later change, whether another Client saved it or the Server found it in the preference file, so an open Settings page shows a TUI's save without a refresh. A preference source names its record's content rather than an ordered revision, so a snapshot observed around a save can be older than its answer: the stream opens again after each save, and its first snapshot, read after the save, has the last word. Resnapshot stays the Server's recovery from an overrun. A stream that closes makes preferences unavailable until a refresh observes them again, because later changes would otherwise go unseen. `ServerTerminalLaunchProfileClient` and `ServerTerminalDefaultSettingsClient` present the launch profiles and scrollback among those preferences through the Settings page's Desktop controls, as [terminal settings](docs/terminal-settings.md#desktop-terminal-launch-profiles) describes. `ServerAgentIntegrationClient` presents Agent integrations through the same page: each Refresh, enable, or disable is one job on the Session's Worker, the writer the CLI uses, as [terminal settings](docs/terminal-settings.md#desktop-agent-integrations) describes.

`createServerWorkbenchAuthority` answers the Workbench's write port with Server commands for Project, Workspace, Tab, and Panel structure and for closing. Each command is decided from the snapshot its revision names, when its turn comes, and settles once a publication carries what it reports, so what a caller reads next already holds it. A refused or unknown command is never repeated, including one refused for a stale revision, apart from the two exceptions below. A created Workspace is named as the TUI names it, from the directory's final name, avoiding every label in that snapshot. New Workspace opens where the TUI's opens: it first asks the Server, for at most five seconds, where the terminal the focused Panel shows is now when the command names no row (Search, the sidebar's fixed New workspace button, or a key), or the focused Workspace's first terminal when a Project row asks, through the [current-directory read](../orchbee-server/README.md#workspace-current-directory-reads). A Panel the Workspace no longer holds is not named, so its first terminal answers. Without an answer it opens where the Workspace's first terminal was last seen, otherwise in that Workspace's directory, otherwise in the home directory.

`server-workbench-tabs.ts` words Tab changes as `tabs.configure` and `tab.create`. A drag's boundary is a place in the order it started from, so a release whose order changed before its turn is not sent and its Tabs return. A group update states the whole group, so what did not change comes from the snapshot it will replace. A new Tab is named by its place until something better shows, as every Client names it, and a copy takes its source's name and whether that name follows the terminal. Tabs start at a default terminal geometry, which the surface corrects once it attaches. A Tab started with a chosen launch profile is resolved against the catalog Settings last observed, and only a profile still saved exactly as shown is started. A custom executable travels as its literal program and arguments. The default system shell travels as no launch at all, so the Worker resolves it as it resolves every default; a default changed in that instant starts the new default. The window cannot resolve a system shell itself, so any other system shell starts nothing.

`server-workbench-panels.ts` words Panel changes. A new Panel is named by its place, as every Client names it, and starts in the directory its source last reported, otherwise wherever the Server starts one. A ratio is the first slot's share, kept within the 10% to 90% the Server accepts and rounded to five decimal places, so repeated steps leave no trailing digits in shared state. It is sent with the ratio it moved from. As in the TUI, a refusal for a stale revision sends the same move again as a new request while the split still holds that ratio, for at most five seconds from the first refusal. Evening a Tab or resetting its splits is one command per split, each decided from what the last one published; the first failure stops the rest, and what already committed stays.

`createServerWorkbenchClose` carries a close to its end, because the Server accepts one without spending a revision and removes the target later. A Workspace, one Tab, one Panel, and a set of Tabs first ask to close only if they run nothing, which the Server decides, and a new request closes the target once the person confirms; a set asks once for all its members. The other exception to never repeating a command is also the TUI's: a conditional, Workspace, or set close refused for a stale revision is sent again, at most four more times, only while its target's record is unchanged, a set's under a new operation, and a `confirmation-required` that described a target that has since changed is asked once more. An accepted close then reads its operation every 50 ms for at most 50 seconds and settles when a publication carries its removal, and until then the window leaves its target out of view (a Tab, a Panel of several, a Workspace, or each Workspace of a Project, at the acceptance; see [publication and Client View](docs/publication-and-client-view.md)); running out ends only the watching, and the Server's work continues. A Workspace close, and a Project close whose first answer has not already stopped, calls the caller's `markLeft` right after its Workspaces leave, so the window says Closed and returns the keyboard at the acceptance instead of at the settlement. The close of one Tab, its last Panel's, or its Resume calls it as the Tab leaves, which returns the keyboard then and, as in the TUI, lets a close still running a second after it started say Closing, while a quicker one and every completion say nothing. A Panel that is not its Tab's last settles on acceptance. A Tab's last Panel takes its Tab, so when the snapshot the Server decided at held that Panel alone in its Tab, the close is followed as the Tab's and a stop stays for Resume. Several Tabs close as one Server operation, `workbench.tab-set-close`, which closes the members one at a time, each by its own Close, and stops at the first that does not close; the window starts it once, at the revision it shows, and reads it as it reads a Project close. A member a stopped set left keeps its own close's stop for Resume, as a Tab's close does. A close the Client cannot follow to an end reports what is known and repeats nothing; an unknown outcome raises no question, since confirming could close the target twice. While the window follows the progressing close of a Tab, its last Panel's included, `ServerLeaving` leaves that Tab out of the projection, as the TUI leaves it out of its strip, and lists it as withheld. The selection moves to the next Tab still shown, else the previous one, the other Tabs keep the numbers their automatic names carry, and the Tab returns with its report when its close ends any other way than its removal. Every member of a set leaves in one change once the Server accepts the set, as in the TUI, so a selected member gives way at once to the next Tab outside the set, else the previous one; the members the Server leaves open return once the closed ones have left, and the selection stays where it moved. A set's progress counts a member closed only once the authority no longer holds it, withheld or shown. `WorkbenchProjectWorkspaceRuntime` keeps the client-local view of each withheld Tab, its focused and maximized Panel, each Panel's displayed child terminal and its Panel focus order, and a Tab that returns takes it up again; a Tab that is neither withheld nor published takes its view with it. The last followed close of each Tab that ended without closing it, because it stopped, its status turned unknown, or the window lost sight of it, is kept, at most 16 per window, until a later close of that Tab settles or the Tab is gone, and one that stopped where the Server may continue it, cleanup blocked or awaiting removal and marked resumable, is kept for Resume. A new Close of that Tab joins the same operation while the Server holds it open, so, as in the TUI, the Tab leaves only once a read finds that close progressing, and otherwise reports the same stop. Resume asks the Server to continue that operation under the attempt its status named, and on a Resumed or Stale answer the Tab leaves and the close is followed as any accepted close. A Stale answer means another attempt replaced that one, so the window follows the close as it is now. A rejection forgets the stop; a denial, a busy Server, or a lost answer keeps it, and after a lost answer a later Resume of the same attempt answers Stale.

`createServerWorktrees` gives the Workbench the TUI's Worktree commands, using the TUI's requests and rules, so every Client offers, starts, and follows the same Server operations. Discovery has ten seconds in all: it asks where the Workspace's first terminal stands now, or the terminal a named Panel shows, and reads the repository there when that differs from the Workspace directory, otherwise the Workspace's own. A Panel the Workspace no longer holds is not asked about, so only the Workspace's directory remains. A failed probe reads as no repository; any other failure leaves the checkouts unavailable. Removal is offered only for a linked, non-bare checkout spelled as the Workspace's directory that Git associates with this Workspace, on the Worker holding that directory. A command acts on the latest discovery of its source, the Workspace or the Panel it names, so a checkout that discovery no longer lists, or a removal whose path changed, fails as a changed source without reaching the Server. A creation that names a Panel showed the person nothing, so it reads that Panel's source first and stops as not a repository, or with the checkouts unavailable, when that read fails. A creation names no branch or base, places the checkout by repository name and operation under the configured Worktrees directory, or leaves the Server to place it beside the repository when none is configured, and never trusts the repository. Every command starts one operation and reads it from 250 ms, doubling up to 2 seconds, for at most 120 seconds; a read the Server cannot serve keeps waiting, and an operation it cannot follow to an end is reported as unknown with its ID, never started again. Paths compare by the TUI's spelling rule: byte for byte on POSIX, and on Windows apart from separators, a trailing separator, and ASCII case. A removal that stopped where it may continue, before its 64th attempt and with nothing about its Workspace's restoration unknown, is kept for Resume, at most 16 per window, and every discovery of its Workspace reports the latest one, whatever Git lists.

`createServerWorkspaceLocations` tells the sidebar where each Workspace stands by the TUI's rules, so a row changes at the same moment in every Client. It reads only while the sidebar shows, and keeps what it learned for the next time it does. A Workspace's source is where its first terminal last reported standing, otherwise its directory, and a Workspace with neither is never asked. A round asks the Server for each Workspace's Git facts in turn, within five seconds in all, then pauses three times as long as it took, from two to four seconds; a failed read retries after half a second, doubling up to thirty. A Workspace whose source has not been asked yet gets one read of its own as soon as no read runs. An answer counts only at the revision of the snapshot it follows; one at another revision is dropped and the round goes on, where the TUI fails the whole read. A row keeps its facts while its source stays. A moved row takes the last answer for its new source, from at most 64 kept, and otherwise holds its earlier name, folder, and facts until Git answers, a complete round finds no answer, the Workspace is renamed, or a second passes from the first move. Facts no read has refreshed for ten seconds are withdrawn, and the folders stay. A folder under the home folder Main reports shows as `~`, matched as the TUI matches it: apart from separators, a trailing separator, and ASCII case.

`ServerWindowTitle` names the window as [the TUI's outer title](../orchbee/docs/windows-terminal-title.md) is named: the Server renders a title set from the CLI, else the configured template, by default `{hostname}: {workspace}`, and sends it only to its foreground Client, the one that attached or gained focus last. A window behind another Client keeps the title it showed, and a cleared title with no template shows `Orchbee`, which is also the page's title before the first one arrives. `bindServerWindowTitle` listens with the window's focus and the TUI's display context: the active Workspace's and Tab's labels as the Server holds them, the focused terminal's own label, and the title its program set, named as its Tab is and only while it comes from the terminal shown. Each change of focus or context opens a new stream before it closes the old one, because closing first would make the Server forget the window and bring it forward as a newly attached Client. Losing focus listens anew too, since only focus turning on brings a window forward. Electron names the Desktop window after its page, so the title shows in the taskbar and the window switcher, never in the hidden title bar.

`ServerClientView` publishes the window's Client View to the Server as [the TUI's control lane](../orchbee/docs/windows-client-control.md) publishes its own, so the CLI's `workspace focus`, `tab focus`, `panel focus`, and `panel zoom` find the window and reach it. A lane of its own owns the view, and its invalidation stream opens before the first Synchronize. Each Synchronize is its own exchange of at most two seconds, because it can carry a control back, and goes when the view changes, when an answer is ready, or when a commit may have queued a control; nothing runs on a timer. The heartbeat advances with every Synchronize and the view revision only when the view changes, and an answer travels with the view it was drawn in. A control that arrives after the window moved past the view the Server admitted it against is refused as stale without running. A failed or refused Synchronize closes the lane, which retires the view and settles a control left unanswered, so the window publishes again only once it reconnects. `bindServerClientView` reports the window ready with the Workspace, Tab, and Panel it shows and runs a delivered control through the Workbench's `showWorkbenchClientTarget`. A control naming a newer Workbench waits at most five seconds for that publication. The answer goes once React has committed the change, in the task after it, and claims a partial change once the window moved for the control; what the window does not run yet answers unsupported.

`ServerClientView.bind` binds a view the window showed to the Server's record of it, as the TUI binds a queued command: an acknowledged record of that view binds at once, otherwise the next record that reports it does, and a view the window left before reporting it answers stale. A lane that ends, or no record within twice the Synchronize deadline, answers unavailable. `ServerConfiguredCommands` holds the Session's configured-command list and invokes a command. It follows the configuration's state stream and reads the list again whenever the key profile changes, since every command's identity binds that profile; an unanswered read keeps the last list. An invocation goes once and waits the TUI's ten seconds, and a lost answer is unknown, never repeated. `bindServerConfiguredCommands` offers every command except a popup to the Workbench's `configuredCommands` and runs one at a time from the Panel the window shows, through the sample and binding that `bindServerClientView` hands out. The source names the terminal that Panel shows when the Server holds its binding, and none otherwise, as in the TUI. The window reports neither a frame nor an attachment, so the Server sizes a new Pane at its 80×24 default. A `not-found` or `conflict` answer reads the list again, and a Pane command shows the new Panel once its publication arrives, waiting at most five seconds.

`listServerAgentSessions` and `getServerAgentSession` read the Server's Agent sessions over the window's connection, as [the TUI's Agent board](../orchbee/docs/agent-board.md) reads them. A list walks at most six pages of 100 sessions, ended ones included, and starts over once when the Server rejects its cursor or its revision moves, so one list never joins two revisions. `bindServerAgentBoard` holds the Workbench's `agentBoard` by the TUI's rules. It reads only while the board is open, for at most three seconds, never within two seconds of the last read's end, and one request at a time; a closed board drops what its reads still owe. A cut-short walk replaces the rows only when no whole one has, and at most 600 rows show. A jump waits for the request slot in place of a waiting read, then reads its session again for at most three seconds: an ended session, one without a navigable terminal, and one whose terminals this window does not hold stop with the TUI's notice. Otherwise `showWorkbenchPresentation` shows the first of its terminals in the shown Tab, then a visible one, then by identity, and the board closes.

`desktop-client/server-workbench/` composes these for every Desktop window. The one command this path sends on its own is the first Workspace of a Session holding no Workspace and no terminal, by the startup rule in [native directory startup](../orchbee/docs/native-directory-startup.md), and only from the first document of a window whose restore lost nothing: a later document follows a person's own actions, and an empty Session may be what they chose. A window has no start directory, so that Workspace opens in the home directory. Select Folder and New Folder use Main's native folder picker through the document's second port, and a new Project still comes from the Server's Workspace launch. The Server settles that launch only once its publication carries the Project, so a new folder's Project counts as opened when this document shows it, with no flush to wait for.

A window whose connection ends, including one that receives a publication it cannot apply, keeps its last view and shows Disconnected with a Reconnect button. As in the TUI, nothing reconnects on its own. Reconnect reloads the document, so Main starts the Session again when it stopped and then connects. The status sits at the bottom center, clear of the Workbench notices and a terminal's View only or ended status at the bottom end. A window that never reaches its Session shows Session Unavailable in place of the Workbench, in the system's language and appearance because the Session holds the preferences, with a strip to move the window by. Try Again is its only effect. When Main reports why the start was refused, the page says what the TUI and CLI say for that refusal, naming Desktop where they name Orchbee: a Session that needs recovery names `orchbee server diagnose`; a Desktop too old for the running Session, or a Session too old for this Desktop, names both versions; a Session from before version checks, which states none, is to be stopped by the Orchbee that started it; and a Session of another environment, or a development build whose protocol differs, says it cannot join. Each command the copy names is inline code that never wraps. Recovering or stopping the Session stays an explicit command.

## Composition boundary

The React Workbench lives in `src/workbench`; [Workbench](docs/workbench.md) describes the shell, its Actions, and its reviewed experience records.

A Desktop window reaches Agent integrations through its Server connection, as jobs on the Session's Worker that project a ten-Agent inventory and run explicit Settings refresh, enable, and disable requests.

`apps/web` is the single product UI that Electron's renderer loads; Desktop must not maintain a parallel renderer UI. Only a Desktop window starts the Workbench: `composition/entry.ts` calls `startWebRenderer`, which runs `startDesktopServerWorkbench` and through it `createServerWorkbenchRuntime` only when `resolveWebRuntimeMode` returns `desktop`. Any other page, including the `pnpm web` development server opened in a browser, renders nothing; Browser ingress to real Sessions waits for remote networking, as the [migration plan's decisions](../../proposals/platform/native-runtime-topology.md#decisions) record. `createServerWorkbenchRuntime` composes projected-terminal presentation, search, Actions, and the Workbench authority over a transport-neutral `ServerConnection`, with no Electron, `MessagePort`, or application-lifetime dependency. The runtime provides each projected renderer session with one stable `TerminalPresentationConfigurationSource`. Its initial complete value opens the surface, and later complete values update that same session and attachment without changing React capability identity or reopening projection. The default source retains unbalanced padding, fixes `fontSize` at 13 to match Ghostty's default 13-point setting, and replaces the source-neutral package face with the bundled Ghostty-aligned JetBrains Mono 2.304 variable faces plus Symbols Nerd Font 3.4.0 fallback. Renderer startup registers those immutable assets before opening any surface, so the renderer never depends on machine-installed fonts. The Desktop composition replaces that fallback with one client-local settings controller that restores a strict complete preference record, exposes the same stable presentation source, and publishes complete runtime updates without reopening projection.

Desktop renderer disposal retires its local lifetime before cleanup. A React unmount failure cannot
prevent the remaining routing, settings, and transport cleanup attempts. These composition and
Workbench cleanup paths share `runApplicationCleanup`: repeated or reentrant disposal is inert, and a
single original error or an ordered aggregate is reported after all independent steps have been
attempted.

Desktop reaches its Session through the Server Client path above, and Main serves only native Project Folder acquisition beside that connection. The shared terminal surface retains measured cell geometry, complete background coverage, clipped direct graphics, bounded bell indication, connection notices, and terminal-outcome precedence. Workbench payloads come from `@orchbee/workbench-client-protocol`; Electron capability contracts come from `@orchbee/desktop-client-protocol`.

Desktop's Server runtime supplies Agent observation through `ServerTerminalAgentSource`, which reads
the Server's per-terminal Agent reports with `panel-agent` `states` State streams of at most 64
terminals each, in topology order. A membership change reopens only the streams whose terminals
changed. A failed answer or a closed stream keeps its last Agents for 10 seconds, a capacity close
reopens after 5 seconds, and a frame that breaks the protocol ends the observation. Workbench
presentation, Agent notifications through the terminal-feedback controller, and the
trailing-blank-row policy read that source.

For a Host-backed visual terminal, the shared surface does not open projection until it has loaded
the current configured fonts and measured a positive mounted viewport. It sends the resulting exact
columns, rows, physical cell size, padding-aware grid, and document DPR through its projection request, then pins that presentation configuration for the first renderer. A
resize observation or animation frame wakes an unmeasurable surface; release aborts that wait. No
timer starts the child and no 80x24 visual fallback exists.

A Presentation whose launch failed, or that the Server bound no terminal to, opens no projection.
Its surface reads Terminal creation failed, with the shell or executable the Server reported, or
Terminal unavailable when no terminal is bound, beside Close and New Tab; nothing retries the
launch or binds a terminal by itself. The Server removes what it closes, so a surface never waits
on a local close.

A terminal that exits or fails at run time keeps its Tab and final frame, as the Server keeps it
for every Client; the projected attachment rejects further PTY input, resize, clear, and reset
control. The surface names the end in the TUI's words, Terminal exited or Terminal failed, beside
a New Tab button at its bottom end, where View only otherwise sits, and drops its input controls.
The Server publishes nothing when a terminal ends, so only the window whose projection saw the
Host settle that exact terminal learns of it, and only while the Presentation stays bound to it;
its close questions then count that terminal among the ended ones whose output closing discards.
The Server closes a Panel whose only terminal is a shell that exits with code 0, so for up to a
second after such an exit the window keeps the final frame without naming the end, as the TUI
does, and a program the Server keeps names its end after that second. The surface names an end
only once its Presentation has ended too, which leaves that timing to the composition.
A connection that stays lost reads Disconnected, and an automatic recovery reads Reconnecting….

An unexpected attachment loss freezes the last completed canvas and accessible rows while releasing
its transport and interaction ownership. The shared surface retries a fresh attachment after 250,
500, and 1,000 milliseconds, and replaces the frozen projection only after the candidate paints its
first frame. Exhausting that budget leaves the honest last frame visible and exposes one localized
manual Retry action; each attempt owns and releases exactly one projection request. A typed
`renderer-backpressure`, `host-transport-backpressure`, or `host-connection-unavailable` failure settles the same attachment-local
transport-unavailable signal after preserving its original operation and correlation diagnostics;
the surface never retries the rejected command or keeps using the causally incomplete attachment.
Transport loss also races font and GPU startup explicitly: it aborts that candidate before startup
can stall recovery, and promotion checks the candidate's current transport availability both before
and after its first-frame wait.

The recovery controller reserves each opening attempt before notifying observers or acquiring its
request. Synchronous retention, resumption, or disposal cannot overwrite a replacement attempt,
leak a late request result, or continue promotion after its owner retires. Resource ownership moves
before cleanup callbacks run, including input unsubscription and frozen-frame replacement. Disposal
attempts every owned cleanup once, preserves cleanup failures, and respects listeners cancelled
during the final notification. Controlled recovery-controller tests cover these reentrant traces,
late results, and throwing cleanup alongside the bounded retry policy; they do not model-check
arbitrary callback programs or replace real renderer and transport integration evidence. The mounted
surface uses the same cleanup discipline for its paste review, mobile-input and control
subscriptions, and controller.

Active-Tab handoff uses the same frozen-session discipline without keeping inactive terminal DOM or
attachments mounted. When a surface becomes retained, it captures only its exact last-painted
semantic presentation, freezes the canvas, and releases transport, input, resize, request, and
attachment ownership. The application-scoped `TerminalPresentationSnapshotCache` retains those
display-only snapshots in a byte-accounted `32 MiB` least-recently-used budget and clears them when
the shared client runtime is disposed. An incoming surface may paint a cached snapshot before its
fresh live attachment produces a frame. That cached draw satisfies only Workbench's display-ready
barrier; input, pointer interaction, and `autoFocus` remain disabled until the candidate paints its
first live frame and satisfies activation-ready. On a miss or oversized snapshot, the outgoing
frozen layer remains visible until live or honest failure content is ready. Returning to a
still-frozen surface reuses its canvas for the display barrier while attaching a fresh candidate
behind it. Superseded or inactive sessions cannot acknowledge frames or publish metadata, and each
surface still releases its per-terminal Canvas, atlas, buffers, and renderer after handoff.

Initial active surfaces also report display readiness from the same painted-frame or non-canvas
outcome evidence. Workbench uses that witness to acknowledge Agent unread activity only after the
terminal is actually displayed. Retained surfaces cannot supply this read witness.

The active terminal adapter reports `onTerminalEdit` only after typed editing input reaches its
transport port successfully. Text, paste and content-editing keys retire the Client-local completed
Agent display; focus, mouse, cursor movement, copy, rejected input and inactive layers cannot.
Workbench owns this presentation override and leaves the Server's Agent observations and notifications intact.

An attachment feature failure remains distinct from transport loss and terminal outcome. The shared
surface retains its operation, code, retryability, Host correlation ID or client request sequence as
bounded diagnostics and applies the matching Terminal consequence. Background input acknowledgement,
history, and resize failures never create a global terminal-content notice: rejected input remains in
the bounded diagnostic and required transport-recovery paths, history feedback belongs to the find
surface, and resize remains in its bounded retry and diagnostic path. Connection, runtime, and
explicit user-action failures retain localized visible feedback. The application does not
automatically replay terminal input, control, clipboard, or navigation effects.

The shared adapter exposes the last completed draw checkpoint as
`data-projected-terminal-rendered-generation` and
`data-projected-terminal-rendered-revision`. Both values come from one renderer telemetry event and
identify the packed Host commit actually drawn after any animation-frame coalescing. This diagnostic
contract is identical in Electron clients; it is not a second acknowledgement
to Terminal Host. Runtime presentation diagnostics additionally expose request, committed, and
drawn epochs on the same surface. They distinguish independently ordered font preparation from the
single local commit and completed current repaint without creating another attachment lifecycle.

Mounting a projected renderer never moves DOM focus. Workbench retains the active product control;
pointer activation, an explicit terminal focus request, and closing the Find or terminal-text viewer
surface are the only adapter paths that move focus into the hidden terminal input. The projected
root contains that input and both surfaces, so moving among them does not publish a terminal focus
transition.

### Coarse-pointer terminal interaction

| Field                   | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and scope       | A terminal surface on a coarse, non-hover pointer must expose terminal input, scrolling, and essential keys without redesigning the complete Workbench. Whole-shell responsive layout, native selection handles, multi-touch, pinch zoom, Panel gestures, and configurable key rows are excluded.                                                                                                                                                                                                                   |
| Inputs and dependencies | The application consumes the current projected-attachment input readiness, terminal textarea focus, localized messages, `(hover: none) and (pointer: coarse)`, and semantic key intents. It never infers Host modes or encodes terminal bytes.                                                                                                                                                                                                                                                                      |
| State and ownership     | One mounted projected-terminal renderer owns `MobileTerminalInputState`: attachment availability, native keyboard focus, and a subset of one-shot `control`/`alt` modifiers. The React surface only renders that prepared state.                                                                                                                                                                                                                                                                                    |
| Modifier transitions    | Tapping Control or Alt toggles that one-shot modifier. The next eligible toolbar key, physical key, or independent single printable ASCII native input consumes all armed modifiers before publication. Composition start, paste, multi-character or non-ASCII committed text, blur, read-only downgrade, attachment replacement, recovery, and disposal clear them without transforming or replaying input. A blocked or failed send still consumes the snapshot so stale modifiers cannot affect a later command. |
| Key row and focus       | The canonical `projected-terminal.mobile-input-bar` appears only for the coarse non-hover media condition and contains Show/Hide Keyboard, Escape, Tab, Control, Alt, and four Arrow controls. Its key controls retain textarea focus and use the exact active attachment revision; the keyboard toggle reflects actual textarea focus and never claims that the operating system displayed a keyboard. Unavailable or read-only input disables input-producing controls.                                           |
| Invariants and failure  | Touch scrolling never arms a modifier; controls never publish to a replaced attachment; no mobile state enters the shared Workbench document, browser storage, or Terminal Host. Missing browser focus capabilities degrade to an unchanged layout and typed unavailable input rather than synthetic success. Desktop pointer and hardware-keyboard behavior remain unchanged.                                                                                                                                      |
| Verification and limits | Deterministic state tests cover every modifier transition and replacement. Renderer and React tests cover native capture ordering, revision pinning, localization, and focus retention. A controlled headed coarse-pointer trace remains required to verify media-conditioned visibility, touch tap/scroll, grid resize, and both locales; real iOS and Android keyboard appearance remains manual release evidence.                                                                                                |

Each attachment publishes only its own focus claim and projection demand. Terminal Host combines
eligible claims for a shared terminal, so one attachment losing focus or visibility cannot emit a
terminal-wide `FocusOut` while another eligible claim remains; reactivation requires a fresh claim.
Every opening or explicit reopening of that compact find surface replaces an empty or retained
query with the bounded preview of the current active Host selection when one is available; without
one, it preserves the current query. Search-owned active ranges remain authoritative Host selections
rather than a second application selection model. Each such invocation focuses and selects the
complete query, while Escape from any find control closes the surface and restores terminal focus.
Match options live inside the query field, ordinary search progress uses an
inline localized count, Left and Right Arrow move between option toggles, and pointer option changes
return focus to the query. Only actionable failures, timeouts, or retained-history notices expand the
toolbar with complete localized feedback.

The Find controller publishes selection-seeded opening as one complete snapshot. Query, option,
and lifecycle transitions fence later search effects across synchronous subscriber and driver
callbacks, so an obsolete call cannot restart or clear newer work. Independent notices and
same-query refocusing preserve pending search admission. The
[controller tests](src/workbench-client/terminal-find-surface/__tests__/TerminalFindController.unit.test.ts)
cover replacement, close, disposal, reentrant reopening, and those admission-preserving updates.

`Ctrl+Shift+G` opens the last shell-marked command output in an application-owned modal viewer. One
replace-latest Action prepares an exact generation-and-revision plain-text document through the
bounded general-buffer lane; stale, evicted, alternate-screen, unavailable, or over-limit reads
never publish partial output. The prepared document remains immutable while literal
case-insensitive search, Enter/Shift+Enter navigation, native text selection, and the separate
`terminal.output.copy` Action operate locally. The viewer traps focus, restores terminal focus on
close, and deliberately does not launch an external pager or re-render ANSI styling. See the
[behavior contract](../../packages/terminal/docs/command-output-viewer.md)
and the `projected-terminal.text-viewer` experience component.

`Ctrl+Shift+H` opens the complete retained primary-screen history through that same private viewer.
The exact snapshot projector joins soft wraps, preserves hard row boundaries, records the captured
visible-top offset, and publishes either one complete document or a typed empty/unavailable result.
Command-output and scrollback preparation replace each other before publication; their immutable
documents retain source-specific copy Actions. See the
[scrollback-viewer contract](../../packages/terminal/docs/scrollback-viewer.md).

Prompt navigation and text-viewer preparation each own a replace-latest coordinator for one Action
registration lifetime. Ownership changes before cancellation invokes observers, so a replacement
started by an abort listener remains current and cancellable. Disposal closes admission before
aborting current work; an already-cancelled caller cannot displace an active request. The controlled
[navigation](src/workbench-client/projected-terminal-actions/__tests__/createProjectedTerminalPromptNavigationCoordinator.unit.test.ts)
and [preparation](src/workbench-client/projected-terminal-actions/__tests__/createProjectedTerminalTextViewerPreparationCoordinator.unit.test.ts)
tests cover reentrant replacement, disposal, and cancelled admission.

After a projected frame commits, the renderer forwards its complete nullable title and
current-directory metadata with the exact `TerminalPresentationId`. The shared client runtime
normalizes that observation with the trusted terminal-machine hostname from the client-session
bootstrap and applies it to the matching Workbench presentation. Natural titles and accepted
current CWD remain transient. The Server holds each Tab's fallback and chosen name; Workbench
shows the chosen name, or else the focused terminal's natural title, launched program, or
fallback. The page never derives terminal trust from its own URL hostname.

Suspicious paste text opens one localized review dialog owned by the shared projected-terminal
surface. It presents a bounded, control-visible preview of the retained original text and identifies
multiline input, controls that will be replaced, and an embedded bracketed-paste terminator. Cancel
publishes nothing; approval sends the sanitized text together with the mode that was reviewed, while
Terminal Host rejects a mode change before encoding or queue admission.

Each projected surface registers the Workbench-owned minimal terminal Action catalog into the
viewport's application runtime and scope: Copy, Paste, Select All, Find, Find Next, Find Previous,
Show Last Command Output, Copy Command Output, Show Scrollback, Copy Scrollback, Scroll to Bottom,
Clear, and Reset, plus Take control where the transport offers it and another Client holds the
terminal. Availability is
synchronous and remains authoritative when a prepared invocation executes. Context-menu and custom-shortcut Paste read text only through an
injected client capability. An idle platform-default paste chord preserves native DOM delivery so
the terminal adapter can apply its plain-text-first and image-only `Control+V` policy without
reading image bytes. When a manual selection Copy or another Paste Action is still in flight, the
default chord uses the Action path; Paste waits for the exact Copy operation to settle before it
reads the current clipboard, then reuses the same terminal paste sanitation and review path. Every
execution settles with a typed
Action result; completed, unchanged, blocked, and failed outcomes that need perceivable feedback
use the shared localized terminal status surface. These immediate terminal Actions add no
confirmation step. An unavailable or unbound Apple Command shortcut remains native instead of
being suppressed.

The stable headed Canvas 2D witnesses `terminal-parity/IN-07-pre-input`,
`terminal-parity/IN-09-action-scope`, `terminal-parity/IN-11-context`, and
`terminal-parity/IN-12-actions` compose the production projected surface with its real containing
Workbench runtime. They assert that buttonless application motion preserves retained history and
Host selection while remaining reported, plus claimed/unclaimed and available/unavailable shortcut
handling, application-mouse ownership, exact pinned context and stale-link rejection, the complete
minimal catalog, synchronous availability, typed outcomes, no new confirmation, and both `en-US`
and `zh-CN` feedback. Their clipboard and external-open effects are captured; they do not claim an
operating-system menu, clipboard, or URL handler.

The projected attachment stream can replace input authority live only after its environment owner
commits an exact next capability revision in Terminal Host. The shared renderer immediately makes
the existing textarea read-only, invalidates composition, native settlement, paste review, held
key/mouse/focus state, and recalculates terminal Action availability from the live terminal. Find,
history, selection, copy, retained output, and approved links remain available. A later upgrade
admits only future input carrying the new revision; it does not replay pre-downgrade DOM or queued
work. Electron uses the same renderer contract, and no renderer command can request its
own upgrade. The Server composition admits one automatic focus activation for the focused Panel,
including its active child Presentation. Initial focus observes an already held grant; focus return
records activation. A read-only target may request takeover once. Window blur, a modal or target
replacement aborts unsubmitted work, and refusals or uncertain outcomes never retry automatically.
Already controlling Clients retain ordinary input admission and their picture. Periodic Server
Observe replies supply current controller, sequence and recent-focus facts, including Server-owned
promotion; the GUI never invents a local focus-order fallback. The manual Take control Action
remains available. Grants arrive through the same next capability revision as manual takeover.
An already submitted transfer cannot be recalled: its receipt is reconciled even after focus loss,
without another transfer. This cancellation boundary is a vetoable decision. An opening projection
waits for the renderer's subscription before automatic activation, so its capability change cannot
be published into an absent listener; closing releases that wait without submitting a transfer.

The stable headed Canvas 2D `terminal-parity/IN-10-read-only` witness controls exact attachment
capability revisions and observes immediate local repair, retained search/selection/history/copy,
zero stale input commands, and only fresh upgraded input. Terminal Host's owner-local tests prove
the authoritative rejection and repair ordering; this Browser fixture does not grant a renderer
capability mutation or claim native permission behavior.

The packaged `orchbee:` origin and Electron User-Agent token select Desktop mode, so a loopback development renderer inside Electron is Desktop too; Desktop authority arrives only through the validated Server-connection announcement from preload. Any other page resolves `unsupported` and renders nothing, and so does a page carrying an `orchbee-runtime` query parameter.

## Navigation

The Desktop composition uses one TanStack Router over memory history, because a window has no address bar. Canonical Workspace routes use `/workspaces/$workspaceId[/tabs/$tabId]`, independent of nullable Project membership. Old Project-qualified Workspace paths are not admitted. Group-only links resolve current membership; unknown Workspace/Tab identities remain unavailable. Navigation pushes history; canonicalization replaces it. Renderer focus recovery restores the exact surviving Workspace, Tab, Panel and child before explicit route reconciliation. See [`docs/navigation-routing.md`](docs/navigation-routing.md).

## Settings

Desktop Settings includes Agent Integrations. Startup, search, and category navigation perform no
mutation; only an explicit switch action changes provider configuration, as one job on the Session's
Worker. Remote setup remains manual-only.

General offers System, English and Simplified Chinese. Desktop reads the same confirmed device
preference as TUI. System follows declared locale
inputs, while explicit locale wins. Confirmed changes update mounted product copy without reload.

An unreadable shared preference record also shows a persistent main-window notice using the Settings recovery wording. Activating it opens Settings; a confirmed read or Reset all clears it.

The common Settings action opens an application-owned full-window overlay rather than navigating to another route. The Workbench subtree becomes inert, but its React tree and every projected terminal attachment remain mounted. Desktop common preferences use the shared device store with source-bound drafts, confirmed saves, conflict and unknown-outcome recovery. Host typography, pixel padding and opacity remain renderer-local. On macOS Desktop, a separate strict local record selects whether neither, left, right, or both Option keys act as terminal Alt; the default is off and accepted changes update every mounted projected terminal without replacing its attachment. Sixty-seven frequent Workbench and focused-terminal shortcuts, including application-wide font-size increase/decrease/reset, creating a terminal Tab in the exact displayed terminal's current directory, four directional Panel-focus commands, previous/next cyclic Panel focus, ten exact positional Panel-focus commands, visible Panel focus and swap, four directional Panel-movement commands, proportional Panel equalization, keyboard Panel resize mode, circular active-Tab movement within its pinned or unpinned section, visible terminal Hints, Show Last Command Output, and Show Scrollback, use a separate strict local override record; accepted changes update the mounted Workbench and every projected terminal without replacing an attachment or connection. Standard defaults reserve unshifted terminal control sequences, while runtime Action availability remains authoritative before any key is consumed. Application theme remains separate from terminal display theme, and terminal overrides never mutate Host metadata, palette, cursor, VT, or OSC state. Desktop additionally injects a semantic shell-profile control over the Session's shared preferences. Its explicit editor can create, rename, select a system or custom executable, preserve literal argument order, select the default, delete, and confirm reset; every accepted change affects only terminals created after the shared catalog commits. Shell integration settings own a separate default-off device-local completion-reminder policy whose notifications never include command text; reminders need OSC 133 command boundaries, which the Terminal Host's PowerShell integration supplies on Windows, while Zsh, Bash, and Fish emit them only through the user's own configuration. The Remote development category is a read-only projection: it explains the `xterm-256color` baseline, exposes one constant shell diagnostic, distinguishes observed environment and terminfo outcomes, and states that Orchbee does not silently forward extra environment variables or modify the remote host. Opening, searching, or switching to this category starts no process, writes no terminal input, and persists no remote data. See [`docs/terminal-settings.md`](docs/terminal-settings.md) and [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md).

The exact headed `terminal-parity/SU-09-theme` witness holds a prepared Host result and its
revision-tagged frame before one release, then observes the terminal defaults, palette, cursor,
overlays, pixels, and application surface publish one matching revision. It uses the controlled
runtime-presentation Canvas fixture and does not claim a persisted settings migration, a native
operating-system theme event, or a renderer-owned terminal palette.

## Desktop composition

Run the Workbench locally with `pnpm desktop`, which serves this renderer and opens the development
Desktop window on it; `pnpm web` alone only serves the renderer for Electron to load. Electron loads
the Web development URL and discovers its adapter from the User-Agent. Packaged Electron serves this
production build from the standard secure `orchbee://app` origin; `apps/desktop` owns packaging and
resource delivery.

Locale preference, automatic fallback resolution, and shortcut-platform setup belong to the Desktop
composition.

Electron Desktop injects native existing-Folder acquisition into the shared application. `Select Folder…` uses the operating-system picker, and local Project Folder attachment uses that same capability. `New Folder…` selects a native parent and explicitly creates one absent child, then observes the existing Project Action, exact canonical publication and Workspace activation. The existing route coordinator reports its accepted current target separately from pending navigation, so creation never acknowledges an unrelated route. Created or uncertain filesystem outcomes are preserved for Select Folder recovery; no new provisioning or filesystem API is introduced.

An OSC 52 write never shares the selection-copy fast path. The shared renderer automatically processes
each exact bounded UTF-8 request without presenting a confirmation surface. Its decision repeats the
exact attachment, Host sequence, and text; the page's platform-effects adapter reports success only
after its local Clipboard API succeeds. A failed client-local effect reports failure and consumes
the request; the adapter retains its secure-context and transient-user-activation requirements.
Attachment replacement, terminal retirement, and renderer disposal suppress late decisions; request
text and decisions never enter browser storage or the shared Workbench document. Where the page's
Clipboard API admits the write, a Host-admitted terminal program can replace the clipboard without
another user gesture; restoring per-request confirmation is the rollback boundary. Each attachment Bell reports
the opaque terminal
identity and Host sequence to the Workbench-lifetime coordinator. Only the first admitted delivery
for that terminal sequence produces the short tokenized visual surface cue; duplicate, stale,
reconnected, and replayed deliveries produce no effect, including when distinct attachment streams
interleave. Optional sound and operating-system notification callbacks are separate explicit
application opt-ins, are absent by default, and fail independently after admission. Neither effect
requests focus. The current OBTF progress sample remains a quiet bottom-edge determinate or
indeterminate indicator, and both visual presentations respect reduced motion.

Terminal link hover reads only generation/revision-pinned Host rows. When application mouse
tracking owns the surface, ordinary context intent stays terminal-owned; Shift explicitly returns
that intent to the product. The projected surface captures only the typed link target and its exact
generation, revision, link epoch, row, and column when the Workbench menu opens. Every composition
keeps URL activation within the `http`/`https`/`mailto` policy. A detected file or directory target
stays inert: no composition carries a file-effects capability.
The prepared Workbench Action revalidates the complete context immediately before selecting its
environment-owned effect. A changed context settles as a localized blocked result without opening
the stale target. Command-click on Apple platforms or Control-click elsewhere uses the same typed
request and revalidation path. Host-projected text and hyperlink metadata remain data, never an
effect capability.

The headed Canvas 2D witnesses `terminal-parity/FS-05-file-links` and
`terminal-parity/FS-06-url-policy` compose the production projected surface, Workbench Action, and
Browser effect. They prove that `http`, `https`, and `mailto` alone expose hover, context, and direct
activation, while automatic FTP text, unsupported explicit metadata, explicit file metadata,
detected paths, and a direct file-effect request remain inactive. The controlled fixture uses
synthetic generation-pinned Host row windows and a captured Browser opener; it does not exercise an
operating-system URL handler or claim filesystem existence.

The headed Canvas 2D witness `terminal-parity/FS-19-local-file-links` enables the optional local
capability against the same real projected surface with a fake local effect. It proves
relative-path resolution and modifier opening. Its captured opener does not launch an
operating-system application, automate Finder, or grant Browser filesystem access.

The headed Canvas 2D witness `terminal-parity/FS-07-keyboard-hints` uses that same production
surface and bounded row fixture. `Mod+Shift+O` labels visible URL, path, file-location, and hash
targets; label keys never reach terminal input, selecting the hash inserts its exact text, selecting
the URL reaches the captured Browser opener, and Escape removes the overlay without an effect. The
witness does not claim off-screen discovery, filesystem/editor activation, custom matchers, or
operating-system URL delivery.

The headed Canvas 2D witness `terminal-parity/FS-08-command-output-viewer` uses the production
surface with synthetic OSC 133 prompt rows. `Ctrl+Shift+G` opens the exact retained command output;
search and visible match navigation stay inside the immutable document, Copy reaches one captured
plain-text writer with the same content, and Escape restores terminal input focus. The witness does
not claim external-pager execution, ANSI preservation, complete scrollback, or operating-system
clipboard mutation.

The headed Canvas 2D witness `terminal-parity/FS-09-scrollback-viewer` uses the production surface
with a bounded synthetic retained-history snapshot. `Ctrl+Shift+H` opens all retained primary-screen
text at the captured visible top; search, match navigation, source-specific Copy, Escape focus
restoration, and unchanged attachment identity use the production viewer and Actions. The witness
does not claim history beyond Host retention, ANSI preservation, an external pager, or operating-
system clipboard mutation.

The headed Canvas 2D witness `terminal-parity/LC-05-lan-capability` composes the production
projected surface, Workbench Copy Action, Host-selection-content path, and Browser effect. On the
loopback test origin it observes the real Chromium secure-context and transient-user-activation
signals: no activation disables Copy without a Host request, while a trusted click admits one Host
request and one captured plain-text writer call. It then proves that one rejected writer receives no
retry, and that dynamically missing and controlled insecure capabilities disable the Action, invoke
neither Host export nor writer, and retain localized feedback. The fixture captures rather than
mutates the operating-system clipboard and controls the capability-loss and insecure-context probes;
it does not claim browser permission-UI behavior or LAN transport security.

Browser-owned request and presentation identifiers use `crypto.getRandomValues`; runtime code does not depend on the secure-context-only `crypto.randomUUID` API.

## Unit verification

Run `pnpm --filter @orchbee/web test` for the unit suite. On Windows, Vitest uses at most four workers (or the available CPU count if smaller), bounding process creation and filesystem contention when other workspace suites run concurrently. All files, assertions and deadlines remain enabled; other platforms retain the runner defaults.

## System verification

`pnpm --filter @orchbee/web test:system` runs the headed Canvas 2D witnesses in `system-tests/` against the `*.system-fixture.ts` pages, which one rsbuild development server compiles. It needs an installed Playwright Chromium, and no routine gate runs it.

## Server-owned Agent completion presentation

The compact terminal Agent source preserves the Server's `completion` fact alongside raw state,
physical binding and provider sequence. Completion-only publications invalidate the cached row
without advancing that sequence. `WorkbenchAgentAttention` owns only per-Client unread versions;
it no longer clears Done on a local terminal-edit callback or infers completion from transitions.

One display projection maps awaiting-input Idle to Done and acknowledged explicit Done to Idle.
Sidebar, Tab strip and Panel header use that projection, and the Agent board overlays the same
fact on its idle/succeeded rows only when the terminal generation matches. Provider failure,
approval and active-work states retain their semantics. Board open and completion publications
reproject cached rows immediately, without waiting for its next list. Raw observations still
feed notifications and the focused-Panel-only unread rules. Missing completion stays unknown/none;
reading or changing focus never consumes it. Only the Server's accepted-input boundary does.

## Unreadable shared preference recovery

An `invalid-source` Settings read presents built-in defaults with the dedicated localized
unreadable-file notice. They carry the virtual `unreadable` recovery source, never a confirmed
save. Ordinary updates remain unavailable; explicit Reset all stays available with confirmation.
Only the complete all-null patch uses the recovery source. The Server rechecks storage under its
writer lock, refuses a concurrent repair, and returns a real digest after confirmed replacement.
Reload and pushed Settings snapshots observe later repairs without silently repairing the record.
The [shared preference contract](../../crates/workbench-protocol/docs/user-preferences.md#explicit-recovery-from-a-refused-record)
owns recovery and admission. Other unavailable/save/unknown-outcome states retain their existing
wording and recovery controls.
