//! Decision table for screen-gated keys: the key is typed only on the frame
//! its sender named, for a process still in the terminal, and only when no
//! person acted since the sender first saw the prompt or within the quiet
//! time. A refusal queues nothing.
const harness = @import("runtime_harness.zig");
const std = harness.std;
const TerminalRuntime = harness.TerminalRuntime;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const commitReaderBatch = harness.commitReaderBatch;
const inputQueueSnapshot = harness.inputQueueSnapshot;
const runtimeState = harness.runtimeState;
const protocol = harness.protocol;

const second = std.time.ns_per_s;
const quiet = protocol.Limits.screen_gate_quiet_milliseconds * std.time.ns_per_ms;
const agent: protocol.ProcessIdentity = .{ .pid = 4242, .creation_time = 77 };

const Membership = enum { member, gone, unreadable };

const GateProbe = struct {
    membership: Membership = .member,
    now: u64 = 1000 * second,

    pub fn hooks(self: *GateProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .has_process = hasProcess,
            .monotonic_time_nanoseconds = monotonic,
            .write_pty = swallow,
        };
    }

    fn hasProcess(context: *anyopaque, identity: protocol.ProcessIdentity) anyerror!bool {
        const self: *GateProbe = @ptrCast(@alignCast(context));
        return switch (self.membership) {
            .member => identity.pid == agent.pid and identity.creation_time == agent.creation_time,
            .gone => false,
            .unreadable => error.InjectedProcessSampleFailure,
        };
    }

    fn monotonic(context: *anyopaque) u64 {
        const self: *GateProbe = @ptrCast(@alignCast(context));
        return self.now;
    }

    /// Accepted keys never reach the test's real child process.
    fn swallow(_: *anyopaque, bytes: []const u8) anyerror!usize {
        return bytes.len;
    }
};

const Fixture = struct {
    probe: harness.GatedPtyProbe(GateProbe) = .{ .probe = .{} },
    runtime: *TerminalRuntime = undefined,

    fn init(self: *Fixture) !void {
        // Cell sizes let pointer input encode once a program tracks the pointer.
        self.runtime = try harness.createIdleRuntimeWithGeometryAndHooks(80, 24, .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
        }, self.probe.hooks());
        self.probe.runtime = self.runtime;
        self.probe.reader.wait();
        try self.output("Do you want to proceed?\r\n> 1. Yes");
    }

    /// The hooks stay installed: the reader and the writer read them until `destroy` joins
    /// those threads, and the probe outlives it.
    fn deinit(self: *Fixture) void {
        self.probe.reader.release();
        self.runtime.destroy();
    }

    fn output(self: *Fixture, bytes: []const u8) !void {
        try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(self.runtime, bytes));
    }

    fn after(self: *Fixture, nanoseconds: u64) void {
        self.probe.probe.now += nanoseconds;
    }

    fn revision(self: *Fixture) u64 {
        return runtimeState(self.runtime).revision.load(.monotonic);
    }

    /// A key for the current frame whose prompt was first seen at `first`.
    fn key(self: *Fixture, first: u64) protocol.ScreenGatedKeyInput {
        return .{ .key = .{
            .key = .enter,
            .process = agent,
            .generation = runtimeState(self.runtime).generation,
            .revision = self.revision(),
            .first_revision = first,
        } };
    }

    /// A key for a prompt first seen in the current frame.
    fn freshKey(self: *Fixture) protocol.ScreenGatedKeyInput {
        return self.key(self.revision());
    }

    fn submit(self: *Fixture, input: protocol.ScreenGatedKeyInput) ![]const u8 {
        var prepared = try self.runtime.prepareScreenGatedKeyInput(input, std.math.maxInt(usize));
        defer prepared.deinit();
        const bytes = try std.testing.allocator.dupe(u8, prepared.bytes orelse "");
        errdefer std.testing.allocator.free(bytes);
        _ = try prepared.commit();
        return bytes;
    }

    /// A refusal reserves no queue entry.
    fn expectRefused(self: *Fixture, expected: anyerror, input: protocol.ScreenGatedKeyInput) !void {
        try waitForEmptyQueue(self.runtime);
        if (self.submit(input)) |bytes| {
            std.testing.allocator.free(bytes);
            return error.TestExpectedRefusal;
        } else |actual| try std.testing.expectEqual(expected, actual);
        const after_refusal = inputQueueSnapshot(self.runtime);
        try std.testing.expectEqual(@as(usize, 0), after_refusal.entry_count);
        try std.testing.expectEqual(@as(usize, 0), after_refusal.client_charge.entries);
    }

    /// The only bytes this path ever writes are one Enter.
    fn expectTyped(self: *Fixture, input: protocol.ScreenGatedKeyInput) !void {
        const bytes = try self.submit(input);
        defer std.testing.allocator.free(bytes);
        try std.testing.expectEqualSlices(u8, "\r", bytes);
        try waitForEmptyQueue(self.runtime);
    }

    fn personTypes(self: *Fixture, bytes: []const u8) !void {
        try std.testing.expectEqual(bytes.len, try self.runtime.enqueueRawInput(bytes));
        try waitForEmptyQueue(self.runtime);
    }

    fn pointer(self: *Fixture, action: protocol.MouseAction) !usize {
        return self.runtime.enqueueMouseInput(.{
            .action = action,
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 1,
            .y = 1,
        });
    }
};

