# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Server Client path

Desktop runs this application as a Server Client; [Native runtime topology](../../proposals/platform/native-runtime-topology.md#web-and-desktop-client-migration-plan) holds the migration steps that remain. `workbench-client/server-connection/` holds the transport-neutral connection: `ServerConnection` multiplexes requests and subscriptions over a `ServerCarrier`, one lane per terminal, and `openServerTerminalProjection` presents one Server terminal through the existing `ProjectedTerminalTransport`, whose only addition is the optional `takeControl`. A connection whose control was taken over becomes read-only without a failure once its next input or resize is refused; the Server sends no notice before then. The surface then shows View only with a Take control button, and the terminal context menu leads with Take control. Both run one Action over `takeControl`, which nothing repeats on its own: a grant resizes the terminal to the pane and returns the keyboard to it, and a refused or unknown outcome is reported for the person to repeat. Poll stays the only way to take a record: the Server's `ViewerRecordReady` hint polls that terminal at once, a lost hint recovers at a 250 ms fallback, and the 8 ms deadline in the [Workbench client runtime](../orchbee/docs/workbench-client-runtime.md) applies before the connection's first hint and while the terminal is owed a full frame. Each terminal keeps its own Poll in flight, so unlike the TUI's single-Poll scheduler it needs no focus order: a focused terminal waits behind at most one Poll per sibling. Selection stays with the Host: gestures and commands answer with its `SelectionChanged` or its own failure, so the renderer retires a stale gesture, and `ServerSelectionCopy` assembles an export's polled chunks, acknowledging each, into the bytes the page writes through its Clipboard API. A program's bell and OSC 52 clipboard write request arrive through Poll like any record, and `ServerTerminalEffects` hands them to the renderer, which shows the bell and writes the clipboard through that API. Its decision then only answers the request, so the Server hears nothing of it, and a request that arrives while one waits is dropped. A Desktop window opens a link itself, as the TUI does, so a link never reaches the projection: the Server path offers no file links, and an address goes to the person's browser through Main. Clear and reset go to the Server in order with the terminal's input, as its transient `clear` and `reset` controls, and each reaches the Host through an attachment of its own rather than the controller's. Revealing a find match that lies in history reads the viewport at that absolute row, which goes to the Server as `read-viewport-rows` with an absolute origin; the Host serves it only at the revision it names. The projection reports `unsupported-message` for what it does not carry yet: image preview. A projection in control reports the window's colors as the Server's `host-colors`, as the TUI reports its outer terminal's, once after each grant and again when they change. The window has colors to report only under the Host terminal theme with the System application theme, the one choice that launches a terminal following its controller. They are the Worker's light or dark terminal defaults for the system's appearance, so a following terminal answers color queries with what the window paints.

`workbench-client/server-workbench/` puts the Workbench on that connection. `ServerWorkbenchSession` observes one Session's published snapshot and submits commands one at a time, and `projectServerSnapshot` turns each snapshot into the document the React Workbench renders, so every shared fact comes from a publication and rendering creates no terminal. A Presentation whose launch the Server recorded as failed, or one bound to no terminal, never reads as starting, because nothing here launches or retries it. Its surface says that creation failed, naming the program when the launch named one, or that no terminal is available, and offers New Tab and its Panel's Close in place of the terminal; it takes the keyboard where the terminal would have, so a key typed as it appears runs nothing. A surface opens whichever terminal its Presentation is bound to in the latest snapshot. It takes control only when no other Client holds it and never takes it over for a layout change; a surface being replaced releases its projection first, and the opens of one terminal run one at a time, so a Client never meets its own control. A released projection still sends the input it accepted before its lane closes, for at most half a second, and the next open of that terminal waits for it, so a key typed as the layout changes reaches the terminal ahead of the keys after it. A surface that only views a terminal cannot change its size, so its first painted frame makes it ready whatever that size is, where a surface in control waits for a frame that fits. A terminal's Host colors are those the Worker set at launch from the Session's preferences, and the Server has no theme transaction, so no renderer joins a theme presentation. As in the TUI, the window instead paints every terminal's default-coded cells in the selected theme's colors, through the renderer's client-only `defaultColorOverride`. It resolves them as the Worker resolves a launch, with the system's appearance in place of an outer terminal, so a theme change recolors open terminals at once while their programs keep querying the Host's colors. Selection and search keep the renderer's dark overlay colors. Tabs and terminals are named by the TUI's rule in [native Tab labels](../orchbee/docs/native-tab-labels.md): a console's default title, which is the path of its launched program, names that program, such as `PowerShell` or `Command Prompt`, and keeps a command that cmd runs, while other titles show as written once one elevation prefix and one activity mark are dropped. A terminal that shows no title takes the program the Server launched for its binding, else the program of its failed launch, else its Tab's place, such as `Tab 2`. A name a person chose still comes first. A language change projects the snapshot again and names the latest titles again, so every name changes at once. Unlike the TUI, a terminal that sets no title but reports its directory is named by that directory, a step the shared Workbench takes. `ServerUserPreferencesClient` reads and saves the Session's shared preferences. As in the TUI, a State stream of the settings read then carries every later change, whether another Client saved it or the Server found it in the preference file, so an open Settings page shows a TUI's save without a refresh. A preference source names its record's content rather than an ordered revision, so a snapshot observed around a save can be older than its answer: the stream opens again after each save, and its first snapshot, read after the save, has the last word. Resnapshot stays the Server's recovery from an overrun. A stream that closes makes preferences unavailable until a refresh observes them again, because later changes would otherwise go unseen. `ServerTerminalLaunchProfileClient` and `ServerTerminalDefaultSettingsClient` present the launch profiles and scrollback among those preferences through the Settings page's Desktop controls, as [terminal settings](docs/terminal-settings.md#desktop-terminal-launch-profiles) describes. `ServerAgentIntegrationClient` presents Agent integrations through the same page: each Refresh, enable, or disable is one job on the Session's Worker, the writer the CLI uses, as [terminal settings](docs/terminal-settings.md#desktop-agent-integrations) describes.

`createServerWorkbenchAuthority` answers the Workbench's write port with Server commands for Project, Workspace, Tab, and Panel structure and for closing. Each command is decided from the snapshot its revision names, when its turn comes, and settles once a publication carries what it reports, so what a caller reads next already holds it. A refused or unknown command is never repeated, including one refused for a stale revision, apart from the two exceptions below. A created Workspace is named as the TUI names it, from the directory's final name, avoiding every label in that snapshot. New Workspace opens where the TUI's opens: it first asks the Server, for at most five seconds, where the focused Workspace's first terminal is now, through the [current-directory read](../orchbee-server/README.md#workspace-current-directory-reads). Without an answer it opens where that terminal was last seen, otherwise in that Workspace's directory, otherwise in the home directory. Resource folders and Workspace duplication have no Server counterpart yet and are unavailable.

`server-workbench-tabs.ts` words Tab changes as `tabs.configure` and `tab.create`. A drag's boundary is a place in the order it started from, so a release whose order changed before its turn is not sent and its Tabs return. A group update states the whole group, so what did not change comes from the snapshot it will replace. A new Tab is named by its place until something better shows, as every Client names it, and a copy takes its source's name and whether that name follows the terminal. Tabs start at a default terminal geometry, which the surface corrects once it attaches.

`server-workbench-panels.ts` words Panel changes. A new Panel or child terminal is named by its place, as every Client names it, and starts in the directory its source last reported, otherwise wherever the Server starts one. A ratio is the first slot's share, kept within the 10% to 90% the Server accepts and rounded to five decimal places, so repeated steps leave no trailing digits in shared state. It is sent with the ratio it moved from. As in the TUI, a refusal for a stale revision sends the same move again as a new request while the split still holds that ratio, for at most five seconds from the first refusal. Evening a Tab or resetting its splits is one command per split, each decided from what the last one published; the first failure stops the rest, and what already committed stays.

`closeServerWorkbench` carries a close to its end, because the Server accepts one without spending a revision and removes the target later. A Workspace first asks to close only if it runs nothing, and a new request closes it once the person confirms. The other exception to never repeating a command is also the TUI's: a Workspace close refused for a stale revision is sent again, at most four more times, only while that Workspace's record and terminal context are unchanged. An accepted close then reads its operation every 50 ms for at most 50 seconds and settles when a publication carries its removal; running out ends only the watching, and the Server's work continues. A Panel settles on acceptance. Tabs close one at a time, each from the facts frozen when the set was asked for, and the set stops at the first that does not close. A close the Client cannot follow to an end reports what is known and repeats nothing. Whether a Tab or Panel asks first follows the close activity its surface last observed, so a terminal never shown here always asks. The Host returns that activity to idle once a submitted line finishes; a line left unfinished keeps it unknown until the next one does, and so does a resize for about half a second.

`createServerWorktrees` gives the Workbench the TUI's Worktree commands, using the TUI's requests and rules, so every Client offers, starts, and follows the same Server operations. Discovery has ten seconds in all: it asks where the Workspace's first terminal stands now and reads the repository there when that differs from the Workspace directory, otherwise the Workspace's own. A failed probe reads as no repository; any other failure leaves the checkouts unavailable. Removal is offered only for a linked, non-bare checkout spelled as the Workspace's directory that Git associates with this Workspace, on the Worker holding that directory. A command acts on the latest discovery of its Workspace, so a checkout that discovery no longer lists, or a removal whose path changed, fails as a changed source without reaching the Server. A creation names no branch or base, places the checkout by repository name and operation under the configured Worktrees directory, or leaves the Server to place it beside the repository when none is configured, and never trusts the repository. Every command starts one operation and reads it from 250 ms, doubling up to 2 seconds, for at most 120 seconds; a read the Server cannot serve keeps waiting, and an operation it cannot follow to an end is reported as unknown with its ID, never started again. Paths compare by the TUI's spelling rule: byte for byte on POSIX, and on Windows apart from separators, a trailing separator, and ASCII case. A removal that stopped where it may continue, before its 64th attempt and with nothing about its Workspace's restoration unknown, is kept for Resume, at most 16 per window, and every discovery of its Workspace reports the latest one, whatever Git lists.

`createServerWorkspaceLocations` tells the sidebar where each Workspace stands by the TUI's rules, so a row changes at the same moment in every Client. It reads only while the sidebar shows, and keeps what it learned for the next time it does. A Workspace's source is where its first terminal last reported standing, otherwise its directory, and a Workspace with neither is never asked. A round asks the Server for each Workspace's Git facts in turn, within five seconds in all, then pauses three times as long as it took, from two to four seconds; a failed read retries after half a second, doubling up to thirty. A Workspace whose source has not been asked yet gets one read of its own as soon as no read runs. An answer counts only at the revision of the snapshot it follows; one at another revision is dropped and the round goes on, where the TUI fails the whole read. A row keeps its facts while its source stays. A moved row takes the last answer for its new source, from at most 64 kept, and otherwise holds its earlier name, folder, and facts until Git answers, a complete round finds no answer, the Workspace is renamed, or a second passes from the first move. Facts no read has refreshed for ten seconds are withdrawn, and the folders stay. A folder under the home folder Main reports shows as `~`, matched as the TUI matches it: apart from separators, a trailing separator, and ASCII case.

`ServerWindowTitle` names the window as [the TUI's outer title](../orchbee/docs/windows-terminal-title.md) is named: the Server renders a title set from the CLI, else the configured template, by default `{hostname}: {workspace}`, and sends it only to its foreground Client, the one that attached or gained focus last. A window behind another Client keeps the title it showed, and a cleared title with no template shows `Orchbee`, which is also the page's title before the first one arrives. `bindServerWindowTitle` listens with the window's focus and the TUI's display context: the active Workspace's and Tab's labels as the Server holds them, the focused terminal's own label, and the title its program set, named as its Tab is and only while it comes from the terminal shown. Each change of focus or context opens a new stream before it closes the old one, because closing first would make the Server forget the window and bring it forward as a newly attached Client. Losing focus listens anew too, since only focus turning on brings a window forward. Electron names the Desktop window after its page, so the title shows in the taskbar and the window switcher, never in the hidden title bar.

`desktop-client/server-workbench/` composes these for every Desktop window. The one command this path sends on its own is the first Workspace of a Session holding no Workspace and no terminal, by the startup rule in [native directory startup](../orchbee/docs/native-directory-startup.md), and only from the first document of a window whose restore lost nothing: a later document follows a person's own actions, and an empty Session may be what they chose. A window has no start directory, so that Workspace opens in the home directory. Select Folder and New Folder use Main's native folder picker through the document's second port, and a new Project still comes from the Server's Workspace launch. The Server settles that launch only once its publication carries the Project, so a new folder's Project counts as opened when this document shows it, with no flush to wait for.

A window whose connection ends, including one that receives a publication it cannot apply, keeps its last view and shows Disconnected with a Reconnect button. As in the TUI, nothing reconnects on its own. Reconnect reloads the document, so Main starts the Session again when it stopped and then connects. The status sits at the bottom center, clear of the Workbench notices and a terminal's View only status at the bottom end. A window that never reaches its Session shows Session Unavailable in place of the Workbench, in the system's language and appearance because the Session holds the preferences, with a strip to move the window by. Try Again is its only effect. When Main reports that the start was refused until the Session is recovered, the page names `orchbee server diagnose`, and recovery stays an explicit command.

## Composition boundary

A Desktop window reaches Agent integrations through its Server connection, as jobs on the Session's Worker that project a ten-Agent inventory and run explicit Settings refresh, enable, and disable requests. Browser Preview omits this capability.

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer. Environment adapters may provide different transport and privileged capabilities, but they compose the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not maintain a parallel renderer UI. The shared Workbench client runtime owns shared-document coordination, projected-terminal presentation, search, actions, and its narrow client-session and projection ports. It has no Electron, `MessagePort`, network, or application-lifetime dependency. The runtime provides each projected renderer session with one stable `TerminalPresentationConfigurationSource`. Its initial complete value opens the surface, and later complete values update that same session and attachment without changing React capability identity or reopening projection. The default source retains unbalanced padding, fixes `fontSize` at 13 to match Ghostty's default 13-point setting, and replaces the source-neutral package face with the bundled Ghostty-aligned JetBrains Mono 2.304 variable faces plus Symbols Nerd Font 3.4.0 fallback. Application startup registers those immutable assets before opening any surface; the same bundle and license links serve Browser Preview and Electron renderers without depending on machine-installed fonts. Each application composition replaces that fallback with one client-local settings controller that restores a strict complete preference record, exposes the same stable presentation source, and publishes complete runtime updates without reopening projection.

The shared client runtime closes local admission before releasing resources. Session retirement
settles pending proposals before invoking transport cleanup; a subscription returned after
synchronous retirement is released immediately. Document startup releases previously acquired
subscriptions when acquisition fails and never reports readiness after disposal. A thrown or
rejected proposal completion supplies no proof that the document was not committed: the coordinator
becomes `control-unavailable`, settles pending flushes, and never automatically resubmits that
proposal. Cleanup attempts every independent owned resource once and preserves single or aggregated
failures. Retired surface capabilities cannot apply late terminal metadata. Deterministic capability
traces cover these local ownership and failure boundaries; they do not claim remote rollback or
exactly-once document delivery.

Browser Preview and Desktop renderer disposal retire their local lifetime before cleanup. A React
unmount failure cannot prevent the remaining routing, settings, and transport cleanup attempts. These composition and Workbench cleanup paths share `runApplicationCleanup`:
repeated or reentrant disposal is inert, and a single original error or an ordered aggregate is
reported after all independent steps have been attempted. Composition tests inject failure at the
React root boundary.

Browser Preview uses an in-memory composition. Desktop reaches its Session through the Server Client path above, and Main serves only native Project Folder acquisition beside that connection. The shared terminal surface retains measured cell geometry, complete background coverage, clipped direct graphics, bounded bell indication, connection notices, and terminal-outcome precedence. Workbench payloads come from `@orchbee/workbench-client-protocol`; Electron capability contracts come from `@orchbee/desktop-client-protocol`.

Desktop's Server runtime supplies Agent observation through `ServerTerminalAgentSource`, which reads
the Server's per-terminal Agent reports with `panel-agent` `states` State streams of at most 64
terminals each, in topology order. A membership change reopens only the streams whose terminals
changed. A failed answer or a closed stream keeps its last Agents for 10 seconds, a capacity close
reopens after 5 seconds, and a frame that breaks the protocol ends the observation. Workbench
presentation, Agent notifications through the terminal-feedback controller, and the
trailing-blank-row policy read that source. Browser Preview supplies none and shows no Agent status.

For a Host-backed visual terminal, the shared surface does not open projection until it has loaded
the current configured fonts and measured a positive mounted viewport. It sends the resulting exact
columns, rows, physical cell size, padding-aware grid, and document DPR through its projection request, then pins that presentation configuration for the first renderer. A
resize observation or animation frame wakes an unmeasurable surface; release aborts that wait. No
timer starts the child and no 80x24 visual fallback exists.

A painted surface entering Workbench's managed `closing`, `close-failed`, or `closed` lifecycle is
retained as a frozen, non-interactive frame instead of disposing its DOM. Entering that lifecycle
releases the projection request, attachment, transport, input, and resize ownership without opening
or retrying another attachment. The frozen renderer is disposed only when Workbench removes the
surface after a surviving Tab crosses its display barrier, or clears the final Tab normally.

A natural exit keeps its final frame readable only while Workbench's managed natural-exit removal
awaits disposal; the projected attachment rejects further PTY input, resize, clear, and reset
control, and the exited presentation is not retained as a Tab after disposal. Stopped presentations
remain a separate Workbench state. Its
Start New Session control returns the Workbench transition result through the shared composition;
an accepted result replaces the presentation through the normal durable workflow, while a rejected
or thrown start remains on the stopped presentation and displays localized failure feedback.

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
surface uses the same cleanup discipline for its paste review, mobile-input subscription, broadcast
registration, and controller: a throwing registration disposer cannot retain the renderer or
projection request. Hook integration tests cover single and multiple cleanup failures.

Each projected surface binds one transient broadcast-input sink to its exact Workbench
Presentation. The sink reports ready only while the recovery controller owns an active transport
and the projected terminal exposes the matching attachment capability revision. Capability change,
retention, transport loss, candidate replacement, freeze, and disposal notify the Workbench before
later input can reuse that sink. One accepted send delegates once to the projected terminal's
attachment-local injection path with the snapshot revision as a compare-and-send guard; unavailable
legacy or test renderers and a changed revision fail closed. The adapter does not coordinate
targets, retry, roll back, or replay a multi-terminal delivery.

Active-Tab handoff uses the same frozen-session discipline without keeping inactive terminal DOM or
attachments mounted. When a surface becomes retained, it captures only its exact last-painted
semantic presentation, freezes the canvas, and releases transport, input, resize, request, and
attachment ownership. The application-scoped `TerminalPresentationSnapshotCache` retains those
display-only snapshots in a byte-accounted `32 MiB` least-recently-used budget and clears them when
the shared client runtime is disposed. An incoming surface may paint a cached snapshot before its
fresh live attachment produces a frame. That cached draw satisfies only Workbench's display-ready
barrier; input, pointer interaction, and `autoFocus` remain disabled until the candidate paints its
first live frame and satisfies activation-ready. On a miss or oversized snapshot, the outgoing
frozen layer remains visible until live, stopped, or honest failure content is ready. Returning to a
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

### Mobile browser terminal interaction

| Field                   | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and scope       | A Browser terminal surface on a coarse, non-hover pointer must expose terminal input, scrolling, and essential keys without redesigning the complete Workbench. Whole-shell responsive layout, native selection handles, multi-touch, pinch zoom, Panel gestures, and configurable key rows are excluded.                                                                                                                                                                                                           |
| Inputs and dependencies | The application consumes the current projected-attachment input readiness, terminal textarea focus, localized messages, `(hover: none) and (pointer: coarse)`, `VisualViewport` geometry, layout-viewport fallback, and semantic key intents. It never infers Host modes or encodes terminal bytes.                                                                                                                                                                                                                 |
| State and ownership     | One mounted projected-terminal renderer owns `MobileTerminalInputState`: attachment availability, native keyboard focus, and a subset of one-shot `control`/`alt` modifiers. The React surface only renders that prepared state. The Web composition adapter owns current visual-viewport width, height, and offsets as reversible presentation geometry; it owns no terminal or Workbench state.                                                                                                                   |
| Modifier transitions    | Tapping Control or Alt toggles that one-shot modifier. The next eligible toolbar key, physical key, or independent single printable ASCII native input consumes all armed modifiers before publication. Composition start, paste, multi-character or non-ASCII committed text, blur, read-only downgrade, attachment replacement, recovery, and disposal clear them without transforming or replaying input. A blocked or failed send still consumes the snapshot so stale modifiers cannot affect a later command. |
| Key row and focus       | The canonical `projected-terminal.mobile-input-bar` appears only for the coarse non-hover media condition and contains Show/Hide Keyboard, Escape, Tab, Control, Alt, and four Arrow controls. Its key controls retain textarea focus and use the exact active attachment revision; the keyboard toggle reflects actual textarea focus and never claims that the operating system displayed a keyboard. Unavailable or read-only input disables input-producing controls.                                           |
| Viewport projection     | The composition applies each finite positive `VisualViewport` resize or scroll sample to the application root before terminal measurement; an unavailable or invalid API uses the layout viewport. The root respects safe-area insets, and the existing terminal `ResizeObserver` remains the only writer of projected grid resize. Keyboard close restores the latest full visual viewport without persisting geometry.                                                                                            |
| Invariants and failure  | Touch scrolling never arms a modifier; controls never publish to a replaced attachment; no mobile state enters the shared Workbench document, browser storage, or Terminal Host. Missing browser focus or viewport capabilities degrade to an unchanged layout and typed unavailable input rather than synthetic success. Desktop pointer and hardware-keyboard behavior remain unchanged.                                                                                                                          |
| Verification and limits | Deterministic state tests cover every modifier transition and replacement. Renderer and React tests cover native capture ordering, revision pinning, localization, and focus retention. A controlled headed mobile-viewport trace remains required to verify media-conditioned visibility, touch tap/scroll, viewport shrink/restore, grid resize, and both locales; real iOS and Android keyboard appearance remains manual release evidence.                                                                      |

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

After a projected frame commits, the renderer forwards its complete nullable title,
current-directory metadata, and transient `idle`/`active`/`unknown` close activity with the exact
`TerminalPresentationId`. The shared client runtime
normalizes that observation with the trusted terminal-machine hostname from the client-session
bootstrap and applies it to the matching Workbench presentation. Natural titles and accepted
current CWD remain transient; durable fallback/custom Tab-title policy and shared-document
projection remain Workbench-owned. The page never derives terminal trust from its own URL hostname.

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
own upgrade. Where the transport offers `takeControl`, only a person's Take control Action asks
for one, outside the command channel, and a grant arrives as the next capability revision.

The stable headed Canvas 2D `terminal-parity/IN-10-read-only` witness controls exact attachment
capability revisions and observes immediate local repair, retained search/selection/history/copy,
zero stale input commands, and only fresh upgraded input. Terminal Host's owner-local tests prove
the authoritative rejection and repair ordering; this Browser fixture does not grant a renderer
capability mutation or claim native permission behavior.

The packaged `orchbee:` origin and Electron User-Agent token select Desktop mode, so a loopback development renderer inside Electron is Desktop too; Desktop authority arrives only through the validated Server-connection announcement from preload. An ordinary HTTP(S) origin selects the in-memory Browser Preview. An `orchbee-runtime` query parameter fails closed.

## Navigation

All compositions use one TanStack Browser Router. Canonical Workspace routes use `/workspaces/$workspaceId[/tabs/$tabId]`, independent of nullable Project membership. Old Project-qualified Workspace paths are not admitted. Group-only links resolve current membership; unknown Workspace/Tab identities remain unavailable. Navigation pushes history; canonicalization replaces it. Renderer focus recovery restores the exact surviving Workspace, Tab, Panel and child before explicit route reconciliation. See [`docs/navigation-routing.md`](docs/navigation-routing.md).

## Settings

Desktop Settings includes Agent Integrations. Startup, search, and category navigation perform no
mutation; only an explicit switch action changes provider configuration, as one job on the Session's
Worker. The launch reminder does not appear, because the window reports no Agent launch to its
integration client. Reminder dismissal stores only the Agent ID, and remote setup remains
manual-only.

General offers System, English and Simplified Chinese. Desktop reads the same confirmed device
preference as TUI; Browser Preview keeps an origin-local record. System follows declared locale
inputs, while explicit locale wins. Confirmed changes update mounted product copy without reload.

The common Settings action opens an application-owned full-window overlay rather than navigating to another route. The Workbench subtree becomes inert, but its React tree and every projected terminal attachment remain mounted. Desktop common preferences use the shared device store with source-bound drafts, confirmed saves, conflict and unknown-outcome recovery. Host typography, pixel padding and opacity remain renderer-local. Browser Preview retains its independent strict origin records. On macOS Desktop, a separate strict local record selects whether neither, left, right, or both Option keys act as terminal Alt; the default is off and accepted changes update every mounted projected terminal without replacing its attachment. Sixty-seven frequent Workbench and focused-terminal shortcuts, including application-wide font-size increase/decrease/reset, creating a terminal Tab in the exact displayed terminal's current directory, four directional Panel-focus commands, previous/next cyclic Panel focus, ten exact positional Panel-focus commands, visible Panel focus and swap, four directional Panel-movement commands, proportional Panel equalization, keyboard Panel resize mode, circular active-Tab movement within its pinned or unpinned section, visible terminal Hints, Show Last Command Output, and Show Scrollback, use a separate strict local override record; accepted changes update the mounted Workbench and every projected terminal without replacing an attachment or connection. Standard defaults reserve unshifted terminal control sequences, while runtime Action availability remains authoritative before any key is consumed. Application theme remains separate from terminal display theme, and terminal overrides never mutate Host metadata, palette, cursor, VT, or OSC state. Desktop additionally injects a semantic shell-profile control over the Session's shared preferences. Its explicit editor can create, rename, select a system or custom executable, preserve literal argument order, select the default, delete, and confirm reset; every accepted change affects only terminals created after the shared catalog commits. Browser Preview compositions receive no profile capability. Shell integration settings own a separate default-off device-local completion-reminder policy whose notifications never include command text; reminders need OSC 133 command boundaries, which the Terminal Host's PowerShell integration supplies on Windows, while Zsh, Bash, and Fish emit them only through the user's own configuration. A paste-only Ghostty migration dry-run reports lossless current targets, manual-review cases, ignored keys, and invalid lines without reading files, applying values, or persisting source text. The Remote development category is another read-only projection: it explains the `xterm-256color` baseline, exposes one constant shell diagnostic, distinguishes observed environment and terminfo outcomes, and states that Orchbee does not silently forward extra environment variables or modify the remote host. Opening, searching, or switching to this category starts no process, writes no terminal input, and persists no remote data. See [`docs/terminal-settings.md`](docs/terminal-settings.md) and [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md).

The exact headed `terminal-parity/SU-09-theme` witness holds a prepared Host result and its
revision-tagged frame before one release, then observes the terminal defaults, palette, cursor,
overlays, pixels, and application surface publish one matching revision. It uses the controlled
runtime-presentation Canvas fixture and does not claim a persisted settings migration, a native
operating-system theme event, or a renderer-owned terminal palette.

## Local development preview

Locale preference, automatic fallback resolution, and shortcut-platform setup belong to the Web
composition shared by Browser Preview and Desktop.

`pnpm web` renders the Workbench shell with its localized sidebar, standalone terminal context, Project and Workspace navigation, and per-tab Panel workspace. The browser composition creates one `ActionRuntime` and one in-memory `WorkbenchProjectWorkspaceRuntime`. It starts with only the standalone Terminal context. A person can create an in-memory Project by entering a folder path; the Project add control first preserves the common creation menu, then `Select Folder…` opens the browser-only path entry in place of a native picker. The browser uses that path as the initial Project Folder and terminal CWD without claiming that the path exists. `New Folder…` remains visible but disabled because the composition performs no filesystem, Git, PTY, or durable-storage effect.

Electron Desktop injects native existing-Folder acquisition into the shared application. `Select Folder…` uses the operating-system picker, and local Project Folder attachment uses that same capability. Browser Preview retains manual Folder path entry and receives no native filesystem effect. `New Folder…` selects a native parent and explicitly creates one absent child, then observes the existing Project Action, exact canonical publication and Workspace activation. The existing route coordinator reports its accepted current target separately from pending navigation, so creation never acknowledges an unrelated route. Created or uncertain filesystem outcomes are preserved for Select Folder recovery; no new provisioning or filesystem API is introduced.

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

With the optional local capability, which no composition supplies today, a fine-pointer hover
over a detected PNG, JPEG, GIF, WebP, AVIF,
or BMP path starts a 300 ms dwell without probing unrelated paths. The resulting request remains
bound to the attachment, target, generation, revision, link epoch, row, and column. Replacement,
pointer leave or press, wheel input, geometry or revision change, attachment retirement, and
disposal cancel the pending presentation; a late completion cannot replace the latest hover. The
surface renders admitted bytes in one pointer-transparent card anchored next to the source cell and
keeps terminal focus and selection unchanged. Missing, unsupported, invalid, or unreadable images
fail silently. Browser Preview never requests or receive local image bytes.

The headed Canvas 2D witnesses `terminal-parity/FS-05-file-links` and
`terminal-parity/FS-06-url-policy` compose the production projected surface, Workbench Action, and
Browser effect. They prove that `http`, `https`, and `mailto` alone expose hover, context, and direct
activation, while automatic FTP text, unsupported explicit metadata, explicit file metadata,
detected paths, and a direct file-effect request remain inactive. The controlled fixture uses
synthetic generation-pinned Host row windows and a captured Browser opener; it does not exercise an
operating-system URL handler or claim filesystem existence.

The headed Canvas 2D witnesses `terminal-parity/FS-19-local-file-links` and
`terminal-parity/FS-20-image-preview` enable the optional local capability against the same real
projected surface with a fake local effect. They prove relative-path resolution, modifier opening,
300 ms image intent, replacement cancellation, rejection of a late completion, and the production
anchored card's final PNG source and pointer-transparent interaction. Their captured opener and
synthetic bytes do not launch an operating-system application, automate Finder, claim native image
decoding, or grant Browser filesystem access.

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

Browser-owned request and presentation identifiers use `crypto.getRandomValues`, which remains available on the explicitly accepted HTTP LAN-preview origin. Runtime code does not depend on the secure-context-only `crypto.randomUUID` API.

For the complete local Desktop flow, run:

```sh
pnpm desktop
```

Electron loads the Web development URL and discovers its adapter from the User-Agent. Packaged Electron serves this production build from the standard secure `orchbee://app` origin; `apps/desktop` owns packaging and resource delivery. Browser Preview remains an independent in-memory development surface.

## Unit verification

Run `pnpm --filter @orchbee/web test` for the unit suite. On Windows, Vitest uses at most four workers (or the available CPU count if smaller), bounding process creation and filesystem contention when other workspace suites run concurrently. All files, assertions and deadlines remain enabled; other platforms retain the runner defaults.

## System verification

`pnpm --filter @orchbee/web test:system` runs the headed Canvas 2D witnesses in `system-tests/` against the `*.system-fixture.ts` pages, which one rsbuild development server compiles. It needs an installed Playwright Chromium, and no routine gate runs it.
