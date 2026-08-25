const std = @import("std");
const subject = @import("terminal-protocol");

fn metadataOnlyDelta(columns: u16, rows: u16) [subject.terminal_projection_buffer_header_bytes]u8 {
    var bytes = [_]u8{0} ** subject.terminal_projection_buffer_header_bytes;
    @memcpy(bytes[0..4], "OBPB");
    bytes[4] = subject.terminal_projection_buffer_format_major;
    bytes[5] = subject.terminal_projection_buffer_format_minor;
    bytes[6] = 2;
    std.mem.writeInt(u32, bytes[8..12], bytes.len, .little);
    std.mem.writeInt(u16, bytes[12..14], bytes.len, .little);
    std.mem.writeInt(u16, bytes[14..16], columns, .little);
    std.mem.writeInt(u16, bytes[16..18], rows, .little);
    @memset(bytes[24..40], 0x11);
    @memset(bytes[40..56], 0x22);
    std.mem.writeInt(u64, bytes[56..64], 2, .little);
    std.mem.writeInt(u64, bytes[64..72], 1, .little);
    std.mem.writeInt(u64, bytes[72..80], 2, .little);
    std.mem.writeInt(u64, bytes[80..88], 1, .little);
    std.mem.writeInt(u16, bytes[100..102], 8, .little);
    std.mem.writeInt(u16, bytes[102..104], 16, .little);
    bytes[104] = 1;
    bytes[106] = 1;
    std.mem.writeInt(u32, bytes[112..116], 0x00ff_ffff, .little);
    std.mem.writeInt(u32, bytes[120..124], 0x0100_0000, .little);
    std.mem.writeInt(u32, bytes[124..128], std.math.maxInt(u32), .little);
    std.mem.writeInt(u32, bytes[132..136], std.math.maxInt(u32), .little);
    bytes[144] = subject.terminal_projection_buffer_format_patch;
    std.mem.writeInt(u64, bytes[145..153], 1, .little);
    for (0..16) |section| {
        const directory_offset = 160 + section * 8;
        std.mem.writeInt(u32, bytes[directory_offset..][0..4], bytes.len, .little);
    }

    return bytes;
}

test "projection validator accepts a metadata-only delta with no dirty rows" {
    const bytes = metadataOnlyDelta(1, 1);

    const summary = try subject.validateTerminalProjectionBuffer(&bytes);
    try std.testing.expectEqual(subject.FrameKind.delta, summary.kind);
    try std.testing.expectEqual(@as(u16, 0), summary.row_count);
    try std.testing.expectEqual(@as(?u64, 1), summary.base_revision);
}

test "projection validator enforces the grid-area cell limit for metadata-only deltas" {
    const maximum_grid = metadataOnlyDelta(512, 512);
    _ = try subject.validateTerminalProjectionBuffer(&maximum_grid);

    const oversized_grid = metadataOnlyDelta(512, 513);
    try std.testing.expectError(
        error.InvalidField,
        subject.validateTerminalProjectionBuffer(&oversized_grid),
    );
}
