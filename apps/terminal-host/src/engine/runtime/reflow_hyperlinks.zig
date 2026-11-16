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

const hyperlink_budget = @import("../hyperlink_budget.zig");

/// Drops hyperlinks from the oldest primary-screen rows when a resize to `columns` would reflow
/// more of them than one page can hold. The caller holds the terminal mutex.
pub fn fitReflow(terminal: *ghostty_vt.Terminal, columns: u16) void {
    if (columns == terminal.cols or !terminal.modes.get(.wraparound)) return;
    const screen = terminal.screens.get(.primary) orelse return;
    // The reflow restores every compressed page it copies, so reading them first adds no work.
    var usage: hyperlink_budget.Usage = .{};
    var node = screen.pages.pages.first;
    while (node) |current| : (node = current.next) {
        const page = current.page();
        usage.linked_cells += page.hyperlinkCount();
        usage.links += page.hyperlink_set.count();
    }
    if (usage.fits()) return;

    // The totals shrink by what each row actually released, so they stay exact and never wrap.
    var dropped: usize = 0;
    node = screen.pages.pages.first;
    pages: while (node) |current| : (node = current.next) {
        const page = current.page();
        for (page.rows.ptr(page.memory)[0..page.size.rows]) |*row| {
            if (usage.fits()) break :pages;
            if (!row.hyperlink) continue;
            const released = hyperlink_budget.dropRowLinks(page, row);
            usage.linked_cells -= released.linked_cells;
            usage.links -= released.links;
            dropped += released.linked_cells;
        }
    }
    std.log.warn("dropped the hyperlinks of {d} older cells that a reflow could not hold", .{dropped});
}
