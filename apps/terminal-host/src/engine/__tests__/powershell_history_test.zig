const harness = @import("runtime_harness.zig");
const screen_text = @import("screen_text.zig");
const std = harness.std;
const builtin = harness.target_builtin;
const platform = harness.platform_boundary;
const pty_process = harness.pty_process;
const TerminalRuntime = harness.TerminalRuntime;

const allocator = std.testing.allocator;
const io = std.testing.io;
const capture_limit_bytes = 512 * 1024;
const output_timeout_ns = 30 * std.time.ns_per_s;

test "managed PowerShell keeps Orchbee-submitted lines out of session and PSReadLine history" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const system_root = try pty_process.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(system_root);
    const program_files = try pty_process.environmentVariableOwned(allocator, "ProgramFiles");
    defer allocator.free(program_files);
    const legacy = try std.fs.path.join(allocator, &.{ system_root, "System32", "WindowsPowerShell", "v1.0", "powershell.exe" });
    defer allocator.free(legacy);
    const modern = try std.fs.path.join(allocator, &.{ program_files, "PowerShell", "7", "pwsh.exe" });
    defer allocator.free(modern);

    // History files live in an owned TEMP directory; the user's history is never opened.
    const temporary = try platform.temp_directory.resolveOwned(allocator);
    defer allocator.free(temporary);
    var random_bytes: [12]u8 = undefined;
    io.random(&random_bytes);
    var name: [16]u8 = undefined;
    _ = std.base64.url_safe.Encoder.encode(&name, &random_bytes);
    const directory = try std.fmt.allocPrint(allocator, "{s}\\orchbee-psreadline-history-{s}", .{ temporary, name });
    defer allocator.free(directory);
    try std.Io.Dir.cwd().createDirPath(io, directory);
    defer std.Io.Dir.cwd().deleteTree(io, directory) catch {};

    var exercised: usize = 0;
    for ([_][]const u8{ legacy, modern }, [_][]const u8{ "legacy", "modern" }) |executable, label| {
        std.Io.Dir.cwd().access(io, executable, .{}) catch |err| switch (err) {
            error.FileNotFound => continue,
            else => return err,
        };
        const history = try std.fmt.allocPrint(allocator, "{s}\\{s}.txt", .{ directory, label });
        defer allocator.free(history);
        try expectSubmittedLinesStayOutOfHistory(executable, history);
        exercised += 1;
    }
    try std.testing.expect(exercised > 0);
}

fn expectSubmittedLinesStayOutOfHistory(executable: []const u8, history: []const u8) !void {
    // The exact canonical startup follows one statement that moves PSReadLine's file.
    const command = try std.fmt.allocPrint(
        allocator,
        "Set-PSReadLineOption -HistorySavePath '{s}'\n{s}",
        .{ history, platform.testing.powershell_startup },
    );
    defer allocator.free(command);
    const runtime = try TerminalRuntime.createWithOptions(allocator, .{
        .arguments = &.{ "-NoLogo", "-NoProfile", "-NoExit", "-Command", command },
        .columns = 200,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 50,
    }, .{});
    defer runtime.destroy();

    try waitForReadyLine(runtime, null);
    _ = try runtime.enqueueRawInput("<#orchbee-run:0123456789abcdef0123456789abcdef#> 'submitted-' + 'probe'\r");
    try waitForReadyLine(runtime, "submitted-probe");
    _ = try runtime.enqueueRawInput("'typed-' + 'probe'\r");
    try waitForReadyLine(runtime, "typed-probe");
    _ = try runtime.enqueueRawInput("Get-History | % { 'sess' + 'ion=' + $_.CommandLine }; " ++
        "[Microsoft.PowerShell.PSConsoleReadLine]::GetHistoryItems() | % { 'read' + 'line=' + $_.CommandLine }; " ++
        "'listed-' + 'probe'\r");
    try waitForReadyLine(runtime, "listed-probe");

    var capture = try screen_text.read(runtime, allocator, capture_limit_bytes);
    defer capture.deinit();
    var session: usize = 0;
    var readline: usize = 0;
    var lines = std.mem.splitScalar(u8, capture.text(), '\n');
    while (lines.next()) |row| {
        const line = std.mem.trimEnd(u8, row, " \r");
        const is_session = std.mem.startsWith(u8, line, "session=");
        const is_readline = std.mem.startsWith(u8, line, "readline=");
        if (!is_session and !is_readline) continue;
        if (std.mem.indexOf(u8, line, "submitted") != null or std.mem.indexOf(u8, line, "orchbee-run") != null) {
            std.debug.print("submitted line kept in history: {s}\n", .{line});
            return error.SubmittedLineInHistory;
        }
        if (is_session) {
            try std.testing.expectEqualStrings("session='typed-' + 'probe'", line);
            session += 1;
        } else {
            readline += 1;
        }
    }
    try std.testing.expectEqual(@as(usize, 1), session);
    try std.testing.expect(readline >= 1);

    const saved = try std.Io.Dir.cwd().readFileAlloc(io, history, allocator, .limited(64 * 1024));
    defer allocator.free(saved);
    try std.testing.expect(std.mem.indexOf(u8, saved, "'typed-' + 'probe'") != null);
    try std.testing.expect(std.mem.indexOf(u8, saved, "submitted") == null);
    try std.testing.expect(std.mem.indexOf(u8, saved, "orchbee-run") == null);

    _ = try runtime.enqueueRawInput("exit\r");
    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
}

/// Waits for the prompt's input marker (OSC 133 B) after an output row equal to `line`.
/// Markers commit with their output, so input state observed after a capture that
/// already holds the row belongs to the prompt that follows that row.
fn waitForReadyLine(runtime: *TerminalRuntime, line: ?[]const u8) !void {
    var timer = try platform.monotonic.Timer.start();
    while (true) {
        const printed = if (line) |expected| printed: {
            var capture = try screen_text.read(runtime, allocator, capture_limit_bytes);
            defer capture.deinit();
            var rows = std.mem.splitScalar(u8, capture.text(), '\n');
            while (rows.next()) |row| {
                if (std.mem.eql(u8, std.mem.trimEnd(u8, row, " \r"), expected)) break :printed true;
            }
            break :printed false;
        } else true;
        if (printed) {
            const state = harness.runtimeState(runtime);
            state.terminal_mutex.lock();
            const ready = state.terminal.screens.active.cursor.semantic_content == .input;
            state.terminal_mutex.unlock();
            if (ready) return;
        }
        if (runtime.primaryFailure() != null) return error.TerminalRuntimeFailed;
        if (timer.read() >= output_timeout_ns) {
            var capture = try screen_text.read(runtime, allocator, capture_limit_bytes);
            defer capture.deinit();
            std.debug.print("PowerShell output timeout: expected {?s}, screen {s}\n", .{ line, capture.text() });
            return error.PowerShellOutputTimedOut;
        }
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }
}
