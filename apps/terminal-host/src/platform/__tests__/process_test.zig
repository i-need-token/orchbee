const std = @import("std");
const builtin = @import("builtin");
const platform_boundary = @import("terminal-host-platform");
const subject = platform_boundary.process;
const windows_api = platform_boundary.testing.windows_api;

const EnvironmentDelta = subject.EnvironmentDelta;
const ExitResult = subject.ExitResult;
const PosixProcess = subject.testing.PosixProcessType;
const Process = subject.Process;
const foregroundCommandActive = subject.testing.foregroundCommandActiveFn;
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

test "POSIX foreground process group identifies an active command" {
    try std.testing.expect(!foregroundCommandActive(41, 41));
    try std.testing.expect(foregroundCommandActive(41, 42));
}

// THCR-041 regression: after WaitForSingleObject returns WAIT_OBJECT_0 the
// child process object has signaled and the process is definitively terminated.
// Every DWORD returned by a successful GetExitCodeProcess is then a genuine
// exit status. STILL_ACTIVE (259) describes a running process only; a
// terminated process may legitimately exit with status 259, so the value must
// be preserved as `exit_code` instead of being rejected as observation failure.
// This pure decision runs on every platform.
test "terminated Windows exit codes are preserved, never observation failure (THCR-041)" {
    const classify = subject.testing.windowsTerminatedExitResultFn;
    for ([_]u32{ 0, 1, 258, 259, 260, 0xffff_ffff }) |code| {
        try std.testing.expectEqual(
            ExitResult{ .exit_code = code },
            try classify(code),
        );
    }
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

// Stable-member termination runs as separate STOP and KILL scans. Exercise the
// native macOS/Linux adapter against a leader that ignores graceful HUP so the
// shutdown path must stop and then kill the same retained process identity.
// The bounded containment poll prevents a signaling regression from turning
// the test runner's cleanup into an unbounded waitpid.
test "POSIX native force termination kills a stopped retained session leader" {
    if (builtin.os.tag == .windows or builtin.os.tag == .freebsd) {
        return error.SkipZigTest;
    }

    var process = try Process.spawn(std.testing.allocator, .{
        .arguments = &.{ "-c", "trap '' HUP; exec sleep 30" },
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    });
    defer {
        if (!process.exit_reaped) {
            std.posix.kill(process.child_id, std.posix.SIG.KILL) catch {};
            _ = process.wait() catch {};
            process.releaseProcessIdentity() catch {};
        }
        process.deinit();
    }

    try process.terminate();
    try process.forceTerminate();
    var contained = false;
    for (0..200) |_| {
        if (try process.processTreeContained()) {
            contained = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(contained);
    _ = try process.wait();
    try process.releaseProcessIdentity();
}

// macOS waitid can surface CLD_STOPPED despite WEXITED. The observer must not
// publish that transient stop as completion: the later SIGKILL is the retained
// child's actual non-reaping exit observation.
test "macOS process observation ignores stop and waits for actual exit" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;

    var process = try Process.spawn(std.testing.allocator, .{
        .arguments = &.{ "-c", "exec sleep 30" },
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    });
    defer {
        if (!process.exit_reaped) {
            std.posix.kill(process.child_id, std.posix.SIG.KILL) catch {};
            _ = process.wait() catch {};
            process.releaseProcessIdentity() catch {};
        }
        process.deinit();
    }

    const StopThenKill = struct {
        child_id: std.posix.pid_t,

        fn run(self: @This()) void {
            std.Thread.sleep(10 * std.time.ns_per_ms);
            std.posix.kill(self.child_id, std.posix.SIG.STOP) catch
                @panic("failed to stop retained test child");
            std.Thread.sleep(50 * std.time.ns_per_ms);
            std.posix.kill(self.child_id, std.posix.SIG.KILL) catch
                @panic("failed to kill retained test child");
        }
    };
    const terminator = try std.Thread.spawn(
        .{},
        StopThenKill.run,
        .{StopThenKill{ .child_id = process.child_id }},
    );

    var result_kind: c_int = 0;
    var result_value: u32 = 0;
    const observe_result = orchbee_observe_process_exit(
        process.child_id,
        &result_kind,
        &result_value,
    );
    terminator.join();
    try std.testing.expectEqual(@as(c_int, 0), observe_result);
    try std.testing.expectEqual(@as(c_int, 2), result_kind);
    try std.testing.expectEqual(@as(u32, std.posix.SIG.KILL), result_value);

    try std.testing.expectEqual(
        ExitResult{ .signal = std.posix.SIG.KILL },
        try process.wait(),
    );
    try process.releaseProcessIdentity();
}

// The final wait owns the last numeric-identity transition, so it must never
// block waiting for a child that containment did not actually prove exited.
// A WNOHANG miss returns EAGAIN through the reaper, leaves the identity fence
// retained, and permits a later observed exit to be reaped normally.
test "macOS final process reap is non-blocking and retains a running identity" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    const CapturingNativeReaper = struct {
        native_result: *c_int,

        pub fn reap(self: @This(), session_id: std.posix.pid_t) !void {
            self.native_result.* = orchbee_reap_process_exit(session_id);
            if (self.native_result.* != 0) return error.ProcessReapFailed;
        }
    };
    const ReleaseCall = struct {
        completed: *TestSignal,
        identity: *SessionIdentityState,
        native_result: c_int = 0,
        release_error: ?anyerror = null,
        session_id: std.posix.pid_t,

        fn run(self: *@This()) void {
            self.identity.release(
                self.session_id,
                CapturingNativeReaper{ .native_result = &self.native_result },
            ) catch |err| {
                self.release_error = err;
            };
            self.completed.notify();
        }
    };

    const child_id = try std.posix.fork();
    if (child_id == 0) {
        // Stay a live direct child indefinitely without relying on allocation
        // or a post-fork library lock. The parent later terminates us with KILL.
        std.posix.raise(std.posix.SIG.STOP) catch
            std.posix.exit(2);
        std.posix.exit(3);
    }
    var child_reaped = false;
    defer if (!child_reaped) {
        std.posix.kill(child_id, std.posix.SIG.KILL) catch {};
        _ = std.posix.waitpid(child_id, 0);
    };

    var identity: SessionIdentityState = .{};
    var completed: TestSignal = .{};
    var immediate_kind: c_int = 0;
    var immediate_value: u32 = 0;
    try std.testing.expectEqual(
        @as(c_int, @intCast(@intFromEnum(std.posix.E.AGAIN))),
        orchbee_try_observe_process_exit_now(
            child_id,
            &immediate_kind,
            &immediate_value,
        ),
    );
    var release_call: ReleaseCall = .{
        .completed = &completed,
        .identity = &identity,
        .session_id = child_id,
    };
    const release_thread = try std.Thread.spawn(.{}, ReleaseCall.run, .{&release_call});
    const returned_promptly = completed.waitFor(2 * std.time.ns_per_s);
    if (!returned_promptly) {
        // Bound the regression even against the former blocking waitpid. That
        // implementation reaps after KILL and therefore releases the fence.
        std.posix.kill(child_id, std.posix.SIG.KILL) catch {};
    }
    release_thread.join();
    if (!returned_promptly) child_reaped = identity.isReleased();
    try std.testing.expect(returned_promptly);

    try std.testing.expectEqual(
        @as(c_int, @intCast(@intFromEnum(std.posix.E.AGAIN))),
        release_call.native_result,
    );
    try std.testing.expectEqual(error.ProcessReapFailed, release_call.release_error.?);
    try std.testing.expect(!identity.isReleased());

    try std.posix.kill(child_id, std.posix.SIG.KILL);
    var result_kind: c_int = 0;
    var result_value: u32 = 0;
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_observe_process_exit(child_id, &result_kind, &result_value),
    );
    try std.testing.expectEqual(@as(c_int, 2), result_kind);
    try std.testing.expectEqual(@as(u32, std.posix.SIG.KILL), result_value);
    try identity.release(child_id, NativeSessionReaper{});
    child_reaped = true;
    try std.testing.expect(identity.isReleased());
}

