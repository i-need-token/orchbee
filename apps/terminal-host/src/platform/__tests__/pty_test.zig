const std = @import("std");
const builtin = @import("builtin");
const platform_boundary = @import("terminal-host-platform");
const monotonic = platform_boundary.monotonic;
const posix_compat = platform_boundary.posix_compat;
const subject = platform_boundary.testing.pty;

const Pty = subject.Pty;
const Read = platform_boundary.process.Read;
const ReadMode = platform_boundary.process.ReadMode;
const thread_sync = platform_boundary.thread_sync;

extern "kernel32" fn CreatePipe(
    read_pipe: *std.os.windows.HANDLE,
    write_pipe: *std.os.windows.HANDLE,
    attributes: ?*const std.os.windows.SECURITY_ATTRIBUTES,
    size: std.os.windows.DWORD,
) callconv(.winapi) std.os.windows.BOOL;

test "PTY opens with bounded dimensions and resizes" {
    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    try instance.setSize(.{ .rows = 40, .columns = 120 });
}

test "Windows PTY rejects resize after pseudoconsole close" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    instance.closeConsole();
    try std.testing.expectError(
        error.PtyClosed,
        instance.setSize(.{ .rows = 40, .columns = 120 }),
    );
}

test "Windows PTY maps a pipe without readers to BrokenPipe" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const windows = std.os.windows;
    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    var read_pipe: windows.HANDLE = undefined;
    var write_pipe: windows.HANDLE = undefined;
    if (!CreatePipe(&read_pipe, &write_pipe, null, 0).toBool())
        return error.TestPipeCreationFailed;
    defer windows.CloseHandle(write_pipe);
    windows.CloseHandle(read_pipe);

    // Closing a pseudoconsole does not prove its input pipe has no readers.
    // Exercise the real write adapter with an owned pipe whose reader is gone.
    const original = instance.input;
    instance.input = write_pipe;
    defer instance.input = original;
    try std.testing.expectError(error.BrokenPipe, instance.write("closed-pipe"));
}

test "POSIX child setup preserves standard streams when openpty reuses low descriptors" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const posix = std.posix;
    const report = try posix_compat.pipe2(.{ .CLOEXEC = true });
    const child_id = try posix_compat.fork();
    if (child_id == 0) {
        _ = posix.system.close(report[0]);
        _ = posix.system.close(posix.STDIN_FILENO);
        _ = posix.system.close(posix.STDOUT_FILENO);
        _ = posix.system.close(posix.STDERR_FILENO);
        const instance = Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 }) catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        // A real parent retains the master while its child installs the slave.
        // Keep that reference above stdio until process exit so Linux does not
        // hang up this fixture before its result has been reported.
        _ = posix_compat.fcntl(
            instance.master,
            posix.F.DUPFD_CLOEXEC,
            posix.STDERR_FILENO + 1,
        ) catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        instance.childSetup() catch {
            _ = posix_compat.write(report[1], &.{0}) catch 0;
            posix_compat.exit(1);
        };
        for ([_]posix.fd_t{
            posix.STDIN_FILENO,
            posix.STDOUT_FILENO,
            posix.STDERR_FILENO,
        }) |descriptor| {
            _ = posix_compat.fcntl(descriptor, posix.F.GETFD, 0) catch {
                _ = posix_compat.write(report[1], &.{0}) catch 0;
                posix_compat.exit(1);
            };
        }
        _ = posix_compat.write(report[1], &.{1}) catch 0;
        posix_compat.exit(0);
    }
    _ = posix.system.close(report[1]);
    var marker: [1]u8 = undefined;
    const marker_length = try posix.read(report[0], &marker);
    _ = posix.system.close(report[0]);
    const result = posix_compat.waitpid(child_id, 0);
    try std.testing.expectEqual(@as(usize, 1), marker_length);
    try std.testing.expectEqual(@as(u8, 1), marker[0]);
    try std.testing.expect(posix.W.IFEXITED(result.status));
    try std.testing.expectEqual(@as(u8, 0), posix.W.EXITSTATUS(result.status));
}

test "PTY abort wakes a reader blocked without child output" {
    const Reader = struct {
        pty: *Pty,
        result: ?usize = null,
        failure: ?anyerror = null,

        fn run(self: *@This()) void {
            var buffer: [32]u8 = undefined;
            const result = self.pty.read(&buffer) catch |err| {
                self.failure = err;
                return;
            };
            self.result = result;
        }
    };

    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    var reader: Reader = .{ .pty = &instance };
    var thread = try std.Thread.spawn(.{}, Reader.run, .{&reader});
    instance.registerReaderThread(thread.getHandle());
    monotonic.sleep(10 * std.time.ns_per_ms);
    try instance.abortRead();
    thread.join();
    try std.testing.expect(reader.failure == null);
    try std.testing.expectEqual(@as(usize, 0), reader.result.?);
}

