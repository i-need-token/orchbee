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
program and closes unasked under [Idle close](#idle-close). Workspace Close keeps the Server's
conditional idle admission.

The count is taken at each complete draw. A settlement arriving under an open prompt changes the
next draw without changing its token or targets. An ended terminal never runs again: a new process
needs a new binding, which changes the revision or Snapshot that the confirmation froze, so the
stale premise cancels the prompt. The executed close is therefore never more destructive than its
last complete draw; that draw can only overstate running work.

## Idle close

A close that ends no running program does not ask. This covers every route to one Tab, the key, its
`[x]`, and a Tab menu or group action that resolves to one Tab, and the Panel route. A set of several
Tabs, a Resume and a ReadOnly Client always present the confirmation.

Each route still arms the confirmation; `workbench_client/ui/idle_close.rs` keeps it hidden while it
reads the terminals the close would end: every Presentation of the target, so a Presentation the
Panel does not show counts, except one that has ended by the facts under [Close
impact](#close-impact). When nothing is left to read, the close runs at once. Two independent facts
decide each terminal:

- **The shell's report.** `TerminalProjection::shell_report` reads the terminal's latest frame. The
  managed PowerShell reader marks its prompt and each command through private markers, carried as
  `TerminalCommandObservation`: `ready` means the reader is waiting for input, and a command without
  a completion means it is still running. Those markers reach the Host only through the ConPTY the
  product bundles; the system ConHost drops them. Every console host passes OSC 133, so the frame's
  row marks answer next: a cursor on a row of the prompt class, or on the wrapped rows of its input,
  is at the prompt, and a cursor below such a row left it. The continuation class carries no fact:
  the terminal also leaves it on blank rows and program output. A screen with no mark, such as
  `cmd`, a shell whose reader the Worker's integration does not recognize, a prompt PSReadLine
  redrew after a clear, or a full-screen view, reports nothing.
- **One process sample.** `agent_start::close_sample` follows the Host's own close gate: only a Job
  member attached to the Terminal's console is a foreground program. The root must be `cmd`,
  `powershell` or `pwsh` started with only interactive switches or the Worker's fixed initialization;
  `conhost` and `openconsole` do not count, nor do detached or console-less members, so a hidden
  helper a profile started does not make a shell busy. A sample at its row limit, an unreadable root
  or a member whose console cannot be observed cannot answer. Agent start keeps its stricter rule.

The terminal runs a program when its shell reports a running command or its sample finds one; the
confirmation then presents at once. A shell reporting its prompt closes even when its sample is
refused, unavailable or late, because an unknown fact on a waiting shell is not a running program. A
shell reporting nothing is judged by its sample alone and asks when that cannot answer. Reads get
1 second, the Worker's bound on one sample; at that point, the terminals still unread close only if
each one reports its prompt. Every read publishes an outcome, so the budget is reached only by a
read that is really slow.

A terminal shown in a pane is read through its live Viewer, and its report comes from that pane's
frames. Any other terminal, in a background Tab or a hidden Presentation, is read through a temporary
Viewer that attaches, takes its first frame and a sample, and detaches, because a Client Connection
may attach to a Terminal only once.

When every terminal runs nothing, the state admits the armed confirmation without a draw and the
ordinary confirmed close runs; its result reaches the status line as `{label} · Closed`, like any
close. A shell on another platform has no process sample and presents the confirmation unless its
shell reports its prompt. The Host's `TerminalCloseActivity` stays with `WorkspaceCloseIfIdle`:
accepted input latches it to unknown, so it cannot answer for a shell a person typed into. A program
the sample cannot see and the shell does not report still closes unasked: `pause` in `cmd`, or under
the system ConHost a cmdlet whose output scrolled its own prompt off the screen.
Harnesses that script every Host exchange build the UI with `confirming_every_close`. The [real
process witness](../tests/cli_windows_idle_close.rs) drives a real PowerShell through each route,
under the system ConHost by default and the product's ConPTY with `ORCHBEE_TEST_CONPTY_DIR`, and the
[decision witness](../crates/workbench-client/src/workbench_client/ui/tests/idle_close.rs) covers
the unknown and late facts a real run cannot force.

## Incremental lifecycle owner

This is a bounded sequence over existing Close/Read/reconciliation owners, not an atomic batch.
The first confirmed admission captures one 50-second sequence cutoff. Each new step uses the
earlier of that cutoff and its ordinary operation budget; reconciliation cleanup/attach budgets
also stay within it while the sequence is active. No progress, queue wait or UI delay renews it.
At cutoff, refusal, uncertainty, unrelated change or retirement, stop new close submissions.
Already submitted work was not cancelled: preserve its `CloseRecord`, operation/alias, attempts,
disposal facts, worker handoffs and ordinary status/recovery paths. Late validated `Closed` facts
update the bounded completed/remaining counts without restarting the sequence.
The successful summary expires after five seconds or the next action/notice, using the existing
bounded-notice convention. This retires only display state; retained counts and Close recovery
facts survive. Halted/partial counts remain available, and current notices take display priority.

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
their priority. The Resume confirmation includes the retained set counts and submitted-work
warning for that exact halted member, so the summary remains available without hiding the status
needed to enter recovery. The halted member and then its Resume confirmation take status priority
over unrelated records on the active Tab.

Before each submission, read the full canonical Snapshot and compare it to the frozen source.
This preflight reserves nothing. The existing revision CAS may still reject with
`RevisionMismatch`; halt without retargeting or retry. An accepted alias is correlated by existing
`CloseRecord` rules, not by assuming its primary ID equals the new request ID.

The next target becomes eligible only after the exact correlated `Closed` result, owned membership
installation and exchange/reconciliation lease release. Immediately before installation changes
state, recheck old local marks, focused incarnation/Presentation/binding, context and full source.
After installation, require the exact previous Snapshot minus that target and the admitted group
cleanup, at exactly the next revision. Surviving trees, identities, metadata and unrelated contexts
must match. Only this owned transition may advance expected survivor selection and source revision.
A wrong/newer Snapshot or a same-tick local change halts; canonical recovery may still install
truth under its existing rules, but that installation cannot authorize the remaining set.

## Group cleanup boundary

The Server publication owner removes only the closed Tab's source group when that removal leaves
it empty. Nonempty source groups and every unrelated group, including empty groups created by a
peer while disposal was in flight, survive. The frozen next-Snapshot expectation admits exactly
this cleanup. It never sweeps the latest document for unrelated empties.

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
