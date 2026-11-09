//! Decision table for prompt-gated input: only a fresh, empty, quiet prompt of
//! the root interactive shell admits the line, and a refusal queues nothing.
const harness = @import("runtime_harness.zig");
const std = harness.std;
const TerminalRuntime = harness.TerminalRuntime;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const clearTestingHooks = harness.clearTestingHooks;
const commitReaderBatch = harness.commitReaderBatch;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;
const inputQueueSnapshot = harness.inputQueueSnapshot;
const runtimeState = harness.runtimeState;

const ms = std.time.ns_per_ms;
const prompt = "\x1b]133;P\x1b\\C:\\>\x1b]133;B\x1b\\";
const line: harness.protocol.PromptLineInput = .{ .content = "echo hi", .right_arrow = false };

const Observation = enum { inactive, active, failed };

const GateProbe = struct {
    observation: Observation = .inactive,
    now: u64 = 0,
    slept: u64 = 0,
    runtime: *TerminalRuntime = undefined,
    /// Output the shell writes while the gate waits out the quiet period.
    output_during_wait: ?[]const u8 = null,

    pub fn hooks(self: *GateProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .has_active_command = hasActiveCommand,
            .monotonic_time_nanoseconds = monotonic,
            .sleep_nanoseconds = sleep,
            .write_pty = swallow,
        };
    }

    fn hasActiveCommand(context: *anyopaque) anyerror!bool {
        const self: *GateProbe = @ptrCast(@alignCast(context));
        return switch (self.observation) {
            .active => true,
            .inactive => false,
            .failed => error.InjectedProcessObservationFailure,
        };
    }

    fn monotonic(context: *anyopaque) u64 {
        const self: *GateProbe = @ptrCast(@alignCast(context));
        return self.now;
    }

    /// The output, when set, arrives halfway through the wait.
    fn sleep(context: *anyopaque, nanoseconds: u64) void {
        const self: *GateProbe = @ptrCast(@alignCast(context));
        self.slept += nanoseconds;
        self.now += nanoseconds / 2;
        if (self.output_during_wait) |bytes| {
            std.debug.assert(commitReaderBatch(self.runtime, bytes) == .committed);
        }
        self.now += nanoseconds - nanoseconds / 2;
    }

    /// Accepted lines never reach the test's real child process.
    fn swallow(_: *anyopaque, bytes: []const u8) anyerror!usize {
        return bytes.len;
    }
};

const Fixture = struct {
    probe: harness.GatedPtyProbe(GateProbe) = .{ .probe = .{} },
    runtime: *TerminalRuntime = undefined,

    fn init(self: *Fixture) !void {
        self.runtime = try createRuntimeWithHooks(self.probe.hooks());
        self.probe.runtime = self.runtime;
        self.probe.probe.runtime = self.runtime;
        self.probe.reader.wait();
        runtimeState(self.runtime).process.interactive_shell = true;
    }

    fn deinit(self: *Fixture) void {
        self.probe.reader.release();
        clearTestingHooks(self.runtime);
        self.runtime.destroy();
    }

    fn output(self: *Fixture, bytes: []const u8) !void {
        try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(self.runtime, bytes));
    }

    fn after(self: *Fixture, nanoseconds: u64) void {
        self.probe.probe.now += nanoseconds;
    }

    /// Prepares and commits one line; returns the committed bytes or the refusal.
    fn submit(self: *Fixture, input: harness.protocol.PromptLineInput) ![]const u8 {
        var prepared = try self.runtime.preparePromptLineInput(input, std.math.maxInt(usize));
        defer prepared.deinit();
        const bytes = try std.testing.allocator.dupe(u8, prepared.bytes.?);
        errdefer std.testing.allocator.free(bytes);
        _ = try prepared.commit();
        return bytes;
    }

    /// A refusal reserves no queue entry.
    fn expectRefused(self: *Fixture, expected: anyerror) !void {
        try waitForEmptyQueue(self.runtime);
        if (self.submit(line)) |bytes| {
            std.testing.allocator.free(bytes);
            return error.TestExpectedRefusal;
        } else |actual| try std.testing.expectEqual(expected, actual);
        const after_refusal = inputQueueSnapshot(self.runtime);
        try std.testing.expectEqual(@as(usize, 0), after_refusal.entry_count);
        try std.testing.expectEqual(@as(usize, 0), after_refusal.client_charge.entries);
    }

    fn expectAccepted(self: *Fixture, input: harness.protocol.PromptLineInput, expected: []const u8) !void {
        const bytes = try self.submit(input);
        defer std.testing.allocator.free(bytes);
        try std.testing.expectEqualSlices(u8, expected, bytes);
        // The shell reads the line before it draws its next prompt.
        try waitForEmptyQueue(self.runtime);
    }

    fn publishTyped(self: *Fixture, bytes: []const u8) !void {
        try std.testing.expectEqual(bytes.len, try self.runtime.enqueueRawInput(bytes));
    }
};