// A stable pidfd does not turn an uncertain getsid failure into a non-member
// result. Only ESRCH means the captured candidate vanished; EPERM and every
// other error must abort the scan conservatively.
test "POSIX membership observation propagates EPERM instead of skipping" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_classify_session_membership_error(
            @intCast(@intFromEnum(std.posix.E.SRCH)),
        ),
    );
    try std.testing.expectEqual(
        @as(c_int, -1),
        orchbee_classify_session_membership_error(
            @intCast(@intFromEnum(std.posix.E.PERM)),
        ),
    );
}

test "POSIX PID-version external identity preserves signed extension" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    try std.testing.expectEqual(
        std.math.maxInt(u64),
        orchbee_pidversion_external_id(-1),
    );
    try std.testing.expectEqual(
        @as(u64, 0xffff_ffff_8000_0000),
        orchbee_pidversion_external_id(std.math.minInt(c_int)),
    );
}

// The Linux pidfd adapter must distinguish an exact unreaped zombie from an
// invalidated reference whose numeric PID may already name a same-session
// replacement. These pure seams are used by the production poll/probe path and
// run on every POSIX test host, including macOS CI.
test "Linux pidfd exact-state classification is fail-conservative" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const esrch: c_int = @intCast(@intFromEnum(std.posix.E.SRCH));
    const eperm: c_int = @intCast(@intFromEnum(std.posix.E.PERM));
    var live: c_int = -1;

    // No readiness means the exact pidfd target is still live.
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_linux_classify_pidfd_poll(0, 0, 0, 0, 0, 0, 0, 0, &live),
    );
    try std.testing.expectEqual(@as(c_int, 1), live);

    // A non-leader readable pidfd needs a same-reference signal-0 probe.
    try std.testing.expectEqual(
        @as(c_int, 1),
        orchbee_linux_classify_pidfd_poll(0, 1, 0, 1, 0, 0, 0, 0, &live),
    );
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_linux_classify_pidfd_probe(0, 0, &live),
    );
    try std.testing.expectEqual(@as(c_int, 0), live);

    // Only the retained leader may bypass signal-0: the Host owns its wait, so
    // readable necessarily denotes the exact unreaped leader zombie even if a
    // credential transition would make signal-0 return EPERM.
    live = -1;
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_linux_classify_pidfd_poll(1, 1, 0, 1, 0, 0, 0, 0, &live),
    );
    try std.testing.expectEqual(@as(c_int, 0), live);

    var result = orchbee_linux_classify_pidfd_probe(-1, esrch, &live);
    var observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.AGAIN, observation_error);

    result = orchbee_linux_classify_pidfd_probe(-1, eperm, &live);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.PERM, observation_error);

    result = orchbee_linux_classify_pidfd_poll(0, 1, 0, 0, 1, 0, 0, 0, &live);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.AGAIN, observation_error);

    result = orchbee_linux_classify_pidfd_poll(0, 1, 0, 0, 0, 1, 0, 0, &live);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.IO, observation_error);

    result = orchbee_linux_classify_pidfd_poll(0, 1, 0, 0, 0, 0, 1, 0, &live);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.BADF, observation_error);

    result = orchbee_linux_classify_pidfd_poll(0, 1, 0, 0, 0, 0, 0, 1, &live);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.IO, observation_error);

    result = orchbee_linux_classify_pidfd_poll(0, -1, eperm, 0, 0, 0, 0, 0, &live);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.PERM, observation_error);

    result = orchbee_linux_classify_pidfd_signal(-1, esrch);
    observation_error = std.posix.errno(result);
    try std.testing.expectEqual(@as(c_int, -1), result);
    try std.testing.expectEqual(std.posix.E.AGAIN, observation_error);
    try std.testing.expectEqual(@as(c_int, 0), orchbee_linux_classify_pidfd_signal(0, 0));
}

