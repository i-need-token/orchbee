//! A pseudoconsole whose output nobody reads stops reading its signal pipe too: OpenConsole's
//! signal thread waits for the console, and the 4 KiB pipe holds 683 six-byte resizes. The next
//! resize used to block its caller while it held the lock the reader takes around every read, so
//! the output was never read again and the caller, the Host's dispatch thread, never returned.
//! The engine suite runs this on system ConHost and the bundled ConPTY suite on the pinned bundle.
const harness = @import("runtime_harness.zig");
const std = harness.std;
const builtin = harness.target_builtin;
const platform = harness.platform_boundary;
const pty_process = harness.pty_process;
const Process = pty_process.Process;
const windows = std.os.windows;

const allocator = std.testing.allocator;
/// More resizes than the signal pipe holds.
const resize_count = 2000;
/// Paced so that a sender writing behind its caller writes nearly every resize and is still
/// blocked in the full pipe when the console closes.
const resize_pace_ns = 100 * std.time.ns_per_us;
const step_timeout_ns = 10 * std.time.ns_per_s;
const drain_timeout_ns = 30 * std.time.ns_per_s;
/// The output a console has written when it stops: well over one screen, so the program is
/// flooding it, and stable, so the console waits for the reader.
const stalled_output_bytes = 2048;
const stalled_quiet_ns = 300 * std.time.ns_per_ms;

extern "kernel32" fn PeekNamedPipe(
    pipe: windows.HANDLE,
    buffer: ?*anyopaque,
    buffer_size: windows.DWORD,
    bytes_read: ?*windows.DWORD,
    total_bytes_available: ?*windows.DWORD,
    bytes_left_this_message: ?*windows.DWORD,
) callconv(.winapi) windows.BOOL;

extern "kernel32" fn ReadFile(
    file: windows.HANDLE,
    buffer: [*]u8,
    bytes_to_read: windows.DWORD,
    bytes_read: *windows.DWORD,
    overlapped: ?*anyopaque,
) callconv(.winapi) windows.BOOL;

test "Windows ConPTY signals never wait for a console whose output is unread" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const comspec = try pty_process.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(comspec);
    var process = try Process.spawn(allocator, .{
        .arguments = &.{ "/D", "/C", "for /L %i in (1,1,100000000) do @echo stalled-output-%i-0123456789abcdefghijklmnopqrstuvwxyz" },
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = comspec,
        .rows = 24,
    });
    defer process.deinit();
    var reader: Reader = .{ .process = &process };
    const reader_thread = try std.Thread.spawn(.{}, Reader.run, .{&reader});
    var reader_joined = false;
    defer if (!reader_joined) {
        process.forceTerminate() catch {};
        reader.start.store(true, .release);
        process.abortRead() catch {};
        reader_thread.join();
    };
    process.registerReaderThread(reader_thread.getHandle());
    try awaitStalledOutput(&process);

    // The dispatch thread's part: every resize and then a clear return while the console reads
    // nothing.
    var resizer: Resizer = .{ .process = &process };
    const resizer_thread = try std.Thread.spawn(.{}, Resizer.run, .{&resizer});
    if (!awaitFlag(&resizer.finished, step_timeout_ns)) {
        const returned = resizer.completed.load(.acquire);
        // Read past the adapter: OpenConsole then reads its signals and the caller returns, so
        // this fails instead of hanging.
        drainUntil(&process, &resizer.finished);
        resizer_thread.join();
        std.debug.print("signal {d} of {d} waited for the console\n", .{ returned + 1, resize_count + 1 });
        return error.SignalWaitedForConsole;
    }
    resizer_thread.join();
    if (resizer.failure) |err| return err;
    try std.testing.expect(try availableOutput(&process) >= stalled_output_bytes);

    // Close does not wait behind the signal the console has not taken.
    var closer: Closer = .{ .process = &process };
    const closer_thread = try std.Thread.spawn(.{}, Closer.run, .{&closer});
    if (!awaitFlag(&closer.finished, step_timeout_ns)) {
        drainUntil(&process, &closer.finished);
        closer_thread.join();
        return error.CloseWaitedForConsole;
    }
    closer_thread.join();

    // The reader reads the output and then the end of the closed console.
    reader.start.store(true, .release);
    if (!awaitFlag(&reader.finished, drain_timeout_ns)) return error.ReaderDidNotFinish;
    reader_thread.join();
    reader_joined = true;
    try std.testing.expect(try reader.result >= stalled_output_bytes);
    process.forceTerminate() catch {};
}

