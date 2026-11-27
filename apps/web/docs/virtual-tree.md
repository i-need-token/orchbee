# Virtual tree

`src/virtual-tree/` is the list and tree engine of the File Bar's Explorer, Search results and
Source Control views. It holds the loaded items, the flat array of visible rows, and focus,
selection and anchor, and turns every change into the fewest row splices it can. It never knows
about files, Git, or where its items come from: a view hands it a `VirtualTreeSource` and renders
the rows it reads back. The public API and its contracts are the TSDoc of `virtual-tree-types.ts`
and `VirtualTreeModel`.

## Connecting a data source

- `key` is the identity that focus, selection, expansion and view state follow. Make it unique
  across the whole tree and stable across refreshes; a path is a good key for a file tree. An item
  that moves to another parent arrives there as a new row, and its old row leaves when the refresh
  that named it ends.
- `children` returns the items in display order. The engine never sorts, so a view that sorts does
  it in its source.
- Return an array when the children are at hand, as Source Control's changes and Search results
  are: the rows change in the same call, and expanding a row loads and shows its children in one
  splice. Return a promise when they must be fetched, as Explorer's directory reads are: the row
  shows expanded and empty until the children arrive, and a result for a row that has left the tree
  is dropped.
- Call `refresh(key)` when the source's data under an item changed, or `refresh(null)` for the top
  level. A collapsed row only forgets its children and fetches them again when it next expands, so
  refreshing a large collapsed folder costs nothing. Expanded rows below the refreshed one fetch
  theirs too; `recursive` also makes collapsed rows below forget theirs.
- A view restores a tree with `restoreViewState`, which expands the saved rows as they load.
- Source callbacks must not call back into the model.

## Performance constraints

- `key` and `hasChildren` run for every child on every refresh of its parent; keep them
  constant-time and free of allocation.
- Return the same item object for an item that did not change. A new object under an existing key
  updates the row in place and bumps its `revision`, so a renderer draws it again, though no splice
  is sent.
- A refresh whose children keep their order merges in linear time, comparing each key with the old
  child in the same place; children that move cost O(n log n) to place, and only the rows that moved
  are respliced. The key index hashes a key once, when its row is created, which is most of the cost
  of a first load.
- `rowAt` allocates nothing, and the row object stays the same while its key stays in the tree.
  `indexOf` is constant-time for rows above the last splice; the first lookup below it walks from
  the splice to the row once.
- Listeners hear an operation's events once it has finished: at most one `rows` event, then its
  collapse, focus, selection and anchor events. A renderer reads only the rows it shows;
  `{ kind: 'rows', splices: [] }` means rows changed in place.
- `pnpm --filter @orchbee/web benchmark:virtual-tree` times the File Bar proposal's reference
  scenarios; run it before and after a change to the model.

## Decisions

- **The node is the row.** Reading rows allocates nothing and a row keeps its identity, where a row
  snapshot per change would cost O(n) for every expansion.
- **Row indexes are cached, not indexed.** A row remembers its last index and a watermark marks
  where the cache stops being trustworthy, so lookups above a splice cost nothing. An
  order-statistic tree would make every lookup O(log n) to speed up only the first one after a
  splice.
- **A refresh rewrites its rows once.** VS Code's identity refresh sends one list splice per change,
  10,000 of them for 10% scattered changes among 100,000 children. Here the merge rebuilds the
  parent's row range in one copy and reports splices composed into final positions: changes fewer
  than eight rows apart merge, and more than 16 in one range or 64 in one operation become one
  replacement, which a renderer handles faster than many small splices.
- **Moved children are the complement of a longest increasing run** of their old positions, so the
  fewest rows are reported as moved.
- **Recursive expansion follows the source.** VS Code's object tree holds every node and expands
  them all, while its asynchronous tree collapses stale descendants back. Here stale descendants
  load when the source answered their parent synchronously, so an in-memory tree expands whole, and
  stay collapsed when it answered asynchronously, so a recursive expansion never starts a fetch per
  folder.
- **A key under a new parent wins.** The node it displaces leaves with its subtree when the refresh
  ends; a key that would make a row its own ancestor is skipped, and a key repeated in one list keeps
  its first row. VS Code leaves these cases undefined, and silent duplicates would corrupt focus and
  selection.
- **Traits follow keys.** Focus, anchor and selection survive collapse and drop only when their key
  leaves the tree. The selection is a set snapshot, so selecting all of 100,000 rows is one O(n)
  pass, where VS Code's mapping of every element back to a list index is O(n²).
