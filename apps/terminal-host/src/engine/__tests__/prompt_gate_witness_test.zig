//! Real-shell witness for prompt-gated input under the executing Windows
//! console host: the engine suite runs it on system ConHost and the bundled
//! ConPTY suite runs it on the pinned bundle. cmd and managed PowerShell admit
//! a line only at an idle prompt; a host whose marks never arrive refuses
//! every line. PowerShell starts without profiles, every line the test types
//! carries the Orchbee run marker, and the first one turns history saving off.
const harness = @import("runtime_harness.zig");
const screen_text = @import("screen_text.zig");
const std = harness.std;
const builtin = harness.target_builtin;
const platform = harness.platform_boundary;
const pty_process = harness.pty_process;
const TerminalRuntime = harness.TerminalRuntime;
const runtimeState = harness.runtimeState;

const allocator = std.testing.allocator;
const io = std.testing.io;
const capture_limit_bytes = 256 * 1024;
const step_timeout_ns = 30 * std.time.ns_per_s;
/// How long a visible prompt may wait for its marks before the host counts as unmarked.
const mark_grace_ns = 3 * std.time.ns_per_s;
const run_marker = "<#orchbee-run:0123456789abcdef0123456789abcdef#> ";
const quiet_ns = harness.protocol.Limits.prompt_line_quiet_milliseconds * std.time.ns_per_ms;

const Host = enum { system, bundled };
/// A recognizable prompt; the Host wraps it in its marks like any user PROMPT.
const cmd_environment = [_]pty_process.EnvironmentDelta{.{ .name = "PROMPT", .value = "obgate$G" }};

test "prompt gate witness: cmd admits a line only at an idle prompt" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const comspec = try pty_process.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(comspec);
    var session = try Session.start(comspec, &.{"/D"}, &cmd_environment, .cmd);
    defer session.deinit();

    try session.idle("echo idle-ok-1", "idle-ok-1");

    // set /p reads the next line; cmd has no child process and sends no C mark.
    try session.typeLine("set /p GATEANSWER=gate-ask-2?\r");
    try session.waitForRow("gate-ask-2?");
    try session.expectRefused("echo wrong-2");
    try session.typeThenPrompt("answer-2\r");

    // pause reads one key.
    try session.typeLine("pause\r");
    try session.waitForRowAfter("obgate>pause");
    try session.expectRefused("echo wrong-3");
    try session.typeThenPrompt(" ");

    // A half-typed line.
    try session.typeLine("echo half-4");
    try session.waitForLastRow("obgate>echo half-4");
    try session.expectRefused("echo wrong-4");
    try session.typeThenPrompt("\r");

    try session.idle("echo idle-ok-5", "idle-ok-5");
    try session.expectAbsent(&.{ "wrong-2", "wrong-3", "wrong-4" });
}

// A resize adds or removes empty cells after an idle prompt; while the prompt
// line fits both widths the prompt stays confirmable. Narrowing below the line
// re-wraps it, and the console host and Ghostty may then disagree on the
// cursor row, so that is not covered.
test "prompt gate witness: cmd keeps an idle prompt confirmable after a resize" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const comspec = try pty_process.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(comspec);
    var session = try Session.startSized(comspec, &.{"/D"}, &cmd_environment, .cmd, 80, 24);
    defer session.deinit();

    try session.idle("echo resize-ok-1", "resize-ok-1");
    try session.resize(109, 39);
    try session.idle("echo resize-ok-2", "resize-ok-2");
    try session.resize(90, 30);
    try session.idle("echo resize-ok-3", "resize-ok-3");
}

// After `echo off` cmd draws no prompt and no marks, so no later line is admitted.
test "prompt gate witness: cmd with echo off admits no line" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const comspec = try pty_process.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(comspec);
    var session = try Session.start(comspec, &.{"/D"}, &cmd_environment, .cmd);
    defer session.deinit();
    try session.typeLine("@echo off\r");
    try session.typeLine("echo still-1\r");
    try session.waitForRow("still-1");
    try session.expectRefused("echo wrong-1");
    platform.monotonic.sleep(500 * std.time.ns_per_ms);
    try session.expectAbsent(&.{"wrong-1"});
}

