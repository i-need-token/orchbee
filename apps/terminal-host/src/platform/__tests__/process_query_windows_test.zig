const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const subject = platform.process;
const query = subject.testing.process_query;
const allocator = std.testing.allocator;

fn rawFacts() query.RawFactsType {
    var raw = std.mem.zeroes(query.RawFactsType);
    raw.creation_time = 19;
    raw.image_length = 3;
    raw.image[0..3].* = .{ 'x', '.', 'e' };
    raw.command_length = 4;
    raw.command[0..4].* = .{ 0x4e2d, ' ', 0xd83d, 0xde00 };
    raw.directory_length = std.math.maxInt(u32);
    return raw;
}

fn decodeWithAllocationFailures(alloc: std.mem.Allocator) !void {
    const raw = rawFacts();
    const decoded = try query.decodeFacts(alloc, 23, &raw);
    defer decoded.deinit(alloc);
    try std.testing.expectEqualStrings("中 😀", decoded.command_line.?);
    try std.testing.expectEqualStrings("x.e", decoded.image.?);
    try std.testing.expectEqual(@as(?u32, null), decoded.parent_pid);
    try std.testing.expectEqual(@as(?[]const u8, null), decoded.current_directory);
}

test "process facts strictly decode Unicode and roll back every allocating prefix" {
    try std.testing.checkAllAllocationFailures(allocator, decodeWithAllocationFailures, .{});
    var raw = rawFacts();
    raw.command[3] = 'x'; // Unpaired surrogate is absent, never replacement text.
    const invalid = try query.decodeFacts(allocator, 23, &raw);
    defer invalid.deinit(allocator);
    try std.testing.expectEqual(@as(?[]const u8, null), invalid.command_line);
    raw.command_length = 4097;
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFacts(allocator, 23, &raw));
    raw = rawFacts();
    raw.command[0] = 0;
    const nul = try query.decodeFacts(allocator, 23, &raw);
    defer nul.deinit(allocator);
    try std.testing.expectEqual(@as(?[]const u8, null), nul.command_line);
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFacts(allocator, 0, &raw));
    raw.creation_time = 0;
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFacts(allocator, 23, &raw));
}

extern "kernel32" fn GetCurrentProcessId() callconv(.winapi) u32;

test "Windows process samples expose owned shell and child facts, deny outsiders, and retain exited root identity" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try subject.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(executable);
    const directory = try subject.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(directory);
    var process = try subject.Process.spawn(allocator, .{
        .arguments = &.{ "/D", "/S", "/C", "ping -t 127.0.0.1 >nul" },
        .columns = 80,
        .rows = 24,
        .current_directory = directory,
        .environment = &.{},
        .executable = executable,
    });
    defer process.deinit();
    var stopped = false;
    defer if (!stopped) {
        process.forceTerminate() catch {};
        _ = process.wait() catch {};
        process.finishPtyAfterChildExit() catch {};
    };
    const outsider = query.openJobProcess(process.job_handle, GetCurrentProcessId());
    defer if (outsider) |handle| {
        _ = std.os.windows.CloseHandle(handle);
    };
    try std.testing.expect(outsider == null);

    var root_pid: u32 = 0;
    var observed = false;
    const timer = try platform.monotonic.Timer.start();
    while (timer.read() < 3 * std.time.ns_per_s) {
        var sample = subject.sampleProcesses(&process, allocator) catch |err| switch (err) {
            error.ProcessSampleChanged, error.ProcessSampleUnavailable => {
                platform.monotonic.sleep(5 * std.time.ns_per_ms);
                continue;
            },
            else => return err,
        };
        defer sample.deinit();
        if (sample.processes.len < 2) {
            platform.monotonic.sleep(5 * std.time.ns_per_ms);
            continue;
        }
        root_pid = sample.root_pid;
        var shell = false;
        var child = false;
        for (sample.processes) |row| {
            try std.testing.expect(row.creation_time > 0);
            if (row.pid == root_pid) {
                try std.testing.expect(std.ascii.endsWithIgnoreCase(row.image.?, "cmd.exe"));
                try std.testing.expect(std.mem.indexOf(u8, row.command_line.?, "ping -t") != null);
                // RTL CurrentDirectory retains a trailing separator.
                try std.testing.expect(std.ascii.eqlIgnoreCase(
                    std.mem.trimEnd(u8, directory, "\\/"),
                    std.mem.trimEnd(u8, row.current_directory.?, "\\/"),
                ));
                shell = true;
            } else if (row.image) |image| {
                if (std.ascii.endsWithIgnoreCase(image, "ping.exe")) {
                    try std.testing.expectEqual(@as(?u32, root_pid), row.parent_pid);
                    child = true;
                }
            }
        }
        observed = shell and child;
        if (observed) break;
        platform.monotonic.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(observed);
    try process.forceTerminate();
    _ = try process.wait();
    try process.finishPtyAfterChildExit();
    stopped = true;
    var ended = try subject.sampleProcesses(&process, allocator);
    defer ended.deinit();
    try std.testing.expectEqual(root_pid, ended.root_pid);
    try std.testing.expectEqual(@as(usize, 0), ended.processes.len);
}