// Session membership survives real/effective credential transitions. The
// macOS candidate source must therefore be PROC_ALL_PIDS (1), not either UID
// filter; exact getsid prefiltering and stable-reference acquisition narrow the
// all-process list afterward.
test "macOS member enumeration is independent of process credentials" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;

    try std.testing.expectEqual(
        @as(c_int, 1),
        orchbee_darwin_process_enumeration_scope(),
    );
}

// A macOS exec keeps the numeric PID and session while replacing the task and
// incrementing its PID version. Capture the pre-exec stable reference, wait for
// CLOEXEC to prove exec committed, and require the old reference to fail rather
// than report non-live. A fresh production scan must still see the live child.
test "macOS exec-invalidated member reference forces a fresh containment scan" {
    if (builtin.os.tag != .macos) return error.SkipZigTest;

    const exec_observation = try std.posix.pipe2(.{ .CLOEXEC = true });
    const exec_gate = try std.posix.pipe2(.{ .CLOEXEC = true });
    const child_id = try std.posix.fork();
    if (child_id == 0) {
        std.posix.close(exec_observation[0]);
        std.posix.close(exec_gate[1]);
        _ = std.posix.setsid() catch std.posix.exit(2);
        _ = std.posix.write(exec_observation[1], &.{1}) catch std.posix.exit(3);
        var gate: [1]u8 = undefined;
        if ((std.posix.read(exec_gate[0], &gate) catch 0) != 1) std.posix.exit(4);
        const arguments = [_:null]?[*:0]const u8{ "/bin/sleep", "30" };
        const environment = [_:null]?[*:0]const u8{};
        std.posix.exit(@truncate(@intFromError(
            std.posix.execveZ("/bin/sleep", &arguments, &environment),
        )));
    }

    std.posix.close(exec_observation[1]);
    std.posix.close(exec_gate[0]);
    var observation_read_open = true;
    var gate_write_open = true;
    var child_reaped = false;
    defer {
        if (observation_read_open) std.posix.close(exec_observation[0]);
        if (gate_write_open) std.posix.close(exec_gate[1]);
        if (!child_reaped) {
            std.posix.kill(child_id, std.posix.SIG.KILL) catch {};
            _ = std.posix.waitpid(child_id, 0);
        }
    }

    var marker: [1]u8 = undefined;
    try std.testing.expectEqual(
        @as(usize, 1),
        try std.posix.read(exec_observation[0], &marker),
    );

    var raw_iterator: ?*anyopaque = null;
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_session_member_iterator_open(child_id, &raw_iterator),
    );
    defer orchbee_session_member_iterator_close(raw_iterator);
    var raw_member_ref: ?*anyopaque = null;
    try std.testing.expectEqual(
        @as(c_int, 1),
        orchbee_session_member_iterator_next(raw_iterator.?, &raw_member_ref),
    );
    defer if (raw_member_ref) |member_ref| orchbee_session_member_ref_close(member_ref);

    try std.testing.expectEqual(
        @as(usize, 1),
        try std.posix.write(exec_gate[1], &.{1}),
    );
    std.posix.close(exec_gate[1]);
    gate_write_open = false;
    try std.testing.expectEqual(
        @as(usize, 0),
        try std.posix.read(exec_observation[0], &marker),
    );
    std.posix.close(exec_observation[0]);
    observation_read_open = false;

    var stale_live: c_int = 0;
    try std.testing.expect(
        orchbee_session_member_ref_is_live(raw_member_ref.?, &stale_live) != 0,
    );
    try std.testing.expectEqual(
        @as(u32, 1),
        try PosixProcess.sessionLiveMembersForTesting(child_id),
    );

    try std.posix.kill(child_id, std.posix.SIG.KILL);
    var result_kind: c_int = 0;
    var result_value: u32 = 0;
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_observe_process_exit(child_id, &result_kind, &result_value),
    );
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_reap_process_exit(child_id),
    );
    child_reaped = true;
}

