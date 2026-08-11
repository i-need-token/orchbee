const std = @import("std");
const builtin = @import("builtin");
const platform_boundary = @import("terminal-host-platform");
const subject = platform_boundary.process;
const windows_api = platform_boundary.testing.windows_api;

const EnvironmentDelta = subject.EnvironmentDelta;
const ExitResult = subject.ExitResult;
const PosixProcess = subject.testing.PosixProcessType;
const Process = subject.Process;
const posixExecutableCandidates = subject.testing.posixExecutableCandidatesFn;
const windowsCommandLine = subject.testing.windowsCommandLineFn;

const ProcessReadResult = anyerror!usize;

test "Windows API test facade exposes only process test dependencies" {
    comptime {
        const declarations = @typeInfo(windows_api).@"struct".decls;
        if (declarations.len != 4) {
            @compileError(std.fmt.comptimePrint(
                "terminal-host-platform.testing.windows_api exposes {d} declarations; expected exactly 4",
                .{declarations.len},
            ));
        } else {
            for ([_][]const u8{
                "CreateProcessW",
                "CREATE_SUSPENDED",
                "ob_windows_terminal_job_assign_and_resume",
                "ob_windows_terminal_job_active_processes",
            }) |name| {
                if (!@hasDecl(windows_api, name)) {
                    @compileError(std.fmt.comptimePrint(
                        "terminal-host-platform.testing.windows_api is missing {s}",
                        .{name},
                    ));
                }
            }
            for ([_][]const u8{
                "CreatePipe",
                "STARTUPINFOEXW",
                "ob_windows_terminal_job_terminate",
            }) |name| {
                if (@hasDecl(windows_api, name)) {
                    @compileError(std.fmt.comptimePrint(
                        "terminal-host-platform.testing.windows_api unexpectedly exposes {s}",
                        .{name},
                    ));
                }
            }
        }
    }
}

const ProcessReader = struct {
    process: *Process,
    output: []u8,
    result: ?ProcessReadResult = null,

    fn run(self: *@This()) void {
        self.result = self.process.read(self.output);
    }
};

test "PTY process preserves exact arguments, environment, and exit" {
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf '%s:%s' \"$ORCHBEE_TEST_VALUE\" \"$1\"",
        "orchbee-shell",
        "argument with spaces",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/S",
        "/C",
        "<nul set /p =%ORCHBEE_TEST_VALUE%:argument with spaces",
    };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const environment = [_]EnvironmentDelta{
        .{ .name = "ORCHBEE_TEST_VALUE", .value = "environment" },
    };
    var process = try Process.spawn(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &environment,
        .executable = executable,
        .rows = 24,
    });
    defer process.deinit();

    var output: [256]u8 = undefined;
    var reader: ProcessReader = .{ .process = &process, .output = &output };
    var reader_thread = try std.Thread.spawn(.{}, ProcessReader.run, .{&reader});
    process.registerReaderThread(reader_thread.getHandle());
    reader_thread.join();
    const length = try reader.result.?;
    try std.testing.expect(std.mem.indexOf(u8, output[0..length], "environment:argument with spaces") != null);
    try std.testing.expectEqual(ExitResult{ .exit_code = 0 }, try process.wait());
    if (builtin.os.tag != .windows) {
        try std.testing.expect(!process.exit_reaped);
    }
    try process.terminate();
    try std.testing.expect(try process.processTreeContained());
    try process.releaseProcessIdentity();
    if (builtin.os.tag != .windows) try std.testing.expect(process.exit_reaped);
}

test "failed exec leaves no retained child" {
    try std.testing.expectError(
        error.SpawnFailed,
        Process.spawn(std.testing.allocator, .{
            .arguments = &.{},
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = if (builtin.os.tag == .windows)
                "Z:\\orchbee\\does\\not\\exist.exe"
            else
                "/orchbee/does/not/exist",
            .rows = 24,
        }),
    );
}

