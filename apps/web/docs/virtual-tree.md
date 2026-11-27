# Virtual tree

`src/virtual-tree/` is the list and tree engine of the File Bar's Explorer, Search results and
Source Control views. It holds the loaded items, the flat array of visible rows, and focus,
selection and anchor, and turns every change into the fewest row splices it can. It never knows
about files, Git, or where its items come from: a view hands it a `VirtualTreeSource` and draws
each row's content in `VirtualTree`. The public API and its contracts are the TSDoc of
`virtual-tree-types.ts`, `VirtualTreeModel` and `VirtualTree`.

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
  shows expanded and empty until the children arrive, and once a read has taken 800 ms its twistie
  turns into a spinner until it ends, as in VS Code.
- The model reads one item's children once at a time, and everyone who needs them while the read
  runs shares it. Pass the `AbortSignal` on to the read: it aborts when the row collapses or leaves
  the tree, or when the model is disposed, and an answer after that is dropped. A failed read
  collapses the row and rejects the operation that asked, and expanding the row reads again. Reject
  with an `AbortError` when the source itself cancels a read: the row collapses the same way, but
  the operation settles without an error, as VS Code takes a cancelled read.
- Call `refresh(key)` when the source's data under an item changed, or `refresh(null)` for the top
  level. A collapsed row only forgets its children and fetches them again when it next expands, so
  refreshing a large collapsed folder costs nothing. Expanded rows below the refreshed one fetch
  theirs too; `recursive` also makes collapsed rows below forget theirs. A refresh asked while an
  older read runs waits for it and reads once more, however many refreshes arrive meanwhile, so a
  burst of watcher events costs a folder at most two reads.
- A view restores a tree with `restoreViewState`, which expands the saved rows as they load, and
  calls `dispose` when the tree goes away for good.
- Source callbacks must not call back into the model.

## Rendering a tree

- `VirtualTree` fills its container, which must give it a definite height. A resize observer hands
  it its size, so it reads no layout while it renders or scrolls, and the rows for a new size paint
  in the frame that resized it.
- `renderRow` draws a row's content; the tree draws the row around it, with VS Code's 8 px a level
  of indentation and the twistie. It runs only when a slot shows another row or the row's revision,
  position, focus or selection changes, so content that depends on other data subscribes to that
  data itself.
- Rows are VS Code's 22 px unless `rowHeight` says otherwise. A function reads each row's height
  into a Fenwick index, which answers position and row lookups in O(log n); it is read for every
  row whenever its identity changes or rows change in place, and for inserted rows otherwise, so
  keep it stable and cheap. The index, `HeightIndex`, and the wheel arithmetic below live in
  `src/list-scrolling`, so other lists that scroll as these do, such as the code editor's
  multi-diff, use them without the tree's components and styles.
- The scroll offset is model state: `scrollTop`, `setScrollTop` and the view state carry it, so a
  tree shown again opens where it was left, and the view pulls an offset that the rows no longer
  reach back within them. `reveal(key, { relativeTop })` expands a row's parents and asks the view
  to scroll it into sight as VS Code does; a request that arrives before the view has a height
  waits for it.
- The wheel scrolls as VS Code's lists do, without smoothing: 50 px a notch, Alt five times as far,
  never sideways, and a list keeps every wheel event. The scrollbar is the Panels' scrollbar as the
  Sidebar shows it.
- Expanded rows whose descendants scroll beneath them stick at the top, as VS Code's sticky scroll
  does: outermost first, at most `maxStickyRows`, 7 unless set and 0 for none, and never more than
  40% of the tree's height. A row slides up beneath its parent as its last descendant scrolls past,
  and `renderRow` draws sticky rows too, never focused or selected. Clicking a sticky row scrolls its
  row to just below its ancestors' sticky rows, then focuses, anchors and selects it; clicking its
  twistie also collapses it, with Alt every row below it too. `reveal` keeps a row clear of the rows
  that would stick above it.

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
  collapse, focus, selection, anchor and scroll events, and a reveal request last, once the rows it
  names are in place. A renderer reads only the rows it shows; `{ kind: 'rows', splices: [] }`
  means rows changed in place.
- Only the rows in sight have elements, plus the focused row while it is scrolled away, so
  `aria-activedescendant` always names an element. Slot `s` shows the row whose index leaves
  remainder `s`, so a row that scrolls in takes the element of the row that scrolled out, and a
  frame of scrolling renders only the rows it brings in. Keep `renderRow` cheap: a fast scroll calls
  it for every row in sight.
- `pnpm --filter @orchbee/web benchmark:virtual-tree` times the File Bar proposal's reference
  scenarios and the height index; run it before and after a change to the model or the index.
  `pnpm --filter @orchbee/web test:system virtual-tree` checks which row elements exist and prints
  the render benchmark: expanding the folder of 100,000 files to its painted frame, selecting all of
  it, and scrolling it at fixed speeds, with each frame's script time, long tasks and the layouts
  that script forced. It needs an installed Playwright Chromium.

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
- **A refresh never takes an answer older than itself.** Each read is stamped with the refresh count
  when it starts: an expansion shares any running read, a refresh only one that started after it was
  asked, and the rest share one follow-up read that starts when the running one ends. VS Code's
  asynchronous tree waits for an intersecting refresh and then refreshes again for each caller, so
  refreshes during one read cost a read each; aborting the running read instead would starve a
  folder whose watcher fires faster than it reads.
- **Collapsing aborts the read.** VS Code lets a collapsed row's read finish; here collapsing aborts
  it, as the File Bar proposal asks. A refresh it aborted, like a failed read, leaves the row stale,
  so the next expansion reads again instead of showing children the read was meant to replace. VS
  Code keeps the old children of a failed row and shows them on the next expansion without reading.
- **A key under a new parent wins.** The node it displaces leaves with its subtree when the refresh
  ends; a key that would make a row its own ancestor is skipped, and a key repeated in one list keeps
  its first row. VS Code leaves these cases undefined, and silent duplicates would corrupt focus and
  selection.
- **The view's first frame has rows.** The root hears its slot count in the layout phase, where an
  external-store subscription would hear the first size only after passive Effects, a frame late.
- **Traits follow keys.** Focus, anchor and selection survive collapse and drop only when their key
  leaves the tree. The selection is a set snapshot, so selecting all of 100,000 rows is one O(n)
  pass, where VS Code's mapping of every element back to a list index is O(n²).
- **Sticky rows are hidden from assistive technology.** They repeat rows the tree already lists, and
  VS Code's copies reuse the rows' ids, which leaves two elements with one id.
- **A reveal clears at most 40% of the tree.** VS Code pads a reveal by every ancestor up to the row
  limit, so in a short tree the padding can push the row out of sight; here the padding counts the
  same ancestors only while they fit the sticky rows' share.
- **A sticky click also sets the anchor**, so a Shift-click after it extends from the row clicked;
  VS Code moves focus and selection and leaves the anchor where it was.
- **Sticky rows show the default cursor**, as every row does; VS Code's pointer stays with links.
