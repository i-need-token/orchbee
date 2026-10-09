# Native keyboard copy mode

## Scope and ownership

On Windows, `Ctrl+B U` enters keyboard copy mode for the exact focused pane. `[keys] copy_mode`
can replace or disable that entry through the existing resolved binding registry. The uppercase
entry does not replace context switching with `Ctrl+B [` or `Ctrl+B ]`.

`NativeUi` owns the volatile episode, pane incarnation and integration. Its deterministic
`terminal_copy_mode` core owns cursor, search draft, selection and pending intent. `ClientView`
only projects whether that owner is active. `TerminalViewport` retains its existing history
ownership; the Terminal Host remains the authority for selection endpoints and exported text.
The existing effect queue and clipboard writer own system submission. Nothing is checkpointed.

`terminal_content` owns exact content provenance and bounded reads on an independent read-only
connection. Both link activation and `terminal_copy_query` consume it; URL/plugin/browser routing
remains link-owned. Copy queries have no terminal input, controller or mutation capability.
No new dependency or public protocol is introduced.

## Entry and content surface

| Surface                                                         | Entry and navigation                                                                                                                                                    |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Primary screen, live                                            | A copy-only entry permits a known nonempty frame even with zero scrollback. `Tail(0)` enters history and waits for accepted `RowsApplied` before using coordinates.     |
| Primary screen, committed history                               | Reuse that exact committed window; do not submit a second history entry.                                                                                                |
| Alternate screen, live                                          | Use the exact current live frame without history demands. Vertical/page movement and `g`/`G` stay within the screen; unavailable history never becomes fabricated rows. |
| Missing frame, unusable geometry or history entering/recovering | Refuse with a localized notice; send no pane input and do not infer a position from stale paint.                                                                        |

The ordinary PageUp/wheel history gate and return-at-tail behavior remain owned by
[native terminal history viewport](native-terminal-viewport.md). Copy navigation adds an absolute
`Newest` intent (`Tail(0)`) that remains in history. Downward copy navigation uses that intent when
necessary rather than the ordinary return-at-tail path.

Use the visible application cursor only when its exact screen, Host epoch, generation, revision
and columns match the admitted surface and its absolute row is in that surface. Otherwise start
at its bottom-left cell. This initial-position projection is adopted from local Herdr revision
`e0507237ad8d41f60aece10d00ef29ce2339ca34`, `src/client/shell/copy_mode.rs`; this is not a complete
Herdr compatibility claim. Normalize movement to complete displayed glyphs.

Copy mode paints every served content row. It omits the ordinary history badge and bottom line
inside the pane; the outer status area shows the persistent mode indicator, hints and search
draft. Within the width left by the indicator, search draft and notice, the hint shows the widest
complete localized action set that fits — full, then short — and otherwise keeps only the
persistent exit hint; a hint segment is never partially drawn. The cursor, selection, read
coordinates and painter use the same content geometry.

## Input ownership and bindings

An exclusive editor, confirmation, popup or machine surface has precedence. Copy mode then owns
the whole decoded input packet before the prefix, board, ordinary history, configured direct
bindings and terminal passthrough. Entry cancels the preceding pointer gesture, resize mode,
help and armed prefix. Modes do not nest. The packet containing entry or exit remains captured;
trailing bytes, releases, unknown sequences and malformed or over-bound paste never reach the pane.
The pane task and output polling continue.

| Input             | Intent                                                                    |
| ----------------- | ------------------------------------------------------------------------- |
| Arrows, `h j k l` | Move one glyph or row                                                     |
| `w b e`, `W B E`  | Word or whitespace-delimited word movement through existing copy policies |
| `{ }`             | Previous/next paragraph through existing copy policies                    |
| PageUp/PageDown   | Move one viewport page                                                    |
| `g G`             | Oldest/newest available content; alternate screen uses its first/last row |
| `/ ?`             | Forward/backward literal search                                           |
| `n N`             | Repeat the last search direction or its reverse                           |
| `v`, Space        | Start or cancel selection                                                 |
| `y`, Enter        | Explicitly request selection copy                                         |
| F1                | Show this mode's registry-derived help                                    |
| `q`, Esc          | Cancel and return to ordinary input; Esc also exits from the search draft |

While editing a search, printable characters including `q` edit the query. Enter submits it.
The draft uses the existing inline editing conventions, whole-grapheme deletion and atomic
paste, with a 512-Unicode-scalar bound and a visible refusal on overflow. Queries containing
uppercase characters are case-sensitive; others are case-insensitive. Empty search performs no
content read. Search results wrap through the existing `terminal_copy::search` policy.

Search command classification uses the existing canonical key identity: a raw carriage return
and named Enter both submit; Ctrl+H and Backspace delete a whole grapheme. Printable insertion
retains the original Unicode scalar and case. A refused paste or edit keeps its draft and caret
and shows a warning beside the exit hint. Repeated `n` follows the last submitted `/` or `?`
direction; repeated `N` always follows its reverse without changing that remembered direction.

