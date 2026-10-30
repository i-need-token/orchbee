const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const Process = platform.process.Process;
const windows = std.os.windows;
const windows_api = platform.testing.windows_api;
const test_options = @import("platform_test_options");

extern "kernel32" fn GetCurrentProcessId() callconv(.winapi) windows.DWORD;
extern "kernel32" fn CreateEventA(
    attributes: ?*anyopaque,
    manual_reset: windows.BOOL,
    initial_state: windows.BOOL,
    name: [*:0]const u8,
) callconv(.winapi) ?windows.HANDLE;
extern "kernel32" fn SetEvent(event: windows.HANDLE) callconv(.winapi) windows.BOOL;

fn freshEvent(name: [:0]const u8) !windows.HANDLE {
    const event = CreateEventA(null, .TRUE, .FALSE, name) orelse return error.EventCreationFailed;
    if (windows.GetLastError() == .ALREADY_EXISTS) {
        windows.CloseHandle(event);
        return error.TestEventAlreadyExists;
    }
    return event;
}

// Test-only handle attenuation exercises real kernel permission failures without
// changing the production Windows API facade or substituting process identity.
extern "kernel32" fn DuplicateHandle(
    source_process: windows.HANDLE,
    source: windows.HANDLE,
    target_process: windows.HANDLE,
    target: *windows.HANDLE,
    access: windows.DWORD,
    inherit: windows.BOOL,
    options: windows.DWORD,
) callconv(.winapi) windows.BOOL;

fn restrictedHandle(handle: windows.HANDLE, access: windows.DWORD) !windows.HANDLE {
    var result: windows.HANDLE = undefined;
    const current = windows.GetCurrentProcess();
    if (!DuplicateHandle(current, handle, current, &result, access, .FALSE, 0).toBool())
        return error.HandleDuplicationFailed;
    return result;
}

fn spawn(command: []const u8) !Process {
    const executable = try platform.process.environmentVariableOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    return Process.spawn(std.testing.allocator, .{
        .arguments = &.{ "/D", "/S", "/C", command },
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
}

fn cleanup(process: *Process) void {
    process.forceTerminate() catch @panic("test process force cleanup failed");
    _ = process.wait() catch @panic("test process exit observation failed");
    process.finishPtyAfterChildExit() catch @panic("test process PTY cleanup failed");
    process.deinit();
}

fn expectExited(process: *Process) !void {
    try std.testing.expectEqual(windows_api.WAIT_OBJECT_0, windows_api.WaitForSingleObject(process.process_handle, 5_000));
}

test "Windows successful force submission makes subsequent graceful termination idempotent" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var process = try spawn("ping -n 31 127.0.0.1 >nul");
    defer cleanup(&process);
    try process.forceTerminate();
    // Do not wait for exit before issuing the same deferred graceful request as
    // runtime teardown. A successful force syscall need not signal the root yet.
    try process.terminate();
    try process.terminate();
    try expectExited(&process);
}

test "Windows denied live root termination cannot fabricate a successful submission" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var process = try spawn("ping -n 31 127.0.0.1 >nul");
    defer cleanup(&process);
    const original = process.process_handle;
    const restricted = try restrictedHandle(original, 0x00100000); // SYNCHRONIZE only.
    defer windows.CloseHandle(restricted);
    {
        process.process_handle = restricted;
        defer process.process_handle = original;
        try std.testing.expectEqual(@as(windows.DWORD, 258), windows_api.WaitForSingleObject(restricted, 0));
        try std.testing.expectError(error.AccessDenied, process.terminate());
        try std.testing.expectError(error.AccessDenied, process.terminate());
    }
    try process.terminate();
    try expectExited(&process);
}

test "Windows denied Job access cannot fabricate a root submission or hide behind root exit" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var process = try spawn("ping -n 31 127.0.0.1 >nul");
    defer cleanup(&process);
    const original = process.job_handle;
    // Without JOB_OBJECT_QUERY the console holders cannot be read, so force falls
    // back to Job termination, which JOB_OBJECT_TERMINATE alone would permit.
    const restricted = try restrictedHandle(original, 0x00100000); // SYNCHRONIZE only.
    defer windows.CloseHandle(restricted);
    {
        process.job_handle = restricted;
        defer process.job_handle = original;
        try std.testing.expectError(error.AccessDenied, process.forceTerminate());
        try process.terminate();
        try expectExited(&process);
        try std.testing.expectError(error.AccessDenied, process.forceTerminate());
    }
}