const ScriptedMemberOwner = enum {
    launch,
    replacement,
};

// Deterministic adapter for the production stable-member orchestration. `next`
// acquires the launch identity before its authoritative numeric session
// membership observation. Tests can recycle either before that observation or
// at the last possible point before signal/liveness.
const ScriptedMemberState = struct {
    numeric_owner: ScriptedMemberOwner = .launch,
    recycle_before_effect: bool = true,
    recycle_before_membership: bool = false,
    replacement_stays_in_owned_session: bool = false,
    recycle_count: usize = 0,
    closed_references: usize = 0,
    signaled: [2]usize = .{ 0, 0 },
    counted_live: [2]usize = .{ 0, 0 },

    fn armLaunchMember(self: *@This()) void {
        self.numeric_owner = .launch;
        self.recycle_before_effect = true;
    }

    fn recycleImmediatelyBeforeEffect(self: *@This()) void {
        if (!self.recycle_before_effect) return;
        self.recycle_before_effect = false;
        self.numeric_owner = .replacement;
        self.recycle_count += 1;
    }
};

const ScriptedSessionMemberSource = struct {
    state: *ScriptedMemberState,

    pub fn open(self: @This(), _: std.posix.pid_t) !ScriptedSessionMemberIterator {
        return .{ .state = self.state };
    }
};

const ScriptedSessionMemberIterator = struct {
    state: *ScriptedMemberState,
    yielded: bool = false,

    pub fn deinit(_: *@This()) void {}

    pub fn next(self: *@This()) !?ScriptedSessionMemberRef {
        if (self.yielded) return null;
        self.yielded = true;

        // Stable identity acquisition precedes this authoritative membership
        // check, matching Linux and the post-prefilter macOS adapter ordering.
        const captured_owner = self.state.numeric_owner;
        if (self.state.recycle_before_membership) {
            self.state.recycle_before_membership = false;
            self.state.recycleImmediatelyBeforeEffect();
        }
        if (self.state.numeric_owner != .launch and
            !self.state.replacement_stays_in_owned_session) return null;
        return .{ .state = self.state, .captured_owner = captured_owner };
    }
};

