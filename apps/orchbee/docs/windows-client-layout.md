# Windows Client Panel geometry

This reference owns native Windows CLI inspection of a live TUI Client's Panel layout. The TUI owns viewport, chrome, focus and zoom. Server routes requests and retains bounded observations; canonical Workbench topology remains authoritative for membership. A layout read never selects a context or Tab, attaches a terminal, changes zoom, or resizes a terminal.

## Observation

A query names an exact Client, TerminalContext, Tab and source Panel and a minimum Workbench revision. The existing authenticated Client lane reads one cancellable canonical Snapshot before handing the request to the UI. An unavailable Snapshot or missing target produces an explicit failure. The UI uses that Snapshot to project the target without installing it as the active context. It rejects a Snapshot older than the requested minimum. For the selected Tab, geometry uses the current rendered membership cache and its revision; if that cache has not reached the requested minimum, the read fails explicitly rather than refreshing membership and causing terminal resize effects. Inactive targets use the fetched Snapshot as an explicitly unselected projection.

The result identifies the target and canonical revision, the sampled cell viewport and terminal area, whether the target Tab is selected, its local zoom flag and required nullable focused Panel. Unvisited and inactive contexts are readable. When the Client has no local focused Panel for the target, focus is null; the projection uses the renderer's first-leaf fallback without claiming that leaf is selected. An inactive Tab's geometry is a projection into the sampled viewport, not a claim that it is currently painted.

Every canonical Panel has required nullable unzoomed outer, unzoomed terminal-content and displayed terminal-content rectangles. Null means the renderer does not place that Panel in that layer. Zoom preserves unzoomed geometry and gives only the focused Panel a displayed full-content rectangle. Minimal viewports expose the renderer's single-Panel fallback. Too-small viewports return no fabricated geometry. Splits report stable split/slot identities, orientation, the rendered ratio and required nullable area/divider rectangles. Canonical traversal order is retained.

All rectangle positions and sizes are integer cells. Chrome, title rows, the shared divider, minimum-size collapse and odd-cell rounding come from the same projection used for drawing and terminal sizing. Terminal pixel dimensions do not affect cell adjacency.

## Neighbors and edges

Navigation uses unzoomed outer leaf rectangles, not content rectangles shortened by titles and not normalized topology coordinates. Candidates must lie completely in the requested direction and overlap positively on the perpendicular axis. Ordering is smallest edge distance, largest overlap, smallest doubled-center distance, then canonical traversal order. Arithmetic widens before adding coordinates. Empty or hidden source geometry is explicitly unavailable.

The observation carries four nullable source neighbors. Directional edges compare the visible unzoomed source rectangle against the terminal area boundary, matching the reference physical-edge handler. No-neighbor and physical-edge are distinct facts. A hidden source has no edge claim. Zoom does not erase an otherwise valid unzoomed neighbor. The adopted navigation ordering is pinned to Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `src/layout.rs::find_in_direction`; native chrome and degradation remain Orchbee's contract. Orchbee widens doubled-center arithmetic across its admitted coordinate range; the reference's u16 center saturation at very large coordinates is not adopted.

## Routing, lifetime and bounds

Layout is a read intent on the existing Client control route, with a distinct layout-read completion and retained status. It inherits exact Client/caller selection, operation identity, freshness, one delivery, queued expiry, disconnect settlement and caller timeout/cancellation behavior from [Client view control](windows-client-control.md). A read result cannot acknowledge a navigation or zoom command, and ordinary applied success cannot acknowledge a layout read.

The UI captures its observation in one tick after validating the target Snapshot. The result describes that point in time; later resize, input or topology changes do not rewrite the receipt. No successful draw is claimed for an inactive Tab. Local interruption before observation rejects the query. Snapshot I/O remains off the UI thread, has an owned deadline and wakeable shutdown, and never performs an automatic retry.

Dedicated OBWP frame kinds 27/28 carry layout completion/results and are bounded to 1 MiB before allocation; ordinary Client frame kinds 25/26 retain their 16 KiB/64 KiB pre-allocation limits; existing ordinary request and small-result bounds remain enforced for their actions. Combined Panel and split entries cannot exceed the canonical 4096-node context bound; identities are unique and all focus, source and neighbor references must resolve. Rectangles stay inside the sampled viewport and the relevant content area.

Server reserves 1 MiB from a 16 MiB encoded-layout budget before queuing each query. Completion replaces that reservation with the bounded encoded observation size. Rejection, disconnect without a result and receipt expiry release the reservation. Existing 256 total receipts and 60-second completed retention still apply. Capacity refusal occurs before delivery; pending or unexpired results are never evicted to make room.

## CLI and evidence

`panel layout`, `panel neighbor --direction left|right|up|down` and `panel edges` accept a positional Panel or exclusive `--panel` / `--current`, with the existing Client selection rules. Parsing finishes before connection or effects. Machine output preserves the full layout observation and a required nullable `projection`: neighbor results carry direction, source availability and a nullable neighbor ID; edges carry source availability and four nullable physical-edge booleans. Projection is null until a layout result arrives; hidden source edge booleans are null. Text output presents the requested projection without hiding unavailable geometry.

Evidence covers exact chrome/rounding/minimum geometry, nested and degenerate trees, independent neighbor-order vectors, generated hierarchy invariants with reduced replay, inactive and unvisited targets, no selection/attachment/resize side effects, two-Client isolation, stale membership, snapshot failure, bounds, capacity, interruption and shutdown. Rust, TypeScript and JSON Schema share a conformance corpus but have independent validators. Controlled Windows TUI traces connect these contracts to a real display. TLA model assets describe the extended receipt lifecycle; execution is deferred by the user and is not claimed. Same-author checks do not constitute independent review.

Ratio mutation, directional focus, swaps and layout import/apply have separate delivery slices. This read contract does not claim those capabilities. The extension is additive within exact cohort admission; rollback discards only volatile Client observations and receipts, with no canonical-state migration.

## Verification mapping

| Claim                                                                   | Maintained evidence                                                                                                                                   |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Geometry, focus retention, nested topology and collapse                 | `src/tui_layout/tests.rs`, `src/tui_layout/tests/generated.rs`: eight fixed seeds, 196 actions per seed, JSON replay and deletion shrinking           |
| Read-only selected/inactive/unvisited targets and snapshot interruption | `src/terminal_session/ui/client_layout/tests.rs`, `src/terminal_session/client_control/tests.rs`, `src/terminal_session/client_control/lane/tests.rs` |
| Exact identity, bounds, physical edges and neighbor ordering            | `crates/workbench-protocol/tests/client_layout.rs`, shared Client view corpus and independent TypeScript validation                                   |
| Reservation, completion, disconnect and expiry                          | `apps/orchbee-server/src/client_view_registry/tests.rs`; `ClientViewControl` model abstracts purity and retained-budget transitions                   |

Paths beginning `src/` are relative to this application; other paths are repository-relative. Run `cargo test --locked -p orchbee --bin orchbee`, `cargo test --locked -p orchbee-workbench-protocol --test client_layout --test client_view`, and `cargo test --locked -p orchbee-server client_view_registry`. The repository gate owns process and binding checks. Reference-capture maintenance compares the unchanged pinned Herdr neighbor functions with the protocol projector for eight deterministic seeds and 65,536 direction queries over positive rectangles with extents at most 1,200 cells; it excludes the reference's saturated-center domain. The maintained protocol vectors retain the decision boundaries independently of that external checkout.
