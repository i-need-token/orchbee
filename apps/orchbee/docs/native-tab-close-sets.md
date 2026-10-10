# Native Tab close sets

## Targets and confirmation

Windows Tab menus close the selected set, other Tabs or trailing Tabs; group menus close the
complete canonical group, including collapsed members. `TabStripContextMenu` and the native owner
share the target corpus in `packages/tab-strip/src/tab-strip/model/__tests__/fixtures/tab-close-targets.json`.
If the menu target is selected, direct Close uses the selected IDs in canonical order; otherwise it
uses only that target. Other/trailing Close exclude pinned Tabs and the target, protecting the
selected set only when the target belongs to it. Trailing begins after the rightmost selected Tab
in that case. Direct selected/group Close includes pinned members where applicable.

One confirmation freezes 1–256 exact IDs, Site/context, full canonical Snapshot, names, positions,
local marks and focused selection. Focus includes the logical `PaneKey`, physical `PaneRef`
incarnation, selected Presentation and binding; a same-key reattach is a change. Background targets
are planned by exact ID without selecting them or impersonating the selected-only Close planner.
The dialog shows the count, location, first three names/positions and an explicit remainder count.
Names and location use sanitized, grapheme-safe previews capped at 64 cells (less on narrow
screens), with an explicit ellipsis for clipping; full frozen IDs and positions remain authoritative.
The draw token covers the entire bounded preview, remainder count, warning and controls.
Its localized warning covers execution in all attached Clients and incremental failure behavior.

Only a complete successful draw arms the ordinary `y`/Esc confirmation. Unseen, cancelled,
stale, resized or unpresentable confirmation cannot submit. The existing input owner captures the
entire decision packet, including a menu-to-confirmation handoff tail. Opening and cancellation
do not allocate a lifecycle request or remove canonical state.

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

| Claim                                     | Refinement/evidence                                                                                                                                                                            | Limit                                                                        |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Exact selected/relative/group IDs         | GUI menu helper and native `close_set_targets` consume the same independently specified corpus                                                                                                 | Canonical target policy, not disposal                                        |
| No next Start after halt or cutoff        | `NativeTabCloseSet` models confirmation, preflight/CAS race, local drift, wrong/newer install and late facts                                                                                   | Two frozen targets, one peer edit and one local edit; finite safety only     |
| Owned survivor transitions only           | State traces use real Close records and membership commits, including same-tick marking and same-`PaneKey` reincarnation before install                                                        | In-memory native lifecycle owner                                             |
| No mutation replay after a preflight race | Scripted native carrier observes Snapshot read followed by exactly one old-revision CAS or a definite unsent result                                                                            | Carrier boundary; no real Host disposal                                      |
| Group ownership preserves peers           | Server publication test closes the last members while preserving nonempty and preexisting/peer-created empty groups                                                                            | Canonical publication owner                                                  |
| Honest confirmation/input                 | Full TestBackend screens and raw keyboard/pointer routes in both locales, standard/compact and unpresentable sizes; 512-scalar ASCII, CJK and combining/emoji previews under native/RGB themes | Composed cell/input evidence, not physical font or live Desktop verification |

`NativeClientCloseControl` and `NativeClientCloseHandoff` remain the detailed alias, transport,
recovery and lease models. The [single-Close contract](native-tab-session.md#close-control-and-recovery-contract)
still governs each submitted member. The [capability matrix](native-tab-groups.md#actual-host-capability-comparison)
records the remaining host gaps. The complete candidate gate belongs to integration; these focused
checks do not establish universal GUI/TUI parity or unconditional eventual closure.
