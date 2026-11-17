//! Test probe: whether two terminals hold the same state, compared as the Host handover's
//! prototype 2 compared a restored terminal. It checks every row, cell, grapheme, width, resolved
//! style, link URI and ID, and row flag except dirty, and the modes, cursors, saved cursors,
//! charsets, Kitty keyboard stack, palette, title, directory, and scroll region. Page-local
//! numbering such as style and link IDs is left out; what they resolve to is compared.
const harness = @import("runtime_harness.zig");
const std = harness.std;
const ghostty_vt = harness.ghostty_vt;

const Terminal = ghostty_vt.Terminal;
const Screen = ghostty_vt.Screen;
const Pin = ghostty_vt.Pin;
const expect = std.testing.expect;
const expectEqual = std.testing.expectEqual;

pub fn expectEqualTerminals(expected: *const Terminal, actual: *const Terminal) !void {
    try expectEqual(expected.cols, actual.cols);
    try expectEqual(expected.rows, actual.rows);
    try expectEqual(expected.width_px, actual.width_px);
    try expectEqual(expected.height_px, actual.height_px);
    try expect(std.meta.eql(expected.modes, actual.modes));
    try expect(std.meta.eql(expected.scrolling_region, actual.scrolling_region));
    try expect(std.meta.eql(expected.colors, actual.colors));
    try expect(std.meta.eql(expected.cursor, actual.cursor));
    try expectEqual(expected.previous_char, actual.previous_char);
    try expectEqual(expected.status_display, actual.status_display);
    try expectEqual(expected.mouse_shape, actual.mouse_shape);
    var expected_flags = expected.flags;
    var actual_flags = actual.flags;
    inline for (.{ &expected_flags, &actual_flags }) |flags| {
        flags.dirty = .{};
        flags.search_viewport_dirty = false;
    }
    try expect(std.meta.eql(expected_flags, actual_flags));
    try std.testing.expectEqualStrings(expected.getTitle() orelse "", actual.getTitle() orelse "");
    try std.testing.expectEqualStrings(expected.getPwd() orelse "", actual.getPwd() orelse "");
    for (0..expected.cols) |column| try expectEqual(expected.tabstops.get(column), actual.tabstops.get(column));
    try expectEqual(expected.screens.active_key, actual.screens.active_key);
    for ([_]ghostty_vt.ScreenSet.Key{ .primary, .alternate }) |key| {
        const expected_screen = expected.screens.get(key);
        const actual_screen = actual.screens.get(key);
        try expectEqual(expected_screen == null, actual_screen == null);
        if (expected_screen) |screen| try expectEqualScreens(screen, actual_screen.?);
    }
}

fn expectEqualScreens(expected: *const Screen, actual: *const Screen) !void {
    const expected_cursor = &expected.cursor;
    const actual_cursor = &actual.cursor;
    try expectEqual(expected_cursor.x, actual_cursor.x);
    try expectEqual(expected_cursor.y, actual_cursor.y);
    try expectEqual(expected_cursor.cursor_style, actual_cursor.cursor_style);
    try expectEqual(expected_cursor.pending_wrap, actual_cursor.pending_wrap);
    try expectEqual(expected_cursor.protected, actual_cursor.protected);
    try expect(expected_cursor.style.eql(actual_cursor.style));
    try expectEqual(expected_cursor.semantic_content, actual_cursor.semantic_content);
    try expectEqual(expected_cursor.semantic_content_clear_eol, actual_cursor.semantic_content_clear_eol);
    try expectEqual(expected_cursor.hyperlink_implicit_id, actual_cursor.hyperlink_implicit_id);
    try expectEqual(expected_cursor.hyperlink == null, actual_cursor.hyperlink == null);
    if (expected_cursor.hyperlink) |link| {
        const other = actual_cursor.hyperlink.?;
        try std.testing.expectEqualStrings(link.uri, other.uri);
        try expectEqualLinkIds(link.id, other.id);
    }
    try expect(std.meta.eql(expected.saved_cursor, actual.saved_cursor));
    try expect(std.meta.eql(expected.charset, actual.charset));
    try expectEqual(expected.protected_mode, actual.protected_mode);
    try expect(std.meta.eql(expected.kitty_keyboard, actual.kitty_keyboard));
    try expect(std.meta.eql(expected.semantic_prompt, actual.semantic_prompt));
    try expectEqual(expected.no_scrollback, actual.no_scrollback);
    try expectEqual(expected.pages.total_rows, actual.pages.total_rows);

    var expected_rows = expected.pages.rowIterator(.right_down, .{ .screen = .{} }, null);
    var actual_rows = actual.pages.rowIterator(.right_down, .{ .screen = .{} }, null);
    while (expected_rows.next()) |expected_row| {
        try expectEqualRows(expected_row, actual_rows.next() orelse return error.TestExpectedEqual);
    }
    try expect(actual_rows.next() == null);
}

