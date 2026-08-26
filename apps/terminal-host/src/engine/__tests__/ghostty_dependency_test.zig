const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

test "the pinned Ghostty VT dependency parses terminal output" {
    const allocator = std.testing.allocator;
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(allocator);

    try terminal.printString("orchbee");
    const plain = try terminal.plainString(allocator);
    defer allocator.free(plain);

    try std.testing.expectEqualStrings("orchbee", plain);
}