/// One registered reader thread that reads only when the test asks, so a pause meets a read in
/// a known place.
const StepReader = struct {
    pty: *Pty,
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    requested: usize = 0,
    finished: usize = 0,
    mode: ReadMode = .stop_at_pause,
    result: ?Read = null,
    bytes: [32]u8 = undefined,
    failure: ?anyerror = null,
    stopping: bool = false,
    thread: std.Thread = undefined,

    fn start(self: *StepReader) !void {
        self.thread = try std.Thread.spawn(.{}, run, .{self});
        self.pty.registerReaderThread(self.thread.getHandle());
    }

    /// Joins the thread once its last read has returned.
    fn stop(self: *StepReader) void {
        self.mutex.lock();
        self.stopping = true;
        self.condition.broadcast();
        self.mutex.unlock();
        self.thread.join();
    }

    fn run(self: *StepReader) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (true) {
            while (self.requested == self.finished and !self.stopping) {
                self.condition.wait(&self.mutex);
            }
            if (self.requested == self.finished) return;
            const mode = self.mode;
            self.mutex.unlock();
            var buffer: [32]u8 = undefined;
            const result = self.pty.readPausable(&buffer, mode);
            self.mutex.lock();
            if (result) |value| {
                self.result = value;
                switch (value) {
                    .bytes => |count| @memcpy(self.bytes[0..count], buffer[0..count]),
                    .end, .paused => {},
                }
            } else |err| self.failure = err;
            self.finished += 1;
            self.condition.broadcast();
        }
    }

    fn read(self: *StepReader, mode: ReadMode) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.mode = mode;
        self.requested += 1;
        self.condition.broadcast();
    }

    fn pending(self: *StepReader) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.finished != self.requested;
    }

    fn wait(self: *StepReader) !Read {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.finished != self.requested) self.condition.wait(&self.mutex);
        if (self.failure) |err| return err;
        return self.result.?;
    }

    /// The next read stays blocked: a pause stands for none of it.
    fn expectBlocked(self: *StepReader) !void {
        // A Windows pause cancels only a read already started, and a read started after it in
        // `through_pause` mode reads through it, with no output ever coming. A sleep alone lets a
        // slowly scheduled reader start late, so the test waits for the start the PTY counts.
        if (builtin.os.tag == .windows) try self.awaitStartedRead();
        monotonic.sleep(20 * std.time.ns_per_ms);
        try std.testing.expect(self.pending());
    }

    fn awaitStartedRead(self: *StepReader) !void {
        const timer = try monotonic.Timer.start();
        while (true) {
            self.pty.read_mutex.lock();
            const started = self.pty.read_active;
            self.pty.read_mutex.unlock();
            if (started) return;
            if (timer.read() >= 10 * std.time.ns_per_s) return error.ReadDidNotStart;
            monotonic.sleep(std.time.ns_per_ms);
        }
    }
};

/// Writes child output on POSIX, where the slave is held here; Windows has no child.
fn writeChildOutput(instance: *Pty, bytes: []const u8) !bool {
    if (builtin.os.tag == .windows) return false;
    _ = try posix_compat.write(instance.slave.?, bytes);
    return true;
}

test "PTY pause stops a blocked read, stands for reads that stop at it, and resumes" {
    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    var reader: StepReader = .{ .pty = &instance };
    try reader.start();
    defer reader.stop();

    // A pause cancels a blocked read even when it reads through pauses.
    reader.read(.through_pause);
    try reader.expectBlocked();
    try instance.pauseRead();
    try std.testing.expectEqual(Read.paused, try reader.wait());
    // A standing pause stops the next read before it reads; a second request changes nothing.
    try instance.pauseRead();
    reader.read(.stop_at_pause);
    try std.testing.expectEqual(Read.paused, try reader.wait());
    // A read through the standing pause reads, and a pause requested after a resume cancels it.
    reader.read(.through_pause);
    try reader.expectBlocked();
    if (try writeChildOutput(&instance, "t")) {
        try std.testing.expectEqual(Read{ .bytes = 1 }, try reader.wait());
        try std.testing.expectEqual(@as(u8, 't'), reader.bytes[0]);
        reader.read(.through_pause);
        try reader.expectBlocked();
    }
    try instance.resumeRead();
    try instance.pauseRead();
    try std.testing.expectEqual(Read.paused, try reader.wait());
    // Resume: a read that stops at pauses reads again.
    try instance.resumeRead();
    reader.read(.stop_at_pause);
    try reader.expectBlocked();
    if (try writeChildOutput(&instance, "r")) {
        try std.testing.expectEqual(Read{ .bytes = 1 }, try reader.wait());
        try std.testing.expectEqual(@as(u8, 'r'), reader.bytes[0]);
    } else {
        try instance.abortRead();
        try std.testing.expectEqual(Read.end, try reader.wait());
    }
}

test "PTY pause withdrawn before a read takes it leaves the next read reading" {
    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    var reader: StepReader = .{ .pty = &instance };
    try reader.start();
    defer reader.stop();

    // On POSIX the resume drains the pause's marker, so the read neither reports a pause nor
    // wakes for nothing.
    try instance.pauseRead();
    try instance.resumeRead();
    reader.read(.stop_at_pause);
    try reader.expectBlocked();
    if (try writeChildOutput(&instance, "w")) {
        try std.testing.expectEqual(Read{ .bytes = 1 }, try reader.wait());
        try std.testing.expectEqual(@as(u8, 'w'), reader.bytes[0]);
        reader.read(.stop_at_pause);
        try reader.expectBlocked();
    }
    try instance.abortRead();
    try std.testing.expectEqual(Read.end, try reader.wait());
}

test "PTY abort ends reads through a pause and a resume does not undo it" {
    var instance = try Pty.open(std.testing.allocator, .{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    var reader: StepReader = .{ .pty = &instance };
    try reader.start();
    defer reader.stop();

    try instance.pauseRead();
    try instance.abortRead();
    reader.read(.stop_at_pause);
    try std.testing.expectEqual(Read.end, try reader.wait());
    try instance.resumeRead();
    try instance.pauseRead();
    for ([_]ReadMode{ .stop_at_pause, .through_pause }) |mode| {
        reader.read(mode);
        try std.testing.expectEqual(Read.end, try reader.wait());
    }
}