test "only a quiet, fresh, empty prompt of the root interactive shell admits a line" {
    {
        // Idle prompt: the whole line and Enter enter the queue as one entry.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.after(100 * ms);
        try fixture.expectAccepted(line, "echo hi\r");
        // The line itself is input after the anchor: a second line waits for a new prompt.
        try fixture.expectRefused(error.PromptInputPending);
        try fixture.output("\r\n\x1b]133;P\x1b\\C:\\>\x1b]133;B\x1b\\");
        fixture.after(100 * ms);
        try fixture.expectAccepted(.{ .content = "codex", .right_arrow = true }, "codex\x1b[C\r");
    }
    {
        // Launch record: a program that is not an interactive shell never admits.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        runtimeState(fixture.runtime).process.interactive_shell = false;
        try fixture.output(prompt);
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptNotShell);
    }
    for ([_]Observation{ .active, .failed }) |observation| {
        // Process evidence: a console program, or an unreadable sample, refuses.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.after(100 * ms);
        fixture.probe.probe.observation = observation;
        try fixture.expectRefused(error.PromptBusy);
    }
    {
        // Alternate screen.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        try fixture.output("\x1b[?1049h");
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptFullScreen);
    }
    {
        // A shell that never marked a prompt, such as `cmd /Q` or an unmanaged PowerShell.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output("C:\\>");
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptUnmarked);
    }
}

test "a prompt that has not reached or has left its input start refuses" {
    const sequences = [_][]const u8{
        // The prompt started but its input start has not arrived.
        "\x1b]133;P\x1b\\C:\\>",
        // Read-Host or Start-Sleep: the command started and no new prompt followed.
        prompt ++ "\x1b]133;C\x1b\\",
        // A command end follows the input start.
        prompt ++ "\x1b]133;D;0\x1b\\",
        // A continuation prompt is not a fresh prompt.
        prompt ++ "\r\n\x1b]133;P;k=c\x1b\\>> \x1b]133;B\x1b\\",
        // An input start that no primary prompt opened.
        prompt ++ "\x1b]133;B\x1b\\",
    };
    for (sequences) |sequence| {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(sequence);
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptNotAtPrompt);
    }
    {
        // A fresh-line prompt start (A) and a right prompt drawn before the input start admit.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output("\x1b]133;A\x1b\\$ \x1b]133;P;k=r\x1b\\\x1b[s\x1b[70G12:00\x1b[u\x1b]133;B\x1b\\");
        fixture.after(100 * ms);
        try fixture.expectAccepted(line, "echo hi\r");
    }
}

test "any input or screen change after the input start refuses" {
    {
        // set /p or pause read a typed line: input arrived after the anchor.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        try fixture.publishTyped("ab");
        try fixture.output("ab");
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptInputPending);
    }
    {
        // Typed and erased again still counts as input after the anchor.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        try fixture.publishTyped("a\x08");
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptInputPending);
    }
    {
        // Input queued before the prompt and written after it.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        const reservation = try harness.reserveInput(fixture.runtime, 1, false, .client);
        try fixture.output(prompt);
        try harness.publishInput(fixture.runtime, reservation, try std.testing.allocator.dupe(u8, "q"));
        try waitForEmptyQueue(fixture.runtime);
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptInputPending);
    }
    {
        // Type-ahead echoed by the shell moves the cursor off the anchor.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        try fixture.output("x");
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptChanged);
    }
    {
        // Text after the anchor changed while the cursor returned to it.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        try fixture.output("abc\x1b[3D");
        fixture.after(100 * ms);
        try fixture.expectRefused(error.PromptChanged);
    }
    {
        // A focus report is not typed input.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output("\x1b[?1004h" ++ prompt);
        _ = try fixture.runtime.enqueueFocusInput(true);
        try waitForEmptyQueue(fixture.runtime);
        fixture.after(100 * ms);
        try fixture.expectAccepted(line, "echo hi\r");
    }
}

test "a young prompt is waited out once and a prompt replaced meanwhile refuses" {
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.after(40 * ms);
        try fixture.expectAccepted(line, "echo hi\r");
        try std.testing.expectEqual(@as(u64, 60 * ms), fixture.probe.probe.slept);
    }
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.probe.probe.output_during_wait = "\r\n" ++ prompt;
        try fixture.expectRefused(error.PromptChanged);
    }
    {
        // Type-ahead echoed during the wait moves the cursor off the anchor.
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.probe.probe.output_during_wait = "dir";
        try fixture.expectRefused(error.PromptChanged);
    }
}

test "publication repeats the rule against output and input after preparation" {
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.after(100 * ms);
        var prepared = try fixture.runtime.preparePromptLineInput(line, std.math.maxInt(usize));
        defer prepared.deinit();
        try fixture.output("x");
        try std.testing.expectError(error.PromptChanged, prepared.commit());
        try std.testing.expectEqual(@as(usize, 0), inputQueueSnapshot(fixture.runtime).client_charge.entries);
    }
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        try fixture.output(prompt);
        fixture.after(100 * ms);
        var prepared = try fixture.runtime.preparePromptLineInput(line, std.math.maxInt(usize));
        defer prepared.deinit();
        try fixture.publishTyped("a");
        try std.testing.expectError(error.PromptInputPending, prepared.commit());
    }
}

fn waitForEmptyQueue(runtime: *TerminalRuntime) !void {
    var attempts: usize = 0;
    while (inputQueueSnapshot(runtime).entry_count != 0) : (attempts += 1) {
        if (attempts == 2000) return error.InputQueueDidNotDrain;
        harness.platform_boundary.monotonic.sleep(ms);
    }
}