const ScriptedSessionMemberRef = struct {
    state: *ScriptedMemberState,
    captured_owner: ScriptedMemberOwner,

    pub fn deinit(self: *@This()) void {
        self.state.closed_references += 1;
    }

    pub fn signal(self: *@This(), _: u8) !void {
        self.state.recycleImmediatelyBeforeEffect();
        // A stable kernel reference continues to denote the vanished launch
        // process, never the replacement now occupying its numeric PID.
        if (self.captured_owner != self.state.numeric_owner) {
            return error.StableMemberIdentityInvalidated;
        }
        self.state.signaled[@intFromEnum(self.captured_owner)] += 1;
    }

    pub fn isLive(self: *@This()) !bool {
        self.state.recycleImmediatelyBeforeEffect();
        if (self.captured_owner != self.state.numeric_owner) {
            return error.StableMemberIdentityInvalidated;
        }
        self.state.counted_live[@intFromEnum(self.captured_owner)] += 1;
        return true;
    }
};

const StableAlgorithmSignaler = struct {
    state: *ScriptedMemberState,
    requested: [3]u8 = undefined,
    requested_count: usize = 0,

    pub fn signal(self: *@This(), session_id: std.posix.pid_t, signal_number: u8) !void {
        self.requested[self.requested_count] = signal_number;
        self.requested_count += 1;
        self.state.armLaunchMember();
        try PosixProcess.signalStableSessionMembersForTesting(
            session_id,
            ScriptedSessionMemberSource{ .state = self.state },
            signal_number,
        );
    }
};

const MultiMemberSignalState = struct {
    stop_attempts: [3]usize = .{ 0, 0, 0 },
    kill_attempts: [3]usize = .{ 0, 0, 0 },
    continue_attempts: [3]usize = .{ 0, 0, 0 },
    closed_references: usize = 0,

    fn attemptsFor(self: *@This(), signal_number: u8) *[3]usize {
        return switch (signal_number) {
            std.posix.SIG.STOP => &self.stop_attempts,
            std.posix.SIG.KILL => &self.kill_attempts,
            std.posix.SIG.CONT => &self.continue_attempts,
            else => unreachable,
        };
    }
};

const MultiMemberSignalSource = struct {
    state: *MultiMemberSignalState,

    pub fn open(self: @This(), _: std.posix.pid_t) !MultiMemberSignalIterator {
        return .{ .state = self.state };
    }
};

const MultiMemberSignalIterator = struct {
    state: *MultiMemberSignalState,
    cursor: usize = 0,

    pub fn deinit(_: *@This()) void {}

    pub fn next(self: *@This()) !?MultiMemberSignalRef {
        if (self.cursor == 3) return null;
        defer self.cursor += 1;
        return .{ .state = self.state, .index = self.cursor };
    }
};

const MultiMemberSignalRef = struct {
    state: *MultiMemberSignalState,
    index: usize,

    pub fn deinit(self: *@This()) void {
        self.state.closed_references += 1;
    }

    pub fn signal(self: *@This(), signal_number: u8) !void {
        self.state.attemptsFor(signal_number)[self.index] += 1;
        if (signal_number == std.posix.SIG.KILL and self.index == 1) {
            return error.InjectedKillSignalFailure;
        }
        if (signal_number == std.posix.SIG.CONT and self.index == 0) {
            return error.InjectedContinueSignalFailure;
        }
    }
};

const MultiMemberStableSignaler = struct {
    state: *MultiMemberSignalState,

    pub fn signal(
        self: *@This(),
        session_id: std.posix.pid_t,
        signal_number: u8,
    ) !void {
        try PosixProcess.signalStableSessionMembersForTesting(
            session_id,
            MultiMemberSignalSource{ .state = self.state },
            signal_number,
        );
    }
};

