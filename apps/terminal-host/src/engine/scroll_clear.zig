//! Keeps Ghostty's scroll clear from pruning the page that holds the cursor. ED 22, and ED 2 when
//! the primary screen ends at a shell prompt, scroll the active area into history one row at a
//! time, and the cursor's row goes into history with it. When a new row needs a page and the
//! scrollback byte limit is spent, `PageList.grow` prunes the oldest page and moves the pins on it
//! to the next page. If the cursor was on that page, `Screen.scrollClear` then releases the
//! cursor's style and hyperlink IDs from the page it was moved to, where they were never added.
//! That corrupts the page's reference counts, and `RefCountedSet.release` panics on that release or
//! a later one, which ends a ReleaseSafe Host and every terminal on it.
//!
//! The Host lifts the screen's byte limit for each such erase and restores it afterwards. Without
//! pruning, the cursor leaves history through `cursorChangePin`, which moves its style and link
//! from the page that holds them. Restoring the limit then prunes only whole history pages before
//! the one that holds the active area's top row, and the cursor is in the active area by then. So
//! the erase scrolls exactly what it would have, the cursor keeps its style and link, and
//! scrollback ends within the same limit. Remove this once the Ghostty pin includes upstream
//! e257bb45e6, which re-adds the cursor's style and link on the page it moves to.

const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

const Screen = ghostty_vt.Screen;

/// A screen whose scrollback byte limit is lifted until `restore`.
pub const LiftedLimit = struct {
    screen: *Screen,
    explicit: usize,

    /// Restores the limit and prunes the history pages it no longer admits.
    pub fn restore(self: LiftedLimit) void {
        // Ghostty stores an absent limit as the largest value.
        const limit: ?usize = if (self.explicit == std.math.maxInt(usize)) null else self.explicit;
        self.screen.pages.setMaxBytes(limit);
    }
};

/// Lifts the byte limit of the screen that an erase-display action is about to clear.
pub fn liftPruning(screen: *Screen) LiftedLimit {
    const lifted: LiftedLimit = .{ .screen = screen, .explicit = screen.pages.limits.bytes.explicit };
    screen.pages.setMaxBytes(null);
    return lifted;
}