fn expectEqualRows(expected: Pin, actual: Pin) !void {
    const expected_row = expected.rowAndCell().row.*;
    const actual_row = actual.rowAndCell().row.*;
    try expectEqual(expected_row.wrap, actual_row.wrap);
    try expectEqual(expected_row.wrap_continuation, actual_row.wrap_continuation);
    try expectEqual(expected_row.grapheme, actual_row.grapheme);
    try expectEqual(expected_row.styled, actual_row.styled);
    try expectEqual(expected_row.hyperlink, actual_row.hyperlink);
    try expectEqual(expected_row.semantic_prompt, actual_row.semantic_prompt);
    try expectEqual(expected_row.kitty_virtual_placeholder, actual_row.kitty_virtual_placeholder);
    const expected_cells = expected.cells(.all);
    const actual_cells = actual.cells(.all);
    try expectEqual(expected_cells.len, actual_cells.len);
    for (expected_cells, actual_cells) |*expected_cell, *actual_cell| {
        try expectEqualCells(expected, expected_cell, actual, actual_cell);
    }
}

fn expectEqualCells(
    expected_pin: Pin,
    expected: *const ghostty_vt.Cell,
    actual_pin: Pin,
    actual: *const ghostty_vt.Cell,
) !void {
    try expectEqual(expected.content_tag, actual.content_tag);
    switch (expected.content_tag) {
        .codepoint, .codepoint_grapheme => try expectEqual(expected.content.codepoint.data, actual.content.codepoint.data),
        .bg_color_palette => try expectEqual(expected.content.color_palette.data, actual.content.color_palette.data),
        .bg_color_rgb => try expect(std.meta.eql(expected.content.color_rgb, actual.content.color_rgb)),
    }
    try expectEqual(expected.wide, actual.wide);
    try expectEqual(expected.protected, actual.protected);
    try expectEqual(expected.semantic_content, actual.semantic_content);
    try expect(expected_pin.style(expected).eql(actual_pin.style(actual)));
    const expected_grapheme: []const u21 = expected_pin.grapheme(expected) orelse &.{};
    const actual_grapheme: []const u21 = actual_pin.grapheme(actual) orelse &.{};
    try std.testing.expectEqualSlices(u21, expected_grapheme, actual_grapheme);
    try expectEqual(expected.hyperlink, actual.hyperlink);
    if (!expected.hyperlink) return;
    const expected_page = expected_pin.node.page();
    const actual_page = actual_pin.node.page();
    const expected_link = expected_page.hyperlink_set.get(expected_page.memory, expected_page.lookupHyperlink(expected).?);
    const actual_link = actual_page.hyperlink_set.get(actual_page.memory, actual_page.lookupHyperlink(actual).?);
    try std.testing.expectEqualStrings(expected_link.uri.slice(expected_page.memory), actual_link.uri.slice(actual_page.memory));
    switch (expected_link.id) {
        .implicit => |id| switch (actual_link.id) {
            .implicit => |other| try expectEqual(id, other),
            .explicit => return error.TestExpectedEqual,
        },
        .explicit => |id| switch (actual_link.id) {
            .explicit => |other| try std.testing.expectEqualStrings(
                id.slice(expected_page.memory),
                other.slice(actual_page.memory),
            ),
            .implicit => return error.TestExpectedEqual,
        },
    }
}

fn expectEqualLinkIds(expected: anytype, actual: @TypeOf(expected)) !void {
    switch (expected) {
        .implicit => |id| switch (actual) {
            .implicit => |other| try expectEqual(id, other),
            .explicit => return error.TestExpectedEqual,
        },
        .explicit => |id| switch (actual) {
            .explicit => |other| try std.testing.expectEqualStrings(id, other),
            .implicit => return error.TestExpectedEqual,
        },
    }
}
