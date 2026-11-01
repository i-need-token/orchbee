# Native terminal selection and effects

## Scope and ownership

This reference governs the native Client's selection gesture, Host-owned selection projection,
plain-text export, clipboard delivery, pane-program clipboard writes, and bell delivery. Maintain it
with `tui_pointer.rs`, `workbench_client/ui.rs`, `workbench_client/connection/native.rs`,
`tui_render.rs`, `tui_backend.rs`, `local_client.rs`, the viewer relays in
`apps/orchbee-worker`, `apps/orchbee-server`, `crates/capability-node-protocol`, and
`crates/client-server-protocol`, and the selection/effect contracts in
`crates/terminal-protocol` and `apps/terminal-host`.

The Terminal Host is the sole authority for selection endpoints and exported text. The native Client owns pointer coordination, visible projection, export assembly and the final external effect. Worker and Server relay exact records without interpreting selection text. Controlled gesture/export tests check publication order, completion and external-effect serialization; these adapter workflows have no separate TLA model.

Selection stays volatile and per attachment. Cross-pane selection, clipboard reads, persistent
screen or selection storage and line selection by triple press are outside this contract.
[Keyboard copy mode](native-keyboard-copy.md) separately owns its entry, cursor, search and input
capture; it consumes this same Host selection/export and clipboard channel. Selection outcomes
retain pane, generation and export sequence; keyboard copies additionally retain their episode.
Only an exact matching delivered keyboard copy exits that mode. Failed or unknown delivery is
never replayed; a later explicit copy requests a fresh export sequence.

## Integration boundary

The native Client publishes selection commands through its existing mailbox, projects Host
endpoints over live and retained-history rows, and assembles complete acknowledged Host exports.
Selection copies use a bounded FIFO and one asynchronous Windows Unicode clipboard writer,
dispatched after a successful draw flush. Its separate hidden window and thread are joined on
retirement. `clipboard-win` 5.4.1, with default features disabled and only `std`, owns the safe
clipboard allocation and transfer; its only added dependency is `error-code` 3.4.0 (both BSL-1.0).

The local carrier accepts the existing opaque Host records for selection gestures, export starts,
and export cancellation. Chunk acknowledgements accept either feedback or a typed-failure record;
the selection decoder owns the record's exact envelope and identity validation. An incompatible
action/result pair retires the carrier without resubmission.

The Server Poll route admits BEL and clipboard requests as dedicated non-frame events. It validates
the complete event envelope and exact viewer attachment, forwards the bytes unchanged, and leaves
the pending frame flight intact. A foreign or malformed record, or an effect on a different action,
still retires that viewer without resubmission.

The native Client also admits pane-program clipboard writes and BEL through the same bounded FIFO.
It validates the Host envelope, epoch, attachment, and independent stream sequences before
admission. Program effects may arrive while the pane is starting, before its first text frame.
Explicit OSC 52 delivery and reloadable clipboard policy remain to be connected. Their requirements
below describe remaining integration. Native-cohort verification and real Windows Terminal
acceptance remain required; model checks and relay tests alone do not establish the user outcome.

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
UTF-8, rejects control characters except TAB, LF, and CR, and limits text to 16 KiB. The native
Client revalidates that text and allows these writes by default without a per-write prompt, using
the same Windows sink as selection copy. Disabling program writes and selecting an explicit OSC 52
sink belong to the remaining clipboard-policy integration.

BEL and permitted clipboard writes are independent Host streams. Each has its own positive,
strictly increasing sequence for the exact viewer attachment, so the same sequence value may be
valid once in each stream. Worker places accepted records from both streams into one bounded FIFO
in arrival order. A foreign attachment or a repeated/regressing sequence within its stream is a
protocol failure. Queue or aggregate-byte saturation never evicts an older record to admit the new
one. It instead retires only the addressed viewer and releases all of that viewer's retained records;
a sibling viewer is unaffected. Poll removes each delivered record at most once. A disconnect or an
uncertain external result does not requeue it. This viewer-isolation claim belongs to Worker. Only
selection state supersedes itself: a newer selection snapshot replaces the queued one, so a resize
burst cannot saturate the FIFO. The existing native executor still retires the session when its
focus viewer is lost. Client recovery from that boundary remains separate work.

