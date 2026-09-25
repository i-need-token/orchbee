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
the raw creation label. An omitted Tab label also requests [automatic positional naming](native-tab-labels.md);
explicit numeric names remain fixed. Split labels remain creation fallbacks. Existing geometry,
exec/args, profile and environment validation retain their owners. Windows directory policy is
resolved before creation as specified below; explicit paths remain per-launch overrides.

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

Windows creation emits `orchbee.creation.result` v2 after a known canonical commit. It preserves
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
The schema catalog is v36 with 69 documents; command-path count remains 81.

## Evidence and compatibility

Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` supplies optional current targeting, default
no-focus, explicit focus/no-focus and split-ratio behavior. Native IDs, Client routing, numeric
creation labels, f64 normalization and volatile Client ratios remain explicit native
contracts. Panel right-click policy is owned separately; dynamic positional naming and persistent
default settings are not claimed by this contract.

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

## Windows directory resolution model

`--cwd` accepts an absolute path or `follow`, `home`, `current`, `workspace`. Omission means
`follow`. An absolute path, the CLI's home/current directory, or `workspace` explicitly bypasses
source observation; `workspace` retains the existing Server Workspace-directory/Worker-home
default. The CLI resolves a policy into existing launch options before submitting creation.
There is no new Server setting or retained directory policy.

Follow uses the exact split source Panel. For a Tab it uses only the validated caller or already
selected Client's Panel in the destination context. Explicit headless Tab creation does not select
a Client to invent a source. A read-only viewer samples the exact primary Terminal's Windows Job;
only the row matching its root PID supplies cwd. It never uses an arbitrary child/Agent process.
A missing source or known unavailable/unusable root cwd retains launch defaults. Denial, invalid
protocol, lost transport, interruption and timeout stop before creation. No failed read causes a
canonical retry. Attachment drop/detach, reads and mutation share the command deadline.

| State     | Permitted transition and evidence                                                                                   |
| --------- | ------------------------------------------------------------------------------------------------------------------- |
| Parsed    | Policy syntax is resolved before effect; other creation inputs keep their existing validation.                      |
| Targeted  | One Snapshot fixes context, Panel and primary Terminal; unrelated Client selection is never used.                   |
| Observing | One cancellable viewer sample has no controller, focus, resize or terminal input effect.                            |
| Resolved  | A usable absolute cwd becomes the per-launch override; a known absence retains launch defaults.                     |
| Creating  | The original Snapshot revision and existing canonical carrier fence the one submission.                             |
| Finished  | Creation receipt v2 records requested policy, observed source/path and source Panel separately from launch success. |

Process cwd is a point-in-time observation, not a lease; a later process-directory change is allowed.
Directory existence can change before launch, whose existing typed failure remains authoritative.
For Windows interactive PowerShell, the Worker-owned prompt integration synchronizes FileSystem
location into process cwd. It applies only to recognized PowerShell/pwsh launches with no arguments
or the exact NoLogo/NoProfile/NoExit switches. Existing terminals and arbitrary startup commands
receive no retrofit; their observable process cwd may differ from a shell's private location.

Core policy/source tests, actual PowerShell 5.1/7 prompt checks, named-pipe cancellation/fault cuts
and Windows ConPTY creation traces cover this model. Existing phase ordering remains unchanged
after directory resolution. TLA execution remains deferred. Persistent configurable defaults remain separate;
[automatic positional Tab labels](native-tab-labels.md) preserve raw creation facts. The receipt version requires a matching
cohort; rollback does not rewrite user profiles or existing terminal processes.
