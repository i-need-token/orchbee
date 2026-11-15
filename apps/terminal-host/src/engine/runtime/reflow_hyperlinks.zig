//! Keeps Ghostty's reflow within the hyperlink capacity it assumes. A reflow copies every
//! hyperlinked cell of the primary screen into fresh pages. A page's hyperlink capacity stops
//! growing once its 16-bit byte budget saturates, yet `ReflowCursor.writeCell` assumes that one
//! capacity increase always makes room and reaches `unreachable` when it does not, which panics a
//! ReleaseSafe Host and ends every terminal. Program output reaches it: about two thousand
//! hyperlinked file names followed by a narrower window are enough.
//!
//! Before a column change that reflows, the oldest primary-screen pages lose their hyperlinks until
//! the whole screen fits half of one saturated page. Their text stays; Ghostty's unchecked builds
//! drop the overflowing links in the same place. Remove this once Ghostty grows or splits the page
//! instead of assuming room.

const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

const saturated = ghostty_vt.page.Page.layout(.{
    .cols = 1,
    .rows = 1,
    .hyperlink_bytes = std.math.maxInt(ghostty_vt.size.HyperlinkCountInt),
});
// Half of each saturated table stays below its maximum load factor (80% for the cell map and
// 81.25% for the link set), so every copy fits after the increases the reflow already makes.
const linked_cell_budget: usize = saturated.hyperlink_map_layout.capacity / 2;
const link_budget: usize = saturated.hyperlink_set_layout.cap / 2;

/// Drops hyperlinks from the oldest primary-screen rows when a resize to `columns` would reflow
/// more of them than one page can hold. The caller holds the terminal mutex.
pub fn fitReflow(terminal: *ghostty_vt.Terminal, columns: u16) void {
    if (columns == terminal.cols or !terminal.modes.get(.wraparound)) return;
    const screen = terminal.screens.get(.primary) orelse return;
    // The reflow restores every compressed page it copies, so reading them first adds no work.
    var linked_cells: usize = 0;
    var links: usize = 0;
    var node = screen.pages.pages.first;
    while (node) |current| : (node = current.next) {
        const page = current.page();
        linked_cells += page.hyperlinkCount();
        links += page.hyperlink_set.count();
    }
    if (linked_cells <= linked_cell_budget and links <= link_budget) return;

    // The totals shrink by what each row actually released, so they stay exact and never wrap.
    var dropped: usize = 0;
    node = screen.pages.pages.first;
    pages: while (node) |current| : (node = current.next) {
        const page = current.page();
        for (page.rows.ptr(page.memory)[0..page.size.rows]) |*row| {
            if (linked_cells <= linked_cell_budget and links <= link_budget) break :pages;
            if (!row.hyperlink) continue;
            const cells_before = page.hyperlinkCount();
            const links_before = page.hyperlink_set.count();
            for (page.getCells(row)) |*cell| {
                if (!cell.hyperlink) continue;
                page.clearHyperlink(cell);
                cell.hyperlink = false;
            }
            row.hyperlink = false;
            row.dirty = true;
            const released = cells_before - page.hyperlinkCount();
            linked_cells -= released;
            dropped += released;
            links -= links_before - page.hyperlink_set.count();
        }
    }
    std.log.warn("dropped the hyperlinks of {d} older cells that a reflow could not hold", .{dropped});
}
