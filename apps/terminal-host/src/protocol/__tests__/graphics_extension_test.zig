const std = @import("std");
const protocol = @import("terminal-protocol");

test "native graphics validator accepts one canonical resource snapshot" {
    const bytes = validGraphics();
    const summary = try protocol.validateTerminalGraphics(&bytes);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.snapshot, summary.kind);
    try std.testing.expectEqual(@as(u16, 1), summary.image_count);
    try std.testing.expectEqual(@as(u16, 1), summary.placement_count);
    try std.testing.expectEqual(@as(u64, 7), summary.resource_revision);
    try std.testing.expectEqual(@as(u64, 9), summary.animation_tick);
}

test "native graphics validator preserves the complete signed z-order domain" {
    var negative_z = validGraphics();
    std.mem.writeInt(i32, negative_z[123..127], -0x4000_0001, .little);
    const summary = try protocol.validateTerminalGraphics(&negative_z);
    try std.testing.expectEqual(protocol.TerminalGraphicsUpdateKind.snapshot, summary.kind);
    try std.testing.expectEqual(@as(u16, 1), summary.placement_count);
}

test "native graphics validator rejects crop overflow and unknown placement flags" {
    var invalid_crop = validGraphics();
    std.mem.writeInt(u32, invalid_crop[99..103], 1, .little);
    try std.testing.expectError(error.InvalidField, protocol.validateTerminalGraphics(&invalid_crop));

    var unknown_flags = validGraphics();
    std.mem.writeInt(u32, unknown_flags[127..131], 2, .little);
    try std.testing.expectError(
        error.UnknownRequiredFlag,
        protocol.validateTerminalGraphics(&unknown_flags),
    );
}

fn validGraphics() [131]u8 {
    var bytes = [_]u8{0} ** 131;
    @memcpy(bytes[0..4], protocol.terminal_graphics_magic);
    bytes[4] = protocol.terminal_graphics_format_major;
    bytes[5] = protocol.terminal_graphics_format_minor;
    bytes[6] = protocol.terminal_graphics_format_patch;
    bytes[7] = @intFromEnum(protocol.TerminalGraphicsUpdateKind.snapshot);
    std.mem.writeInt(u32, bytes[8..12], bytes.len, .little);
    std.mem.writeInt(u64, bytes[12..20], 7, .little);
    std.mem.writeInt(u64, bytes[20..28], 9, .little);
    std.mem.writeInt(u16, bytes[28..30], 1, .little);
    std.mem.writeInt(u16, bytes[30..32], 1, .little);
    std.mem.writeInt(u32, bytes[32..36], 3, .little);

    std.mem.writeInt(u32, bytes[40..44], 11, .little);
    bytes[44] = 3;
    std.mem.writeInt(u64, bytes[48..56], 5, .little);
    std.mem.writeInt(u32, bytes[56..60], 1, .little);
    std.mem.writeInt(u32, bytes[60..64], 1, .little);
    std.mem.writeInt(u32, bytes[64..68], 3, .little);
    const pixels = [_]u8{ 1, 2, 3 };
    @memcpy(bytes[72..75], &pixels);

    std.mem.writeInt(u32, bytes[75..79], 11, .little);
    std.mem.writeInt(u32, bytes[79..83], 2, .little);
    std.mem.writeInt(u32, bytes[107..111], 1, .little);
    std.mem.writeInt(u32, bytes[111..115], 1, .little);
    std.mem.writeInt(u32, bytes[115..119], 8, .little);
    std.mem.writeInt(u32, bytes[119..123], 16, .little);
    return bytes;
}