// A middle-member KILL failure triggers CONT compensation, and a first-member
// CONT failure must not strand either later stopped member. Every captured
// stable reference is attempted and released before the first error is returned.
test "POSIX stable member signaling completes KILL and CONT scans after member errors" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    var state: MultiMemberSignalState = .{};
    var signaler: MultiMemberStableSignaler = .{ .state = &state };
    try std.testing.expectError(
        error.InjectedKillSignalFailure,
        PosixProcess.forceTerminateSession(4242, &signaler),
    );
    try std.testing.expectEqualSlices(usize, &.{ 1, 1, 1 }, &state.stop_attempts);
    try std.testing.expectEqualSlices(usize, &.{ 1, 1, 1 }, &state.kill_attempts);
    try std.testing.expectEqualSlices(usize, &.{ 1, 1, 1 }, &state.continue_attempts);
    try std.testing.expectEqual(@as(usize, 9), state.closed_references);
}

test "POSIX stable member signaling rejects recycle before graceful and force effects" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    const session_id: std.posix.pid_t = 4242;

    var graceful_state: ScriptedMemberState = .{};
    var graceful_signaler: StableAlgorithmSignaler = .{ .state = &graceful_state };
    var graceful_identity: SessionIdentityState = .{};
    try std.testing.expectError(
        error.StableMemberIdentityInvalidated,
        graceful_identity.signal(session_id, &graceful_signaler, std.posix.SIG.HUP),
    );
    try std.testing.expectEqualSlices(
        u8,
        &.{std.posix.SIG.HUP},
        graceful_signaler.requested[0..graceful_signaler.requested_count],
    );
    try std.testing.expectEqual(@as(usize, 1), graceful_state.recycle_count);
    try std.testing.expectEqual(@as(usize, 1), graceful_state.closed_references);
    try std.testing.expectEqualSlices(usize, &.{ 0, 0 }, &graceful_state.signaled);

    // Each invalidated stable scan fails only after binding the launch identity,
    // so the real force orchestration still executes its CONT compensation.
    // No scan may retarget the replacement process.
    var force_state: ScriptedMemberState = .{};
    var force_signaler: StableAlgorithmSignaler = .{ .state = &force_state };
    var force_identity: SessionIdentityState = .{};
    try std.testing.expectError(
        error.StableMemberIdentityInvalidated,
        force_identity.forceTerminate(session_id, &force_signaler),
    );
    try std.testing.expectEqualSlices(
        u8,
        &.{ std.posix.SIG.STOP, std.posix.SIG.KILL, std.posix.SIG.CONT },
        force_signaler.requested[0..force_signaler.requested_count],
    );
    try std.testing.expectEqual(@as(usize, 3), force_state.recycle_count);
    try std.testing.expectEqual(@as(usize, 3), force_state.closed_references);
    try std.testing.expectEqualSlices(usize, &.{ 0, 0 }, &force_state.signaled);
}

// pidfd_send_signal returning ESRCH after an authoritative membership match
// means the captured process was reaped; the numeric PID may now name an owned
// replacement. The stable scan must fail for retry without targeting that
// replacement or reporting a successful signal.
test "Linux invalidated stable member reference cannot report signal success" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    var state: ScriptedMemberState = .{
        .recycle_before_membership = true,
        .replacement_stays_in_owned_session = true,
    };
    var signaler: StableAlgorithmSignaler = .{ .state = &state };
    var identity: SessionIdentityState = .{};
    try std.testing.expectError(
        error.StableMemberIdentityInvalidated,
        identity.signal(4242, &signaler, std.posix.SIG.HUP),
    );
    try std.testing.expectEqual(@as(usize, 1), state.recycle_count);
    try std.testing.expectEqual(@as(usize, 1), state.closed_references);
    try std.testing.expectEqualSlices(usize, &.{ 0, 0 }, &state.signaled);
}

test "POSIX invalidated stable member identity cannot prove empty for a foreign replacement" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    const StableAlgorithmQuery = struct {
        state: *ScriptedMemberState,

        pub fn liveMembers(self: *@This(), session_id: std.posix.pid_t, count: *u32) !void {
            count.* = try PosixProcess.countStableSessionMembersForTesting(
                session_id,
                ScriptedSessionMemberSource{ .state = self.state },
            );
        }
    };

    var state: ScriptedMemberState = .{};
    var query: StableAlgorithmQuery = .{ .state = &state };
    var identity: SessionIdentityState = .{};
    try std.testing.expectError(
        error.StableMemberIdentityInvalidated,
        identity.contained(4242, &query),
    );
    try std.testing.expect(!identity.isReleased());
    try std.testing.expectEqual(@as(usize, 1), state.recycle_count);
    try std.testing.expectEqual(@as(usize, 1), state.closed_references);
    try std.testing.expectEqualSlices(usize, &.{ 0, 0 }, &state.counted_live);
}

