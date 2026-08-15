const harness = @import("runtime_aggregate_harness.zig");
const builtin = @import("builtin");
const std = harness.std;
const core_boundary = harness.core_boundary;
const engine_boundary = harness.engine_boundary;
const service_boundary = harness.service_boundary;
const core_module = harness.core_module;
const input_admission = harness.input_admission;
const pty_process = harness.pty_process;
const subject = harness.subject;
const terminal_launch = harness.terminal_launch;
const terminal_runtime = harness.terminal_runtime;
const commitReaderBatch = harness.commitReaderBatch;
const createModel = harness.createModel;
const fenceResizeFailure = harness.fenceResizeFailure;
const recordReaderFailure = harness.recordReaderFailure;
const recordUnsafeResizeFailure = harness.recordUnsafeResizeFailure;
const cleanup_retry_inspection_budget = harness.cleanup_retry_inspection_budget;
const features = harness.features;
const palette = harness.palette;
const testId = harness.testId;
const publishPendingOutcome = harness.publishPendingOutcome;
const validMissingExecutableLaunch = harness.validMissingExecutableLaunch;
const validatedImmediateExitLaunch = harness.validatedImmediateExitLaunch;
const validatedLongRunningLaunch = harness.validatedLongRunningLaunch;
const createSkippedTarget = harness.createSkippedTarget;
const CountingTerminationDispatcher = harness.CountingTerminationDispatcher;
const FailingTerminationDispatcher = harness.FailingTerminationDispatcher;
const SkippedTargetObservingFencer = harness.SkippedTargetObservingFencer;
const SkippedTargetObservingContainment = harness.SkippedTargetObservingContainment;
const RetainedCleanupProbe = harness.RetainedCleanupProbe;
const HookedThreadSpawner = harness.HookedThreadSpawner;
const longRunningProcessLaunch = harness.longRunningProcessLaunch;
const commitHookedRuntime = harness.commitHookedRuntime;
const normalExitWithRetainedDescendantProcessLaunch =
    harness.normalExitWithRetainedDescendantProcessLaunch;
const commitHookedRuntimeWithLaunch = harness.commitHookedRuntimeWithLaunch;
const LogicalCleanupClock = harness.LogicalCleanupClock;
const readLogicalCleanupNs = harness.readLogicalCleanupNs;

