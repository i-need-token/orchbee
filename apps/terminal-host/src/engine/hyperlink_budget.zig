//! Keeps Ghostty's row moves within the hyperlink capacity it assumes. A page's hyperlink capacity
//! stops growing once its 16-bit byte budget saturates, at 26,214 linked cells. Scrolling a region,
//! inserting or deleting lines, and scrolling without scrollback copy rows across a page boundary
//! after they have already rotated rows, so `cloneRowGrowCapacity` panics when the receiving page
//! cannot grow. That panic ends a ReleaseSafe Host and every terminal on it.
//!
//! Every such copy moves rows between pages that intersect the active area, and its destination
//! only ever holds rows from those pages. So the Host keeps the hyperlinked cells, and the links,
//! of the pages that intersect each screen's active area within half of one saturated page:
//!
//! - a print that would exceed the budget prints without the cursor link, and a new link that
//!   would exceed it is not started; the text always stays;
//! - after each resize, whose active area can reach into older pages, the oldest rows of the
//!   active pages lose their hyperlinks until the budget holds again.
//!
//! Row moves never add hyperlinked cells or link values across those pages, so they keep the
//! budget, and the page that receives a row can always grow to hold it. Remove this once Ghostty
//! moves a row into a new page instead of panicking when a saturated page cannot grow.

const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

const Screen = ghostty_vt.Screen;
const Page = ghostty_vt.page.Page;
const Row = ghostty_vt.page.Row;
const Node = ghostty_vt.PageList.List.Node;

const saturated = Page.layout(.{
    .cols = 1,
    .rows = 1,
    .hyperlink_bytes = std.math.maxInt(ghostty_vt.size.HyperlinkCountInt),
});
// Half of each saturated table stays below its maximum load factor (80% for the cell map and
// 81.25% for the link set), so every row a page receives fits after the increases Ghostty makes.
pub const linked_cell_budget: usize = saturated.hyperlink_map_layout.capacity / 2;
pub const link_budget: usize = saturated.hyperlink_set_layout.cap / 2;

/// The most cells one printed codepoint links: a spacer head, the wide cell and its spacer tail.
const cells_per_codepoint = 3;

pub const Usage = struct {
    linked_cells: usize = 0,
    links: usize = 0,

    pub fn fits(self: Usage) bool {
        return self.linked_cells <= linked_cell_budget and self.links <= link_budget;
    }
};

/// Clears every hyperlink of one row and returns what its page released. The text stays.
pub fn dropRowLinks(page: *Page, row: *Row) Usage {
    const cells_before = page.hyperlinkCount();
    const links_before = page.hyperlink_set.count();
    for (page.getCells(row)) |*cell| {
        if (!cell.hyperlink) continue;
        page.clearHyperlink(cell);
        cell.hyperlink = false;
    }
    row.hyperlink = false;
    row.dirty = true;
    return .{
        .linked_cells = cells_before - page.hyperlinkCount(),
        .links = links_before - page.hyperlink_set.count(),
    };
}

const ActivePages = struct {
    first: ?*Node,
    usage: Usage,
};

/// The oldest page that intersects the active area and the usage of all pages from it to the end.
/// The count of link values sums each page's set, so it bounds the distinct links from above.
fn activePages(screen: *Screen) ActivePages {
    var result: ActivePages = .{ .first = null, .usage = .{} };
    var rows: usize = 0;
    var node = screen.pages.pages.last;
    while (node) |current| : (node = current.prev) {
        const page = current.page();
        result.first = current;
        result.usage.linked_cells += page.hyperlinkCount();
        result.usage.links += page.hyperlink_set.count();
        rows += current.rows();
        if (rows >= screen.pages.rows) break;
    }
    return result;
}

/// Restores the budget on every screen after a resize. The caller holds the terminal mutex.
pub fn fitActive(terminal: *ghostty_vt.Terminal) void {
    for ([_]ghostty_vt.ScreenSet.Key{ .primary, .alternate }) |key| {
        if (terminal.screens.get(key)) |screen| fitScreen(screen);
    }
}

fn fitScreen(screen: *Screen) void {
    const active = activePages(screen);
    var usage = active.usage;
    if (usage.fits()) return;

    // The totals shrink by what each row actually released, so they stay exact and never wrap.
    var dropped: usize = 0;
    var node = active.first;
    pages: while (node) |current| : (node = current.next) {
        const page = current.page();
        for (page.rows.ptr(page.memory)[0..page.size.rows]) |*row| {
            if (usage.fits()) break :pages;
            if (!row.hyperlink) continue;
            const released = dropRowLinks(page, row);
            usage.linked_cells -= released.linked_cells;
            usage.links -= released.links;
            dropped += released.linked_cells;
        }
    }
    std.log.warn("dropped the hyperlinks of {d} older cells that the active screen could not hold", .{dropped});
}

/// Admits prints and new links on the active screen against the budget. It remembers the headroom
/// from its last count and charges each admission, so a linked print costs O(1) until that headroom
/// runs out. Between counts only admissions add to the usage: row moves keep it, a screen switch
/// recounts, and every resize must `invalidate` after `fitActive`.
pub const Admission = struct {
    screen: ?*const Screen = null,
    linked_cells_left: usize = 0,
    links_left: usize = 0,
    warned: bool = false,

    pub fn invalidate(self: *Admission) void {
        self.screen = null;
    }

    /// Ends the cursor link before a print of `codepoints` that the budget cannot hold.
    pub fn admitPrint(self: *Admission, screen: *Screen, codepoints: usize) void {
        if (screen.cursor.hyperlink_id == 0) return;
        const cells = std.math.mul(usize, codepoints, cells_per_codepoint) catch std.math.maxInt(usize);
        if (self.admit(screen, cells, 0)) return;
        screen.endHyperlink();
        self.warn();
    }

    /// Returns whether a new link fits. When it does not, the current link ends as the new one
    /// would have ended it, and the text that follows prints without a link.
    pub fn admitLink(self: *Admission, screen: *Screen) bool {
        if (self.admit(screen, 0, 1)) return true;
        screen.endHyperlink();
        self.warn();
        return false;
    }

    fn admit(self: *Admission, screen: *Screen, cells: usize, links: usize) bool {
        if (self.screen != screen or
            cells > self.linked_cells_left or
            links > self.links_left)
        {
            const usage = activePages(screen).usage;
            self.screen = screen;
            self.linked_cells_left = linked_cell_budget -| usage.linked_cells;
            self.links_left = link_budget -| usage.links;
            if (cells > self.linked_cells_left or links > self.links_left) return false;
        }
        self.linked_cells_left -= cells;
        self.links_left -= links;
        return true;
    }

    fn warn(self: *Admission) void {
        if (self.warned) return;
        self.warned = true;
        std.log.warn("printed text without hyperlinks that the active screen could not hold", .{});
    }
};
