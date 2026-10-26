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
at its bottom-left cell. Normalize movement to complete displayed glyphs.

Copy mode takes its coordinates from the complete admitted row window or alternate-screen frame,
independently of the ordinary live crop. Its local display reserves the permanent outer footer and,
in Minimal layout, the first-row mouse toolbar. It crops the served rows and columns to keep the
actual cursor glyph fully visible above the footer, including the second cell of a wide glyph.
Search submission and keyboard or toolbar repetition reveal the matching served text and cursor
together; the cursor never substitutes a footer cell for an obscured result. This display does not
resize the physical terminal, move a committed history bookmark or change ordinary input-method
coordinates. The mouse toolbar replaces the Tab strip outside Minimal layout. Copy omits the ordinary
history badge and bottom line inside the pane; the outer status area shows the persistent mode
indicator, search draft or notice, and owner-ordered keyboard hints. Dispatch, footer keys and
labels derive from the same local binding definitions. The footer keeps the longest ordered prefix
of complete key-and-label items that fits, with the local exit route first; it never clips an item
or paints a key without its localized action. Cursor and selection paint use the same local crop;
pointer hits carry its source origin and translate back to the original admitted coordinates.
Read and Host selection commands retain the complete source fence. A changed crop revokes a drag
against the preceding painted rectangle and origin.

## Mouse controls

The pane action menu opens the same copy episode. The toolbar uses the shared action grammar for
Copy, Search and Help, a stable Boolean value for `[ ]` Select / `[x]` Select, and fixed three-cell
previous, next and close actions. A search draft exposes Find and Back, then a static direction
value beside the labeled Reverse action. Controls keep their geometry while disabled, suppress
activation, and act only on a matching plain-left release with the same painted token and
rectangle. Keyboard input, focus loss, capture release, new content or geometry revokes a pending
click. The decision packet and late release cannot become pane input.

Plain left press on served content anchors selection at the normalized glyph; while selecting, presses and drags extend the existing selection. Clear ends that selection before a new anchor. Drag motion coalesces through the existing Host selection acknowledgement; releasing retains selection for an explicit Copy action. The original episode and exact screen, generation, revision and row window fence every coordinate. No separate clipboard writer or terminal-input path is introduced.

Wheel inside the served content contributes three rows per notch, bounded by the painted viewport per pass. Reversal drops old backlog and retains the first opposite notch. Source/geometry change, keyboard input, query or export cancels unused work. Wheel outside content is inert. Search, selection, read and clipboard failures retain their original native-owner behavior.

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

While editing a search, printable characters including `q` edit the query. Enter submits it;
Ctrl+G returns to copy navigation without leaving the mode, and Ctrl+R reverses the draft's current
search direction without submitting it.
The draft uses the existing inline editing conventions, whole-grapheme deletion and atomic
paste, with a 512-Unicode-scalar bound and a visible refusal on overflow. Queries containing
uppercase characters are case-sensitive; others are case-insensitive. Empty search performs no
content read. Search results wrap through the existing `terminal_copy::search` policy.

Search command classification uses the existing canonical key identity: a raw carriage return
and named Enter both submit, Ctrl+G returns, and Ctrl+R reverses direction; Ctrl+H and Backspace
delete a whole grapheme. Printable insertion retains the original Unicode scalar and case. A
refused paste or edit keeps its draft and caret and shows a warning beside the exit hint. Repeated
`n` follows the last submitted `/` or `?` direction; repeated `N` always follows its reverse
without changing that remembered direction.

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

Replacing tab, context or creation projections cancels the obsolete episode and joins its query
lane while preserving the Client-session reconnect origin. A subsequent keyboard or toolbar
search can open a fresh exact read. Only Client-session retirement closes that origin;
cursor movement or query retry never needs to repair connection configuration.

## Correctness evidence

The owning implementation tests cover input packet ownership, admitted geometry, stale query/effect rejection, final movement before release, effect ordering, at-most-once submission and matching success exit.

Maintain the concrete witnesses in `src/terminal_copy_mode/tests.rs`,
`src/workbench_client/terminal_selection/tests.rs`,
`src/workbench_client/ui/copy_mode/tests/mouse.rs`, `src/tui_render/copy_mode/tests.rs` and the
adjacent state, carrier, input and rendering suites. The mouse visibility suite composes the full
shell at 120×40, 80×24, Compact and Minimal sizes in both locales and native/RGB themes. Controlled
search completions check visible matching glyphs and a cursor above the footer for primary and
alternate screens, keyboard/toolbar search and repeat, and translated wide-cell selection/drag.
These local crop checks do not exercise query transport. Use controlled completion/clock order and
generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee --all-targets --all-features`. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