test "POSIX effective PATH preserves empty components as the child current directory" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    var environment = std.process.EnvMap.init(std.testing.allocator);
    defer environment.deinit();
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    environment.put("PATH", "") catch unreachable;
    const candidates = try posixExecutableCandidates(arena_state.allocator(), &environment, "probe");

    // An explicitly empty PATH is one empty component, and leading/trailing
    // separators each retain their own current-directory search slot.
    try std.testing.expectEqual(@as(usize, 1), candidates.len);
    try std.testing.expectEqualStrings("./probe", std.mem.sliceTo(candidates[0], 0));

    environment.put("PATH", ":bin::") catch unreachable;
    const repeated = try posixExecutableCandidates(arena_state.allocator(), &environment, "probe");
    try std.testing.expectEqual(@as(usize, 4), repeated.len);
    try std.testing.expectEqualStrings("./probe", std.mem.sliceTo(repeated[0], 0));
    try std.testing.expectEqualStrings("bin/probe", std.mem.sliceTo(repeated[1], 0));
    try std.testing.expectEqualStrings("./probe", std.mem.sliceTo(repeated[2], 0));
    try std.testing.expectEqualStrings("./probe", std.mem.sliceTo(repeated[3], 0));
}

test "POSIX force termination recovers from partial signaling failures" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const RecordingSignaler = struct {
        calls: [3]u8 = undefined,
        count: usize = 0,
        fail_signal: ?u8,

        pub fn signal(self: *@This(), _: std.posix.pid_t, signal_number: u8) !void {
            self.calls[self.count] = signal_number;
            self.count += 1;
            if (self.fail_signal == signal_number) return error.InjectedSignalFailure;
        }
    };

    var stop_failure: RecordingSignaler = .{ .fail_signal = std.posix.SIG.STOP };
    try PosixProcess.forceTerminateSession(1, &stop_failure);
    try std.testing.expectEqualSlices(
        u8,
        &.{ std.posix.SIG.STOP, std.posix.SIG.KILL },
        stop_failure.calls[0..stop_failure.count],
    );

    var kill_failure: RecordingSignaler = .{ .fail_signal = std.posix.SIG.KILL };
    try std.testing.expectError(
        error.InjectedSignalFailure,
        PosixProcess.forceTerminateSession(1, &kill_failure),
    );
    try std.testing.expectEqualSlices(
        u8,
        &.{ std.posix.SIG.STOP, std.posix.SIG.KILL, std.posix.SIG.CONT },
        kill_failure.calls[0..kill_failure.count],
    );
}

const TestSignal = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,

    fn notify(self: *TestSignal) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    fn wait(self: *TestSignal) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }
};

const TestGate = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,
    released: bool = false,

    fn pause(self: *TestGate) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        while (!self.released) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn waitReached(self: *TestGate) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn release(self: *TestGate) void {
        self.mutex.lock();
        self.released = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

// THCR-006 regression: the POSIX process-identity owner must never consult the
// numeric session identifier after the session leader has been reaped. Once
// releaseProcessIdentity has freed the SID, a reused numeric session must
// receive no HUP, STOP, KILL, or CONT from a graceful or force request.
test "POSIX session identity never signals a reused numeric session after release" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    const RecordingSignaler = struct {
        calls: [16]u8 = undefined,
        count: usize = 0,

        pub fn signal(self: *@This(), _: std.posix.pid_t, signal_number: u8) !void {
            if (self.count >= self.calls.len) return error.TooManySignals;
            self.calls[self.count] = signal_number;
            self.count += 1;
        }
    };
    const RecordingReaper = struct {
        reaped: usize = 0,

        pub fn reap(self: *@This(), _: std.posix.pid_t) !void {
            self.reaped += 1;
        }
    };

    var identity: SessionIdentityState = .{};
    var signaler: RecordingSignaler = .{};
    var reaper: RecordingReaper = .{};
    const session_id: std.posix.pid_t = 4242;

    // While the original identity is retained, graceful and force requests
    // signal exactly the owned session.
    try identity.signal(session_id, &signaler, std.posix.SIG.HUP);
    try identity.forceTerminate(session_id, &signaler);
    try std.testing.expectEqual(@as(usize, 3), signaler.count);
    try std.testing.expectEqualSlices(
        u8,
        &.{ std.posix.SIG.HUP, std.posix.SIG.STOP, std.posix.SIG.KILL },
        signaler.calls[0..signaler.count],
    );
    try std.testing.expectEqual(@as(u64, 2), identity.signals_issued.load(.monotonic));

    // Release the original identity: the numeric SID is now free and may be
    // rebound to an unrelated same-user session (PID churn).
    try identity.release(session_id, &reaper);
    try std.testing.expectEqual(@as(usize, 1), reaper.reaped);
    try std.testing.expect(identity.isReleased());

    // Bind the same numeric SID to a foreign session and request termination.
    // Both modes must succeed as idempotent no-ops without a single signal.
    const signals_before = signaler.count;
    try identity.signal(session_id, &signaler, std.posix.SIG.HUP);
    try identity.forceTerminate(session_id, &signaler);
    try std.testing.expectEqual(signals_before, signaler.count);
    try std.testing.expectEqual(
        @as(u64, 2),
        identity.signals_issued.load(.monotonic),
    );

    // Containment confirmation is trivially true after release: the freed
    // session identifier is never enumerated again.
    const CountingQuery = struct {
        calls: *std.atomic.Value(usize),
        pub fn liveMembers(self: *@This(), _: std.posix.pid_t, count: *u32) !void {
            _ = self.calls.fetchAdd(1, .monotonic);
            count.* = 1;
        }
    };
    var query_calls: std.atomic.Value(usize) = .init(0);
    var query: CountingQuery = .{ .calls = &query_calls };
    try std.testing.expect(try identity.contained(session_id, &query));
    try std.testing.expectEqual(@as(usize, 0), query_calls.load(.monotonic));
}

