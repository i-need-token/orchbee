# Native terminal selection and effects

## Scope and ownership

This reference governs the native Client's selection gesture, Host-owned selection projection,
plain-text export, clipboard delivery, pane-program clipboard writes, and bell delivery. Maintain it
with `tui_pointer.rs`, `terminal_session/ui.rs`, `terminal_session/connection/native.rs`,
`tui_render.rs`, `tui_backend.rs`, `local_client.rs`, the viewer relays in
`apps/orchbee-worker`, `apps/orchbee-server`, `crates/capability-node-protocol`, and
`crates/client-server-protocol`, and the selection/effect contracts in
`crates/terminal-protocol` and `apps/terminal-host`.

The Terminal Host is the sole authority for selection endpoints and exported text. The native
Client owns pointer coordination, visible projection, export assembly, and the final external
effect. Worker and Server relay exact records without interpreting selection text. The shared
[`TerminalSelectionGestureCoordination`](../../../packages/terminal/formal/tla/TerminalSelectionGestureCoordination.tla)
model owns gesture publication order; [`NativeTerminalSelectionEffects`](../formal/tla/NativeTerminalSelectionEffects.tla)
owns export completion and external-effect serialization.

Selection stays volatile and per attachment. Cross-pane selection, clipboard reads, persistent
screen or selection storage, search and copy mode, a new keyboard binding, and line selection by
triple press are outside this contract.

## Integration boundary

The current foundation contains the viewer contracts, Worker and Server relays, and native pointer
coordination. The native Client still needs selection-command publication, highlight projection,
export assembly, clipboard policy and its Win32 adapter, and the serialized effect writer. Those
consumers must implement the contract below before selection copy or pane-program effects become
available. Pointer regression convergence and native-cohort verification remain required; model
checks and relay tests alone do not establish the user outcome.

## Pointer ownership and selection

- An unmodified primary-button gesture over a pane whose current frame reports no mouse tracking
  belongs to Client selection. The same gesture over a tracking pane belongs to the pane program.
  A `Shift`-modified gesture remains host-terminal native selection and never starts Client work.
- The press captures the exact painted pane incarnation, attachment, projection generation,
  content rectangle, and live or retained-history viewport. A focus-changing press is released
  only after the same pane becomes ready; it never follows a replacement pane.
- Drag endpoints are pane-content cells. Leaving the painted content clamps the endpoint to that
  pane; chrome, borders, and neighboring panes never enter the selection.
- A single press starts a cell selection and a double press starts a word selection. Release ends
  the gesture and requests automatic copy only when the Host accepts that exact `end` with an
  active selection.
- Host absolute endpoints are projected through the current live scrollback origin or retained
  row-window `start_row`. Output revisions preserve the Host pins and move the visible highlight
  with their rows. A new projection generation, pane replacement, invalid history anchor, screen
  change, or endpoint eviction cancels local gesture work; eviction also clears the highlight with
  a localized notice.

## Gesture publication

The Client admits one selection command in flight. A pending `update` may be replaced by the newest
endpoint, but `end` remains behind the final retained `update`; `cancel` removes obsolete queued
movement. Every command keeps the generation captured by its `start`. Revision-only output does
not restamp or cancel work. A stale-generation result clears the flight and queue, releases local
capture, preserves the last Host selection, performs no copy, and emits no feature failure. Other
typed failures remain visible and are never replayed automatically.

## Export and clipboard delivery

- Only an accepted active `end` starts a plain selection export. One export is active at a time and
  is fenced by exact attachment and export sequence.
- The Client accepts at most one chunk before the start response, then requires contiguous chunk
  sequences, the announced total length, a final chunk at that exact length, acknowledgements in
  order, and a matching completed result. The existing Host bounds remain 1 MiB per chunk and
  16 MiB per export.
- Clipboard admission occurs only after the complete buffer passes strict UTF-8 decoding. A
  cancelled, foreign, malformed, partial, over-limit, or invalid UTF-8 export writes nothing.
  Lifecycle cancellation removes a completed copy while it is still queued. Once its external
  dispatch starts, success or an unknown result retires the attempt without replay.
- Windows uses the Win32 Unicode clipboard by default. An explicit OSC 52 mode selects the outer
  terminal instead. One copy uses exactly one sink: an OSC 52 write is not followed by a Win32
  fallback because OSC 52 has no positive acknowledgement.
- Clipboard work runs outside the render path. Definite failure and unknown delivery produce
  localized status; neither blocks subsequent drawing.

## Pane-program effects

The Host accepts OSC 52 writes from a pane program, rejects reads and queries, validates base64 and
UTF-8, excludes control characters, and limits text to 16 KiB. The native Client allows these
writes by default and may disable them through its clipboard policy. It never prompts per write and
uses the same configured single clipboard sink as direct selection copy.

BEL and permitted clipboard writes are independent Host streams. Each has its own positive,
strictly increasing sequence for the exact viewer attachment, so the same sequence value may be
valid once in each stream. Worker places accepted records from both streams into one bounded FIFO
in arrival order. A foreign attachment or a repeated/regressing sequence within its stream is a
protocol failure. Queue or aggregate-byte saturation never evicts an older record to admit the new
one. It instead retires only the addressed viewer and releases all of that viewer's retained records;
a sibling viewer is unaffected. Poll removes each delivered record at most once. A disconnect or an
uncertain external result does not requeue it.

