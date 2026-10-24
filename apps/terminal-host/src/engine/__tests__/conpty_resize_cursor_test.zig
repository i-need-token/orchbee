const harness = @import("runtime_harness.zig");
const std = harness.std;
const builtin = harness.target_builtin;
const platform = harness.platform_boundary;
const pty_process = harness.pty_process;
const TerminalRuntime = harness.TerminalRuntime;
const options = @import("conpty_resize_test_options");

const capture_limit_bytes = 512 * 1024;
const marker_timeout_ns = 20 * std.time.ns_per_s;

test "bundled Windows ConPTY keeps Windows PowerShell input on its prompt line" {
    if (builtin.os.tag != .windows or builtin.cpu.arch != .x86_64) return error.SkipZigTest;
    const directory = try pty_process.environmentVariableOwned(std.testing.allocator, "SystemRoot");
    defer std.testing.allocator.free(directory);
    const executable = try std.fs.path.join(std.testing.allocator, &.{ directory, "System32", "WindowsPowerShell", "v1.0", "powershell.exe" });
    defer std.testing.allocator.free(executable);
    try expectPowerShellPromptCursor(executable);
}

test "bundled Windows ConPTY keeps PowerShell 7 input on its prompt line" {
    if (builtin.os.tag != .windows or builtin.cpu.arch != .x86_64) return error.SkipZigTest;
    const directory = try pty_process.environmentVariableOwned(std.testing.allocator, "ProgramFiles");
    defer std.testing.allocator.free(directory);
    const executable = try std.fs.path.join(std.testing.allocator, &.{ directory, "PowerShell", "7", "pwsh.exe" });
    defer std.testing.allocator.free(executable);
    const file = std.Io.Dir.openFileAbsolute(std.testing.io, executable, .{}) catch |err| switch (err) {
        error.FileNotFound => return error.SkipZigTest,
        else => return err,
    };
    file.close(std.testing.io);
    try expectPowerShellPromptCursor(executable);
}

fn expectPowerShellPromptCursor(executable: []const u8) !void {
    try std.testing.expect(try platform.testing.conpty.initializedFromBundle());
    const drive = try pty_process.environmentVariableOwned(std.testing.allocator, "SystemDrive");
    defer std.testing.allocator.free(drive);
    const directory = try std.fmt.allocPrint(std.testing.allocator, "{s}\\", .{drive});
    defer std.testing.allocator.free(directory);
    const prompt = try std.fmt.allocPrint(std.testing.allocator, "PS {s}> ", .{directory});
    defer std.testing.allocator.free(prompt);
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &.{ "-NoLogo", "-NoProfile" },
        .columns = 80,
        .current_directory = directory,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    }, .{});
    defer runtime.destroy();

    try waitForPowerShellInput(runtime, prompt);
    try expectPowerShellCursor(runtime, prompt.len);
    // No wrapping or history is involved: changing geometry must preserve the input anchor.
    _ = try runtime.resizeGeometry(89, 37, 0, 0);
    try expectPowerShellCursor(runtime, prompt.len);
    try waitForPowerShellInput(runtime, prompt);

    const input = "cursor_probe";
    const draft = try std.fmt.allocPrint(std.testing.allocator, "{s}{s}", .{ prompt, input });
    defer std.testing.allocator.free(draft);
    _ = try runtime.enqueueRawInput(input);
    try waitForPowerShellInput(runtime, draft);
    try expectPowerShellCursor(runtime, draft.len);
}