// THCR-006 regression: the identity fence serializes check-and-signal against
// release. When termination wins the fence first it signals only the retained
// launch session, and release cannot pass the fence until signaling completes.
test "POSIX session identity release waits for in-flight session signaling" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    const BlockingSignaler = struct {
        entered: *TestSignal,
        gate: *TestGate,

        pub fn signal(self: *@This(), _: std.posix.pid_t, _: u8) !void {
            self.entered.notify();
            // Hold the identity fence until the test releases the gate.
            self.gate.pause();
        }
    };
    const Reaper = struct {
        reaped: *std.atomic.Value(usize),
        pub fn reap(self: @This(), _: std.posix.pid_t) !void {
            _ = self.reaped.fetchAdd(1, .monotonic);
        }
    };
    const ReleaseObserver = struct {
        attempted: TestSignal = .{},
        completed: TestSignal = .{},
        reaped: std.atomic.Value(usize) = .init(0),

        pub fn run(
            self: *@This(),
            identity: *SessionIdentityState,
            session_id: std.posix.pid_t,
        ) void {
            self.attempted.notify();
            identity.release(session_id, Reaper{ .reaped = &self.reaped }) catch
                std.debug.panic("unexpected release failure", .{});
            self.completed.notify();
        }
    };

    var identity: SessionIdentityState = .{};
    var entered = TestSignal{};
    var gate = TestGate{};
    var signaler: BlockingSignaler = .{ .entered = &entered, .gate = &gate };
    var observer: ReleaseObserver = .{};
    const session_id: std.posix.pid_t = 4242;

    // Signal holds the identity fence (paused inside the signaler).
    var signal_thread = try std.Thread.spawn(.{}, signalThreadRun, .{
        &identity,
        session_id,
        &signaler,
        std.posix.SIG.HUP,
    });
    defer signal_thread.join();
    entered.wait();

    // A concurrent release starts but cannot pass the fence until the in-flight
    // signal completes.
    var release_thread = try std.Thread.spawn(.{}, ReleaseObserver.run, .{
        &observer,
        &identity,
        session_id,
    });
    defer release_thread.join();
    observer.attempted.wait();
    try std.testing.expect(!identity.isReleased());
    try std.testing.expectEqual(@as(usize, 0), observer.reaped.load(.monotonic));

    // Releasing the gate completes the signal; release then passes the fence.
    gate.release();
    observer.completed.wait();
    try std.testing.expect(identity.isReleased());
    try std.testing.expectEqual(@as(usize, 1), observer.reaped.load(.monotonic));
}

fn signalThreadRun(
    identity: *subject.testing.SessionIdentityStateType,
    session_id: std.posix.pid_t,
    signaler: anytype,
    signal_number: u8,
) void {
    identity.signal(session_id, signaler, signal_number) catch
        std.debug.panic("unexpected signal failure", .{});
}