test "Windows force after graceful submission still terminates retained descendants" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var begin_buffer: [128]u8 = undefined;
    var ready_buffer: [128]u8 = undefined;
    const begin_name = try std.fmt.bufPrintZ(&begin_buffer, "Local\\OrchbeeHostDescendant-{d}-begin", .{GetCurrentProcessId()});
    const ready_name = try std.fmt.bufPrintZ(&ready_buffer, "Local\\OrchbeeHostDescendant-{d}-ready", .{GetCurrentProcessId()});
    const begin = try freshEvent(begin_name);
    defer windows.CloseHandle(begin);
    const ready = try freshEvent(ready_name);
    defer windows.CloseHandle(ready);
    var process = try Process.spawn(std.testing.allocator, .{
        .arguments = &.{ begin_name, ready_name },
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = test_options.retained_descendant_fixture,
        .rows = 24,
    });
    defer cleanup(&process);
    var timer = try platform.monotonic.Timer.start();
    while (!try process.hasActiveCommand()) {
        if (timer.read() > 5 * std.time.ns_per_s) return error.DescendantDidNotStart;
        platform.monotonic.sleep(std.time.ns_per_ms);
    }
    // Job membership alone cannot prove that the child has reached user code.
    // Keep that counterexample deterministic, then admit the real witness only
    // after the child's explicit acknowledgment. It has no natural exit timer.
    try std.testing.expectEqual(@as(windows.DWORD, 258), windows_api.WaitForSingleObject(ready, 0));
    try std.testing.expect(SetEvent(begin).toBool());
    try std.testing.expectEqual(windows_api.WAIT_OBJECT_0, windows_api.WaitForSingleObject(ready, 5_000));
    try process.terminate();
    try expectExited(&process);
    try std.testing.expect(!try process.processTreeContained());
    try process.forceTerminate();
    try process.finishPtyAfterChildExit();
    try std.testing.expect(try process.processTreeContained());
}

const ConcurrentTermination = struct {
    process: *Process,
    ready: std.atomic.Value(u32) = .init(0),
    start: std.atomic.Value(bool) = .init(false),
    graceful_error: ?anyerror = null,
    force_error: ?anyerror = null,

    fn run(self: *@This(), force: bool) void {
        _ = self.ready.fetchAdd(1, .release);
        while (!self.start.load(.acquire)) std.atomic.spinLoopHint();
        if (force) {
            self.process.forceTerminate() catch |err| {
                self.force_error = err;
            };
        } else {
            self.process.terminate() catch |err| {
                self.graceful_error = err;
            };
        }
    }
};

test "Windows concurrent force and graceful submissions both complete without losing containment" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var process = try spawn("ping -n 31 127.0.0.1 >nul");
    defer cleanup(&process);
    var race: ConcurrentTermination = .{ .process = &process };
    const graceful = try std.Thread.spawn(.{}, ConcurrentTermination.run, .{ &race, false });
    const force = std.Thread.spawn(.{}, ConcurrentTermination.run, .{ &race, true }) catch |err| {
        race.start.store(true, .release);
        graceful.join();
        return err;
    };
    while (race.ready.load(.acquire) != 2) std.atomic.spinLoopHint();
    race.start.store(true, .release);
    graceful.join();
    force.join();
    try std.testing.expectEqual(@as(?anyerror, null), race.graceful_error);
    try std.testing.expectEqual(@as(?anyerror, null), race.force_error);
    try expectExited(&process);
    try process.finishPtyAfterChildExit();
    try std.testing.expect(try process.processTreeContained());
}

test "Windows signaled root exit status 259 remains idempotent without prior termination" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var process = try spawn("exit 259");
    defer cleanup(&process);
    try expectExited(&process);
    try std.testing.expectEqual(platform.process.ExitResult{ .exit_code = 259 }, try process.wait());
    try process.terminate();
}