## External-effect writer

One Client-owned effect writer serializes selection copies, permitted pane-program clipboard
writes, and BEL. It dispatches only after the current synchronized-output frame has closed and its
flush succeeded; a new frame cannot begin while a terminal-output effect is in flight. BEL emits
one `0x07` byte. OSC 52 emits one write-only UTF-8 payload. Win32 delivery writes one null-terminated
Unicode value through the platform adapter. No effect bytes may appear inside a DEC 2026 frame.

On Windows, the clipboard writer thread owns a dedicated hidden, nonactivating winsafe
`WindowMain` and its message pump; it does not borrow the notification window. That thread creates,
uses, and destroys the `HWND`, with no authored unsafe code. A bare
`WindowMessageOnly` is insufficient here because winsafe exposes neither a safe public pump for it
nor automatic destruction on drop. Every Win32 copy opens the clipboard with the writer's non-null
owner through
[`OpenClipboard`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard)
before calling `EmptyClipboard`; opening with a null owner is forbidden because emptying in that
state prevents a subsequent
[`SetClipboardData`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setclipboarddata).
The adapter encodes `CF_UNICODETEXT` as native little-endian UTF-16 with one trailing null code unit
and allocates movable global memory. Until `SetClipboardData` succeeds, the adapter owns that
allocation and frees it on every error. Success transfers ownership to Windows, after which
application code neither retains nor frees the `HGLOBAL`. No PowerShell or visible-window helper
participates in delivery.

Shutdown first closes effect admission and cancels every undispatched record. If a Win32 or terminal
dispatch has begun, the Client waits for its success, definite failure, or unknown result. The
writer thread then destroys its hidden owner window, exits, and is joined before retirement
completes. This keeps a non-cancellable Win32 call inside the writer lifetime and prevents replay
after teardown.

## State model

`NativeTerminalSelectionEffects.tla` composes one selection export with one bounded effect queue and
one writer. Its semantic actions cover accepted and rejected release, an early export chunk,
ordered and invalid chunks, completion, UTF-8 admission, lifecycle draining, exact and rejected pane
effects, frame open/flush, dispatch, success, unknown settlement, and retirement after the active
dispatch has settled.

| Claim                                                      | Model property                                                                   |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Partial or invalid export never reaches a sink             | `SelectionEffectRequiresCompleteExport`, `TerminalCopyNeverCommits`              |
| Selection copy commits at most once                        | `SelectionCopyCommitsAtMostOnce`, `AttemptedEffectsAreUnique`                    |
| A copy uses only its configured sink                       | `EffectsUseOneSink`, `DeniedProgramWritesHaveNoEffect`                           |
| Effects retain FIFO order and never requeue after dispatch | `QueuedEffectIdsIncrease`, `AttemptOrderIncreases`, `AttemptedEffectsNeverQueue` |
| Rendering and terminal-output effects never overlap        | `FrameAndTerminalWriterAreExclusive`                                             |
| Draining removes undispatched effects                      | `DrainingClientHasNoUndispatchedEffects`                                         |
| Retirement waits for the dispatched effect to settle       | `RetiredClientHasNoEffects`, `CoverageRetirementWaitsForDispatchedEffect`        |

TLC checks one selection copy, up to two chunks, up to three admitted effects, a two-record queue,
Win32 and OSC 52 sinks, and both pane-program-write policy values. Bytes, Unicode decoding,
geometry, transport framing, OS clipboard behavior, and real terminal rendering remain deterministic
adapter and system evidence. No liveness claim assumes that the OS or outer terminal confirms an
effect; unknown dispatch is a terminal non-replay result.

## Verification map

| Claim                                  | Lowest effective evidence                                         | Integrated evidence                                                       | Limit                                             |
| -------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------- |
| Final movement precedes release        | shared gesture TLC properties and deterministic coordinator trace | live and history rapid-release trace                                      | One pointer gesture per model run                 |
| Export commits complete Host text only | selection-effect TLC properties and chunk-state tests             | soft/hard wrap, CJK width, combining text, reverse drag, and history copy | Plain text, 16 MiB maximum                        |
| Pane effects reach only their viewer   | Worker exact-attachment FIFO and saturation tests                 | Host–Worker–Server–Client record trace                                    | Matching native cohort                            |
| Mouse ownership remains exclusive      | pointer ownership and generation-fence tests                      | `vim mouse=a`, untracked drag, and `Shift` native selection               | Cell-addressed input                              |
| External effects do not split a frame  | effect-writer state tests and backend byte trace                  | Windows Terminal OSC 52 and BEL observation                               | Host-terminal behavior outside Orchbee is trusted |
| Win32 copy is the Windows default      | non-null owner, UTF-16, ownership, contention, and teardown tests | selection, Codex/Claude, and Vim clipboard observation                    | Windows only; OSC 52 is explicit                  |

## Compatibility and recovery

Selection viewer actions and effect routing are additive contracts delivered by one exact native
Client, Server, Worker, and Host cohort. They create no durable selection or screen-data migration.
A rollback replaces the cohort together. Old binaries may continue their older behavior but cannot
use the new actions in a mixed cohort.