test "Windows descendants start inside the terminal kill-on-close Job" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    const arguments = [_][]const u8{
        "/D",
        "/S",
        "/C",
        "start \"\" /B ping -t 127.0.0.1 >nul & ping -t 127.0.0.1 >nul",
    };
    var process = try Process.spawn(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer process.deinit();

    var observed_descendant = false;
    for (0..400) |_| {
        var active_processes: std.os.windows.DWORD = 0;
        if (windows_api.ob_windows_terminal_job_active_processes(
            process.job_handle,
            &active_processes,
        ) == 0) {
            return std.os.windows.unexpectedError(std.os.windows.kernel32.GetLastError());
        }
        if (active_processes >= 2) {
            observed_descendant = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(observed_descendant);
    try process.forceTerminate();
    _ = try process.wait();
    var contained = false;
    for (0..400) |_| {
        if (try process.processTreeContained()) {
            contained = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(contained);
}

test "Windows Job assignment failure terminates the suspended child" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const windows = std.os.windows;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const arguments = [_][]const u8{
        executable,
        "/D",
        "/S",
        "/C",
        "ping -n 31 127.0.0.1 >nul",
    };
    const command_line = try windowsCommandLine(arena, &arguments);
    const command_line_w = try std.unicode.utf8ToUtf16LeAllocZ(arena, command_line);
    var startup_info: windows.STARTUPINFOW = std.mem.zeroes(windows.STARTUPINFOW);
    startup_info.cb = @sizeOf(windows.STARTUPINFOW);
    var process_information: windows.PROCESS_INFORMATION = undefined;
    if (windows_api.CreateProcessW(
        null,
        command_line_w.ptr,
        null,
        null,
        windows.FALSE,
        windows_api.CREATE_SUSPENDED,
        null,
        null,
        &startup_info,
        &process_information,
    ) == 0) return windows.unexpectedError(windows.kernel32.GetLastError());
    defer _ = windows.CloseHandle(process_information.hProcess);
    defer _ = windows.CloseHandle(process_information.hThread);
    defer _ = windows.kernel32.TerminateProcess(process_information.hProcess, 1);

    try std.testing.expectEqual(
        windows.FALSE,
        windows_api.ob_windows_terminal_job_assign_and_resume(
            windows.INVALID_HANDLE_VALUE,
            process_information.hProcess,
            process_information.hThread,
        ),
    );
    try std.testing.expectEqual(
        @as(windows.DWORD, windows.WAIT_OBJECT_0),
        windows.kernel32.WaitForSingleObject(process_information.hProcess, 5_000),
    );
}

fn windowsCandidateStrings(allocator: std.mem.Allocator, candidates: []const [:0]u16) ![][]const u8 {
    const strings = try allocator.alloc([]const u8, candidates.len);
    for (candidates, 0..) |candidate, index| {
        strings[index] = try std.unicode.utf16LeToUtf8Alloc(allocator, std.mem.sliceTo(candidate, 0));
    }
    return strings;
}

fn hasString(strings: []const []const u8, needle: []const u8) bool {
    for (strings) |string| {
        if (std.mem.eql(u8, string, needle)) return true;
    }
    return false;
}

fn indexOfString(strings: []const []const u8, needle: []const u8) ?usize {
    for (strings, 0..) |string, index| {
        if (std.mem.eql(u8, string, needle)) return index;
    }
    return null;
}

test "Windows absent PATH does not reintroduce the excluded current directory" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const candidates = try subject.testing.windowsSearchCandidatesFn(
        arena,
        "probe",
        "C:\\app",
        "C:\\work",
        "C:\\Windows\\System32",
        false,
        null,
    );
    const strings = try windowsCandidateStrings(arena, candidates);
    try std.testing.expectEqual(@as(usize, 4), candidates.len);
    try std.testing.expect(!hasString(strings, "C:\\work\\probe.exe"));
    try std.testing.expect(!hasString(strings, "C:\\work\\probe"));
    try std.testing.expect(hasString(strings, "C:\\app\\probe.exe"));
    try std.testing.expect(hasString(strings, "C:\\Windows\\System32\\probe.exe"));
}

test "Windows explicit relative PATH entries resolve against the parent current directory" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const candidates = try subject.testing.windowsSearchCandidatesFn(
        arena,
        "probe",
        "C:\\app",
        "C:\\work",
        "C:\\Windows\\System32",
        false,
        "bin;other",
    );
    const strings = try windowsCandidateStrings(arena, candidates);
    try std.testing.expect(hasString(strings, "C:\\work\\bin\\probe.exe"));
    try std.testing.expect(hasString(strings, "C:\\work\\other\\probe.exe"));
    try std.testing.expect(!hasString(strings, "C:\\work\\probe.exe"));
}

test "Windows implicit current-directory slot follows the policy flag only" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const with_cwd = try subject.testing.windowsSearchCandidatesFn(
        arena,
        "probe",
        "C:\\app",
        "C:\\work",
        "C:\\Windows\\System32",
        true,
        null,
    );
    const without_cwd = try subject.testing.windowsSearchCandidatesFn(
        arena,
        "probe",
        "C:\\app",
        "C:\\work",
        "C:\\Windows\\System32",
        false,
        null,
    );
    const with_strings = try windowsCandidateStrings(arena, with_cwd);
    const without_strings = try windowsCandidateStrings(arena, without_cwd);
    try std.testing.expect(hasString(with_strings, "C:\\work\\probe.exe"));
    try std.testing.expect(!hasString(without_strings, "C:\\work\\probe.exe"));
    try std.testing.expectEqual(@as(usize, 5), with_cwd.len);
    try std.testing.expectEqual(@as(usize, 4), without_cwd.len);
}