fn waitForPowerShellInput(runtime: *TerminalRuntime, expected: []const u8) !void {
    var timer = try platform.monotonic.Timer.start();
    while (true) {
        const state = harness.runtimeState(runtime);
        state.terminal_mutex.lock();
        const ready = state.close_prompt_phase == .input_ready;
        state.terminal_mutex.unlock();
        if (ready) {
            var capture = try runtime.captureScrollbackText(std.testing.allocator, capture_limit_bytes);
            defer capture.deinit();
            if (std.mem.eql(u8, std.mem.trim(u8, capture.text(), " \r\n"), std.mem.trim(u8, expected, " "))) return;
        }
        if (runtime.primaryFailure() != null) return error.TerminalRuntimeFailed;
        if (timer.read() >= marker_timeout_ns) {
            var capture = try runtime.captureScrollbackText(std.testing.allocator, capture_limit_bytes);
            defer capture.deinit();
            state.terminal_mutex.lock();
            defer state.terminal_mutex.unlock();
            std.debug.print("PowerShell input timeout: expected {s}, phase {s}, cursor {d},{d}, screen {s}\n", .{
                expected,
                @tagName(state.close_prompt_phase),
                state.terminal.screens.active.cursor.x,
                state.terminal.screens.active.cursor.y,
                capture.text(),
            });
            return error.PowerShellInputTimedOut;
        }
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }
}

fn expectPowerShellCursor(runtime: *TerminalRuntime, column: usize) !void {
    const state = harness.runtimeState(runtime);
    state.terminal_mutex.lock();
    defer state.terminal_mutex.unlock();
    try std.testing.expectEqual(@as(u16, 0), state.terminal.screens.active.cursor.y);
    try std.testing.expectEqual(column, state.terminal.screens.active.cursor.x);
    try std.testing.expect(state.terminal.cursorIsAtPrompt());
}

fn captureWhenPhaseCommitted(
    runtime: *TerminalRuntime,
    marker: []const u8,
    bell_sequence: u64,
) !harness.subject.ScrollbackTextCapture {
    var timer = try platform.monotonic.Timer.start();
    while (true) {
        if (runtime.bellSequence() >= bell_sequence) {
            var capture = try runtime.captureScrollbackText(
                std.testing.allocator,
                capture_limit_bytes,
            );
            if (std.mem.indexOf(u8, capture.text(), marker) != null) return capture;
            capture.deinit();
        }
        if (runtime.primaryFailure() != null) return error.TerminalRuntimeFailed;
        if (timer.read() >= marker_timeout_ns) return error.TerminalMarkerTimedOut;
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }
}

fn expectNumberedLines(
    text: []const u8,
    prefix: []const u8,
    count: usize,
) !void {
    var ordered_offset: usize = 0;
    for (0..count) |index| {
        var expected_buffer: [96]u8 = undefined;
        const expected = try std.fmt.bufPrint(
            &expected_buffer,
            "{s}-{d:0>3}|abcdefghijklmnopqrstuvwxyz0123456789",
            .{ prefix, index },
        );
        const occurrences = std.mem.count(u8, text, expected);
        if (occurrences != 1) {
            std.debug.print("expected one {s}, observed {d}\n", .{ expected, occurrences });
            return error.UnexpectedTerminalLineCount;
        }
        const relative = std.mem.indexOf(u8, text[ordered_offset..], expected) orelse
            return error.ExpectedOrderedTerminalLine;
        ordered_offset += relative + expected.len;
    }
}

fn expectOrderedMarkers(text: []const u8, markers: []const []const u8) !void {
    var ordered_offset: usize = 0;
    for (markers) |marker| {
        const relative = std.mem.indexOf(u8, text[ordered_offset..], marker) orelse {
            std.debug.print("ordered terminal marker missing after offset {d}: {s}\n", .{
                ordered_offset,
                marker,
            });
            return error.ExpectedOrderedTerminalMarker;
        };
        ordered_offset += relative + marker.len;
    }
}

fn expectContains(text: []const u8, expected: []const u8) !void {
    if (std.mem.indexOf(u8, text, expected) != null) return;
    std.debug.print("terminal text missing: {s}\n", .{expected});
    var lines = std.mem.splitScalar(u8, text, '\n');
    while (lines.next()) |line| {
        if (std.mem.indexOf(u8, line, "CURSOR-") != null) {
            std.debug.print("observed cursor line: {s}\n", .{line});
        }
    }
    return error.ExpectedTerminalText;
}