During a query, export or clipboard flight, incompatible new actions report Busy and are not
queued or replayed. Cursor movement while a viewport or selection update settles is represented
by one latest desired endpoint. A copy request cannot bind to an older endpoint: it reports Busy
until the requested endpoint and Host gesture have settled. Esc remains available.

## State and transitions

The core has typed intents and effects; transport, clocks, threads, clipboard and UI handles
remain in adapters. Episode/query/export identities increase without reuse; exhaustion refuses
new work. The wrapper holds the exact `PaneRef`, active screen kind and content fence.

| Event                                                                        | Guard and result                                                                                                                                                                                            |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Enter                                                                        | Read-ready exact pane; install one episode and the appropriate surface, or report refusal.                                                                                                                  |
| Primary window installed                                                     | Paint may update, but selection/query coordinates wait for exact accepted `RowsApplied`.                                                                                                                    |
| Cursor move                                                                  | Validate whole-glyph target; navigate if needed, then update selection only on the committed target surface.                                                                                                |
| Revision-only output                                                         | A valid primary committed anchor rebases cursor by its previous local offset. Alternate preserves local cell. Invalidate prior query matches; do not reinterpret old absolute rows as permanent identities. |
| Query                                                                        | Capture episode/query, pane/member, screen, Host epoch, generation, revision, columns, total and committed surface; admit one bounded read.                                                                 |
| Query completion                                                             | Adopt only a matching current token and complete source fence; drift/failure reports a notice and leaves the mode open, without automatic retry.                                                            |
| Start selection                                                              | Cancel obsolete selection effects, then use the semantic Host gesture adapter, independent of application mouse tracking.                                                                                   |
| Copy                                                                         | End the settled gesture and export, or explicitly re-export an active settled selection after a previous failed attempt.                                                                                    |
| Validated export                                                             | Admit complete acknowledged UTF-8 text with pane/generation/export/episode identity; queue refusal becomes a matching failure, never a waiting success.                                                     |
| Clipboard outcome                                                            | Only matching `Delivered` exits. Failed/Unknown retain the mode for a later explicit user request. Program, pointer, stale-episode and cancelled outcomes cannot exit it.                                   |
| Esc, pane/context change, reload, retirement                                 | Cancel obsolete local/query/selection work once. Primary recovery restores following-bottom; alternate only clears the local mode and selection.                                                            |
| Screen switch, generation/geometry change, clear or endpoint/anchor eviction | Cancel obsolete state and display a notice. Never guess new coordinates.                                                                                                                                    |

A primary revision refresh keeps the episode while its exact same-incarnation window waits for
`RowsApplied`. During this interval the status shows pending, the semantic cursor is hidden, and
coordinate, query and export keys are captured without queuing work. `q` outside search and
`Esc` still exit.
Local search-draft editing and valid paste remain available; submission waits with the other
terminal effects.
Accepted acknowledgement rebases the cursor against the new rows; generation, screen, geometry
or anchor invalidation still cancels the episode.

Cancel Host selection before primary viewport recovery so the ordinary mailbox preserves their
order. Cancellation removes undispatched effects; it cannot retract a system call already begun.
Each still-valid, successfully queued export token submits to the system at most once and is never
automatically replayed. An Unknown result is not proof of delivery. A later user `y`/Enter is a
new export and a new attempt.

Each projection's selection driver fences viewport admission behind its last exact cleanup
command. Entry displays loading and captures input while a preceding gesture settles; it
revalidates pane, attachment, Host epoch, screen, generation and geometry before admitting Enter
or using a committed surface. Revision-only output may refresh that valid source. Exit clears
the local episode immediately and stages recovery, while the same per-pane fence also blocks
ordinary history/input paths from admitting recovery early. Admission of the final cleanup
command normally releases the fence: the FIFO worker executes it before the viewport command.
An exact current prior/cleanup flight's explicit rejection terminates its remaining cleanup,
reports failure and releases that boundary as a rejected attempt. Its pending viewport intent
may then continue; cleanup is not replayed or reported as successful. The model keeps this
rejection receipt distinct from final-cleanup admission and completion. Foreign-incarnation
rejection cannot release a new boundary. Generation change or removal discards obsolete work;
another pane is independent; retirement admits no further commands.

An acknowledged Start or Update may legitimately contain no Host selection before the mouse
gesture threshold is crossed. That settles the local selection intent without closing it. End
with no active Host selection and no export reports NoSelection and returns Ready rather than
waiting for an export that was never started. Every endpoint committed locally, including entry,
same-window page/end navigation and revision rebasing, is normalized to a whole displayed glyph.

Host Gesture Cancel preserves an already settled selection. A copy episode therefore owns
selection only after its semantic Start; entering and exiting without that Start preserves an
older pointer selection, which this episode cannot export. Clearing a copy-owned selection uses existing wire commands: a single
click Start at a valid cell in its admitted nonzero geometry replaces selection with None, then
Cancel resets that replacement gesture. Any active export is cancelled before that replacement.
The two/three-command chain is fenced through its final Cancel. Missing or stale ownership is an explicit
failure/invalidation, never an inferred successful clear; ordinary pointer Cancel is unchanged.
An export cancellation already queued locally remains owned even after the active export has
been taken. Replacing obsolete queued gestures during entry, exit, clearing or generation changes
preserves that cancellation at the head of the cleanup chain and includes it in admission fencing;
an export cancellation already in flight keeps its existing order. The barrier model's cleanup
plans abstract this retained obligation, including a standalone CancelExport.

