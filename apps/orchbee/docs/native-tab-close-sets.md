# Native Tab close sets

## Targets and confirmation

Native Tab menus close the selected set or other Tabs, the global menu closes the Tabs trailing the
current Tab, and group menus close the complete canonical group, including collapsed members. `TabStripContextMenu` and the native owner
share the [Tab Close target corpus](../../../schemas/workbench/README.md#cross-client-parity-sources).
If the menu target is selected, direct Close uses the selected IDs in canonical order; otherwise it
uses only that target. Other/trailing Close exclude pinned Tabs and the target, protecting the
selected set only when the target belongs to it. Trailing begins after the rightmost selected Tab
in that case. Direct selected/group Close includes pinned members where applicable.

One confirmation freezes 1–256 exact IDs, Session/context, full canonical Snapshot, names, positions,
local marks and focused selection. Focus includes the logical `PaneKey`, physical `PaneRef`
incarnation, selected Presentation and binding; a same-key reattach is a change. Background targets
are planned by exact ID without selecting them or impersonating the selected-only Close planner.
The dialog uses the same terminology as its menu: **Tab** in English and **标签页** in Chinese,
with **group** / **分组** kept distinct. It shows the count, location, first three
names/positions and an explicit remainder count. Single/selected/group menu scope comes from the
[typed menu projection](native-action-menus.md#tab-scope-and-current-values), not translated counts.
Names and location use sanitized, grapheme-safe previews capped at 64 cells (less on narrow
screens), with an explicit ellipsis for clipping; full frozen IDs and positions remain authoritative.
The draw token covers the entire bounded preview, remainder count, warning and controls.
Its localized warning names what the close ends for everyone connected to the Workspace, as
[close impact](#close-impact) counts it.

The shared Close and Cancel buttons carry their own Enter/Esc labels. Close and Resume use the
existing destructive action tone; Cancel remains ordinary. Neither changes the Close owner or
represents an error status. Only a complete successful draw authorizes unmodified `Enter`;
local `Esc` cancels an armed
confirmation even while it is not shown. An evaluated geometry block before the first successful
draw refuses the new confirmation, and the status line says it cannot open there. After a successful draw, the same block suspends the existing
confirmation while preserving its token, frozen set, local marks and focus; `Enter` and pointer release
are inert until the same token is completely drawn again. A missing or covered draw receipt revokes
shown authority without classifying geometry or discarding the presentation phase. Unseen,
cancelled, stale, resized or unpresentable confirmation cannot submit. The existing input owner
captures the entire decision packet, including a menu-to-confirmation handoff tail. Opening and
cancellation do not allocate a lifecycle request or remove canonical state.

A confirmed `Enter`, or Close release, never ends without a close or a visible reason. When another
close's status read or exchange, or a refresh, holds the command slot the close needs, the Client
holds the decision and submits it, once, in the first tick that finds the slot free and before
further background reads start. Meanwhile the confirmation stays drawn and armed: `Esc` withdraws
the decision with nothing sent, and a later `Enter` confirms again. A target that drifts meanwhile
cancels the confirmation with its changed-target notice, as an open one does; a decision still
waiting after the held-intent bound (5 s) is dropped with `Client busy · no command sent`, and the
confirmation stays armed for another `Enter`. The set confirmation and a Panel confirmation follow
the same rule.

## Close impact

Single, set and Resume confirmations name what the close ends from terminal facts this Client
already holds. The count changes copy only: confirmation, frozen premises, admission and execution
keep the rules above, and no close re-validates its impact.

A terminal counts as ended only when its canonical Presentation carries a launch failure, when the
attachment bound to that exact Presentation binding has received the Host's exit or failure
settlement, or, for Resume, when the Server has reported disposal. Every other terminal counts as
running: background Tabs, child Presentations, panes without a viewer, a settlement not yet
delivered and a target missing from the current projection. The Server publishes no terminal run
state, so the Client never infers that an unobserved terminal ended.

The warning names only running work when nothing counted has ended, only discarded retained output
when everything has ended, and otherwise the number of running terminals together with the
discarded retained output. A close that presents a confirmation still warns about that retained
output, which it discards for every connected Client; a close of only ended terminals runs no
program, and the Server closes it unasked under [Idle close](#idle-close), as it does a Workspace
Close.

The count is taken at each complete draw. A settlement arriving under an open prompt changes the
next draw without changing its token or targets. An ended terminal never runs again: a new process
needs a new binding, which changes the revision or Snapshot that the confirmation froze, so the
stale premise cancels the prompt. The executed close is therefore never more destructive than its
last complete draw; that draw can only overstate running work.

## Idle close

A close that ends no running program does not ask. This covers every route to one Tab, the key, its
`[x]`, and a Tab menu or group action that resolves to one Tab, the Panel route and a
[set](#set-close) of several Tabs. A Resume and a ReadOnly Client always present the confirmation
and send nothing conditional.

The Server decides, by the rules of its
[conditional Tab and Panel Close](../../orchbee-server/docs/terminal-tab-close.md#tab-and-panel)
and of an `if-idle` [Tab set close](../../orchbee-server/docs/terminal-tab-close.md#tab-set-close).
Each route still arms the confirmation; `workbench_client/ui/idle_close.rs` keeps it hidden and
sends `tab.close-if-idle`, `panel.close-if-idle` or the set's `if-idle` Start as the close's first
exchange, `CloseOperationKind::CloseIfIdle`, `PanelIntent::CloseIfIdle` or
`CloseOperationKind::TabSet`. Accepted, the close goes on as a confirmed one: a Tab close is
retained only now, so the Tab leaves the strip only once the Server has accepted it, a set's members
leave together, and a Panel close applies as the ordinary one. `confirmation-required` presents the
armed confirmation, and confirming it sends the ordinary close, or the set's `confirmed` Start, under
a new request ID. A set presents as it stands then: a focus, mark or other change around members
that are still the Tabs it froze captures it again, and a changed member withdraws it with the
cancelled notice. Any other refusal drops the confirmation and reports as the ordinary close does.
An unknown outcome is followed by status reads and never sent again, and no confirmation follows it,
since confirming could close the Tab twice.

The hidden confirmation captures no input while the Server decides, so no key typed before the
prompt draws can decide it: only `Enter` after a complete draw confirms, and `Esc` cannot stop the
close before the answer. Keys typed meanwhile go where they would without a close. A close of
another Tab queues them in order behind its exchange, its status reads and its reconciliation, so
typing into the focused pane loses nothing; a close of the focused pane's own Tab, or a set Start
naming it, refuses them with the input-unavailable notice, as it does through the rest of that
close. A Panel close queues them
behind its exchange: an accepted close ends the pane before they arrive, and `confirmation-required`
delivers them to the terminal before the prompt draws. A close the Client cannot send at once, because
another close exchange is in flight or the mailbox refuses it, presents the confirmation instead.
Harnesses that script every Server exchange build the UI with `confirming_every_close`. The [real
process witness](../tests/cli_windows_idle_close.rs) drives a real PowerShell through each route,
under the system ConHost by default and the product's ConPTY with `ORCHBEE_TEST_CONPTY_DIR`, and the
[route witness](../crates/workbench-client/src/workbench_client/ui/tests/idle_close.rs) covers which
closes ask the Server and which present at once.

## Close status

A close of one Tab behaves the same from every route. That includes the close of a Tab's last
Panel, confirmed or closed unasked because it was idle: the Server admits it as the close of the
Tab, so from its acceptance the Client retains it as it retains an accepted Tab close, with the
leaving, selection, status, report and Resume described here. The Tab leaving reports a normal
close, so it adds no notice and no count. From the draw after its close is submitted, or accepted when the Server
decided it unasked, while that close is progressing normally, the Client treats the Tab as gone
instead of waiting for the Server's publication to remove it from the canonical list: the strip
leaves it out, next, previous and numbered Tab selection skip it and count positions without it, and
nothing selects it. Once the Server accepts the close of the selected Tab, the selection moves in
the same tick to the next Tab still shown, else the previous one, the survivor the removal itself
selects, while the terminal ends in the background. The draw that leaves the Tab out of the strip
already shows that selection; the Panel keeps the closing Tab's last frame until the next Tab's
first one, as the [navigation hold](native-terminal-viewport.md#direct-sidebar-switching)
describes. The last Tab leaves the empty Workbench in the Panel at once. A refused, blocked, unknown
or unobserved close shows the Tab again with its report, and the selection stays where it moved.
Until 1 second after submission, or
after the user asked for a close the Server accepted unasked, an unreported close also withholds
the selected Tab's connection and exit cues; the Panel title still says how its terminal ended. On a
debug build a confirmed close shows the next Tab 40 to 60 ms after it starts. A close the Server
decides unasked waits for its answer, 15 to 35 ms after the user asks: the Tab, background or
selected, then leaves the strip in the next draw, by 45 ms, and the Panel of a selected Tab moves to
the next Tab when that Tab's first frame arrives, about 90 to 110 ms after the ask. A PowerShell Tab
at its prompt leaves the canonical list 0.1 to 0.15 seconds after its close starts, but a slower
shell or a busy machine would flash the 200 ms delay of other pending indications. Past the delay
`{label} · Closing` appears, as when a program running in the shell keeps the console after
Terminate ends the shell and the close waits out the five-second grace.

Status reads of a retained close are 250 ms apart, except that a Workbench publication lets the
next one go at once: the Server publishes a closed Tab as it finishes, and the Client holds every
refresh until a read confirms it, so the Tab leaves without waiting out the interval. A wake admits
one read; reads stay one at a time. A read names the close by its operation,
`workbench.close-operation.read`, once the Client knows that operation. Only a close whose
acceptance never arrived is read by its Tab, `workbench.tab-close.read`, which answers only with a
close of the whole Tab, never with an earlier close of one of its Panels.

The accepted close of one Panel of several is retained and read the same way, by its operation,
but it is no Tab close: it hides only its leaf, names the Panel in its report, offers no Resume,
holds no navigation or structure between its reads, and never refuses the other Panels of its Tab.
It ends when a read sees it `Closed` or a publication removes the Panel; a stop shows the Panel
again with its report ([native panel lifecycle](native-panel-lifecycle.md)). Several such closes
are retained side by side, each hiding its own leaf until its own settlement, and their reads take
turns under the same pacing. A Tab none of whose Panels is left to draw leaves the strip with them,
and the selection follows it as it follows an accepted Tab close.

Blocked, unknown, unavailable and uncertain closes report at once, and a later validated `Closed`
clears the report. A refusal keeps the Tab and reports only its notice. `close_status` stays
unfiltered because Client control uses it as a busy gate; rendering reads `announced_close_status`.

## Set close

A confirmed set of several Tabs closes as one Server operation,
[`workbench.tab-set-close`](../../orchbee-server/docs/terminal-tab-close.md#tab-set-close), which
`workbench_state/tab_close/set.rs` follows. One Tab is no set: it closes as the key closes it. The
Client sends one Start naming the frozen members in their canonical order, at the revision the
confirmation froze, and reads nothing first. The Server closes the members one at a time, each
through its own Close, and stops at the first that fails; each removal is an ordinary Tab Close
publication. While the set runs, no other Tab Close, Resume or set begins on this Client.

When the Server accepts the set, every member leaves the strip at once, and next, previous and
numbered Tab selection skip the members. A selected member moves the selection in the same tick to
the next Tab outside the set, else the previous one; when no Tab stays, the last removal empties the
Workbench. A focus or mark change afterwards stops nothing, and the set holds back no refresh: each
publication installs as it lands, and a focused viewer lost meanwhile parks and re-attaches without
waiting for the set. The members the Server leaves open come back with its report, and the selection
stays where it moved.

The Client reads the accepted set 250 ms apart, or at once after a Workbench publication, one
exchange per Close lease. A Start that a revision race refused closed nothing: once this Client
holds the revision the refusal named, the Start goes again at the revision the Client then holds,
under a new operation ID, at most four times and within the refused Start's 50-second budget, while
every member's Tab is unchanged in the canonical Snapshot and in the same order. A changed member
or a fifth race gives up with the request-not-applied notice; any other refusal reports at once and
keeps every member. The hidden confirmation of an `if-idle` set goes again with each resend.

From the Server's acceptance the status line counts the set: `Closing Tabs: 1 closed, 2 remaining`,
whose last value becomes `N Tabs closed`. A member the Server found already gone counts as closed.
A set the Server stops reports `Closing stopped`, separating the Tabs left open from the one member
whose own Close may still finish, as in `Closing stopped: 1 closed, 1 unconfirmed, 1 still open`.
The Server leaves that member's Close retained; the Client keeps it as an ordinary close record, so
its status reads, its report and its Resume continue it, and its confirmed closure joins the count.
A stopped set whose members were all later confirmed closed stops reporting. A Client that loses its
connection while following a set stops following it and reports the set as stopped, while the
Server may still close the remaining members; their publications remove them.

The final report expires after five seconds or the next action or notice, using the existing
bounded-notice convention. A report is final once nothing is left in flight: the set finished, or it
stopped and the member it left, if any, reached a known outcome. A close in doubt keeps the report
on the status line through any idle time, because its outcome and Resume are still to come. This
retires only display state; retained counts and Close recovery facts survive, and current notices
take display priority. A Tab the set left behind is an ordinary open Tab: its `[x]`, Close Tab key
and Tab menu work again as soon as the set stops.

For an already presented resumable status, the exact Tab's menu offers **Resume closing this Tab**
separately from set closure, including background members. It uses the existing single-Close
confirmation and Resume operation, preserving the known request and attempt generation the set
reported. It never restarts the remaining set, submits another Start, or navigates to a different
Tab; a set has no Resume. Unknown or unseen statuses do not offer this action. The keyboard recovery
entry addresses the presented operation too. Targeted Resume freezes the current target selection,
Session/context/revision and concrete local focus; it does not reuse the retained record's older
selection revision. One predicate validates these premises at confirmation retirement, planning and
final admission. Only Resume may carry this focus fence, and the exact displayed record is required
before opening so a missing record cannot fall back to Start. An unrelated active record cannot
replace the modal's status token. The resumable single-Close status takes priority over the set
summary. Ordinary footer hints may yield so the complete recovery status can be drawn; input-mode
indicators and newer notices keep their priority. The Resume confirmation includes the set's counts
for that exact stopped member, so the summary remains available without hiding the status needed to
enter recovery. The stopped member and then its Resume confirmation take status priority over
unrelated records on the active Tab.

## Group cleanup boundary

Each member's Close publication removes only that Tab's source group when the removal leaves it
empty. Nonempty source groups and every unrelated group, including empty groups created by a peer
while disposal was in flight, survive; nothing sweeps the latest document for unrelated empties.

This intentionally differs from GUI `TabModel.closeTabs`, whose `removeEmptyGroups` prunes all
empty groups in that context. The narrower terminal lifecycle owner avoids deleting unrelated
canonical data during a long operation. Target-set semantics remain equal. The set adds no shared
selection/collapse state, persistence path or local synthetic removal.

## Evidence and limits

The owning implementation tests cover frozen ordered close sets, first-draw refusal, admitted
suspension and restoration, cancellation, the set's acceptance, selection, revision races, stopped
and lost sets and the Resume of a stopped member. `workbench_client/ui/tests/close_impact.rs` covers
the ended, mixed and unobserved close-impact counts and a settlement under an open prompt.

Maintain the concrete witnesses in `crates/workbench-client/src/workbench_state/tests/tab_close/set.rs`, `crates/workbench-client/src/workbench_client/mailbox/tests/tab_close.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