The Client FIFO holds at most eight pending entries and 16 MiB of queued plus unsettled active
payload. Saturation rejects the new effect with a localized failure notice and never replaces an
older entry. Selection entries retain their captured projection generation; program entries retain
the exact pane incarnation, attachment, and Host epoch independently of frame availability or
generation. Starting a new selection cancels only selection entries. Pane replacement, membership
removal, or retirement cancels all undispatched entries for the affected owner.

## External-effect writer

One Client-owned effect writer serializes selection copies, permitted pane-program clipboard
writes, and BEL. It dispatches only after the current synchronized-output frame has closed and its
flush succeeded; a new frame cannot begin while a terminal-output effect is in flight. BEL emits
one `0x07` byte. OSC 52 emits one write-only UTF-8 payload. Win32 delivery writes one null-terminated
Unicode value through the platform adapter. No effect bytes may appear inside a DEC 2026 frame.

The shell synchronously emits BEL through the admitted inherited-terminal lifecycle writer after
`after_draw`. The lifecycle refuses a bell while a frame is open, after restoration, or after output
has been fenced. A failed write or flush is an unknown terminal result, fences future frame output,
and ends the loop without replay. Clipboard dispatch remains asynchronous, so subsequent frames
may draw while the Win32 call runs.

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

Selection export and external effects retain these implementation obligations:

- Partial or invalid export never reaches a sink, and one copy commits at most once.
- A copy uses only its configured sink; denied program writes have no effect.
- Admitted effects preserve FIFO order and never requeue after dispatch.
- Rendering and terminal-output writes never overlap.
- Draining removes undispatched effects; retirement waits for an already dispatched effect to settle.

Controlled export/queue/writer tests and integrated traces in the verification map check these obligations. Bytes, Unicode, geometry, framing, clipboard behavior and rendering remain concrete adapter/system evidence. Unknown dispatch is a terminal non-replay result; no liveness claim assumes OS or outer-terminal confirmation. There is no separate selection-effects TLA model.

## Verification map

| Claim                                  | Lowest effective evidence                                         | Integrated evidence                                                       | Limit                                             |
| -------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------- |
| Final movement precedes release        | controlled gesture and coordinator traces                         | live and history rapid-release trace                                      | Named pointer traces                              |
| Export commits complete Host text only | controlled export and chunk-state tests                           | soft/hard wrap, CJK width, combining text, reverse drag, and history copy | Plain text, 16 MiB maximum                        |
| Pane effects reach only their viewer   | Worker exact-attachment FIFO and saturation tests                 | Host–Worker–Server–Client record trace                                    | Matching native cohort                            |
| Mouse ownership remains exclusive      | pointer ownership and generation-fence tests                      | `vim mouse=a`, untracked drag, and `Shift` native selection               | Cell-addressed input                              |
| External effects do not split a frame  | effect-writer state tests and backend byte trace                  | Windows Terminal OSC 52 and BEL observation                               | Host-terminal behavior outside Orchbee is trusted |
| Win32 copy is the Windows default      | non-null owner, UTF-16, ownership, contention, and teardown tests | selection, Codex/Claude, and Vim clipboard observation                    | Windows only; OSC 52 is explicit                  |

## Compatibility and recovery

The maintained deterministic evidence includes
`accepted_release_and_complete_host_export_reach_the_copy_queue_once`, which runs the real
WorkbenchClientUi/mailbox command and Host-record intake boundary without changing the shared clipboard,
and `reverse_drag_uses_absolute_rows_and_highlights_a_wide_glyph_as_one_unit`. Copy-queue tests
exercise cancellation before dispatch, retirement after dispatch, definite adapter loss, and
unknown loss without replay. The native hidden-owner test creates and pumps a real Windows window,
injects the final write, and verifies dispatch and join; it does not prove OS clipboard contents.
Real Windows Terminal clipboard-content and pointer acceptance remain required.

`selection_replies_cross_the_local_carrier_without_retiring_or_replaying` exercises every selection
action through the real local IPC codec. `selection_wrong_reply_pairings_retire_without_replay`
checks that incompatible replies still close that carrier without another request. These transport
tests relay opaque records; Host-record decoding and OS contents have separate evidence owners.

Selection viewer actions and effect routing are additive contracts delivered by one exact native
Client, Server, Worker, and Host cohort. They create no durable selection or screen-data migration.
A rollback replaces the cohort together. Old binaries may continue their older behavior but cannot
use the new actions in a mixed cohort.
