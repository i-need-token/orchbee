const std = @import("std");
const common = @import("encode_common.zig");
const protocol = @import("terminal-protocol");

pub fn processInfo(allocator: std.mem.Allocator, attachment_id: anytype, terminal_id: anytype, sample: anytype) ![]u8 {
    // Fixed-capacity encoding prevents a large process command line from
    // allocating beyond the wire budget before the limit is detected.
    const buffer = try allocator.alloc(u8, protocol.Limits.control_json_bytes);
    defer allocator.free(buffer);
    var writer = std.Io.Writer.fixed(buffer);
    writeProcessInfo(&writer, attachment_id, terminal_id, sample) catch return error.ProcessInfoTooLarge;
    return allocator.dupe(u8, writer.buffered());
}

pub fn writeProcessInfo(writer: *std.Io.Writer, attachment_id: anytype, terminal_id: anytype, sample: anytype) !void {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    try writer.print("{{\"attachmentId\":\"{s}\",\"terminalId\":\"{s}\",\"kind\":\"windows-job\",\"rootPid\":{d},\"processes\":[", .{
        common.identifierText(attachment_id, &attachment_buffer),
        common.identifierText(terminal_id, &terminal_buffer),
        sample.root_pid,
    });
    for (sample.processes, 0..) |row, index| {
        if (index > 0) try writer.writeAll(",");
        try writer.print("{{\"pid\":{d},\"creationTime\":\"{d}\",\"parentPid\":", .{ row.pid, row.creation_time });
        try std.json.Stringify.value(row.parent_pid, .{}, writer);
        try writer.writeAll(",\"image\":");
        try std.json.Stringify.value(row.image, .{}, writer);
        try writer.writeAll(",\"commandLine\":");
        try std.json.Stringify.value(row.command_line, .{}, writer);
        try writer.writeAll(",\"currentDirectory\":");
        try std.json.Stringify.value(row.current_directory, .{}, writer);
        try writer.writeAll("}");
    }
    try writer.writeAll("]}");
}
