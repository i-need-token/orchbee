# Native Tab close sets

## Targets and confirmation

Windows Tab menus close the selected set, other Tabs or trailing Tabs; group menus close the
complete canonical group, including collapsed members. `TabStripContextMenu` and the native owner
share the target corpus in `packages/tab-strip/src/tab-strip/model/__tests__/fixtures/tab-close-targets.json`.
If the menu target is selected, direct Close uses the selected IDs in canonical order; otherwise it
uses only that target. Other/trailing Close exclude pinned Tabs and the target, protecting the
selected set only when the target belongs to it. Trailing begins after the rightmost selected Tab
in that case. Direct selected/group Close includes pinned members where applicable.

One confirmation freezes 1–256 exact IDs, Session/context, full canonical Snapshot, names, positions,
local marks and focused selection. Focus includes the logical `PaneKey`, physical `PaneRef`
incarnation, selected Presentation and binding; a same-key reattach is a change. Background targets
are planned by exact ID without selecting them or impersonating the selected-only Close planner.
The dialog shows the count, location, first three names/positions and an explicit remainder count.
Names and location use sanitized, grapheme-safe previews capped at 64 cells (less on narrow
screens), with an explicit ellipsis for clipping; full frozen IDs and positions remain authoritative.
The draw token covers the entire bounded preview, remainder count, warning and controls.
Its localized warning covers execution in all attached Clients and incremental failure behavior.

Only a complete successful draw authorizes ordinary lowercase `y`; local `Esc` cancels an armed
confirmation even while it is not shown. An evaluated geometry block before the first successful
draw refuses the new confirmation. After a successful draw, the same block suspends the existing
confirmation while preserving its token, frozen set, local marks and focus; `y` and pointer release
are inert until the same token is completely drawn again. A missing or covered draw receipt revokes
shown authority without classifying geometry or discarding the presentation phase. Unseen,
cancelled, stale, resized or unpresentable confirmation cannot submit. The existing input owner
captures the entire decision packet, including a menu-to-confirmation handoff tail. Opening and
cancellation do not allocate a lifecycle request or remove canonical state.

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
this tab** separately from set closure, including background members. It uses the existing single-Close confirmation and Resume
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
outcomes, peer removal and survivor selection.

Maintain the concrete witnesses in `src/terminal_session/state/tab_close.rs`, `src/terminal_session/mailbox/tests/tab_close.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