## Read and lifetime bounds

Motion reuses the CLI's complete-row collector: a single row for line endpoints, bounded paragraph
context, and directional 64-row expansion for word movement. The assembled context has at most
65,536 rows. Search reads the complete retained buffer up to that bound. Each read call has the
same row bound; cumulative budgets are 1,048,576 glyphs and 8 MiB of row payload. The shared reader
does not claim an additional cumulative row counter. The query job also enforces the existing
4096-byte search-query limit; the inline draft is smaller.

Buffer points use the existing `u32` row representation. A total beyond `u32::MAX + 1`, an
unrepresentable route/point, exhausted identity or exceeded budget is an explicit refusal, never
truncation. Query reads keep one exact revision; continuously changing content can reject a
query, which remains a visible failure rather than a mixed-revision snapshot.

The query lane admits one job, owns its wake stream and thread, stops admission, wakes waits,
joins and closes its independent connection. A temporary viewer preallocates Detach identity,
attempts disposal once, and requires known disposal plus a final canonical member fence before
publishing success. Unknown disposal drops the independent connection and cannot yield success.
The existing clipboard writer retains its own shutdown contract; there is no claim that an
arbitrary Win32 call finishes within a deadline.

## Correctness evidence

| Claim                                                      | Owner and witness                                                                                                 | Limits                                                                                              |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Exclusive input, no packet-tail leak                       | Mode binding/dispatcher ownership tests; `KeyboardCopyMode` input and tail actions                                | Bounded decoded packets; existing decoder contract                                                  |
| Coordinates and selection updates use an admitted surface  | Copy core/NativeUi viewport traces; `KeyboardCopyMode` commit/move invariants                                     | Geometry, glyph and exact-frame tests; Host pins remain trusted                                     |
| Stale query/effect completions cannot act on a new episode | Correlated core/adapter traces; `KeyboardCopyMode` query/delivery actions                                         | Two episodes, two exports and two source versions in the merge model                                |
| At-most-once effect submission and matching success exit   | Effect queue/selection traces; `KeyboardCopyMode` attempt and success invariants; existing selection/effect model | Existing sink/OS trust; no clipboard-read or universal delivery claim                               |
| Bounded complete content policy                            | Shared-reader, motion/search, query and overflow tests                                                            | Existing Unicode policy is reused; no new universal Lean claim                                      |
| Integrated keyboard, output, clipboard and cancellation    | Controlled Windows trace and normal repository gate                                                               | Automated inherited-terminal/ConPTY evidence; physical Windows Terminal acceptance remains separate |

Deterministic tests include replayable generated stateful sequences and shrinking, stale and
reordered completions, both surfaces, zero history, mouse-tracking programs, boundary geometry,
clear/resize/eviction, explicit retry and matched success. Rendering covers supported size tiers,
both locales and the native plus an RGB theme. No new text transformation makes an unbounded
claim; the finite model abstracts text into source identity and does not prove Unicode semantics.

The model retains exact query completion IDs and a pending/ready selection handshake; an old query ID leaves the current query intact, and copy admission waits for settled Host endpoints. Its source tuple abstracts the full `ContentSource`: Host identity/screen/generation represent incarnation and invalidating geometry changes; revision represents the pinned revision/columns/total-row facts, while committed surface represents accepted anchor/window feedback. Concrete field equality and rebase behavior remain adapter/core obligations. Input and foreign-result observations use one toggling witness bit rather than independent monotonic history counters; this permits arbitrarily repeated observations without multiplying unrelated state histories. Retirement clears pending export intent.

Run `node scripts/tla/run-models.mjs --models keyboard-copy-mode` for the registered bounded copy
owner. Existing affected model families remain change-triggered obligations. Run focused native
tests while iterating and the complete `pnpm verify` gate against frozen final inputs before merge.

Run `node scripts/tla/run-models.mjs --models keyboard-copy-mode-selection-barrier` for the
independent per-pane cleanup/admission dimension. It models old gesture flights, exact final
cleanup admission, FIFO execution, late responses, pane invalidation and retirement separately
from text/query/export episode combinations. Its admission-order invariant maps to actual
driver/mailbox FIFO traces for two pane incarnations, including a retained CancelExport before
Cancel. Admission requires either the final cleanup's FIFO admission or an exact known rejection
that terminates its boundary; execution requires that cleanup to settle or that known rejection.
Typed proof states distinguish admitted, settled and rejected attempts without retaining consumed
history. Neither cleanup success nor transport fairness is promised. `SelectionSettled` in `KeyboardCopyMode` denotes the local acknowledged
gesture intent, including valid Host None replies; `ExportFailure` includes an End for which the
Host starts no export. Direction and whole-glyph rules remain deterministic core/adapter
obligations rather than unbounded text claims.
