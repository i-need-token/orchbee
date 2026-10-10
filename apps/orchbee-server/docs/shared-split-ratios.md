# Shared split ratios

## Scope and ownership

Workbench owns the nominal proportion of each binary split. Adjusting a split in one Windows TUI
Client changes the proportions seen by every Client and retained by the restart checkpoint. Each
Client still owns its viewport, focus and zoom. Terminal dimensions remain the controller's
projection; changing a shared ratio never acquires terminal control or replays input.

This contract owns canonical ratio mutation and its native Client refinement. Keyboard resize,
divider gestures, live upgrade and offline downgrade conversion are separate capabilities. The
existing CLI `panel resize`, `layout set-split-ratio` and layout export/apply are consumers.

## Data and boundary

A canonical `PanelNode::Split` contains a finite `ratio` in `0.1..=0.9`, meaning the first ordered
slot's share before cell rounding. Absence means `0.5`; serialization omits the equal default.
The split ID and both ordered slot IDs identify the split. New splits start equal; moving an
existing subtree retains its ratios, while removing that split removes its proportion.

`layout.set-split-ratio` is an authenticated Workbench command with a Site, request ID, exact
expected Workbench revision, TerminalContext, Tab, split identity, previous ratio and requested
ratio. The existing navigation mutation grant admits it. Neither viewport geometry nor process
handles are accepted as canonical state. Invalid ratios reject at the bounded protocol boundary;
CLI normalization retains its existing finite-number and clamping rules.

Workbench protocol version 1 admits this additive field and command within a matched executable
cohort. Old documents without ratios remain readable. Checkpoint version 5 captures shared ratios;
versions 1–4 restore equal ratios and cannot introduce nondefault proportions. Session catalog
version 4 is published before activating or opening upgraded storage. Earlier cohorts reject that
record without changing its bytes. Catalog versions 3 and 4 use the existing framed checkpoint
store; the frame format, three-copy retention, byte bounds and autosave timings do not change.
Rollback uses another Site or a separately authorized offline conversion. No live Client override
import, automatic downgrade or cross-cohort attachment is supported.

## Canonical transition

The existing Workbench authority and request ledger own one atomic commit of the candidate
projection, publication and retained result. Validation, allocation, encoding and ledger capacity
checks precede it. A post-commit transport failure cannot undo the ratio.

| Event or guard                                                    | State and result                                                                                    |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Unauthorized, wrong Site, retired authority or exhausted capacity | Existing typed denial/unavailability; no ratio write                                                |
| Same request ID and same complete command                         | Replay the retained result without another delta or revision                                        |
| Same request ID with different command                            | Request identity conflict; no write                                                                 |
| Wrong revision or previous ratio                                  | Revision mismatch; no write and no automatic retry                                                  |
| Missing context/Tab/split, or either ordered slot replaced        | Typed unavailable target; no write                                                                  |
| Context has a pending close fence                                 | Terminal busy; no write                                                                             |
| Requested ratio equals current ratio                              | Retained `unchanged`; revision and publication unchanged                                            |
| All guards pass and ratio changes                                 | Exactly one revision advance, canonical ratio replacement and publication; `split-ratio-configured` |
| Restart from an admitted checkpoint                               | Reconstructed graph preserves nominal ratios without live bindings or controller grants             |

Ratios do not alter membership, labels, selection or the allocation high-water counter. A stale
Client cannot assign a previously resolved resize delta to a replacement split. Concurrent
commands against the same revision cannot both commit; the losing caller must explicitly obtain a
new view before issuing a new operation.

## Native Client workflow

The UI resolves geometry once against an admitted canonical Snapshot and its rendered cache.
Resize preserves the existing nearest eligible border, direction, signed amount and clamp rules;
the path setter retains its optional expected identity. An unchanged ratio or absent border is a
no-op without a command, requested draw or terminal resize.

| Phase and event                                                            | Next state and observable effect                                                                  |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Prepared changed adjustment                                                | One owned cancellable worker submits the canonical command; UI performs no optimistic ratio write |
| Definitely not submitted or typed rejection                                | Rejected receipt; displayed and canonical state are unchanged by this operation                   |
| Submission has an unknown outcome                                          | Incomplete receipt; preserve observed state, never replay the command automatically               |
| Known canonical commit on inactive/unvisited Tab                           | Acknowledge the committed revision without selecting, attaching or drawing the target             |
| Known canonical commit on selected Tab                                     | Await the existing publication refresh and one successful compatible draw, then acknowledge       |
| Input, modal interruption, expiry, disconnect or shutdown after submission | Incomplete or unsettled result; no rollback claim, no implicit resubmission                       |
| Publication after acknowledgement                                          | Normal shared projection; never add the original delta again                                      |

The worker has a deadline and a shutdown wake endpoint; its creator joins it before releasing its
resources. Heartbeats remain independent. A stale completion cannot settle a different operation.
Selected success requires the committed ratio in a rendered cache at or beyond the committed revision while preserving
the original selection and zoom. Failure after commit may leave the new ratio visible. Successful
drawing does not claim that the asynchronous Host resize has completed.

Every tree projection reads canonical ratios: drawing, terminal sizing, pointer regions and layout
inspection. Axis space excludes the shared divider; existing half-up cell rounding, minimum-size
clamping, title rows, zoom and small-viewport collapse remain authoritative. Nominal ratios stay
observable even where cell rounding or minimums constrain the physical result. Read-only panes
observe the layout while the current controller alone resizes their terminals.

## Correctness and verification

| Claim                                                                  | Evidence and refinement                                                                                       |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| One writer, exact identity/revision fences, replay and no-op stability | Workbench authority deterministic and generated state traces, with serialized replay and reduction            |
| Bounded finite field and command in both languages                     | Rust codec, independent TypeScript/Zod and JSON Schema contract cases                                         |
| Commit, publication, draw and interruption are independently ordered   | Deterministic Client control and worker traces cover commit, publication, drawing and interruption            |
| Old default, upgraded admission and durable preservation               | Checkpoint roundtrip/version rejection and Windows catalog admission cases                                    |
| Shared nominal ratios refine into actual geometry                      | Windows ConPTY two-Client, controller/read-only, detach/reattach, stop/restart and layout export/apply traces |

The abstract model checks one admitted Site generation, two Clients, two split incarnations,
three ratio classes and one request per Client; selected/inactive phases are explicit. Checkpoint
publication and activation remain separate storage/startup models rather than multiplying their
state spaces into the ratio-commit model. Its only fairness assumption is eventual Server resolution
of a fully submitted request; UI interruption and missing receipts do not assume successful drawing.
It does not prove
unbounded topology, Win32 I/O or scheduler fairness beyond its declared assumptions. Generated
hierarchy traces complement that bound. Real Windows traces exercise normal binaries and pipes;
physical reboot, human Windows Terminal interaction and other platforms remain separate acceptance
evidence. The repository merge gate applies in addition to focused checks.

Selected success can use a later revision while the split identity and committed ratio still match; the receipt records the drawn cache revision. `splitRevision` uses the projected cache's canonical Workbench revision.
