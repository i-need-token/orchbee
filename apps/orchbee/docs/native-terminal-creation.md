# Native terminal creation

## Inputs and ownership

Windows `tab create` and `panel split` create one canonical resource before optional Client-local
work. They use an existing Site; they never start a Site or retry an uncertain creation.
`--focus` and `--no-focus` are exclusive, with no focus as the default. `--client` names one exact
interactive Client. The total `--timeout` defaults to 30000 ms and admits 1 through 3600000 ms.
There is no detached mode or caller-supplied creation identity.

`tab create` accepts exclusive `--workspace` or `--standalone`. Without either, use the validated
calling Panel's context, then an unambiguous current Client's selected context. `panel split`
accepts an optional positional Panel or exclusive `--panel` / `--current`; omission uses validated
caller context, then the selected Client's Panel. Explicit Client routing ignores caller environment.
Partial, malformed or stale caller identity never falls back to another target. Explicit context
or Panel with no Client work remains usable headlessly. Multiple viable Clients are an error.

Creation labels are optional. The native fallback is the context's Tab count plus one for a Tab,
or the owning Tab's Panel count plus one for a split, captured from the preflight Snapshot. It is
a creation label, not a dynamic terminal title or a unique identifier. Existing geometry,
exec/args, profile, cwd and environment validation retain their owners. Omitted cwd retains the
native Workspace-directory/Worker-home policy; following an active process's cwd is separate work.

`panel split --ratio` supplies the first child's share, independent of split direction. Finite
numbers clamp to 0.1–0.9. The ratio belongs only to the selected TUI session; other Clients retain
their own weights. A ratio does not imply focus. Right-click input policy is a separate capability.

The invocation parser owns optional input. The CLI creation workflow fixes exact identities and
owns phase order. Existing Workbench mutation resolution and typed Tab/Panel creation carriers
own the one canonical command. Server/Worker/Host retain topology, launch and terminal lifecycle.
The TUI owns its ratio and focus through existing Client control. No owner acquires another's state.

## Workflow and guards

| Phase     | Guard and result                                                                                                                                                                                                                                                                                                                                |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Preflight | Parse every input; arm interruption; admit one session and read one Snapshot. Resolve explicit/caller targets. Select a ready Client before mutation whenever routing, ratio or focus requires one.                                                                                                                                             |
| Create    | Submit one exact canonical Tab create or Panel split. Preserve its Pending/Created launch facts and allocated identities. A refusal or unknown outcome stops all later phases.                                                                                                                                                                  |
| Observe   | After a known commit, observe the original Client at or beyond that revision; unchanged empty views can use command-driven refresh below. Context, Tab, Panel and zoom must remain selected as before; only the count increase caused by this creation is allowed. An empty destination may naturally select the exact first created Tab/Panel. |
| Ratio     | Read one fresh canonical Snapshot, locate the committed split and both ordered slots in the original Tab, and submit one path setter carrying that exact expected identity. A replaced split or slot is a refusal, never a different target.                                                                                                    |
| Focus     | If requested, observe the original Client again and submit one focus to the newly created Panel. Existing view-revision, interruption, attachment and draw guards apply.                                                                                                                                                                        |
| Finish    | Report canonical creation separately from requested Client effects. A later failure, signal or deadline never hides a known creation or ratio result.                                                                                                                                                                                           |

All phases share one absolute deadline. Reads may poll within that budget; canonical and Client
submissions are never replayed. Signals stop future phases and waiting, not already accepted effects.
There is no automatic close, inverse ratio or other compensation. Known canonical commits survive
all later failures. An unknown canonical result never causes a speculative ratio or focus.

Empty views have no terminal poll to carry publication hints. When the original empty selection is unchanged and its observed revision has not regressed, submit the pinned Client command to drive the existing refresh instead of waiting for a publication that cannot arrive. The command still carries the post-create minimum revision and freshly observed view revision. If the empty view already displays its first Tab, only this exact created Tab/Panel transition is allowed.

Client selection is compared at observations; selection ABA between observations is not detected.
Each Client submission also carries its freshly observed view revision. A concurrent canonical
change can make the captured path stale; the expected split/slot identity prevents it from adjusting
another split. Selected ratio changes require drawing; inactive ratios store without selecting.
Focus and ratio are not atomic with creation or with each other. A successful draw is not a Host
resize acknowledgment. The canonical terminal launch may still be pending or have failed.

## Receipts and recovery

Windows creation emits `orchbee.creation.result` v1 after a known canonical commit. It preserves
the existing canonical `source` and `result` facts and separately records `postCreate`: requested
ratio/focus, exact Client when needed, each submitted Client observation, stage and failure.
Only completion of every requested Client phase is process success; a canonical commit alone does
not imply requested focus succeeded. Headless creation with no requested Client work is complete
after the canonical receipt. Failures before a known commit retain the existing mutation failure
envelopes, including request identity and outcome-unknown semantics.

JSON emits one complete bounded document to stdout. Failed text output goes to stderr and includes
known effects. After an incomplete result, inspect the returned identities and current state before
choosing a follow-up; repeating creation may create another resource. Client control-status can
observe a submitted Client operation. No durable workflow journal or restart restoration is added.
The schema catalog is v31 with 66 documents; command-path count remains 79.

## Evidence and compatibility

Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` supplies optional current targeting, default
no-focus, explicit focus/no-focus and split-ratio behavior. Native IDs, Client routing, numeric
creation labels, cwd policy, f64 normalization and volatile Client ratios remain explicit native
contracts. Right-click, dynamic naming and cwd-following are not claimed by this slice.

Grammar and completion witnesses cover omission, explicit forms, ambiguity, conflicts and bounds.
Windows named-pipe process traces fault each phase and assert no replay and known-effect retention.
Both canonical carriers exercise cancellation wake. Independent Rust, TypeScript and JSON Schema
validators cover optional split identity and mismatched receipts; the TUI rejects identity/slot
replacement before writing ratios. Two real Windows TUIs check headless/default/explicit focus,
nested ratios, existing terminal identity and other-Client isolation. A finite phase model records
ordering; TLA execution remains user-deferred. Same-author review is not independent review.

The additive Client guard requires a matched development cohort; older strict readers may reject
it. Existing explicit non-Windows creation keeps its former route; new platform runtimes are not
added. There is no durable migration. Source rollback is coordinated; already created resources
remain user-owned and are closed only by an explicit later action.
