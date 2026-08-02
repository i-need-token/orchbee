# VS Code Terminal Search Alignment

## Product contract

`ProjectedTerminal` follows the VS Code integrated-terminal find model while searching bounded,
Host-provided text pages rather than a local VT buffer.

| Behavior             | Orchbee contract                                                                                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Incremental input    | `updateSearch()` runs on a 200 ms trailing-edge debounce. A newer query cancels the pending query and its row reads.                                                |
| Immediate navigation | `findNext()` and `findPrevious()` navigate the current result set immediately, reveal the match, and wrap.                                                          |
| Search modes         | Literal, case-sensitive, whole-word, and JavaScript regular-expression modes are available. Invalid expressions produce an explicit result error.                   |
| Match enumeration    | Matches do not overlap and advance by the complete match. Soft-wrapped rows remain one logical line; hard line breaks are crossed only when `spanLines` is enabled. |
| Result status        | `onSearchResults` reports query, bounded matches, active index, revision/generation, regex validation, lifecycle state, and whether the result cap was reached.     |
| Highlight bound      | Decorations and result counts collect at most 20,000 matches by default. `maxResults` may lower but never raise that bound.                                         |
| Revision changes     | A changed Host revision cancels stale work and restarts the latest non-empty query after the same debounce.                                                         |
| Empty input          | An empty query cancels row reads, terminates the active scan, clears decorations, and returns to the idle state.                                                    |

The application find surface owns shortcut focus, seeding from selection, mode controls,
result-count presentation, query history, and localized accessible labels. `@orchbee/terminal`
owns bounded matching, navigation, decoration state, result events, cancellation, and debounce.

## Data flow and authority

The first non-empty query lazily creates one disposable Worker. A source-neutral reader streams
revision- and generation-pinned text-detail row windows with one bounded row request in flight.
The Worker scans completed logical lines while the next page is being requested.

Query text and matches remain in the renderer client. They never enter Electron Main or Terminal
Host. Search therefore cannot inspect rows evicted from Host history, and it never reconstructs
content from GPU state.

Narrow ASCII pages use their byte/UTF-16/terminal-column identity mapping without materializing a
mapping object per cell. Unicode, wide, and combined graphemes retain explicit mappings so a match
can be projected back to exact terminal coordinates.

Search timeout, stale or evicted pages, invalid regular expressions, unavailable Workers, and
revision replacement are explicit states rather than UI-thread exceptions.
