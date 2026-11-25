const std = @import("std");
const common = @import("encode_common.zig");
const protocol = @import("terminal-protocol");

pub fn processInfo(allocator: std.mem.Allocator, attachment_id: anytype, terminal_id: anytype, sample: anytype) ![]u8 {
    return processInfoWithLaunchKind(allocator, attachment_id, terminal_id, sample, null);
}

/// A present launch kind wire name (`managed-powershell`, `cmd`, `managed-zsh`,
/// `managed-bash` or `other`) is
/// written after `kind`; a reader asks for it with `includeLaunchKind`, so
/// earlier readers keep receiving their exact shape.
///
/// A sample over the control payload bound keeps every member: rows give up their command line
/// and current directory, the rows farthest from the root first, until it fits. Both fields are
/// already absent for a member whose text is unreadable, so readers need no new shape. A sample
/// that does not fit even without them fails.
pub fn processInfoWithLaunchKind(
    allocator: std.mem.Allocator,
    attachment_id: anytype,
    terminal_id: anytype,
    sample: anytype,
    launch_kind: ?[]const u8,
) ![]u8 {
    // Fixed-capacity encoding prevents a large process command line from
    // allocating beyond the wire budget before the limit is detected.
    const limit = protocol.Limits.control_json_bytes;
    const buffer = try allocator.alloc(u8, limit);
    defer allocator.free(buffer);
    var writer = std.Io.Writer.fixed(buffer);
    if (writeProcessInfo(&writer, attachment_id, terminal_id, sample, launch_kind, null)) |_| {
        return allocator.dupe(u8, writer.buffered());
    } else |_| {}
    const kept = try allocator.alloc(bool, sample.processes.len);
    defer allocator.free(kept);
    @memset(kept, true);
    var scratch: [256]u8 = undefined;
    var counter = std.Io.Writer.Discarding.init(&scratch);
    writeProcessInfo(&counter.writer, attachment_id, terminal_id, sample, launch_kind, null) catch return error.ProcessInfoTooLarge;
    var size = std.math.cast(usize, counter.fullCount()) orelse return error.ProcessInfoTooLarge;
    const order = try farthestFirst(allocator, sample);
    defer allocator.free(order);
    for (order) |index| {
        if (size <= limit) break;
        kept[index] = false;
        const row = sample.processes[index];
        // Each field is part of `size`, so taking its bytes back out cannot underflow.
        inline for (.{ row.command_line, row.current_directory }) |text| {
            size = size + "null".len - try encodedBytes(text);
        }
    }
    if (size > limit) return error.ProcessInfoTooLarge;
    writer = std.Io.Writer.fixed(buffer);
    writeProcessInfo(&writer, attachment_id, terminal_id, sample, launch_kind, kept) catch return error.ProcessInfoTooLarge;
    return allocator.dupe(u8, writer.buffered());
}

/// Row indexes in the order they give up text: no chain of parents to the root before any chain,
/// a longer chain before a shorter one, and a newer process before an older one. An Agent is
/// usually the root's child or grandchild, so its own text stays while descendants and stray
/// members, such as a test session's shells, lose theirs. The chain follows the same parent
/// links a reader trusts: a parent created after its child is a reused identifier and ends it.
fn farthestFirst(allocator: std.mem.Allocator, sample: anytype) ![]usize {
    const processes = sample.processes;
    const unlinked = std.math.maxInt(usize);
    const distance = try allocator.alloc(usize, processes.len);
    defer allocator.free(distance);
    for (processes, distance) |row, *value| value.* = if (row.pid == sample.root_pid) 0 else unlinked;
    // Each pass links at least one more generation, so the rows' count bounds the passes.
    var linked = true;
    var passes: usize = 0;
    while (linked and passes < processes.len) : (passes += 1) {
        linked = false;
        for (processes, distance) |row, *value| {
            if (value.* != unlinked) continue;
            const parent_pid = row.parent_pid orelse continue;
            for (processes, distance) |parent, parent_distance| {
                if (parent.pid != parent_pid or parent_distance == unlinked) continue;
                if (parent.creation_time > row.creation_time) break;
                value.* = parent_distance + 1;
                linked = true;
                break;
            }
        }
    }
    const order = try allocator.alloc(usize, processes.len);
    errdefer allocator.free(order);
    for (order, 0..) |*index, value| index.* = value;
    const Rank = struct {
        processes: @TypeOf(processes),
        distance: []const usize,

        fn before(self: @This(), left: usize, right: usize) bool {
            if (self.distance[left] != self.distance[right]) return self.distance[left] > self.distance[right];
            const left_created = self.processes[left].creation_time;
            const right_created = self.processes[right].creation_time;
            if (left_created != right_created) return left_created > right_created;
            return left > right;
        }
    };
    std.mem.sort(usize, order, Rank{ .processes = processes, .distance = distance }, Rank.before);
    return order;
}

/// The bytes one text field occupies in a row, `null` included.
fn encodedBytes(value: ?[]const u8) error{ProcessInfoTooLarge}!usize {
    var scratch: [256]u8 = undefined;
    var counter = std.Io.Writer.Discarding.init(&scratch);
    std.json.Stringify.value(value, .{}, &counter.writer) catch return error.ProcessInfoTooLarge;
    return std.math.cast(usize, counter.fullCount()) orelse error.ProcessInfoTooLarge;
}

/// `kept` is null to write every row's text; otherwise a false entry writes that row's command line
/// and current directory as absent.
fn writeProcessInfo(
    writer: *std.Io.Writer,
    attachment_id: anytype,
    terminal_id: anytype,
    sample: anytype,
    launch_kind: ?[]const u8,
    kept: ?[]const bool,
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
        const text = if (kept) |rows| rows[index] else true;
        const command_line: ?[]const u8 = if (text) row.command_line else null;
        const current_directory: ?[]const u8 = if (text) row.current_directory else null;
        if (index > 0) try writer.writeAll(",");
        try writer.print("[{d},", .{row.pid});
        try std.json.Stringify.value(row.parent_pid, .{}, writer);
        try writer.print(",\"{d}\",\"{s}\",", .{ row.creation_time, @tagName(row.console) });
        try std.json.Stringify.value(row.image, .{}, writer);
        try writer.writeAll(",");
        try std.json.Stringify.value(command_line, .{}, writer);
        try writer.writeAll(",");
        try std.json.Stringify.value(current_directory, .{}, writer);
        try writer.writeAll("]");
    }
    try writer.writeAll("]}");
}