test "prompt gate witness: managed PowerShell admits a line only at an idle prompt" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const system_root = try pty_process.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(system_root);
    const program_files = try pty_process.environmentVariableOwned(allocator, "ProgramFiles");
    defer allocator.free(program_files);
    const legacy = try std.fs.path.join(allocator, &.{ system_root, "System32", "WindowsPowerShell", "v1.0", "powershell.exe" });
    defer allocator.free(legacy);
    const modern = try std.fs.path.join(allocator, &.{ program_files, "PowerShell", "7", "pwsh.exe" });
    defer allocator.free(modern);
    var exercised: usize = 0;
    for ([_][]const u8{ legacy, modern }) |executable| {
        std.Io.Dir.cwd().access(io, executable, .{}) catch |err| switch (err) {
            error.FileNotFound => continue,
            else => return err,
        };
        try witnessOnePowerShell(executable);
        exercised += 1;
    }
    try std.testing.expect(exercised > 0);
}

fn witnessOnePowerShell(executable: []const u8) !void {
    // The Host appends -NoExit and the canonical startup to these plain flags.
    var session = try Session.start(executable, &.{ "-NoLogo", "-NoProfile" }, &.{}, .powershell);
    defer session.deinit();
    if (!session.marked) {
        // Without marks the managed reader may be absent, so nothing is typed.
        try std.testing.expectError(error.PromptUnmarked, session.submit(run_marker ++ "'wrong-' + '0'"));
        return;
    }

    try session.idle(run_marker ++ "Set-PSReadLineOption -HistorySaveStyle SaveNothing; 'idle-' + 'ok-1'", "idle-ok-1");

    try session.typeLine(run_marker ++ "$gate = Read-Host 'gate-ask-2'\r");
    try session.waitForRow("gate-ask-2:");
    try session.expectRefused(run_marker ++ "'wrong-' + '2'");
    try session.typeThenPrompt("answer-2\r");

    try session.typeLine(run_marker ++ "Start-Sleep -Seconds 3; 'slept-' + 'done-3'\r");
    try session.waitForGateToLeaveInput();
    try session.expectRefused(run_marker ++ "'wrong-' + '3'");
    try session.waitForNewPrompt();
    try session.waitForRow("slept-done-3");

    try session.typeLine(run_marker ++ "'half-' + 'typed-4'");
    try session.waitForLastRowEnding("'half-' + 'typed-4'");
    try session.expectRefused(run_marker ++ "'wrong-' + '4'");
    try session.typeThenPrompt("\r");
    try session.waitForRow("half-typed-4");

    try session.idle(run_marker ++ "'idle-' + 'ok-5'", "idle-ok-5");
    try session.expectAbsent(&.{ "wrong-2", "wrong-3", "wrong-4" });
}

const Shell = enum { cmd, powershell };

