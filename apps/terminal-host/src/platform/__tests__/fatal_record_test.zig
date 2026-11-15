const std = @import("std");
const fatal_record = @import("terminal-host-platform").fatal_record;
const test_options = @import("platform_test_options");

const closed_keys = [_][]const u8{
    "format", "role",          "kind",       "reason",       "error",
    "thread", "module_offset", "process_id", "time_unix_ms",
};

fn parseClosedRecord(bytes: []const u8) !std.json.Parsed(std.json.Value) {
    try std.testing.expect(bytes.len != 0 and bytes[bytes.len - 1] == '\n');
    try std.testing.expect(std.mem.indexOfScalar(u8, bytes[0 .. bytes.len - 1], '\n') == null);
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, bytes, .{});
    errdefer parsed.deinit();
    const object = parsed.value.object;
    try std.testing.expectEqual(closed_keys.len, object.count());
    for (closed_keys) |key| try std.testing.expect(object.contains(key));
    try std.testing.expectEqualStrings("terminal-host", object.get("role").?.string);
    return parsed;
}

test "the fatal record holds only its closed keys and drops an error name that is not an identifier" {
    var buffer: [fatal_record.max_record_bytes]u8 = undefined;
    const named = try parseClosedRecord(try fatal_record.encode(&buffer, .{
        .kind = .panic,
        .reason = .@"unwrap-error",
        .error_name = "OutOfMemory",
        .thread = .@"terminal-output",
        .module_offset = 4096,
        .process_id = 7,
        .unix_milliseconds = 1,
    }));
    defer named.deinit();
    try std.testing.expectEqualStrings("OutOfMemory", named.value.object.get("error").?.string);
    try std.testing.expectEqual(@as(i64, 4096), named.value.object.get("module_offset").?.integer);

    var longest_buffer: [fatal_record.max_record_bytes]u8 = undefined;
    const longest = try parseClosedRecord(try fatal_record.encode(&longest_buffer, .{
        .kind = .exception,
        .reason = .@"integer-part-out-of-bounds",
        .error_name = "\"title\" from program output",
        .thread = .@"terminal-animation",
        .module_offset = null,
        .process_id = std.math.maxInt(u32),
        .unix_milliseconds = std.math.maxInt(u64),
    }));
    defer longest.deinit();
    try std.testing.expect(longest.value.object.get("error").? == .null);
    try std.testing.expect(longest.value.object.get("module_offset").? == .null);
}

test "a panic leaves one closed fatal record without its message and exits with the fatal code" {
    if (test_options.fatal_record_fixture.len == 0) return error.SkipZigTest;
    const io = std.testing.io;
    const allocator = std.testing.allocator;
    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();
    const directory = try tmp.dir.realPathFileAlloc(io, ".", allocator);
    defer allocator.free(directory);

    var child = try std.process.spawn(io, .{
        .argv = &.{ test_options.fatal_record_fixture, directory, "secret-title" },
        .stdin = .ignore,
        .stdout = .ignore,
        .stderr = .ignore,
    });
    const term = try child.wait(io);
    try std.testing.expectEqual(std.process.Child.Term{ .exited = fatal_record.exit_code }, term);

    const bytes = try tmp.dir.readFileAlloc(
        io,
        fatal_record.file_name,
        allocator,
        .limited(fatal_record.max_record_bytes + 1),
    );
    defer allocator.free(bytes);
    try std.testing.expect(std.mem.indexOf(u8, bytes, "secret-title") == null);
    const record = try parseClosedRecord(bytes);
    defer record.deinit();
    const object = record.value.object;
    try std.testing.expectEqualStrings("panic", object.get("kind").?.string);
    try std.testing.expectEqualStrings("explicit-panic", object.get("reason").?.string);
    try std.testing.expectEqualStrings("dispatch", object.get("thread").?.string);
    try std.testing.expect(object.get("module_offset").? == .integer);
}
