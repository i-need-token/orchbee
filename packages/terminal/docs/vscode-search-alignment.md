# VS Code Terminal Search Alignment

## Product contract

Orchbee terminal search follows the VS Code integrated-terminal find model:

| Behavior             | Orchbee contract                                                                                                                                                                                                                    |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Incremental input    | `Terminal.updateSearch()` runs on a 200 ms trailing-edge debounce. A newer query cancels the pending query.                                                                                                                         |
| Immediate navigation | `findNext()` searches forward and `findPrevious()` searches backward; both wrap and cancel pending incremental input.                                                                                                               |
| Search modes         | Literal, case-sensitive, whole-word, and JavaScript regular-expression modes are available. Invalid regular expressions produce an empty result plus `regexError`, not an asynchronous exception.                                   |
| Match enumeration    | Matches do not overlap and advance by the complete match, matching VS Code's xterm search decoration traversal.                                                                                                                     |
| Result status        | `onSearchResults` reports the bounded collection's active index and count, query, regex validation, and whether the result cap was reached. An active match beyond the cap has index `-1`.                                          |
| Highlight bound      | Decorations and result counts collect at most 20,000 matches by default, matching VS Code's integrated-terminal highlight limit. Directional navigation can still reach later matches. `maxResults` lowers only the collection cap. |
| Buffer changes       | Terminal output and resize clear stale decorations immediately and restart the latest query after the same 200 ms debounce without scrolling.                                                                                       |
| Empty input          | An empty incremental query clears decorations and the terminal search selection immediately.                                                                                                                                        |

The application find widget owns `Ctrl+F` focus, seeding the input from a single-line selection, the case/whole-word/regex toggles, result-count presentation, Enter/Shift+Enter navigation, Escape-to-close, search history, and accessible labels. The terminal package owns bounded matching, selection navigation, decoration state, result events, and debounce guarantees.

`highlightSearch()` and `findAll()` remain immediate low-level APIs for programmatic consumers. Interactive search inputs use `updateSearch()` so callers cannot accidentally rescan scrollback on every key event.

## Projected Host surface

`ProjectedTerminal.updateSearch()` preserves the same debounce, modes, result cap, navigation, and
result-event contract without loading the browser/WASM VT engine. The first non-empty query lazily
creates a disposable Worker. A source-neutral reader streams revision- and generation-pinned
text-detail row windows into that Worker with one bounded row request in flight; query text and
matches never enter Electron Main or Terminal Host. A changed Host revision cancels the scan and
restarts the latest query after the debounce. Search timeout, stale/evicted input, invalid regular
expressions, and unavailable Workers are explicit result states rather than UI-thread exceptions.

Only matches intersecting the current viewport are indexed for per-cell renderer lookup, so the
20,000-result decoration cap does not turn every rendered cell into a full result-list scan.