test "screen gate: a key is typed once, on the frame it names, for a process in the terminal" {
    var fixture: Fixture = .{};
    try fixture.init();
    defer fixture.deinit();

    var other_generation = fixture.freshKey();
    other_generation.key.generation += 1;
    try fixture.expectRefused(error.ScreenChanged, other_generation);
    var earlier_frame = fixture.freshKey();
    earlier_frame.key.revision -= 1;
    earlier_frame.key.first_revision -= 1;
    try fixture.expectRefused(error.ScreenChanged, earlier_frame);

    for ([_]Membership{ .gone, .unreadable }) |membership| {
        fixture.probe.probe.membership = membership;
        try fixture.expectRefused(error.ScreenAgentGone, fixture.freshKey());
    }
    fixture.probe.probe.membership = .member;
    var other_process = fixture.freshKey();
    other_process.key.process.creation_time += 1;
    try fixture.expectRefused(error.ScreenAgentGone, other_process);

    const typed_on = fixture.freshKey();
    try fixture.expectTyped(typed_on);
    // The key is not a person's input, yet a second key needs a new frame.
    try fixture.expectRefused(error.ScreenAlreadyTyped, typed_on);
    try fixture.output("\r\nDo you want to proceed?\r\n> 1. Yes");
    try fixture.expectRefused(error.ScreenChanged, typed_on);
    try fixture.expectTyped(fixture.freshKey());
}

test "screen gate: a person who acted since the prompt was first seen keeps it, and the quiet time follows them" {
    var fixture: Fixture = .{};
    try fixture.init();
    defer fixture.deinit();

    const first_seen = fixture.revision();
    try fixture.personTypes("n");
    // On the screen they typed on, however long ago.
    fixture.after(60 * second);
    try fixture.expectRefused(error.ScreenPersonInput, fixture.key(first_seen));

    // The prompt redraws: it was still first seen before they typed.
    try fixture.output("\x1b[1G> 1. Yes");
    fixture.after(60 * second);
    try fixture.expectRefused(error.ScreenPersonInput, fixture.key(first_seen));

    // A prompt first seen after they typed waits out the quiet time.
    try fixture.personTypes("y");
    try fixture.output("\r\nDo you want to proceed?\r\n> 1. Yes");
    fixture.after(quiet - 1);
    try fixture.expectRefused(error.ScreenPersonRecent, fixture.freshKey());
    fixture.after(1);
    try fixture.expectTyped(fixture.freshKey());
}

