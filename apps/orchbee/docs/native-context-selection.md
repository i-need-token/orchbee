# Native context selection

## Scope and ownership

The native Client selects a Workspace in the connected local
Session. Server owns navigation, membership, revision and terminal lifetime; the Client owns its active
location and selection. This reference governs the context-selection extension to the
[native Tab Session](workbench-client-runtime.md). Maintain it with the Session reducer, transport,
navigation rendering and their correctness evidence.

`Ctrl+b [` and `Ctrl+b ]` cycle through real Workspaces in canonical display order, including empty Workspaces. Sorting and nullable Project membership never change location identity. Selection uses the Workspace/Context pair and restores this Client's surviving Tab/Panel memory, otherwise the first member or Empty. Entering or leaving Empty creates no terminal. Explicit `c` requires the Workspace's admitted directory and never falls back to home. Sidebar activation shares this Session selection owner; exact Session/location validation, Busy precedence and latest unadmitted target retention follow [machine navigation](machines.md#cross-machine-workspace-navigation). [Directory startup](native-directory-startup.md) may create the first Workspace only in an eligible empty Session. Other startup paths read and select existing membership before attachment.

`ClientContextLocation::NoWorkspace` represents only a Workbench with zero Workspaces and zero Contexts. It has no Context or Workspace ID, is not a navigation row, and is never persisted as a saved position. Creating a Workspace supplies a real owner; removing the last Workspace returns to this empty landing state. Every existing Workspace, including an ungrouped or empty one, retains its canonical ID and ordinary rename, directory, grouping and Close rules. Snapshot validation requires a one-to-one Workspace/Context relation; all 1,024 context slots are available to real Workspaces.

On Windows, `c` in the no-Workspace state instead admits the existing quick Workspace launch.
Only a freshly accepted redirect returns a typed Workspace-creation disposition to the shell;
the next control tick preserves that operation's entry intent. Its exact committed Workspace,
Context, Tab, Panel and Presentation must install and reach Ready, including after a settled
last-Workspace Close. Newer focus still revokes entry without replaying the launch. A genuine
empty canonical Workspace continues to create a Tab under its own directory policy.

Workspace creation, optional Project organization and first-directory configuration belong to
[Workspace setup](native-workspace-setup.md); directory startup owns automatic empty-Session creation.
Both reuse this context installation owner.
Panel layouts, remote access and production recovery are excluded.
This unpublished cohort rolls out and rolls back together.
Changing the Client location does not change canonical navigation or dispose retained execution.
[Location label editing](native-location-labels.md) changes canonical Project or Workspace labels
while preserving these typed location identities.

## Capturing navigator

`Ctrl+b w`, its configured replacement or the action menu opens a capturing navigator. The
existing entry refresh and known-result recovery remain Session-owned and are requested once.
Closing does not cancel accepted work. Ordinary shortcut Help remains noncapturing.

The two columns contain canonical locations and the current location's Tabs. ClientView owns a
volatile episode, exact source and local focused target; it owns no navigation mutation. Focus
starts at the current Tab, otherwise the current location. Up/Down, Home/End and PageUp/PageDown
move within a column without activation; Left/Right choose a column and Tab/Shift+Tab switch
columns, initially selecting that column's current row. Windows around focus expose long lists.
The active marker remains distinct from keyboard focus. Enter opens the focused row; an
unmodified primary click opens its exact row. Esc returns to the terminal. The configured
navigator toggle can also close it; its local prefix consumes every other action, including a
double prefix. Printable input, signals, paste and unknown keys never reach the terminal here.

| Transition         | Guard, output and ownership                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Enter              | Acquire bracketed paste before capture, clear the old prefix, cancel the existing pointer gesture without export, and capture the opening packet. Initially absent rows may acquire their first source and current focus once.                                                                                                                                                                                       |
| Paint              | Publish an episode/source/target proof for each complete visible row only after successful drawing. Source is Session, canonical revision and current location. Close carries the same exact DrawSource. Keyboard activation also requires painted focus; pointer activation requires matching primary release on the exact row. Labels may ellipsize; partial geometry, hidden rows, counters and footer are inert. |
| Browse / reconcile | Movement and resize revoke old proofs. A changed canonical revision can preserve a still-present focused identity under the same Session/location, but requires repaint. Focused identity loss or Session/location change cancels; terminal output alone changes neither source nor proof.                                                                                                                           |
| Activate           | Revalidate exact source and membership at WorkbenchClientUi admission. Existing refresh, mutation and conflicting-selection guards precede any no-op. Return distinct `ExactCurrent`, `Admitted(generation)` or `Refused`; only the first two close. A proven current target submits nothing. Otherwise resolve its current ordinal once and use the existing context/Tab operation.                                 |
| Retire             | Esc, an outside press, accepted activation, covering capture or unpresentable geometry clears local capture. Preserve the retiring packet, pending decoder fragments and input fence until a successful fresh frame and fresh packet. A consumed pointer decision separately owns its matching release beyond repaint, ending on release, focus loss or a new qualifying press.                                      |

Hover and pressed feedback do not move the provisional cursor or current context. Leaving an armed row or close target permanently cancels activation; returning does not rearm it. A covered, failed, replaced or unpresented candidate has no new pointer authority.

Pointer rows use their own navigator proof, never the ordinary Tab-strip route. Each painted
column has its own wheel region: one notch moves local focus by three rows without activating,
coalesced work is capped at one visible column per pass, and reversal discards older work while
preserving the new notch. The header close control returns to the terminal. Backdrop wheel does
not dismiss or reach the pane; Shift retains the existing host bypass. A refused
activation stays open with the existing notice. No target is inferred from a highlighted pending
selection or an ambiguous `None`/`KeepNavigator`. Existing Session state continues to own
asynchronous selection, failure, remembered Tab/Panel restoration and accepted side effects.

The navigator suppresses pane cursor/focus chrome and presents localized column, focus, exit and
activation hints. Esc and Enter survive compact degradation before optional headings or revision
text. Geometry that cannot present a row and exit instruction closes with the existing refusal.

Navigator core, dispatcher and shell tests cover capture, complete-row paint, one-shot transfer, release ownership and mouse-mask restoration. Those concrete input-ownership traces replace a separate sidebar TLA model.

## State and transition contract

The inputs are a validated Workbench snapshot, an explicit local direction, current Session state,
exact attachment identity and the existing monotonic operation budget. Navigation and the active
context projection must come from one snapshot revision. A nonactive context with an unsupported
layout does not invalidate the active context. Selecting an unsupported target is a definite local
failure and preserves the previous location.

| Transition               | Guard and result                                                                                                                                                                              |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admit location selection | A stable Session with no unresolved mutation, Close confirmation/reconciliation or membership operation retains an exact target identity and operation generation; otherwise report busy.     |
| Read target snapshot     | Revalidate the target's canonical owner, Session and revision before changing attachments. Definite unavailable or unsupported target preserves the previous location.                        |
| Clean old attachment     | Fence old input, frames and grants, then detach the exact previous Viewer before installing a different selection. No terminal Dispose or Start is authorized.                                |
| Install target           | Atomically publish the new context, membership, provenance and optional desired attachment through the existing membership owner and mailbox transaction. Empty has no invented Tab identity. |
| Attach selected Tab      | Use the exact retained binding and the existing setup, first-frame and controller geometry rules. Late old-generation results cannot become current.                                          |
| Fail after handoff       | Preserve known facts and apply the existing deadline/retirement rules; never retry an ambiguous effect or synthesize successful cleanup.                                                      |

Known Create provenance survives leaving and returning to a Workspace. A snapshot binding cannot turn pending, failed, unavailable or outcome-unknown Create into attachment permission. Retained provenance is bounded by Context/Tab limits and pruned only with validated membership evidence. Pending Close retains its exact target and blocks location changes until settlement permits them. An accepted Workspace close instead permits selection among the remaining Workspaces while its settlement is read, once the current Workspace is not a leaving one; see [native Workspace close](native-workspace-close.md#accepted-close-leaves-navigation). Refresh and mutation results preserve the selected Workspace or follow the accepted-close landing or explicit canonical removal recovery.

## Client selection memory

Read-only layout queries resolve an inactive Tab's `focusedPanelId` from this Client's remembered
Panel and current canonical membership, falling back to the first member. `selected` remains false;
the query neither visits the target nor changes navigation, attachments or terminal geometry.

`state/navigation_memory.rs` owns volatile, Client-private selection history. Each exact location
holds its last Tab ID and each visited Tab's last Panel ID. Labels, ordinals and terminal bindings
are not keys. The Session is fixed by the owning Session; this history is neither Server state nor
persistent storage.

| Input                           | Guard and result                                                                                                                                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admit Context departure         | Remember the current stable selection after existing busy and target guards pass. Empty adds no invented selection.                                                                                                  |
| Admit Tab departure             | Remember the logically stable departure, including a Ready frame-proof gap; superseding switches keep the original memory and never record an unvisited switching target.                                            |
| Resolve ordinary destination    | Validate canonical membership and provenance first; use the remembered Tab and Panel only while each belongs to its exact location and parent Tab. Missing history or removed members fall back to the first member. |
| Resolve explicit Panel or Agent | The explicit stable target overrides history and keeps existing refusal when absent; memory cannot substitute another Panel.                                                                                         |
| Install canonical membership    | Prune absent navigation locations, Tabs and Panels. Current-context membership takes precedence over any older retained entry; departed-context history uses retained membership until revalidation.                 |
| Refuse, supersede or cancel     | Only the previously stable departure may have been remembered; no target is marked visited and no additional command is emitted. Existing cleanup and generation settlement remain authoritative.                    |

Memory is bounded by the protocol's known Context and Tab membership, with at most one Panel ID
per visited Tab. Local Tab creation and Workspace setup admission also remember the stable
departure before their existing creation policy selects a new member. Retaining uncertain Create provenance does not retain navigation memory for a
canonically removed location. Restoration never changes Create permission, chooses another Client,
resizes a terminal or activates an OS window. Ordinary keyboard, pointer and CLI navigation share
this Session policy. After Tab switch admission the exact Panel ID is fixed: a reordered candidate
re-resolves its ordinal. A missing Panel in a surviving Tab fails explicitly; removal of the whole
Tab retains existing membership fallback to another Tab or Empty. The prepared candidate owns
the same Panel identity for state installation and mailbox publication. Explicit creation, membership repair and Close keep their existing selection
rules. Restart discards this per-location history; Windows preserves only the last displayed
position as described below. Cohort rollback requires no checkpoint migration.

Deterministic traces cover remembered nonfirst members, reorder by identity, removal to first/Empty,
explicit override, superseded/refused switches, stale retained membership, independent Clients and
known nonattachable Create provenance. Windows process evidence exercises two live ConPTY Clients
through CLI Tab/Workspace/Panel focus and checks unchanged peer selection and OS foreground. These are named implementation traces, not exhaustive context-selection model checks.

## Last Panel

`ui/last_panel.rs` owns a second volatile, Client-private history: the Panels this Client focused
across the whole Session Workbench, most recent first, each Panel at most once and at most sixteen.
After each presented draw it records the settled focus, the current Tab's selected Panel once no
navigation is pending and no switch is landing, so pointer, keyboard, CLI and Agent focus changes
all count; a plugin overlay Panel never enters. Last Panel (`Ctrl+b ;`, `[keys].last_pane`, the
global menu's Panel group) records the current focus, drops entries whose Panel, Tab or Workspace is
gone or leaving with an accepted close, and steps to the most recent other Panel that remains. A
Panel of the current Tab takes the ordinary Panel focus move. For a Panel of another Tab or location
it names that Panel and Tab as the location's remembered selection above, then the ordinary Tab
switch or context selection lands there; a Panel that disappears in between falls back as that
selection does. Without a target the step does nothing and reports nothing, and the global menu row
asks the same target search, so it is disabled with the no-earlier-Panel reason. A step requested while a
background read or the user's own switch is landing is held, then chooses its target where the
switch landed. Restart discards the history, and the GUI keeps its own with the same model.

A Tab close that holds the focused Panel returns focus from the same history, to the most recent
Panel still open in that Tab's Workspace. A Panel of another Workspace is skipped, so focus never
moves into another Workspace on a close; when no such Panel exists, the post-close landing in that
Workspace applies.

## Windows startup preference

Bare local Windows startup remembers the last successfully displayed location, Tab, Panel and
presentation for the current Windows user and local Session runtime. This is Client-owned navigation
advice, separate from Server checkpoints and terminal lifetime. Its versioned, bounded JSON record
is the Session folder's `tui-position.json`, `sessions/<session>/tui-position.json` in the data root, and is removed with
its Session. Only schema `orchbee.tui.position`, version 1, with required position/location/member fields is
readable; member is explicitly nullable. Workspace/context identity excludes mutable Project membership.
Only the TUI writes the record: an earlier version loads migrated in memory, and the position writer
backs the original up under `internal/backups/` and saves the migration before its first
replacement. A record with a version this build cannot use, saved by a newer version or not updated from an
earlier one, counts as no saved record and is never rewritten: the TUI opens at the first
Workspace, remembers no position for that run, and shows the record message once. Old shapes and
any other unreadable or oversized record count as no saved record, and the next published position
replaces them, so a format change never retires
position memory. Only an unavailable cache disables writes for that invocation.
Stable structural IDs survive checkpoint restart. The new Server's Session ID and bindings always
come from its validated snapshot.

The renderer returns the stable position and focused pane incarnation only when the workbench is
uncovered and drawable. Successful shell draw/flush precedes revalidation and publication.
Covered, failed, stale and switching draws do not publish. Empty locations invent no member.
An ordinary status-line notice does not cover an Empty location or suppress its publication.
Thus closing the last Tab replaces the old member preference with the valid empty Workspace;
after a removed-member fallback, the newly displayed empty position becomes the next preference.
An app-private worker coalesces pending positions to one record, performs private locked atomic
replacement off the input/draw path, and joins at shutdown. Windows keep independent live focus
and read the preference only at startup. The latest atomic publication supplies the next startup
preference; unchanged redraws do not write again. Storage failure displays a localized notice
without retiring a live terminal.

Restoration validates current canonical ownership before ordinary bootstrap/attachment. A removed
member selects the first surviving member in its location. A removed location or saved compatibility
position selects the first canonical Workspace, even when empty; with none, it restores nonempty
legacy content or the no-Workspace state. A valid legacy preference retains its exact member only
when there is no canonical Workspace. Fallback produces a localized notice. Without a saved record,
directory startup chooses eligible empty-Session creation or the same existing-Workspace order
without Open. An unreadable record follows the same directory startup and reports the
location fallback notice once. Explicit context or
Agent startup retains its exact target. Preferences never grant input, change Create provenance,
launch, dispose, change checkpoint data or move another live Client. An existing empty Workspace
with an absent saved member is an exact restoration without a warning. A fallback to Empty
reports that destination instead of claiming that a surviving member was opened.

This contract separates durable advice, Client-local selection, pending draw proof and asynchronous storage publication. Exact membership and pane incarnation guard publication; restart resolves current membership before bootstrap. Failure preserves terminal state. Codec/resolution, private-cache and carrier tests cover stale/covered/failed draws, independent Clients, coalescing and atomic publication. Windows ConPTY detach/reopen and removed-member traces connect those cases to the carrier. These named implementation traces do not prove exhaustive interleavings or physical Windows Terminal appearance.

## Correctness evidence

The owning implementation tests cover selection memory, exact Workspace/Tab/Panel identity, cancellation, stale completion, cleanup-before-install, first-frame admission, startup preference and navigator input ownership.

Compatibility navigation traces cover peer removal/addition while either canonical or legacy
content is selected, cycling from a hidden empty compatibility slot, and right-clicking noncurrent
legacy content without changing canonical focus. Removal landing on an empty canonical Workspace
must release the old pane and controller authority before installation. Fresh peer-added membership
requires a fresh attachment; visibility alone never grants input or recovers a live process.

Maintain the concrete witnesses in `crates/workbench-client/src/workbench_state/context_selection.rs`, `crates/workbench-client/src/workbench_client/mailbox/tests/context_selection.rs`, `crates/workbench-client/src/workbench_client/connection/native/tests/context_selection.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.

The selected pane-membership model still abstracts departure, cleanup and anchor advancement;
an empty Workspace landing creates no pane. The control-takeover and viewport models retain their
existing grant and epoch fences. These abstractions do not establish Workspace ordering, compatibility
filtering, startup admission or adapter/UI behavior; those remain concrete implementation and native
acceptance obligations.