test "Windows drive-relative PATH components keep drive-relative semantics and search order" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const candidates = try subject.testing.windowsSearchCandidatesFn(
        arena,
        "probe",
        "D:\\app",
        "D:\\work",
        "C:\\Windows\\System32",
        false,
        "C:tools;bin;C:\\Program Files\\tools;\\rooted;D:\\fully\\qualified;",
    );
    const strings = try windowsCandidateStrings(arena, candidates);

    // Fixed application and system slots retain their existing precedence.
    try std.testing.expect(hasString(strings, "D:\\app\\probe.exe"));
    try std.testing.expect(hasString(strings, "C:\\Windows\\System32\\probe.exe"));
    try std.testing.expect(hasString(strings, "C:\\Windows\\probe.exe"));

    // A drive-relative component keeps its drive-relative syntax and is never
    // prefixed with the parent current directory.
    try std.testing.expect(hasString(strings, "C:tools\\probe.exe"));
    try std.testing.expect(!hasString(strings, "D:\\work\\C:tools\\probe.exe"));

    // Ordinary-relative, rooted, and fully qualified entries resolve exactly
    // as before, and PATH entries retain their supplied search order.
    const expected_path_order = [_][]const u8{
        "C:tools\\probe.exe",
        "D:\\work\\bin\\probe.exe",
        "C:\\Program Files\\tools\\probe.exe",
        "\\rooted\\probe.exe",
        "D:\\fully\\qualified\\probe.exe",
        "D:\\work\\probe.exe",
    };
    var previous_index: ?usize = null;
    for (expected_path_order) |expected_string| {
        const found_index = indexOfString(strings, expected_string) orelse
            return error.TestExpectedResultMissing;
        if (previous_index) |prior| try std.testing.expect(prior < found_index);
        previous_index = found_index;
    }
}

test "Windows drive-only PATH component joins without a backslash" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const candidates = try subject.testing.windowsSearchCandidatesFn(
        arena,
        "probe",
        "D:\\app",
        "D:\\work",
        "C:\\Windows\\System32",
        false,
        "C:;D:\\work",
    );
    const strings = try windowsCandidateStrings(arena, candidates);
    try std.testing.expect(hasString(strings, "C:probe.exe"));
    try std.testing.expect(!hasString(strings, "C:\\probe.exe"));
    try std.testing.expect(hasString(strings, "D:\\work\\probe.exe"));
}

// THCR-028 regression. Linux writes the task name unescaped inside
// /proc/<pid>/stat. A name embedding '\n' truncates a single fgets line, and a
// name embedding the bytes ") Z\n" can then masquerade a sleeping process as a
// non-live zombie. The parser reads the complete record and classifies by the
// kernel-added final ") <state> " delimiter, failing closed on truncation.
const native_procstat = struct {
    extern "c" fn orchbee_process_stat_record_is_live(
        record: [*]const u8,
        length: usize,
        live: *c_int,
    ) c_int;

    fn isLive(record: []const u8) !bool {
        var live: c_int = 0;
        if (orchbee_process_stat_record_is_live(record.ptr, record.len, &live) != 0) {
            return error.InvalidProcessStatRecord;
        }
        return live != 0;
    }
};