const Reader = struct {
    process: *Process,
    start: std.atomic.Value(bool) = .init(false),
    finished: std.atomic.Value(bool) = .init(false),
    result: anyerror!usize = error.NotRead,

    fn run(self: *Reader) void {
        defer self.finished.store(true, .release);
        while (!self.start.load(.acquire)) platform.monotonic.sleep(std.time.ns_per_ms);
        var buffer: [16 * 1024]u8 = undefined;
        var total: usize = 0;
        while (true) {
            const count = self.process.read(&buffer) catch |err| {
                self.result = err;
                return;
            };
            if (count == 0) break;
            total += count;
        }
        self.result = total;
    }
};

const Resizer = struct {
    process: *Process,
    completed: std.atomic.Value(usize) = .init(0),
    finished: std.atomic.Value(bool) = .init(false),
    failure: ?anyerror = null,

    fn run(self: *Resizer) void {
        defer self.finished.store(true, .release);
        for (0..resize_count) |index| {
            // Alternating sizes: each request changes the size.
            self.process.resize(80 + @as(u16, @intFromBool(index % 2 == 1)), 24) catch |err| {
                self.failure = err;
                return;
            };
            self.completed.store(index + 1, .release);
            pace();
        }
        _ = self.process.clearConsoleKeepingCursorRow();
        self.completed.store(resize_count + 1, .release);
    }

    fn pace() void {
        var timer = platform.monotonic.Timer.start() catch return;
        while (timer.read() < resize_pace_ns) std.atomic.spinLoopHint();
    }
};

const Closer = struct {
    process: *Process,
    finished: std.atomic.Value(bool) = .init(false),

    fn run(self: *Closer) void {
        defer self.finished.store(true, .release);
        self.process.pty.closeConsole();
    }
};

fn awaitFlag(flag: *const std.atomic.Value(bool), timeout_ns: u64) bool {
    var timer = platform.monotonic.Timer.start() catch return flag.load(.acquire);
    while (!flag.load(.acquire)) {
        if (timer.read() >= timeout_ns) return false;
        platform.monotonic.sleep(std.time.ns_per_ms);
    }
    return true;
}

fn availableOutput(process: *Process) !windows.DWORD {
    var available: windows.DWORD = 0;
    if (!PeekNamedPipe(process.pty.output, null, 0, null, &available, null).toBool())
        return error.OutputPeekFailed;
    return available;
}

/// Waits until the console has filled the output pipe and stopped writing to it.
fn awaitStalledOutput(process: *Process) !void {
    var timer = try platform.monotonic.Timer.start();
    var last: windows.DWORD = 0;
    var quiet = try platform.monotonic.Timer.start();
    while (timer.read() < step_timeout_ns) {
        const available = try availableOutput(process);
        if (available != last) {
            last = available;
            quiet = try platform.monotonic.Timer.start();
        } else if (available >= stalled_output_bytes and quiet.read() >= stalled_quiet_ns) {
            return;
        }
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }
    std.debug.print("console output did not stall: {d} bytes unread\n", .{last});
    return error.OutputDidNotStall;
}

/// Reads the output directly from the pipe until `flag` is set, bounded in time.
fn drainUntil(process: *Process, flag: *const std.atomic.Value(bool)) void {
    var timer = platform.monotonic.Timer.start() catch return;
    var buffer: [16 * 1024]u8 = undefined;
    while (!flag.load(.acquire) and timer.read() < drain_timeout_ns) {
        const available = availableOutput(process) catch return;
        if (available == 0) {
            platform.monotonic.sleep(std.time.ns_per_ms);
            continue;
        }
        var count: windows.DWORD = 0;
        if (!ReadFile(process.pty.output, &buffer, buffer.len, &count, null).toBool()) return;
    }
}
