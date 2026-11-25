const std = @import("std");
const control_boundary = @import("terminal-host-control");
const model = @import("terminal-host-model");
const protocol = @import("terminal-protocol");

const encode = control_boundary.encode;
const Console = enum { attached, detached, none, unknown };
const Row = struct {
    pid: u32,
    parent_pid: ?u32,
    creation_time: u64,
    console: Console,
    image: ?[]const u8,
    command_line: ?[]const u8,
    current_directory: ?[]const u8,
};

fn identifiers() struct { model.identifiers.AttachmentId, model.identifiers.TerminalId } {
    return .{
        model.identifiers.AttachmentId.fromBytes(.{0x33} ** 16),
        model.identifiers.TerminalId.fromBytes(.{0x44} ** 16),
    };
}

test "an oversized process sample keeps every member and the text nearest its root" {
    const allocator = std.testing.allocator;
    const long = "C:\\work\\" ++ ("a" ** 3800);
    var rows: [42]Row = undefined;
    rows[0] = .{ .pid = 10, .parent_pid = null, .creation_time = 100, .console = .attached, .image = "C:\\pwsh.exe", .command_line = long, .current_directory = "C:\\work" };
    rows[1] = .{ .pid = 11, .parent_pid = 10, .creation_time = 200, .console = .attached, .image = "C:\\claude.exe", .command_line = long, .current_directory = "C:\\work" };
    // Members of a nested session whose launcher exited: no parent chain reaches the root.
    for (rows[2..], 0..) |*row, index| row.* = .{
        .pid = 1000 + @as(u32, @intCast(index)),
        .parent_pid = 999,
        .creation_time = 300 + index,
        .console = .detached,
        .image = "C:\\pwsh.exe",
        .command_line = long,
        .current_directory = "C:\\work",
    };
    const attachment_id, const terminal_id = identifiers();
    const payload = try encode.processInfo(allocator, attachment_id, terminal_id, .{ .root_pid = @as(u32, 10), .processes = rows[0..] });
    defer allocator.free(payload);
    try std.testing.expect(payload.len <= protocol.Limits.control_json_bytes);

    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, payload, .{});
    defer parsed.deinit();
    const processes = parsed.value.object.get("processes").?.array.items;
    try std.testing.expectEqual(rows.len, processes.len);
    for (processes, rows) |process, row| {
        try std.testing.expectEqual(@as(i64, row.pid), process.array.items[0].integer);
        try std.testing.expectEqualStrings(row.image.?, process.array.items[4].string);
    }
    for (processes[0..2]) |process| {
        try std.testing.expectEqualStrings(long, process.array.items[5].string);
        try std.testing.expectEqualStrings("C:\\work", process.array.items[6].string);
    }
    // The newest stray members gave up their text first, and only as many as the bound needed.
    var dropped: usize = 0;
    for (processes[2..]) |process| {
        const absent = process.array.items[5] == .null;
        try std.testing.expectEqual(absent, process.array.items[6] == .null);
        if (absent) dropped += 1 else try std.testing.expectEqual(@as(usize, 0), dropped);
    }
    try std.testing.expect(dropped > 0 and dropped < rows.len - 2);
    try std.testing.expect(processes[processes.len - 1].array.items[5] == .null);
}

test "a process sample that exceeds the bound without any text still fails" {
    const allocator = std.testing.allocator;
    const image = "C:\\" ++ ("b" ** 4000);
    var rows: [256]Row = undefined; // The Host's row bound.
    for (&rows, 0..) |*row, index| row.* = .{
        .pid = 10 + @as(u32, @intCast(index)),
        .parent_pid = if (index == 0) null else 10,
        .creation_time = 100 + index,
        .console = .attached,
        .image = image,
        .command_line = null,
        .current_directory = null,
    };
    const attachment_id, const terminal_id = identifiers();
    try std.testing.expectError(
        error.ProcessInfoTooLarge,
        encode.processInfo(allocator, attachment_id, terminal_id, .{ .root_pid = @as(u32, 10), .processes = rows[0..] }),
    );
}