// On Linux and macOS, exec or PID reuse can invalidate an old stable reference
// while the current numeric binding remains in the owned session. That old
// reference becoming non-live is a retryable scan failure, not an empty
// containment result that could omit the replacement.
test "POSIX invalidated stable member reference cannot prove containment" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const SessionIdentityState = subject.testing.SessionIdentityStateType;
    const StableAlgorithmQuery = struct {
        state: *ScriptedMemberState,

        pub fn liveMembers(self: *@This(), session_id: std.posix.pid_t, count: *u32) !void {
            count.* = try PosixProcess.countStableSessionMembersForTesting(
                session_id,
                ScriptedSessionMemberSource{ .state = self.state },
            );
        }
    };

    var state: ScriptedMemberState = .{
        .recycle_before_membership = true,
        .replacement_stays_in_owned_session = true,
    };
    var query: StableAlgorithmQuery = .{ .state = &state };
    var identity: SessionIdentityState = .{};
    try std.testing.expectError(
        error.StableMemberIdentityInvalidated,
        identity.contained(4242, &query),
    );
    try std.testing.expect(!identity.isReleased());
    try std.testing.expectEqual(@as(usize, 1), state.recycle_count);
    try std.testing.expectEqual(@as(usize, 1), state.closed_references);
    try std.testing.expectEqualSlices(usize, &.{ 0, 0 }, &state.counted_live);
}

const ScriptedFreebsdReaperPass = struct {
    live_members: u32,
    targeted_zombies_remain: bool,
};

const ScriptedFreebsdReaperQuery = struct {
    passes: []const ScriptedFreebsdReaperPass,
    cursor: usize = 0,

    pub fn observe(
        self: *@This(),
        _: std.posix.pid_t,
    ) !ScriptedFreebsdReaperPass {
        if (self.cursor == self.passes.len) return error.UnexpectedReaperPass;
        defer self.cursor += 1;
        return self.passes[self.cursor];
    }
};

// Deterministically models the native pass in which a member is live in the
// first snapshot, exits after that pass's targeted reap, and is a direct-child
// zombie in the fresh snapshot. The production convergence must run another
// pass to reap it; treating that fresh zombie as zero would release the subtree
// identity while a targeted Host child remained.
test "FreeBSD containment retries a fresh direct-child zombie before zero" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const passes = [_]ScriptedFreebsdReaperPass{
        .{ .live_members = 0, .targeted_zombies_remain = true },
        .{ .live_members = 0, .targeted_zombies_remain = false },
    };
    var query: ScriptedFreebsdReaperQuery = .{ .passes = &passes };
    try std.testing.expectEqual(
        @as(u32, 0),
        try PosixProcess.countFreebsdReaperMembersForTesting(4242, &query),
    );
    try std.testing.expectEqual(@as(usize, 2), query.cursor);
}

test "FreeBSD zombie retry predicate excludes the retained leader and non-child" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const subtree: c_int = 4242;
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_freebsd_member_is_targeted_zombie(subtree, subtree, 1, 1),
    );
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_freebsd_member_is_targeted_zombie(4243, subtree, 0, 1),
    );
    try std.testing.expectEqual(
        @as(c_int, 0),
        orchbee_freebsd_member_is_targeted_zombie(4243, subtree, 1, 0),
    );
    try std.testing.expectEqual(
        @as(c_int, 1),
        orchbee_freebsd_member_is_targeted_zombie(4243, subtree, 1, 1),
    );

    const leader_only = [_]ScriptedFreebsdReaperPass{
        .{ .live_members = 0, .targeted_zombies_remain = false },
    };
    var query: ScriptedFreebsdReaperQuery = .{ .passes = &leader_only };
    try std.testing.expectEqual(
        @as(u32, 0),
        try PosixProcess.countFreebsdReaperMembersForTesting(subtree, &query),
    );
    try std.testing.expectEqual(@as(usize, 1), query.cursor);
}

