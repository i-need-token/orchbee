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
[close impact](#close-impact) counts it, and that Tabs close one at a time with further closes
stopped by failure.

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
`[x]`, and a Tab menu or group action that resolves to one Tab, and the Panel route. A set of several
Tabs, a Resume and a ReadOnly Client always present the confirmation and send nothing conditional.

The Server decides, by the rules of its
[conditional Tab and Panel Close](../../orchbee-server/docs/terminal-tab-close.md#tab-and-panel).
Each route still arms the confirmation; `workbench_client/ui/idle_close.rs` keeps it hidden and
sends `tab.close-if-idle` or `panel.close-if-idle` as the close's first exchange,
`CloseOperationKind::CloseIfIdle` or `PanelIntent::CloseIfIdle`. Accepted, the close goes on as a
confirmed one: a Tab close is retained only now, so the Tab leaves the strip only once the Server has
accepted it, and a Panel close applies as the ordinary one. `confirmation-required` presents the
armed confirmation, and confirming it sends the ordinary close under a new request ID. Any other
refusal drops the confirmation and reports as the ordinary close does. An unknown outcome is
followed by status reads and never sent again, and no confirmation follows it, since confirming
could close the Tab twice.

The hidden confirmation still owns input, so `Esc` cancels it: a later `confirmation-required` then
presents nothing, but an accepted close goes on. A close the Client cannot send at once, because
another close exchange is in flight or the mailbox refuses it, presents the confirmation instead.
Harnesses that script every Server exchange build the UI with `confirming_every_close`. The [real
process witness](../tests/cli_windows_idle_close.rs) drives a real PowerShell through each route,
under the system ConHost by default and the product's ConPTY with `ORCHBEE_TEST_CONPTY_DIR`, and the
[route witness](../crates/workbench-client/src/workbench_client/ui/tests/idle_close.rs) covers which
closes ask the Server and which present at once.

## Close status

A close of one Tab behaves the same from every route. The Tab leaving reports a normal close, so it
adds no notice and no count. From the draw after its close is submitted, or accepted when the Server
decided it unasked, while that close is progressing normally, the Client treats the Tab as gone
instead of waiting for the Server's publication to remove it from the canonical list: the strip
leaves it out, next, previous and numbered Tab selection skip it and count positions without it, and
nothing selects it. Once the Server accepts the close of the selected Tab, the selection moves to
the next Tab still shown, else the previous one, the survivor the removal itself selects, so the
Panel goes with the strip entry while the terminal ends in the background. The last Tab leaves the
empty Workbench in the Panel at once. While several Tabs close as one sequence the selection stays,
because a focus change stops the sequence. A refused, blocked, unknown or unobserved close shows the
Tab again with its report, and the selection stays where it moved; a Tab of a set leaves as its own
close is submitted. Until 1 second after submission, or after the user asked for a close the Server
accepted unasked, an unreported close also withholds the selected Tab's connection and exit cues;
the Panel title still says how its terminal ended. On a debug build a confirmed close shows the
next Tab 40 to 60 ms after it starts. A close the Server decides unasked waits for its answer, 15
to 35 ms after the user asks: a background Tab then leaves the strip by 45 ms, and the selected Tab
leaves with the Panel moving to the next Tab 80 to 105 ms after the ask, since a selection change
keeps the last frame until the next Tab's first one. A PowerShell Tab at its prompt leaves the
canonical list 0.1 to 0.15 seconds after its close starts, but a slower shell or a busy machine
would flash the 200 ms delay of other pending indications. Past the delay `{label} · Closing`
appears, as when a program running in the shell keeps the console after Terminate ends the shell
and the close waits out the five-second grace.

Status reads of a retained close are 250 ms apart, except that a Workbench publication lets the
next one go at once: the Server publishes a closed Tab as it finishes, and the close set holds
every refresh until a read confirms it, so the Tab leaves without waiting out the interval. A wake
admits one read; reads stay one at a time.

Blocked, unknown, unavailable and uncertain closes report at once. A lone Tab's cutoff reads `Close
not confirmed`, and a later validated `Closed` clears it. A refusal keeps the Tab and reports only
its notice. A local change around a lone Tab does not halt it, because no later Start exists to
stop.

Several Tabs show their running count from the first draw. Its last value becomes `N Tabs closed`.
A halted count separates the Tabs still open from the one submitted close whose outcome is unknown,
as in `Closing stopped: 1 closed, 1 unconfirmed, 1 still open`, and reports a refused member itself,
without a separate notice. A halted set whose members were all later confirmed closed stops
reporting. `close_status` stays unfiltered because Client control uses it as a busy gate; rendering
reads `announced_close_status`.

## Incremental lifecycle owner

This is a bounded sequence over existing Close/Read/reconciliation owners, not an atomic batch.
The first confirmed admission captures one 50-second sequence cutoff. Each new step uses the
earlier of that cutoff and its ordinary operation budget; reconciliation cleanup/attach budgets
also stay within it while the sequence is active. No progress, queue wait or UI delay renews it.
At cutoff, refusal, uncertainty, retirement, or an unrelated change while another member remains,
stop new close submissions.
Already submitted work was not cancelled: preserve its `CloseRecord`, operation/alias, attempts,
disposal facts, worker handoffs and ordinary status/recovery paths. Late validated `Closed` facts
update the bounded completed/remaining counts without restarting the sequence.
The final report of several Tabs expires after five seconds or the next action/notice, using the
existing bounded-notice convention. A report is final once nothing is left in flight: the set
finished, or it stopped and its last submitted close, if any, reached a known outcome. A close in
doubt keeps the report on the status line through any idle time, because its outcome and Resume are
still to come. This retires only display state; retained counts and Close recovery facts survive,
and current notices take display priority. A Tab the set left behind is an ordinary open Tab: its
`[x]`, Close Tab key and Tab menu work again as soon as the set stops.

For an already presented resumable status, the exact Tab's menu offers **Resume closing
this Tab** separately from set closure, including background members. It uses the existing single-Close confirmation and Resume
operation, preserving the known request and attempt generation. It never restarts the remaining
set, submits another Start, or navigates to a different Tab. Unknown or unseen statuses do not
offer this action. The keyboard recovery entry addresses the presented operation too.
Targeted Resume freezes the current target selection, Session/context/revision and concrete local
focus; it does not reuse the retained record's older selection revision. One predicate validates
these premises at confirmation retirement, planning and final admission. Only Resume may carry
this focus fence, and the exact displayed record is required before opening so a missing record
cannot fall back to Start. An unrelated active record cannot replace the modal's status token.
The resumable single-Close status takes priority over the set summary. Ordinary footer hints may
yield so the complete recovery status can be drawn; input-mode indicators and newer notices keep
their priority. The Resume confirmation includes the retained set counts for that exact halted
member, so the summary remains available without hiding the status needed to enter recovery. The halted member and then its Resume confirmation take status priority
over unrelated records on the active Tab.

Before each submission, read the full canonical Snapshot and compare it to the frozen source.
The comparison leaves out only what the Server publishes on its own as a first terminal moves: the
revision, every Workspace's followed directory and an automatic Workspace name. Closing a first
terminal causes that change itself, in a revision of its own and at any moment around the close, so
it never stops a set; every other difference does. The Close command carries the revision this read
returned. This preflight reserves nothing. The existing revision CAS may still reject with
`RevisionMismatch`, which refuses that request whole: the same target goes again under a new request
ID and a new preflight, which decides whether the change that moved the revision stops the set. The
sequence cutoff bounds these. One Tab has no sequence to keep alive and reports the refusal instead.
An accepted alias is correlated by existing `CloseRecord` rules, not by assuming its primary ID
equals the new request ID.

The next target becomes eligible only after the exact correlated `Closed` result, owned membership
installation and exchange/reconciliation lease release. Immediately before installation changes
state, recheck old local marks, focused incarnation/Presentation/binding, context and full source.
After installation, require the previous Snapshot minus that target and the admitted group cleanup,
under the same comparison, at or after the revision its `Closed` receipt names. Surviving trees,
identities, metadata and unrelated contexts must match. Only this owned transition may advance
expected survivor selection and source, which becomes the Snapshot it installed. A Snapshot that
differs in any other way or a same-tick local change halts; canonical recovery may still install
truth under its existing rules, but that installation cannot authorize the remaining set.

## Group cleanup boundary

The Server publication owner removes only the closed Tab's source group when that removal leaves
it empty. Nonempty source groups and every unrelated group, including empty groups created by a
peer while disposal was in flight, survive. The frozen expectation admits exactly this cleanup. It
never sweeps the latest document for unrelated empties.

This intentionally differs from GUI `TabModel.closeTabs`, whose `removeEmptyGroups` prunes all
empty groups in that context. The narrower terminal lifecycle owner avoids deleting unrelated
canonical data during a long operation. Target-set semantics remain equal. No new wire command,
shared selection/collapse state, persistence path or local synthetic removal is introduced.

## Evidence and limits

The owning implementation tests cover frozen ordered close sets, exact membership validation,
first-draw refusal, admitted suspension and restoration, receipt retention, cancellation, unknown
outcomes, peer removal and survivor selection. `workbench_client/ui/tests/close_impact.rs` covers
the ended, mixed and unobserved close-impact counts and a settlement under an open prompt.

Maintain the concrete witnesses in `crates/workbench-client/src/workbench_state/tab_close.rs`, `crates/workbench-client/src/workbench_client/mailbox/tests/tab_close.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
