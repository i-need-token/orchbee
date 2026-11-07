const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const engine_boundary = @import("terminal-host-engine");
const output_seed = engine_boundary.output_seed;
const terminal_stream = engine_boundary.testing.terminal_stream;

// Admission is the boundary that keeps replayed output from ever answering into the PTY.
test "Output seed admits only text, line breaks and SGR" {
    try std.testing.expect(output_seed.admit("plain\r\n\x1b[1;38;2;1;2;3mbold\x1b[0m \xe4\xb8\xad\x1b[4:3m"));
    const refused = [_][]const u8{
        "",
        "\x1b]0;title\x07",
        "\x1b[6n",
        "\x1b[?1049h",
        "\x1bP+q\x1b\\",
        "\x1b",
        "\x1b[1",
        "tab\t",
        "bell\x07",
        "\x7f",
        "\xc2\x9b6n",
        "\xff",
    };
    for (refused) |bytes| try std.testing.expect(!output_seed.admit(bytes));
}

test "Applied output seed sits in scrollback above a blank screen with the cursor home" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 20,
        .rows = 3,
        .max_scrollback_bytes = null,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream: terminal_stream.Stream = .init(.{ .handler = .init(&terminal) });
    defer stream.deinit();

    output_seed.apply(&stream, "one\r\ntwo\x1b[2m\r\ndivider");
    try std.testing.expectEqual(@as(u16, 0), terminal.screens.active.cursor.x);
    try std.testing.expectEqual(@as(u16, 0), terminal.screens.active.cursor.y);
    {
        const screen = try terminal.plainString(std.testing.allocator);
        defer std.testing.allocator.free(screen);
        try std.testing.expectEqualStrings("", screen);
    }
    stream.nextSlice("PS>");
    const all = try terminal.screens.active.dumpStringAlloc(std.testing.allocator, .{ .screen = .{} });
    defer std.testing.allocator.free(all);
    try std.testing.expectEqualStrings("one\ntwo\ndivider\nPS>", all);
}