test "Linux /proc stat record parser uses the final kernel-added state (THCR-028)" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    // An ordinary newline inside the task name: the real final state is live 'S'.
    try std.testing.expect(try native_procstat.isLive("123 (name\n) S 456 ...\n"));
    // A task name forging ") Z" before a newline: still live 'S' from the
    // kernel-added delimiter, never the forged byte.
    try std.testing.expect(try native_procstat.isLive("123 (worker) Z\n) S 456 ...\n"));
    // A task name ending in ')' keeps the kernel delimiter as the last ')'.
    try std.testing.expect(try native_procstat.isLive("123 (abc)) S 456 ...\n"));
    // A genuinely terminal zombie is still non-live, including one with a
    // newline-bearing name.
    try std.testing.expect(!try native_procstat.isLive("123 (sleep) Z 456 ...\n"));
    try std.testing.expect(!try native_procstat.isLive("123 (worker\n) Z 456 ...\n"));
    // The truncated first line that fgets would have produced fails closed
    // instead of reporting the forged 'Z' as the process state.
    try std.testing.expectError(
        error.InvalidProcessStatRecord,
        native_procstat.isLive("123 (worker) Z\n"),
    );
    try std.testing.expectError(
        error.InvalidProcessStatRecord,
        native_procstat.isLive("123 (x) \n"),
    );
    try std.testing.expectError(error.InvalidProcessStatRecord, native_procstat.isLive(""));
}

const NativeSessionQuery = struct {
    pub fn liveMembers(_: @This(), session_id: std.posix.pid_t, count: *u32) !void {
        if (orchbee_process_session_live_members(session_id, count) != 0) {
            return error.ProcessContainmentQueryFailed;
        }
    }
};

const NativeSessionReaper = struct {
    pub fn reap(_: @This(), session_id: std.posix.pid_t) !void {
        if (orchbee_reap_process_exit(session_id) != 0) return error.ProcessReapFailed;
    }
};

// THCR-028 regression: a session leader whose main-thread comm embeds an
// ordinary newline must still complete containment observation and be reaped.
// On the base parser the zombie's stat record was truncated at the newline, so
// every live-members query failed with EIO and identity release gated on
// containment could never progress.
test "Linux session leader with a newline comm completes containment and reaps (THCR-028)" {
    if (builtin.os.tag != .linux) return error.SkipZigTest;

    const PR_SET_NAME: c_int = 15;
    const SessionIdentityState = subject.testing.SessionIdentityStateType;

    const child_id = try std.posix.fork();
    if (child_id == 0) {
        // Child: become the session leader, set a newline-bearing main-thread
        // comm, and exit; the zombie retains that comm.
        _ = std.posix.setsid() catch std.posix.exit(1);
        _ = std.os.linux.prctl(PR_SET_NAME, @intFromPtr("worker\n"), 0, 0, 0);
        std.posix.exit(0);
    }
    var identity: SessionIdentityState = .{};
    defer identity.release(child_id, NativeSessionReaper{}) catch {};

    // Observe the exit without reaping so the leader stays a zombie with the
    // newline comm.
    var result_kind: c_int = 0;
    var result_value: u32 = 0;
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_observe_process_exit(child_id, &result_kind, &result_value),
    );

    // Containment must observe no live members (the zombie is non-live) and the
    // leader must then be reaped, completing instead of failing the query.
    try std.testing.expect(try identity.contained(child_id, NativeSessionQuery{}));
    try std.testing.expect(!identity.isReleased());
    try identity.release(child_id, NativeSessionReaper{});
    try std.testing.expect(identity.isReleased());
}

extern "c" fn orchbee_process_session_live_members(session_id: c_int, count: *u32) c_int;
extern "c" fn orchbee_observe_process_exit(
    process_id: c_int,
    result_kind: *c_int,
    result_value: *u32,
) c_int;
extern "c" fn orchbee_reap_process_exit(process_id: c_int) c_int;