test "screen gate: a click that encodes to nothing is a person; a focus report and a bare pointer move are not" {
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        // No pointer tracking: the press reaches no program.
        try std.testing.expectEqual(@as(usize, 0), try fixture.pointer(.{ .press = .left }));
        fixture.after(60 * second);
        try fixture.expectRefused(error.ScreenPersonInput, fixture.freshKey());
    }
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        // Focus reporting and any-motion tracking are on, so both reach the program.
        try fixture.output("\x1b[?1004h\x1b[?1003h\x1b[?1006h");
        try std.testing.expect(try fixture.runtime.enqueueFocusInput(true) > 0);
        try std.testing.expect(try fixture.pointer(.{ .motion = .hover }) > 0);
        try waitForEmptyQueue(fixture.runtime);
        try fixture.expectTyped(fixture.freshKey());
    }
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        // Neither mode is on: both encode to nothing and still are no person.
        _ = try fixture.runtime.enqueueFocusInput(true);
        try std.testing.expectEqual(@as(usize, 0), try fixture.pointer(.{ .motion = .hover }));
        try fixture.expectTyped(fixture.freshKey());
    }
}

test "screen gate: a hold types nothing and refuses every key decided before it" {
    var fixture: Fixture = .{};
    try fixture.init();
    defer fixture.deinit();

    var decided = try fixture.runtime.prepareScreenGatedKeyInput(fixture.freshKey(), std.math.maxInt(usize));
    defer decided.deinit();
    const held = try fixture.submit(.hold);
    defer std.testing.allocator.free(held);
    try std.testing.expectEqual(@as(usize, 0), held.len);
    try std.testing.expectError(error.ScreenPersonInput, decided.commit());
    try std.testing.expectEqual(@as(usize, 0), inputQueueSnapshot(fixture.runtime).client_charge.entries);

    // A key decided earlier and arriving later names the same or an older frame.
    fixture.after(60 * second);
    try fixture.expectRefused(error.ScreenPersonInput, fixture.freshKey());
    // Turned on again: a prompt first seen afterwards is answered.
    try fixture.output("\r\nDo you want to proceed?\r\n> 1. Yes");
    try fixture.expectTyped(fixture.freshKey());
}

test "screen gate: publication repeats the rules against output and input after preparation" {
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        var prepared = try fixture.runtime.prepareScreenGatedKeyInput(fixture.freshKey(), std.math.maxInt(usize));
        defer prepared.deinit();
        try fixture.output("x");
        try std.testing.expectError(error.ScreenChanged, prepared.commit());
        try std.testing.expectEqual(@as(usize, 0), inputQueueSnapshot(fixture.runtime).client_charge.entries);
    }
    {
        var fixture: Fixture = .{};
        try fixture.init();
        defer fixture.deinit();
        var prepared = try fixture.runtime.prepareScreenGatedKeyInput(fixture.freshKey(), std.math.maxInt(usize));
        defer prepared.deinit();
        try std.testing.expectEqual(@as(usize, 1), try fixture.runtime.enqueueRawInput("a"));
        // The person's byte is still queued or already written; either way the key is refused.
        if (prepared.commit()) |_| {
            return error.TestExpectedRefusal;
        } else |refusal| try std.testing.expect(refusal == error.ScreenInputPending or
            refusal == error.ScreenPersonInput);
    }
}

test "screen gate: a runtime that does not know who typed refuses until a person acts" {
    var fixture: Fixture = .{};
    try fixture.init();
    defer fixture.deinit();
    // What a runtime built from a handover record starts with.
    runtimeState(fixture.runtime).screen_gate_input = .{};
    fixture.after(60 * second);
    try fixture.expectRefused(error.ScreenPersonInput, fixture.freshKey());
    try fixture.output("\r\nDo you want to proceed?\r\n> 1. Yes");
    try fixture.expectRefused(error.ScreenPersonInput, fixture.freshKey());

    try fixture.personTypes("y");
    try fixture.output("\r\nDo you want to proceed?\r\n> 1. Yes");
    fixture.after(quiet);
    try fixture.expectTyped(fixture.freshKey());
}

fn waitForEmptyQueue(runtime: *TerminalRuntime) !void {
    var attempts: usize = 0;
    while (inputQueueSnapshot(runtime).entry_count != 0) : (attempts += 1) {
        if (attempts == 2000) return error.InputQueueDidNotDrain;
        harness.platform_boundary.monotonic.sleep(std.time.ns_per_ms);
    }
}