test "FreeBSD containment fails conservatively on persistent zombie churn" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const zombie = ScriptedFreebsdReaperPass{
        .live_members = 0,
        .targeted_zombies_remain = true,
    };
    const passes = [_]ScriptedFreebsdReaperPass{
        zombie,
        zombie,
        zombie,
        zombie,
        zombie,
        zombie,
        zombie,
        zombie,
    };
    var query: ScriptedFreebsdReaperQuery = .{ .passes = &passes };
    try std.testing.expectError(
        error.ProcessContainmentQueryFailed,
        PosixProcess.countFreebsdReaperMembersForTesting(4242, &query),
    );
    try std.testing.expectEqual(passes.len, query.cursor);
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

    fn waitFor(self: *TestSignal, timeout_ns: u64) bool {
        var timer = std.time.Timer.start() catch return false;
        self.mutex.lock();
        defer self.mutex.unlock();
        while (!self.reached) {
            const elapsed = timer.read();
            if (elapsed >= timeout_ns) return false;
            self.condition.timedWait(&self.mutex, timeout_ns - elapsed) catch
                return self.reached;
        }
        return true;
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

// THCR-041 regression: a Windows child that terminates with the DWORD exit
// status 259 (the numeric value of STILL_ACTIVE) must be observed as that exact
// exit_code, never as ProcessObservationFailed. Native Windows execution
// required; compiled for `x86_64-windows-gnu` on non-Windows hosts.
test "Windows wait preserves exact exit codes 258/259/260 (THCR-041)" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    for ([_]u32{ 258, 259, 260 }) |expected_code| {
        var command_buffer: [32]u8 = undefined;
        const exit_command = try std.fmt.bufPrint(
            &command_buffer,
            "exit {d}",
            .{expected_code},
        );
        const arguments = [_][]const u8{ "/D", "/S", "/C", exit_command };
        var process = try Process.spawn(std.testing.allocator, .{
            .arguments = &arguments,
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = executable,
            .rows = 24,
        });
        defer process.deinit();
        try std.testing.expectEqual(
            ExitResult{ .exit_code = expected_code },
            try process.wait(),
        );
    }
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

const NativeSessionQuery = struct {
    pub fn liveMembers(_: @This(), session_id: std.posix.pid_t, count: *u32) !void {
        count.* = try PosixProcess.sessionLiveMembersForTesting(session_id);
    }
};

const NativeSessionReaper = struct {
    pub fn reap(_: @This(), session_id: std.posix.pid_t) !void {
        if (orchbee_reap_process_exit(session_id) != 0) return error.ProcessReapFailed;
    }
};

// THCR-028 regression: a session leader whose main-thread comm embeds an
// ordinary newline must still complete containment observation and be reaped.
// Stable-reference liveness does not parse the newline-bearing /proc stat comm,
// so containment and identity release can still progress.
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

extern "c" fn orchbee_observe_process_exit(
    process_id: c_int,
    result_kind: *c_int,
    result_value: *u32,
) c_int;
extern "c" fn orchbee_try_observe_process_exit_now(
    process_id: c_int,
    result_kind: *c_int,
    result_value: *u32,
) c_int;
extern "c" fn orchbee_reap_process_exit(process_id: c_int) c_int;
extern "c" fn orchbee_classify_session_membership_error(observation_error: c_int) c_int;
extern "c" fn orchbee_pidversion_external_id(pid_version: c_int) u64;
extern "c" fn orchbee_darwin_process_enumeration_scope() c_int;
extern "c" fn orchbee_session_member_iterator_open(
    session_id: c_int,
    iterator: *?*anyopaque,
) c_int;
extern "c" fn orchbee_session_member_iterator_close(iterator: ?*anyopaque) void;
extern "c" fn orchbee_session_member_iterator_next(
    iterator: *anyopaque,
    member_ref: *?*anyopaque,
) c_int;
extern "c" fn orchbee_session_member_ref_close(member_ref: ?*anyopaque) void;
extern "c" fn orchbee_session_member_ref_is_live(
    member_ref: *anyopaque,
    live: *c_int,
) c_int;
extern "c" fn orchbee_freebsd_member_is_targeted_zombie(
    process_id: c_int,
    subtree: c_int,
    is_direct_child: c_int,
    is_zombie: c_int,
) c_int;
extern "c" fn orchbee_linux_classify_pidfd_poll(
    is_retained_leader: c_int,
    poll_result: c_int,
    poll_error: c_int,
    readable: c_int,
    hangup: c_int,
    error_event: c_int,
    invalid: c_int,
    unknown: c_int,
    live: *c_int,
) c_int;
extern "c" fn orchbee_linux_classify_pidfd_probe(
    probe_result: c_int,
    probe_error: c_int,
    live: *c_int,
) c_int;
extern "c" fn orchbee_linux_classify_pidfd_signal(
    signal_result: c_int,
    signal_error: c_int,
) c_int;
