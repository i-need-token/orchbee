# Native terminal output

## Authority and grammar

This reference owns source-selected `terminal read` and `agent read`. The legacy visible,
`--history --rows`, `--unwrap` and presentation-tail forms remain governed by
[native CLI queries](native-cli-query.md). Existing invocations keep their documents and behavior.

`--source visible|recent|recent-unwrapped|detection` selects a text snapshot. `--plain` exports
its UTF-8 text directly, without headings or quoting, and defaults the source to `recent`.
It conflicts with `--format json`. Source-selected reads reject `--history`, `--rows` and
`--unwrap`; `recent-unwrapped` supplies the explicit joining policy. `--lines` accepts an
unsigned 32-bit integer, clamps it to 1000, and permits zero. All grammar resolves before effects.
Terminal targets remain exact IDs or typed suffixes; Agent selectors retain their existing resolver.

## Selection model

The applied frame owns screen geometry, cursor, alternate-screen state, scrollback count,
revision and generation. Absolute rows cover retained history followed by the live screen.

| Source             | Selected range and text                                                                                         |
| ------------------ | --------------------------------------------------------------------------------------------------------------- |
| `visible`          | The live screen; remove trailing blank rows, then apply the optional physical-line tail.                        |
| `recent`           | At most `--lines` physical rows, default 80, ending at the recent endpoint; remove trailing blank rows.         |
| `recent-unwrapped` | The same recent range, joining only verified soft-wrap continuations.                                           |
| `detection`        | A recent range of one screen height, followed by the optional physical-line tail. It does not read Agent state. |

On the primary screen, the recent endpoint is the later of the cursor row and last nonblank
live-screen row. An entirely blank screen or alternate screen uses the physical final row.
The cell-capacity bound may narrow a history request at its beginning; the result reports that
fact. Zero lines produces empty text. Text drops grid padding and wide-character continuation
cells, preserves Unicode, separates hard rows with newlines, and terminates nonempty output with
a newline. A selected fragment can start or end inside a wrapped logical line; the result exposes
those edge facts. Limits count physical rows before joining, never guessed logical lines.

