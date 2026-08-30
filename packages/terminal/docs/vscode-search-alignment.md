# VS Code Terminal Search Alignment

## Product contract

`ProjectedTerminal` follows the VS Code integrated-terminal find model while searching bounded,
Host-provided text pages rather than a local VT buffer.

| Behavior          | Orchbee contract                                                                                                                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Incremental input | `updateSearch()` runs on a 50 ms trailing-edge debounce. A newer query cancels the pending query, its accepted navigation offset, and its row reads.                                                    |
| Navigation intent | Navigation requested during debounce or scanning is accepted as a signed offset and applied when the immutable result is available; same-query revision invalidation preserves that offset.             |
| Navigation        | `findNext()` and `findPrevious()` navigate from the Host selection anchor and wrap across the admitted retained search domain, independently of the bounded decoration collection.                      |
| Search modes      | Literal, case-sensitive, whole-word, and JavaScript regular-expression modes are available. Invalid expressions produce an explicit result error.                                                       |
| Match enumeration | Matches do not overlap and advance by the complete match. Soft-wrapped rows remain one logical line; hard line breaks are crossed only when `spanLines` is enabled.                                     |
| Result status     | `onSearchResults` reports query, bounded matches, the independent active match, its bounded-collection index when present, revision/generation, validation, lifecycle state, and cap.                   |
| Highlight bound   | Decorations and result counts collect at most 20,000 matches by default. `maxResults` may lower but never raise that bound.                                                                             |
| Source bound      | A scan admits at most 16 MiB of UTF-8 row text. Exceeding that bound fails the whole immutable scan without publishing partial matches.                                                                 |
| Watchdog          | Ten seconds without a completed row read or Worker page credit retires the scan as `timeout`; later abort or Worker outcomes cannot replace that terminal state.                                        |
| Revision changes  | A changed Host revision cancels stale work and restarts the latest non-empty query after a separate 200 ms refresh debounce. Refreshed results preserve the viewport unless queued navigation resolves. |
| Empty input       | An empty query cancels row reads, terminates the active scan, clears decorations, and returns to the idle state.                                                                                        |

The application find surface owns shortcut focus, seeding from selection, mode controls,
result-count presentation, query history, and localized accessible labels. `@orchbee/terminal`
owns bounded matching, navigation, decoration state, result events, cancellation, and debounce.

## Data flow and authority

The first non-empty query lazily creates one disposable Worker. A source-neutral reader streams
revision- and generation-pinned text-detail row windows with one bounded row request in flight.
The Worker scans completed logical lines while the client prefetches at most one next page. A
generation/revision-pinned page credit admits that prefetch and refreshes the inactivity watchdog,
so fast row reads cannot queue an unbounded source ahead of the Worker's 16 MiB all-or-nothing
decision. The Worker retains only the bounded decoration collection plus one directional active
match. The client selects each settled active range through Terminal Host. An explicit query or
navigation action also reveals its absolute row; a passive same-query revision refresh updates the
selection and results without revealing the match again. An active match beyond the decoration cap
has an `activeIndex` of `-1` without becoming unreachable.

The Host selection and canvas overlay remain authoritative, but an active search match does not
occupy the document-global DOM range. The application-owned find input therefore retains keyboard
focus while incremental results settle. Where any search decoration overlaps its Host selection,
the corresponding search palette takes precedence; the active-search palette remains distinct from
regular matches, and this priority is independent of whether terminal input or the find surface owns
focus. The selection palette composites a low-saturation blue accent at 40% over the effective
terminal default background and uses its effective default foreground, preserving contrast across
light, dark, Host, and custom terminal themes.

Query text and matches remain in the renderer client. They never enter Electron Main or Terminal
Host. Search therefore cannot inspect rows evicted from Host history, and it never reconstructs
content from GPU state.

Narrow ASCII pages use their byte/UTF-16/terminal-column identity mapping without materializing a
mapping object per cell. Unicode, wide, and combined graphemes retain explicit mappings so a match
can be projected back to exact terminal coordinates.

Search timeout, stale or evicted pages, invalid regular expressions, unavailable Workers, and
revision replacement are explicit states rather than UI-thread exceptions.