test "runtime destruction starts only after every shutdown succeeds" {
    const FakeRuntime = struct {
        destroy_calls: usize = 0,
        shutdown_error: ?anyerror = null,
        shutdown_calls: usize = 0,

        pub fn shutdown(self: *@This()) !void {
            self.shutdown_calls += 1;
            if (self.shutdown_error) |err| return err;
        }

        pub fn destroyAfterShutdown(self: *@This()) void {
            self.destroy_calls += 1;
        }
    };
    const FakeOwner = struct {
        count: usize,
        engines: []?*FakeRuntime,

        pub fn ownedTerminalCount(self: *const @This()) usize {
            return self.count;
        }

        pub fn runtimeAtOwnedTerminal(self: *@This(), index: usize) ?*FakeRuntime {
            return self.engines[index];
        }

        pub fn destroyLastOwnedTerminal(self: *@This()) void {
            self.count -= 1;
            const engine = self.engines[self.count];
            self.engines[self.count] = null;
            if (engine) |runtime| runtime.destroyAfterShutdown();
        }
    };

    var runtimes = [_]FakeRuntime{
        .{},
        .{ .shutdown_error = error.TestShutdownFailure },
        .{ .shutdown_error = error.LaterShutdownFailure },
    };
    var engines = [_]?*FakeRuntime{
        &runtimes[0],
        &runtimes[1],
        &runtimes[2],
    };
    var owner: FakeOwner = .{ .count = engines.len, .engines = &engines };
    const shutdownAndReleaseTerminalEngines = subject.testing.shutdownAndReleaseTerminalEnginesFn;

    try std.testing.expectError(
        error.TestShutdownFailure,
        shutdownAndReleaseTerminalEngines(&owner),
    );
    try std.testing.expectEqual(@as(usize, 1), runtimes[0].shutdown_calls);
    try std.testing.expectEqual(@as(usize, 1), runtimes[1].shutdown_calls);
    try std.testing.expectEqual(@as(usize, 1), runtimes[2].shutdown_calls);
    for (runtimes) |runtime| {
        try std.testing.expectEqual(@as(usize, 0), runtime.destroy_calls);
    }
    try std.testing.expectEqual(engines.len, owner.ownedTerminalCount());
    for (engines) |engine| try std.testing.expect(engine != null);

    runtimes[1].shutdown_error = null;
    runtimes[2].shutdown_error = null;
    try shutdownAndReleaseTerminalEngines(&owner);
    try std.testing.expectEqual(@as(usize, 2), runtimes[0].shutdown_calls);
    try std.testing.expectEqual(@as(usize, 2), runtimes[1].shutdown_calls);
    try std.testing.expectEqual(@as(usize, 2), runtimes[2].shutdown_calls);
    for (runtimes) |runtime| {
        try std.testing.expectEqual(@as(usize, 1), runtime.destroy_calls);
    }
    try std.testing.expectEqual(@as(usize, 0), owner.ownedTerminalCount());
    for (engines) |engine| try std.testing.expect(engine == null);
}
test "THDD-001 fatal teardown retains cleanup ownership for an unconfirmed process tree" {
    var probe: RetainedCleanupProbe = .{ .failures_remaining = 1 };
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x41),
    );
    const terminal_id = testId(core_module.TerminalId, 0x44);
    _ = try commitHookedRuntime(&aggregate, connection_id, terminal_id, &probe);

    // The bounded containment window fails on the single fatal-teardown pass.
    try std.testing.expectError(
        error.ProcessContainmentFailed,
        aggregate.deinitChecked(false),
    );
    // Retained cleanup ownership: the terminal stays observable with an
    // explicit blocked containment instead of being abandoned or released.
    const retained = aggregate.core().findTerminal(terminal_id) orelse
        return error.TerminalNotRetained;
    try std.testing.expect(retained.failureContainment() != null);
}
test "TH-009 retained cleanup synchronizes a stale normal outcome before retrying" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var probe: RetainedCleanupProbe = .{ .failures_remaining = 1 };
    var logical_clock: LogicalCleanupClock = .{};
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    var finished = false;
    defer if (!finished) aggregate.finishRetainedCleanup() catch {};
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x45),
    );
    const terminal_id = testId(core_module.TerminalId, 0x48);
    const runtime = try commitHookedRuntimeWithLaunch(
        &aggregate,
        connection_id,
        terminal_id,
        &probe,
        normalExitWithRetainedDescendantProcessLaunch(),
    );
    subject.testing.setCleanupClockForTesting(&aggregate, readLogicalCleanupNs, &logical_clock);

    // The engine has a normal root-child + PTY-drain result while Core is
    // deliberately stale and the detached descendant remains in the session.
    const completion = try runtime.waitForCompletion();
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expectEqual(
        core_module.TerminalPhase.running,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    try std.testing.expect(!runtime.containmentConfirmed());

    try std.testing.expectError(
        error.ProcessContainmentFailed,
        aggregate.deinitChecked(false),
    );
    const retained = aggregate.core().findTerminal(terminal_id) orelse
        return error.TerminalNotRetained;
    try std.testing.expectEqual(core_module.TerminalPhase.exited, retained.phase());
    try std.testing.expect(retained.runtimeFailure() == null);
    try std.testing.expect(retained.failureContainment() == null);
    try std.testing.expect(retained.pendingOutcome() != null);
    const retry_at_ns = switch (retained.retainedCleanup()) {
        .blocked => |blocked| deadline: {
            try std.testing.expectEqual(
                core_module.RuntimeCleanupFailure.contain_process,
                blocked.failure,
            );
            break :deadline blocked.retry_at_ns;
        },
        else => return error.ExpectedRetainedCleanupBlock,
    };

    logical_clock.now_ns = retry_at_ns;
    try std.testing.expect(aggregate.runRetainedCleanupPass());
    const completed = aggregate.core().findTerminal(terminal_id) orelse
        return error.TerminalNotRetained;
    try std.testing.expectEqual(core_module.TerminalPhase.exited, completed.phase());
    try std.testing.expect(completed.runtimeFailure() == null);
    try std.testing.expect(completed.failureContainment() == null);
    try std.testing.expectEqual(
        core_module.RetainedCleanup.complete,
        completed.retainedCleanup(),
    );

    try aggregate.finishRetainedCleanup();
    finished = true;
}
test "TH-009 retained cleanup preserves an already-settled normal outcome" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var probe: RetainedCleanupProbe = .{ .failures_remaining = 1 };
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    var finished = false;
    defer if (!finished) aggregate.finishRetainedCleanup() catch {};
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x49),
    );
    const terminal_id = testId(core_module.TerminalId, 0x4c);
    const runtime = try commitHookedRuntimeWithLaunch(
        &aggregate,
        connection_id,
        terminal_id,
        &probe,
        normalExitWithRetainedDescendantProcessLaunch(),
    );

    _ = try runtime.waitForCompletion();
    aggregate.synchronizeRuntimes();
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );

    try std.testing.expectError(
        error.ProcessContainmentFailed,
        aggregate.deinitChecked(false),
    );
    const retained = aggregate.core().findTerminal(terminal_id) orelse
        return error.TerminalNotRetained;
    try std.testing.expectEqual(core_module.TerminalPhase.exited, retained.phase());
    try std.testing.expect(retained.runtimeFailure() == null);
    try std.testing.expect(retained.failureContainment() == null);
    switch (retained.retainedCleanup()) {
        .blocked => |blocked| try std.testing.expectEqual(
            core_module.RuntimeCleanupFailure.contain_process,
            blocked.failure,
        ),
        else => return error.ExpectedRetainedCleanupBlock,
    }

    try aggregate.finishRetainedCleanup();
    finished = true;
}
test "THDD-001 fatal teardown retries containment on the bounded backoff schedule and releases only after containment" {
    var probe: RetainedCleanupProbe = .{ .failures_remaining = 2 };
    var logical_clock: LogicalCleanupClock = .{};
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    var finished = false;
    defer if (!finished) aggregate.finishRetainedCleanup() catch {};
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x51),
    );
    const terminal_id = testId(core_module.TerminalId, 0x54);
    _ = try commitHookedRuntime(&aggregate, connection_id, terminal_id, &probe);
    subject.testing.setCleanupClockForTesting(&aggregate, readLogicalCleanupNs, &logical_clock);

    // The containment window fails on the fatal-teardown pass.
    try std.testing.expectError(
        error.ProcessContainmentFailed,
        aggregate.deinitChecked(false),
    );

    // Retained cleanup ownership: observable blocked containment and status.
    const terminal = aggregate.core().findTerminal(terminal_id) orelse
        return error.TerminalNotRetained;
    const containment = terminal.failureContainment() orelse
        return error.TerminalNotRetained;
    const first_retry_at_ns = switch (containment) {
        .blocked => |blocked| blocked.retry_at_ns,
        .stopping => return error.TerminalNotBlocked,
    };
    const initial_status = aggregate.retainedCleanupStatus();
    try std.testing.expect(initial_status.active);
    try std.testing.expect(!initial_status.exhausted);
    try std.testing.expect(aggregate.retainedCleanupPending());
    try std.testing.expect(!aggregate.runtimeFor(terminal_id).?.containmentConfirmed());

    // Bounded backoff: the first retry is not due before its deadline and
    // becomes due exactly at it.
    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    try std.testing.expect(takeDueCleanupRetry(&aggregate, first_retry_at_ns - 1) == null);
    try std.testing.expectEqual(
        terminal_id,
        takeDueCleanupRetry(&aggregate, first_retry_at_ns).?,
    );

    // The first retry still fails containment, so the retained ownership is
    // re-scheduled on the same bounded backoff cadence.
    logical_clock.now_ns = first_retry_at_ns;
    try std.testing.expect(!aggregate.runRetainedCleanupPass());
    const second_containment = aggregate.core().findTerminal(terminal_id).?.failureContainment() orelse
        return error.TerminalNotRetained;
    const second_retry_at_ns = switch (second_containment) {
        .blocked => |blocked| blocked.retry_at_ns,
        .stopping => return error.TerminalNotBlocked,
    };
    try std.testing.expect(second_retry_at_ns > first_retry_at_ns);
    try std.testing.expect(aggregate.retainedCleanupPending());

    // A later successful containment observation completes cleanup.
    logical_clock.now_ns = second_retry_at_ns;
    try std.testing.expect(aggregate.runRetainedCleanupPass());
    try std.testing.expect(!aggregate.retainedCleanupPending());
    try std.testing.expect(aggregate.runtimeFor(terminal_id).?.containmentConfirmed());

    // The release owner may release only now, with the tree contained. The
    // successful finish destroys the runtimes and releases the Core record;
    // after this point the aggregate is consumed (undefined).
    try aggregate.finishRetainedCleanup();
    finished = true;
}
test "THDD-001 retained-cleanup budget exhaustion leaves an explicit retained failed state, never a silent orphan" {
    var probe: RetainedCleanupProbe = .{ .failures_remaining = std.math.maxInt(usize) };
    var logical_clock: LogicalCleanupClock = .{};
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer {
        probe.failures_remaining = 0;
        aggregate.finishRetainedCleanup() catch {};
    }
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x61),
    );
    const terminal_id = testId(core_module.TerminalId, 0x64);
    _ = try commitHookedRuntime(&aggregate, connection_id, terminal_id, &probe);
    subject.testing.setCleanupClockForTesting(&aggregate, readLogicalCleanupNs, &logical_clock);

    try std.testing.expectError(
        error.ProcessContainmentFailed,
        aggregate.deinitChecked(false),
    );
    try std.testing.expect(aggregate.retainedCleanupPending());

    // A static clock means no retry ever becomes due; the bounded window
    // exhausts its budget instead of abandoning the tree.
    try std.testing.expectEqual(
        subject.testing.RetainedCleanupResultType.retained_failed,
        aggregate.runRetainedCleanupWindow(),
    );

    // Permanent-failure policy: explicit retained failed state, observable.
    const status = aggregate.retainedCleanupStatus();
    try std.testing.expect(status.active);
    try std.testing.expect(status.exhausted);
    try std.testing.expect(aggregate.retainedCleanupPending());

    // No release while unconfirmed: containment is not confirmed and the
    // terminal stays observable with a stable never-due blocked cleanup.
    const runtime = aggregate.runtimeFor(terminal_id).?;
    try std.testing.expect(!runtime.containmentConfirmed());
    const terminal = aggregate.core().findTerminal(terminal_id) orelse
        return error.TerminalNotRetained;
    const containment = terminal.failureContainment() orelse
        return error.TerminalNotRetained;
    switch (containment) {
        .blocked => |blocked| {
            try std.testing.expectEqual(
                core_module.RuntimeCleanupFailure.contain_process,
                blocked.cleanup_failure,
            );
            try std.testing.expectEqual(@as(i128, std.math.maxInt(i128)), blocked.retry_at_ns);
        },
        .stopping => return error.TerminalNotBlocked,
    }
    try std.testing.expectEqual(
        core_module.TerminalPhase.containing_failure,
        terminal.phase(),
    );
}
test "THDD-001 retained-cleanup state is surfaced as observable status" {
    var probe: RetainedCleanupProbe = .{ .failures_remaining = 1 };
    var logical_clock: LogicalCleanupClock = .{};
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.finishRetainedCleanup() catch {};
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x71),
    );
    const terminal_id = testId(core_module.TerminalId, 0x74);
    _ = try commitHookedRuntime(&aggregate, connection_id, terminal_id, &probe);
    subject.testing.setCleanupClockForTesting(&aggregate, readLogicalCleanupNs, &logical_clock);

    try std.testing.expectError(
        error.ProcessContainmentFailed,
        aggregate.deinitChecked(false),
    );
    const after_failure = aggregate.retainedCleanupStatus();
    try std.testing.expect(after_failure.active);
    try std.testing.expect(!after_failure.exhausted);

    // A retained-cleanup pass is counted even when no retry is due (the static
    // clock never advances past the bounded backoff deadline).
    _ = aggregate.runRetainedCleanupPass();
    const after_pass = aggregate.retainedCleanupStatus();
    try std.testing.expect(after_pass.active);
    try std.testing.expect(after_pass.attempts >= 1);

    // The permanent-failure policy marks the exhausted outcome.
    subject.testing.markRetainedCleanupExhausted(&aggregate);
    const exhausted = aggregate.retainedCleanupStatus();
    try std.testing.expect(exhausted.active);
    try std.testing.expect(exhausted.exhausted);
    try std.testing.expectEqual(
        @as(i128, std.math.maxInt(i128)),
        switch (aggregate.core().findTerminal(terminal_id).?.failureContainment().?) {
            .blocked => |blocked| blocked.retry_at_ns,
            .stopping => return error.TerminalNotBlocked,
        },
    );
}
test "runtime shutdown errors retain their exact cleanup phase" {
    const cleanupFailure = subject.testing.cleanupFailureFn;
    try std.testing.expectEqual(
        core_module.RuntimeCleanupFailure.contain_process,
        cleanupFailure(error.ProcessContainmentFailed),
    );
    try std.testing.expectEqual(
        core_module.RuntimeCleanupFailure.release_output,
        cleanupFailure(error.OutputReleaseFailed),
    );
    try std.testing.expectEqual(
        core_module.RuntimeCleanupFailure.release_process_identity,
        cleanupFailure(error.ProcessIdentityReleaseFailed),
    );
}
test "dispose returns the retained cleanup phase for every blocked containment variant" {
    const cases = [_]core_module.RuntimeCleanupFailure{
        .contain_process,
        .release_output,
        .release_process_identity,
    };
    for (cases) |cleanup_failure| {
        var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
        defer aggregate.deinitChecked(false) catch unreachable;
        const connection_id: core_module.ConnectionId = 1;
        _ = try aggregate.core().connect(
            connection_id,
            testId(core_module.ClientId, 0x11),
        );
        const terminal_id = testId(core_module.TerminalId, 0x12);
        const attachment_id = testId(core_module.AttachmentId, 0x13);
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, 0x14),
            terminal_id,
            [_]u8{0x15} ** 32,
            80,
            24,
        );
        _ = try aggregate.core().attach(
            connection_id,
            attachment_id,
            terminal_id,
            .{ .input = false, .read = true, .resize = false },
            false,
        );
        _ = try aggregate.core().runtimeFailed(terminal_id, .read_output);
        // A never-due retry deadline keeps the blocked phase stable for this
        // deterministic request; the independent maintenance pass retries it.
        _ = try aggregate.core().failureContainmentBlocked(
            terminal_id,
            cleanup_failure,
            std.math.maxInt(i128),
        );

        switch (try aggregate.disposeTerminal(connection_id, terminal_id)) {
            .cleanup_blocked => |retained| try std.testing.expectEqual(
                cleanup_failure,
                retained,
            ),
            .transition => return error.ExpectedCleanupBlockedDisposal,
        }
        try std.testing.expect(aggregate.core().findTerminal(terminal_id) != null);
        try std.testing.expect(aggregate.core().findAttachment(attachment_id) != null);
        const containment = aggregate.core().findTerminal(terminal_id).?.failureContainment() orelse
            return error.ExpectedBlockedContainment;
        switch (containment) {
            .blocked => |blocked| {
                try std.testing.expectEqual(cleanup_failure, blocked.cleanup_failure);
                try std.testing.expectEqual(@as(i128, std.math.maxInt(i128)), blocked.retry_at_ns);
            },
            .stopping => return error.ExpectedBlockedContainment,
        }
        _ = aggregate.disconnect(connection_id);
    }
}
test "blocked cleanup retry selection is fair across retained terminals" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x91),
    );
    const first_terminal_id = testId(core_module.TerminalId, 0x92);
    const second_terminal_id = testId(core_module.TerminalId, 0x93);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x94),
        first_terminal_id,
        [_]u8{0x95} ** 32,
        80,
        24,
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x96),
        second_terminal_id,
        [_]u8{0x97} ** 32,
        80,
        24,
    );
    for ([_]core_module.TerminalId{ first_terminal_id, second_terminal_id }) |terminal_id| {
        _ = try aggregate.core().runtimeFailed(terminal_id, .read_output);
        _ = try aggregate.core().failureContainmentBlocked(
            terminal_id,
            .contain_process,
            0,
        );
    }

    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    try std.testing.expectEqual(first_terminal_id, takeDueCleanupRetry(&aggregate, 0).?);
    try std.testing.expectEqual(second_terminal_id, takeDueCleanupRetry(&aggregate, 0).?);
    try std.testing.expectEqual(first_terminal_id, takeDueCleanupRetry(&aggregate, 0).?);
}
test "cleanup retry deadlines live in a monotonic scheduling domain" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xa1),
    );
    const terminal_id = testId(core_module.TerminalId, 0xa2);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0xa3),
        terminal_id,
        [_]u8{0xa4} ** 32,
        80,
        24,
    );

    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.requestTermination(connection_id, terminal_id, .graceful),
    );
    const containment = aggregate.core().findTerminal(terminal_id).?.failureContainment() orelse
        return error.TestUnexpectedResult;
    const retry_at_ns = switch (containment) {
        .blocked => |blocked| blocked.retry_at_ns,
        .stopping => return error.TestUnexpectedResult,
    };
    // The retry deadline must be one second past the Host-lifetime monotonic
    // clock reading, not a wall-clock epoch value. A fresh aggregate reads the
    // clock for the first time during this transition, so the deadline is
    // exactly one second of monotonic elapsed time.
    try std.testing.expect(retry_at_ns < std.time.ns_per_min);
}
test "blocked cleanup retry becomes due exactly at the monotonic deadline" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xa5),
    );
    const terminal_id = testId(core_module.TerminalId, 0xa6);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0xa7),
        terminal_id,
        [_]u8{0xa8} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().runtimeFailed(terminal_id, .read_output);
    const schedule_at_ns: i128 = 2 * std.time.ns_per_s;
    _ = try aggregate.core().failureContainmentBlocked(
        terminal_id,
        .contain_process,
        schedule_at_ns + std.time.ns_per_s,
    );

    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    // A wall-clock reading moved one hour backward must not postpone the
    // monotonic deadline's eligibility.
    try std.testing.expect(
        takeDueCleanupRetry(
            &aggregate,
            schedule_at_ns + std.time.ns_per_s - std.time.ns_per_hour,
        ) == null,
    );
    // Advancing the monotonic reading to exactly the deadline makes it due.
    try std.testing.expectEqual(
        terminal_id,
        takeDueCleanupRetry(&aggregate, schedule_at_ns + std.time.ns_per_s).?,
    );
}
test "synchronizeRuntimes uses the maintenance monotonic clock for due checks" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xa9),
    );
    const first_terminal_id = testId(core_module.TerminalId, 0xaa);
    const second_terminal_id = testId(core_module.TerminalId, 0xab);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0xac),
        first_terminal_id,
        [_]u8{0xad} ** 32,
        80,
        24,
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0xae),
        second_terminal_id,
        [_]u8{0xaf} ** 32,
        80,
        24,
    );
    // A one-nanosecond deadline is due only to a clock that has already read
    // past one nanosecond. A freshly created maintenance clock reads zero.
    _ = try aggregate.core().runtimeFailed(first_terminal_id, .read_output);
    _ = try aggregate.core().failureContainmentBlocked(
        first_terminal_id,
        .contain_process,
        1,
    );

    aggregate.synchronizeRuntimes();

    // The first terminal must not be selected as a due cleanup retry: its
    // one-nanosecond deadline is ahead of the monotonic reading, so the bounded
    // miss scan advances the cursor by the inspected slice instead of returning
    // it.
    try std.testing.expectEqual(
        @as(usize, 0),
        subject.testing.cleanupRetryScanIndexFn(&aggregate),
    );
}
test "cleanup retry miss advances the cursor by exactly one bounded slice" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xb0),
    );
    for (0..cleanup_retry_inspection_budget * 2) |offset| {
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, @intCast(0xc0 + offset)),
            testId(core_module.TerminalId, @intCast(0xd0 + offset)),
            [_]u8{0xe0} ** 32,
            80,
            24,
        );
    }
    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    // A miss inspects exactly one bounded slice and advances the cursor by the
    // number of terminals actually inspected.
    try std.testing.expectEqual(
        cleanup_retry_inspection_budget,
        subject.testing.cleanupRetryScanIndexFn(&aggregate),
    );
}
test "due cleanup retry beyond the first slice is selected on the next pass" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xb1),
    );
    for (0..cleanup_retry_inspection_budget) |offset| {
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, @intCast(0xc0 + offset)),
            testId(core_module.TerminalId, @intCast(0xd0 + offset)),
            [_]u8{0xe0} ** 32,
            80,
            24,
        );
    }
    const due_terminal_id = testId(core_module.TerminalId, 0xf0);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0xf1),
        due_terminal_id,
        [_]u8{0xf2} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().runtimeFailed(due_terminal_id, .read_output);
    _ = try aggregate.core().failureContainmentBlocked(
        due_terminal_id,
        .contain_process,
        0,
    );

    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    // The only due terminal sits immediately beyond the first bounded slice.
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    try std.testing.expectEqual(
        cleanup_retry_inspection_budget,
        subject.testing.cleanupRetryScanIndexFn(&aggregate),
    );
    try std.testing.expectEqual(
        due_terminal_id,
        takeDueCleanupRetry(&aggregate, 0).?,
    );
}
test "cleanup retry selection stays round-robin and bounded across slices" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xb2),
    );
    // Running terminals occupy indices 0..15 and 17..49; the only due blocked
    // terminals sit on slice boundaries at indices 16 and 33.
    for (0..cleanup_retry_inspection_budget) |offset| {
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, @intCast(0x10 + offset)),
            testId(core_module.TerminalId, @intCast(0x30 + offset)),
            [_]u8{0x40} ** 32,
            80,
            24,
        );
    }
    const first_due_id = testId(core_module.TerminalId, 0x50);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x51),
        first_due_id,
        [_]u8{0x52} ** 32,
        80,
        24,
    );
    for (0..cleanup_retry_inspection_budget) |offset| {
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, @intCast(0x20 + offset)),
            testId(core_module.TerminalId, @intCast(0x60 + offset)),
            [_]u8{0x70} ** 32,
            80,
            24,
        );
    }
    const second_due_id = testId(core_module.TerminalId, 0x80);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x81),
        second_due_id,
        [_]u8{0x82} ** 32,
        80,
        24,
    );
    for (0..cleanup_retry_inspection_budget) |offset| {
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, @intCast(0x90 + offset)),
            testId(core_module.TerminalId, @intCast(0xa0 + offset)),
            [_]u8{0xb0} ** 32,
            80,
            24,
        );
    }
    _ = try aggregate.core().runtimeFailed(first_due_id, .read_output);
    _ = try aggregate.core().failureContainmentBlocked(
        first_due_id,
        .contain_process,
        0,
    );
    _ = try aggregate.core().runtimeFailed(second_due_id, .read_output);
    _ = try aggregate.core().failureContainmentBlocked(
        second_due_id,
        .contain_process,
        0,
    );

    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    try std.testing.expectEqual(first_due_id, takeDueCleanupRetry(&aggregate, 0).?);
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    try std.testing.expectEqual(second_due_id, takeDueCleanupRetry(&aggregate, 0).?);
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    try std.testing.expectEqual(first_due_id, takeDueCleanupRetry(&aggregate, 0).?);
}
test "cleanup retry cursor normalization stays safe across terminal removal" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xb3),
    );
    var running_ids: [cleanup_retry_inspection_budget * 2]core_module.TerminalId = undefined;
    for (0..cleanup_retry_inspection_budget * 2) |offset| {
        const terminal_id = testId(core_module.TerminalId, @intCast(0xc0 + offset));
        running_ids[offset] = terminal_id;
        _ = try createModel(
            aggregate.core(),
            connection_id,
            testId(core_module.CreationId, @intCast(0xd0 + offset)),
            terminal_id,
            [_]u8{0xe0} ** 32,
            80,
            24,
        );
    }
    const due_terminal_id = testId(core_module.TerminalId, 0xf0);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0xf1),
        due_terminal_id,
        [_]u8{0xf2} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().runtimeFailed(due_terminal_id, .read_output);
    _ = try aggregate.core().failureContainmentBlocked(
        due_terminal_id,
        .contain_process,
        0,
    );

    const takeDueCleanupRetry = subject.testing.takeDueCleanupRetryFn;
    const terminal_count_before = aggregate.core().terminalCount();
    // The first bounded pass scans only the first slice and misses.
    try std.testing.expect(takeDueCleanupRetry(&aggregate, 0) == null);
    const cursor_before_removal = subject.testing.cleanupRetryScanIndexFn(&aggregate);
    try std.testing.expect(cursor_before_removal <= terminal_count_before);

    // Remove every running terminal; swap-remove reorders the retained slots
    // and shrinks the registry far below the stored cursor.
    for (running_ids) |terminal_id| {
        _ = try aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
        _ = try aggregate.core().ptyDrained(terminal_id);
        try publishPendingOutcome(&aggregate, terminal_id);
        try std.testing.expectEqual(
            core_module.TransitionOutcome.applied,
            try aggregate.core().disposeTerminal(connection_id, terminal_id),
        );
    }

    // The stored cursor must re-normalize against the shrunken registry and no
    // due retained terminal may be skipped indefinitely.
    var attempts: usize = 0;
    var selected = false;
    while (attempts < terminal_count_before * 2) : (attempts += 1) {
        if (takeDueCleanupRetry(&aggregate, 0)) |selected_id| {
            try std.testing.expect(selected_id.eql(due_terminal_id));
            selected = true;
            break;
        }
    }
    try std.testing.expect(selected);
    try std.testing.expect(
        subject.testing.cleanupRetryScanIndexFn(&aggregate) <
            aggregate.core().terminalCount(),
    );
}