The source vocabulary follows Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`.
Orchbee's viewer starts at the live screen: another Client's private scroll position is not shared
state. Only the current authoritative projection is returned; an older cached Windows screen is
not substituted for a blank screen. ANSI snapshots and source-selected waits are separate consumers
of this selection model; this text contract does not claim original PTY-byte capture.

## Observation lifecycle

The CLI owns `admit → snapshot → resolve → attach → frame → rows → detach → project → emit`.
Agent resolution precedes attachment on the same session. A visible or empty selection needs no
row request. Other selections use one buffer `ReadRows` at the frame's exact revision and generation;
transport, route, extent and decoded rows must all agree. A stale revision, generation or evicted
history returns a typed failure, never a stitched snapshot or hidden retry. Frame feedback is
acknowledged; buffer reads do not send `RowsApplied`.

The existing read network deadline bounds all session work. Best-effort detach runs after every
attached outcome, and the session drops before output preparation. There is no controller claim,
resize, terminal input, process launch or Client focus change. A malformed or over-budget answer
fails before stdout. Read cleanup and platform admission retain the existing query contracts.

## Output and compatibility

Structured text stays localized and escaped. JSON uses `orchbee.terminal.output` v1 with canonical
snapshot provenance, terminal identity, source selection, revision/generation, physical range,
capacity truncation, fragment edges and text. The catalog is v44 with 72 documents; no protocol
shape or command path is added. `--plain` emits exactly the JSON text member's UTF-8 bytes, under
the same preparation cap; failures leave text stdout empty. It is an explicit data export under
the [CLI output standard](../../../docs/cli/output.md), not ordinary human text rendering.

Older binaries reject the new flags. Rollback restores the previous executable; there is no stored
migration. Same-author checks cover grammar, range/Unicode/wrap boundaries, strict row replies,
output bounds and Windows process/ConPTY behavior. TLA execution is deferred; these are executable
checks and model review, not an unrun model-checking claim.

## Source-selected waits

Windows `terminal wait-output` selects the same sources with `--source`, `--lines` or `--forever`.
Any of these chooses source mode, defaults to `recent`, and excludes legacy `--rows`. Existing
invocations retain their 1000-row retained window, 30000 ms deadline and v1/v2 documents. Source mode
also defaults to 30000 ms; `--forever` instead removes the total ceiling and conflicts with `--timeout`.
Every individual admission and I/O remains bounded to five seconds. Finite timeouts keep the existing
positive range and patterns keep their existing nonempty and compiled-program bounds.

The lifecycle is `admit → snapshot → attach → observe → detach → emit`. Each applied frame is
acknowledged before sampling. Visible and zero selections need no row request; other sources pin
one complete row window. A stale revision, stale generation or evicted history abandons the sample
and observes a fresh frame, retaining the original deadline and increasing row-request sequence.
There is no sample splicing, input, controller claim or focus change. Quiet polls back off from
10 to 100 ms. Cancellation and finite expiry are checked after I/O, matching and detach; a late
signal or expiry wins over success. Cleanup retains confirmed/requested/unconfirmed distinctions.

Recent and recent-unwrapped waits both join verified soft wraps before matching; visible and
detection match physical lines after the same blank-row removal and tail selection as reads.
Literal and regex patterns match individual observed fragments. Regex edge flags still describe
the original wrap edges when physical rows are searched separately. Empty selected text never
matches; zero-line infinite waits therefore end only on cancellation or a runtime failure.
Existing output can satisfy a wait immediately: success never proves a new command or completion.

Source mode emits `orchbee.terminal.wait-output` v3. Its required `selection` contains source and
optional line limit; observed `timeoutMs` is null only for an explicit infinite wait. `requestedRows`
is the physical source budget before capacity/retained clamps: screen height for visible/detection,
or the explicit/default recent limit. `windowTruncated` reports capacity loss only. Completed
observations count validated samples that finished a match pass, including visible-frame samples.
Version 1/2 documents and their constraints remain unchanged; the catalog is v44 with 72 documents.

Pinned Herdr waits also search text lines, map recent to recent-unwrapped, and permit an absent
total deadline. Its raw switch only forwards a stripping field that the read handler ignores;
there is no raw PTY-byte matcher to reproduce. No inert compatibility switch is added. Windows
process checks, controlled clock/cancellation evidence and actual terminal waits must validate the
source and lifetime rules. TLA execution remains deferred.

## ANSI snapshot export

Windows `terminal read` and `agent read` accept `--ansi` as a styled data export. It defaults to
`recent`, accepts the four source selectors and source-mode line limits, and excludes `--plain`,
`--history`, `--rows` and `--unwrap`. Use `recent-unwrapped` to join soft wraps. Grammar is resolved
before runtime admission. `--format json` remains available; otherwise stdout contains the prepared
UTF-8 ANSI string directly, without headings. This is the same explicit data-export exception as
plain output, not a change to ordinary escaped human text.

JSON ANSI output uses `orchbee.terminal.output` v2 with `result.encoding:"ansi"`; `content.text`
contains the ANSI string. Text/plain source reads retain v1 and their existing shape. The catalog
is v44 with 72 schemas and 86 command paths. Empty selection exports no bytes. Preparation and
serialization preserve the existing 8 MiB output limit and finish before stdout; an over-budget
result fails completely. Raw export and JSON content have identical UTF-8 bytes when both fit.

The source range still comes from the acknowledged frame. Visible ANSI uses that frame's validated
cell styles; recent sources request a render row window at exactly its revision and generation.
Zero lines require no row read. Following pinned Herdr behavior, detection remains a text projection
even with ANSI selected. Styled blank cells are content: trim only unstyled padding and trailing
unstyled blank rows, then apply visible line tails. Recent line budgets remain physical and apply
before joining. Wide continuation and spacer cells never duplicate a grapheme or insert a spacer
into a joined logical line.

The CLI synthesizes SGR from canonical cell colors, style flags, underline kind and underline color.
It emits RGB or default colors, bold, italic, faint, blink, inverse, invisible, strike and overline,
plus single/double/curly/dotted/dashed underline. Style transitions reset before applying their full
style; row/end boundaries restore SGR so a selected tail is independently displayable. Unicode
graphemes stay intact. Control characters carried inside cell strings, including detection text
rows, are escaped as data under the same output bound. Only
generated SGR sequences and row separators are terminal controls: no cursor movement, terminal-mode
change, OSC, hyperlink or graphics replay is generated. This is a synthesized styled snapshot,
not original PTY bytes. The inert Herdr raw switch is not added.

## Render observation boundary

The lifecycle remains `admit → snapshot → resolve → attach → frame → rows → detach → project → emit`.
Source selection, Agent resolution, exact pins, acknowledgements, one-shot stale refusal, the
existing read deadline and cleanup remain unchanged. Reading styles never claims a controller,
resizes a terminal, sends input or changes Client focus.

The existing buffer `ReadRows` gains an optional `detail` of `text` or `render`; omission means text
and preserves the old OBCS encoding. The private OBCN carrier retains action tag 6 and its exact body
for text and uses tag 13 with the same bounded fields for render. Unknown details/tags fail closed.
Server validates the requested detail along with the existing route; Worker forwards it to the
existing Host row-access operation. Viewport and TUI text reads remain text. No Host product output
format or new Host protocol is introduced.

Rust's strict render decoder validates a complete OBRW record, one OBPB snapshot and one complete
OBKG resource snapshot before exposing any style or text. The projection must agree with the outer
identity, sequence, revision, generation, columns, row count and absolute row origin. Every header,
section, alignment pad, UTF-8 range, row/wrap/wide-cell invariant, color/style value, image byte count,
placement bound and crop reference is checked. Borrowed string ranges avoid expansion through
repeated aliases; graphics bytes are validated without copying pixels or replaying them.

The read payload limit remains 8 MiB. A render window can exceed it because of graphics even when
text would fit. Report that bounded failure; never silently change detail, discard payload bytes,
increase the budget or retry a different request. Old binaries reject the new detail/tag. Deploy or
roll back the matching Client/Server/Worker/Host development cohort together; no durable state or
migration is added. Windows owns the new command runtime; shared wire/codecs remain platform-neutral.

| Observation                                                | Required outcome                                                           |
| ---------------------------------------------------------- | -------------------------------------------------------------------------- |
| Visible or zero selection                                  | Use the applied frame or empty selection; no render-row request.           |
| Recent styled selection                                    | One exact render request after frame acknowledgement.                      |
| Wrong detail, pin, extent or malformed projection/graphics | Fail before stdout; release the read-only viewer.                          |
| Stale pin or history eviction                              | One-shot typed failure; no replay or sample splicing.                      |
| Valid styled blanks or wide characters                     | Preserve styling and grapheme order under physical limits and wrap policy. |
| Oversized prepared output or render payload                | Bounded failure before output; no truncation disguised as success.         |

## ANSI evidence and limits

The shared TS/Zig packed-projection and render-window corpus supplies independently authored wire
oracles, including malformed headers/sections and sparse delta cases. Rust tests additionally cover
graphics bounds/references, canonical text encodings, render detail routing and exact pin rejection.
CLI checks cover grammar, schemas, completion, style transitions, control data, Unicode/wide cells,
blank padding, wrap edges and output limits. Windows process peers verify the actual lifecycle;
matched-cohort PowerShell 5.1/7 sessions verify styled Unicode exports, pipes, retained output and
unchanged TUI focus. These checks must pass before delivery. Review is by the implementation author;
TLA execution remains deferred and no independent review or unexecuted model proof is claimed.

Packed OBTF and OBPB geometry preserves unknown cell pixels as `(0, 0)`, matching the Host and
control geometry contract. Otherwise both dimensions are positive; mixed zero/positive pairs are
invalid. Grid and pixel-extent bounds remain unchanged. A terminal snapshot never invents pixel
dimensions merely to export text or styles.

Blank classification is cached by borrowed string identity and length; repeated aliases do not multiply padding scans or allocate expanded text before the output budget is enforced.

Source range discovery classifies borrowed visible cells without expanding shared strings. Visible
text reads and waits remove trailing blank rows and apply their physical tail before materializing
text. Every append, including row separators, checks the aggregate 8 MiB preparation bound first;
omitted rows and shared blank aliases never allocate expanded text. Recent and ANSI selection do
not prepare an unused plain visible copy. This preserves padding, wide-cell and control-data rules
while preventing shared grapheme aliases from bypassing the output budget during preparation.
