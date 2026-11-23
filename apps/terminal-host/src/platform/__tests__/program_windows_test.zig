const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const test_options = @import("platform_test_options");

const program = platform.program;
const allocator = std.testing.allocator;

/// The fixture's absolute path; the build names it relative to the build root.
fn fixturePath() ![:0]u8 {
    return std.Io.Dir.cwd().realPathFileAlloc(std.testing.io, test_options.retained_descendant_fixture, allocator);
}

fn release(spawned: program.Spawned) void {
    _ = program.endMembers(spawned.job);
    _ = program.waitExit(spawned.process, 5000);
    for (spawned.streams) |stream| if (stream) |handle| program.close(handle);
    program.close(spawned.process);
    program.close(spawned.job);
}

fn drain(stream: program.Handle, buffer: []u8) []const u8 {
    var length: usize = 0;
    while (length < buffer.len) {
        const count = program.read(stream, buffer[length..]);
        if (count == 0) break;
        length += count;
    }
    return buffer[0..length];
}

fn awaitEmptyJob(job: program.Handle) !void {
    var timer = try platform.monotonic.Timer.start();
    while ((program.activeMembers(job) orelse return error.JobQueryFailed) != 0) {
        if (timer.read() > 5 * std.time.ns_per_s) return error.JobMembersRemain;
        platform.monotonic.sleep(std.time.ns_per_ms);
    }
}

test "Windows background program captures both streams, reports its code, and leaves no member behind its root" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const root = try platform.process.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(root);
    const shell = try std.fmt.allocPrint(allocator, "{s}\\System32\\cmd.exe", .{root});
    defer allocator.free(shell);
    const fixture = try fixturePath();
    defer allocator.free(fixture);
    // The started descendant inherits both pipes and outlives the shell; only the Job ends it.
    const raw = try std.fmt.allocPrint(
        allocator,
        "start \"\" /b \"{s}\" begin ready& echo out& echo err 1>&2& exit /b 7",
        .{fixture},
    );
    defer allocator.free(raw);
    const spawned = try program.spawn(allocator, .{
        .executable = shell,
        .arguments = &.{ "/d", "/c" },
        .raw_argument = raw,
        .directory = root,
        .environment = &.{.{ .name = "ORCHBEE_PROGRAM_PROBE", .value = "1" }},
        .capture = .{ true, true },
    });
    defer release(spawned);
    try std.testing.expect(spawned.pid != 0 and spawned.created != 0);
    try std.testing.expect(program.waitExit(spawned.process, 10_000));
    try std.testing.expectEqual(@as(?u32, 7), program.exitCode(spawned.process));
    try std.testing.expect((program.activeMembers(spawned.job) orelse 0) >= 1);
    try std.testing.expect(program.endMembers(spawned.job));
    try awaitEmptyJob(spawned.job);
    var output: [256]u8 = undefined;
    var errors: [256]u8 = undefined;
    try std.testing.expect(std.mem.indexOf(u8, drain(spawned.streams[0].?, &output), "out") != null);
    try std.testing.expect(std.mem.indexOf(u8, drain(spawned.streams[1].?, &errors), "err") != null);
}

test "Windows background program ends with its Job when cancelled and refuses a missing or relative program" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const root = try platform.process.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(root);
    const fixture = try fixturePath();
    defer allocator.free(fixture);
    const spawned = try program.spawn(allocator, .{
        .executable = fixture,
        .arguments = &.{ "begin", "ready" },
        .directory = root,
        .environment = &.{},
        .capture = .{ false, false },
    });
    defer release(spawned);
    try std.testing.expect(spawned.streams[0] == null and spawned.streams[1] == null);
    try std.testing.expect(!program.waitExit(spawned.process, 0));
    try std.testing.expect(program.endMembers(spawned.job));
    try std.testing.expect(program.waitExit(spawned.process, 5000));
    try std.testing.expectEqual(@as(?u32, 1), program.exitCode(spawned.process));
    try awaitEmptyJob(spawned.job);

    const missing = try std.fmt.allocPrint(allocator, "{s}\\orchbee-no-such-program.exe", .{root});
    defer allocator.free(missing);
    const launch: program.Launch = .{
        .executable = missing,
        .arguments = &.{},
        .directory = root,
        .environment = &.{},
        .capture = .{ true, true },
    };
    try std.testing.expectError(error.NotFound, program.spawn(allocator, launch));
    var relative = launch;
    relative.executable = "cmd.exe";
    try std.testing.expectError(error.InvalidCommand, program.spawn(allocator, relative));
}
