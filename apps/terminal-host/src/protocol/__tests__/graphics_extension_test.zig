const std = @import("std");
const protocol = @import("terminal-protocol");

test "native graphics validator accepts one canonical resource snapshot" {
    const bytes = validGraphics();
    const summary = try protocol.validateTerminalGraphics(&bytes);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.snapshot, summary.kind);
    try std.testing.expectEqual(@as(u16, 1), summary.image_count);
    try std.testing.expectEqual(@as(u16, 1), summary.placement_count);
    try std.testing.expectEqual(@as(u64, 7), summary.resource_revision);
}

test "native graphics validator rejects crop overflow and unknown placement flags" {
    var invalid_crop = validGraphics();
    std.mem.writeInt(u32, invalid_crop[91..95], 1, .little);
    try std.testing.expectError(error.InvalidField, protocol.validateTerminalGraphics(&invalid_crop));

    var unknown_flags = validGraphics();
    std.mem.writeInt(u32, unknown_flags[119..123], 2, .little);
    try std.testing.expectError(
        error.UnknownRequiredFlag,
        protocol.validateTerminalGraphics(&unknown_flags),
    );
}

fn validGraphics() [123]u8 {
    var bytes = [_]u8{0} ** 123;
    @memcpy(bytes[0..4], protocol.terminal_graphics_magic);
    bytes[4] = protocol.terminal_graphics_format_major;
    bytes[5] = protocol.terminal_graphics_format_minor;
    bytes[6] = protocol.terminal_graphics_format_patch;
    bytes[7] = @intFromEnum(protocol.TerminalGraphicsUpdateKind.snapshot);
    std.mem.writeInt(u32, bytes[8..12], bytes.len, .little);
    std.mem.writeInt(u64, bytes[12..20], 7, .little);
    std.mem.writeInt(u16, bytes[20..22], 1, .little);
    std.mem.writeInt(u16, bytes[22..24], 1, .little);
    std.mem.writeInt(u32, bytes[24..28], 3, .little);

    std.mem.writeInt(u32, bytes[32..36], 11, .little);
    bytes[36] = 3;
    std.mem.writeInt(u64, bytes[40..48], 5, .little);
    std.mem.writeInt(u32, bytes[48..52], 1, .little);
    std.mem.writeInt(u32, bytes[52..56], 1, .little);
    std.mem.writeInt(u32, bytes[56..60], 3, .little);
    const pixels = [_]u8{ 1, 2, 3 };
    @memcpy(bytes[64..67], &pixels);

    std.mem.writeInt(u32, bytes[67..71], 11, .little);
    std.mem.writeInt(u32, bytes[71..75], 2, .little);
    std.mem.writeInt(u32, bytes[99..103], 1, .little);
    std.mem.writeInt(u32, bytes[103..107], 1, .little);
    std.mem.writeInt(u32, bytes[107..111], 8, .little);
    std.mem.writeInt(u32, bytes[111..115], 16, .little);
    return bytes;
}
