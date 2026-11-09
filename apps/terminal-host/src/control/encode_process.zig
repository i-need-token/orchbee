const std = @import("std");
const common = @import("encode_common.zig");
const protocol = @import("terminal-protocol");

pub fn processInfo(allocator: std.mem.Allocator, attachment_id: anytype, terminal_id: anytype, sample: anytype) ![]u8 {
    return processInfoWithLaunchKind(allocator, attachment_id, terminal_id, sample, null);
}

/// A present launch kind wire name (`managed-powershell`, `cmd` or `other`) is
/// written after `kind`; a reader asks for it with `includeLaunchKind`, so
/// earlier readers keep receiving their exact shape.
pub fn processInfoWithLaunchKind(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    terminal_id: anytype,
    sample: anytype,
    launch_kind: ?[]const u8,
) ![]u8 {
    // Fixed-capacity encoding prevents a large process command line from
    // allocating beyond the wire budget before the limit is detected.
    const buffer = try allocator.alloc(u8, protocol.Limits.control_json_bytes);
    defer allocator.free(buffer);
    var writer = std.Io.Writer.fixed(buffer);
    writeProcessInfo(&writer, attachment_id, terminal_id, sample, launch_kind) catch return error.ProcessInfoTooLarge;
    return allocator.dupe(u8, writer.buffered());
}

pub fn writeProcessInfo(
    writer: *std.Io.Writer,
    attachment_id: anytype,
    terminal_id: anytype,
    sample: anytype,
    launch_kind: ?[]const u8,
) !void {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    try writer.print("{{\"attachmentId\":\"{s}\",\"terminalId\":\"{s}\",\"kind\":\"windows-job\",", .{
        common.identifierText(attachment_id, &attachment_buffer),
        common.identifierText(terminal_id, &terminal_buffer),
    });
    if (launch_kind) |value| try writer.print("\"launchKind\":\"{s}\",", .{value});
    try writer.print("\"rootPid\":{d},\"processes\":[", .{sample.root_pid});
    // Compact positional rows keep the 256-row bound within the control payload:
    // [pid, parentPid, creationTime, console, image, commandLine, currentDirectory].
    for (sample.processes, 0..) |row, index| {
        if (index > 0) try writer.writeAll(",");
        try writer.print("[{d},", .{row.pid});
        try std.json.Stringify.value(row.parent_pid, .{}, writer);
        try writer.print(",\"{d}\",\"{s}\",", .{ row.creation_time, @tagName(row.console) });
        try std.json.Stringify.value(row.image, .{}, writer);
        try writer.writeAll(",");
        try std.json.Stringify.value(row.command_line, .{}, writer);
        try writer.writeAll(",");
        try std.json.Stringify.value(row.current_directory, .{}, writer);
        try writer.writeAll("]");
    }
    try writer.writeAll("]}");
}
