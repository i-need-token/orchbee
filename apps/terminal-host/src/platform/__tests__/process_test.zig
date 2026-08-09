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