test "bundled Windows ConPTY synchronizes cursor after history resize" {
    if (builtin.os.tag != .windows or builtin.cpu.arch != .x86_64) {
        return error.SkipZigTest;
    }
    try std.testing.expect(options.fixture.len > 0);
    try std.testing.expect(try platform.testing.conpty.initializedFromBundle());

    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &.{},
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = options.fixture,
        .rows = 24,
    }, .{ .scrollback_bytes = 2 * 1024 * 1024 });
    defer runtime.destroy();

    var seeded = try captureWhenPhaseCommitted(runtime, "SEED-READY|", 1);
    defer seeded.deinit();
    try std.testing.expectEqual(@as(u16, 80), seeded.geometry.columns);
    try std.testing.expectEqual(@as(u16, 24), seeded.geometry.rows);
    try std.testing.expect(seeded.total_rows >= 400);
    try expectNumberedLines(seeded.text(), "SEED", 400);

    _ = try runtime.resizeGeometry(89, 37, 0, 0);
    var first_growth = try captureWhenPhaseCommitted(runtime, "PHASE-A-DONE|", 2);
    defer first_growth.deinit();
    try std.testing.expectEqual(@as(u16, 89), first_growth.geometry.columns);
    try std.testing.expectEqual(@as(u16, 37), first_growth.geometry.rows);
    try expectContains(first_growth.text(), "CURSOR-GROW-1:0,36:89x37");
    try expectNumberedLines(first_growth.text(), "SEED", 400);
    try expectNumberedLines(first_growth.text(), "GROW-A", 31);

    _ = try runtime.resizeGeometry(80, 24, 0, 0);
    var shrunk = try captureWhenPhaseCommitted(runtime, "SHRINK-DONE|", 3);
    defer shrunk.deinit();
    try std.testing.expectEqual(@as(u16, 80), shrunk.geometry.columns);
    try std.testing.expectEqual(@as(u16, 24), shrunk.geometry.rows);
    try expectContains(shrunk.text(), "CURSOR-SHRINK:0,23:80x24");
    try expectNumberedLines(shrunk.text(), "SEED", 400);
    try expectNumberedLines(shrunk.text(), "GROW-A", 31);

    _ = try runtime.resizeGeometry(89, 37, 0, 0);
    var final = try captureWhenPhaseCommitted(runtime, "PHASE-B-DONE|", 4);
    defer final.deinit();
    try std.testing.expectEqual(@as(u16, 89), final.geometry.columns);
    try std.testing.expectEqual(@as(u16, 37), final.geometry.rows);
    try expectContains(final.text(), "CURSOR-GROW-2:0,36:89x37");
    try expectNumberedLines(final.text(), "SEED", 400);
    try expectNumberedLines(final.text(), "GROW-A", 31);
    try expectNumberedLines(final.text(), "GROW-B", 31);
    try expectOrderedMarkers(final.text(), &.{
        "SEED-000|",
        "SEED-399|",
        "CURSOR-GROW-1:0,36:89x37",
        "GROW-A-000|",
        "GROW-A-030|",
        "CURSOR-SHRINK:0,23:80x24",
        "CURSOR-GROW-2:0,36:89x37",
        "GROW-B-000|",
        "GROW-B-030|",
        "PHASE-B-DONE|",
    });
    try std.testing.expect(std.mem.indexOf(u8, final.text(), "\x1b[6n") == null);
    try std.testing.expect(std.mem.indexOf(u8, final.text(), "[37;1R") == null);
    try std.testing.expect(std.mem.indexOf(u8, final.text(), "[24;1R") == null);
    var signal = (try runtime.prepareApplicationSignal(std.testing.allocator)).?;
    defer signal.deinit();
    try std.testing.expectEqual(@as(u64, 1), signal.sequence);
    try std.testing.expectEqualStrings(
        "{\"kind\":\"correlation\",\"version\":1}",
        signal.payload,
    );
    runtime.commitApplicationSignal(signal);
    try std.testing.expect((try runtime.prepareApplicationSignal(std.testing.allocator)) == null);
    const completion = try runtime.waitForCompletion();
    try std.testing.expectEqual(
        pty_process.ExitResult{ .exit_code = 0 },
        completion.exit_result.?,
    );
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    try std.testing.expect(completion.writer_stopped);
    try runtime.shutdown();
}