const Session = struct {
    runtime: *TerminalRuntime,
    host: Host,
    shell: Shell,
    /// The first prompt's marks arrived; otherwise every line is refused.
    marked: bool,
    prompts: usize,
    serial: u64,

    fn start(
        executable: []const u8,
        arguments: []const []const u8,
        environment: []const pty_process.EnvironmentDelta,
        shell: Shell,
    ) !Session {
        return startSized(executable, arguments, environment, shell, 120, 30);
    }

    fn startSized(
        executable: []const u8,
        arguments: []const []const u8,
        environment: []const pty_process.EnvironmentDelta,
        shell: Shell,
        columns: u16,
        rows: u16,
    ) !Session {
        const host: Host = if (try platform.testing.conpty.initializedFromBundle()) .bundled else .system;
        const runtime = try TerminalRuntime.createWithOptions(allocator, .{
            .arguments = arguments,
            .columns = columns,
            .current_directory = null,
            .environment = environment,
            .executable = executable,
            .rows = rows,
        }, .{});
        errdefer runtime.destroy();
        var session: Session = .{ .runtime = runtime, .host = host, .shell = shell, .marked = false, .prompts = 0, .serial = 0 };
        try session.waitForPromptRows(1);
        session.marked = session.waitForMarks(mark_grace_ns);
        // The pinned bundle carries the marks in place; system ConHost may not.
        if (host == .bundled) try std.testing.expect(session.marked);
        std.debug.print("prompt gate witness: {t} {t} marked={}\n", .{ host, shell, session.marked });
        return session;
    }

    fn deinit(self: *Session) void {
        self.runtime.destroy();
    }

    /// Resizes the terminal, then waits out the console host's repaint, as a
    /// later start would.
    fn resize(self: *Session, columns: u16, rows: u16) !void {
        _ = try self.runtime.resizeGeometry(columns, rows, 0, 0);
        platform.monotonic.sleep(500 * std.time.ns_per_ms);
    }

    /// Submits one gated line as its caller does: a prompt still settling is
    /// waited out once, off the Host, and the line is submitted again.
    fn submit(self: *Session, content: []const u8) !void {
        self.submitOnce(content) catch |err| {
            if (err != error.PromptSettling) return err;
            platform.monotonic.sleep(quiet_ns);
            return self.submitOnce(content);
        };
    }

    fn submitOnce(self: *Session, content: []const u8) !void {
        var prepared = try self.runtime.preparePromptLineInput(.{ .content = content, .right_arrow = false }, std.math.maxInt(usize));
        defer prepared.deinit();
        _ = try prepared.commit();
    }

    /// At an idle prompt a marked host admits the line; an unmarked one refuses it.
    fn idle(self: *Session, content: []const u8, output: []const u8) !void {
        if (!self.marked) {
            try std.testing.expectError(error.PromptUnmarked, self.submit(content));
            try self.typeLine(content);
            try self.typeThenPrompt("\r");
        } else {
            try self.submit(content);
            try self.waitForNewPrompt();
        }
        try self.waitForRow(output);
    }

    fn expectRefused(self: *Session, content: []const u8) !void {
        if (self.submit(content)) {
            std.debug.print("prompt gate witness typed into a busy shell: {s}\n", .{content});
            return error.PromptGateAdmittedBusyShell;
        } else |err| switch (err) {
            error.PromptBusy,
            error.PromptFullScreen,
            error.PromptUnmarked,
            error.PromptNotAtPrompt,
            error.PromptInputPending,
            error.PromptChanged,
            => {},
            // Settling twice, or any other failure, is not a refusal by the gate's rules.
            else => return err,
        }
    }

    fn typeLine(self: *Session, bytes: []const u8) !void {
        try std.testing.expectEqual(bytes.len, try self.runtime.enqueueRawInput(bytes));
    }

    /// Types input that ends the shell's current read, then waits for its next prompt.
    fn typeThenPrompt(self: *Session, bytes: []const u8) !void {
        try self.typeLine(bytes);
        try self.waitForNewPrompt();
    }

    fn waitForNewPrompt(self: *Session) !void {
        try self.waitForPromptRows(self.prompts + 1);
        if (self.marked and !self.waitForMarks(step_timeout_ns)) {
            return self.timeout("prompt marks");
        }
    }

    /// Waits until the screen ends at a bare prompt drawn at least `count` times.
    fn waitForPromptRows(self: *Session, count: usize) !void {
        var timer = try platform.monotonic.Timer.start();
        while (true) {
            var capture = try screen_text.read(self.runtime, allocator, capture_limit_bytes);
            defer capture.deinit();
            var prompts: usize = 0;
            var last: []const u8 = "";
            var rows = std.mem.splitScalar(u8, capture.text(), '\n');
            while (rows.next()) |raw| {
                const row = std.mem.trimEnd(u8, raw, " \r");
                if (row.len == 0) continue;
                last = row;
                if (self.isPromptRow(row)) prompts += 1;
            }
            if (prompts >= count and self.isBarePrompt(last)) {
                self.prompts = prompts;
                return;
            }
            try self.checkTimer(&timer, "prompt");
        }
    }

    /// Waits for the input start of a prompt newer than the last one observed.
    fn waitForMarks(self: *Session, limit_ns: u64) bool {
        var timer = platform.monotonic.Timer.start() catch return false;
        while (timer.read() < limit_ns) {
            const state = runtimeState(self.runtime);
            state.terminal_mutex.lock();
            const phase = state.prompt_gate.phase;
            const serial = state.prompt_gate.serial;
            state.terminal_mutex.unlock();
            if (phase == .input and serial != self.serial) {
                self.serial = serial;
                return true;
            }
            platform.monotonic.sleep(10 * std.time.ns_per_ms);
        }
        return false;
    }

    /// Waits for the command start (C) that follows a submitted PowerShell line.
    fn waitForGateToLeaveInput(self: *Session) !void {
        var timer = try platform.monotonic.Timer.start();
        while (true) {
            const state = runtimeState(self.runtime);
            state.terminal_mutex.lock();
            const phase = state.prompt_gate.phase;
            state.terminal_mutex.unlock();
            if (phase != .input) return;
            try self.checkTimer(&timer, "command start");
        }
    }

    fn isPromptRow(self: *const Session, row: []const u8) bool {
        return switch (self.shell) {
            .cmd => std.mem.startsWith(u8, row, "obgate>"),
            .powershell => std.mem.startsWith(u8, row, "PS ") and std.mem.indexOfScalar(u8, row, '>') != null,
        };
    }

    fn isBarePrompt(self: *const Session, row: []const u8) bool {
        return switch (self.shell) {
            .cmd => std.mem.eql(u8, row, "obgate>"),
            .powershell => std.mem.startsWith(u8, row, "PS ") and std.mem.endsWith(u8, row, ">") and
                std.mem.indexOf(u8, row, "<#") == null,
        };
    }

    fn waitForRow(self: *Session, expected: []const u8) !void {
        try self.waitForScreen(.{ .row = expected });
    }

    fn waitForLastRow(self: *Session, expected: []const u8) !void {
        try self.waitForScreen(.{ .last_row = expected });
    }

    fn waitForLastRowEnding(self: *Session, suffix: []const u8) !void {
        try self.waitForScreen(.{ .last_row_ending = suffix });
    }

    fn waitForRowAfter(self: *Session, row: []const u8) !void {
        try self.waitForScreen(.{ .row_after = row });
    }

    const Expectation = union(enum) {
        row: []const u8,
        last_row: []const u8,
        last_row_ending: []const u8,
        row_after: []const u8,
    };

    fn waitForScreen(self: *Session, expectation: Expectation) !void {
        var timer = try platform.monotonic.Timer.start();
        while (true) {
            var capture = try screen_text.read(self.runtime, allocator, capture_limit_bytes);
            defer capture.deinit();
            if (matches(capture.text(), expectation)) return;
            try self.checkTimer(&timer, @tagName(expectation));
        }
    }

    fn matches(text: []const u8, expectation: Expectation) bool {
        var last: []const u8 = "";
        var after_marker = false;
        var rows = std.mem.splitScalar(u8, text, '\n');
        while (rows.next()) |raw| {
            const row = std.mem.trimEnd(u8, raw, " \r");
            switch (expectation) {
                .row => |expected| if (std.mem.eql(u8, row, expected)) return true,
                .row_after => |marker| {
                    if (std.mem.eql(u8, row, marker)) {
                        after_marker = true;
                    } else if (after_marker and row.len > 0) return true;
                },
                .last_row, .last_row_ending => {},
            }
            if (row.len > 0) last = row;
        }
        return switch (expectation) {
            .last_row => |expected| std.mem.eql(u8, last, expected),
            .last_row_ending => |suffix| std.mem.endsWith(u8, last, suffix),
            .row, .row_after => false,
        };
    }

    fn expectAbsent(self: *Session, needles: []const []const u8) !void {
        var capture = try screen_text.read(self.runtime, allocator, capture_limit_bytes);
        defer capture.deinit();
        for (needles) |needle| {
            if (std.mem.indexOf(u8, capture.text(), needle) != null) {
                std.debug.print("refused line reached the shell: {s}\nscreen:\n{s}\n", .{ needle, capture.text() });
                return error.RefusedLineReachedShell;
            }
        }
    }

    fn checkTimer(self: *Session, timer: *platform.monotonic.Timer, what: []const u8) !void {
        if (self.runtime.primaryFailure() != null) return error.TerminalRuntimeFailed;
        if (timer.read() >= step_timeout_ns) return self.timeout(what);
        platform.monotonic.sleep(10 * std.time.ns_per_ms);
    }

    fn timeout(self: *Session, what: []const u8) error{PromptGateWitnessTimedOut} {
        if (screen_text.read(self.runtime, allocator, capture_limit_bytes)) |capture| {
            var owned = capture;
            defer owned.deinit();
            std.debug.print("prompt gate witness timed out waiting for {s} ({t} {t}); screen:\n{s}\n", .{ what, self.host, self.shell, owned.text() });
        } else |_| {}
        return error.PromptGateWitnessTimedOut;
    }
};
