const harness = @import("runtime_aggregate_harness.zig");
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
const LogicalCleanupClock = harness.LogicalCleanupClock;
const readLogicalCleanupNs = harness.readLogicalCleanupNs;

test "runtime aggregate sensors stay behind the testing seam" {
    try std.testing.expect(
        !@hasField(subject.HostRuntimeAggregate, "projection_release_decisions"),
    );
    try std.testing.expect(@hasDecl(subject.testing, "projectionReleaseDecisions"));
}
test "runtime input errors close over the Host admission vocabulary" {
    try std.testing.expectEqual(
        input_admission.Rejection.backpressure,
        input_admission.fromRuntimeError(error.InputBackpressure),
    );
    try std.testing.expectEqual(
        input_admission.Rejection.resource_exhausted,
        input_admission.fromRuntimeError(error.OutOfMemory),
    );
    try std.testing.expectEqual(
        input_admission.Rejection.terminal_runtime_unavailable,
        input_admission.fromRuntimeError(error.TerminalResizeFailed),
    );
    try std.testing.expectEqual(
        input_admission.Rejection.terminal_exited,
        input_admission.fromRuntimeError(error.MissingAttachment),
    );
    try std.testing.expectEqual(
        input_admission.Rejection.encoding_failed,
        input_admission.fromRuntimeError(error.WriteFailed),
    );
}
test "failed runtime creation leaves no retained terminal commit" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const creation_id = testId(core_module.CreationId, 0x11);
    const validated = try terminal_launch.inspect(
        std.testing.allocator,
        validMissingExecutableLaunch(),
    );
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x10),
    );
    const inventory_generation = aggregate.core().inventoryGeneration();

    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        creation_id,
        validated,
    );
    defer preparation.deinit();
    try std.testing.expect(preparation.requiresEffect());
    try std.testing.expectEqual(@as(usize, 0), aggregate.core().terminalCount());
    try std.testing.expectEqual(inventory_generation, aggregate.core().inventoryGeneration());
    try std.testing.expectError(
        error.RuntimeSpawnFailed,
        aggregate.commitPreparedTerminal(&preparation),
    );
    try std.testing.expectEqual(@as(usize, 0), aggregate.core().terminalCount());
    try std.testing.expectEqual(@as(usize, 0), aggregate.runtimeCount());
    try std.testing.expect(!core_module.testing.containsCreation(aggregate.core(), creation_id));
    try std.testing.expectEqual(
        inventory_generation,
        aggregate.core().inventoryGeneration(),
    );
}
test "deferred creation retains launch ownership and failed start stays retryable" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x14),
    );
    var launch = validMissingExecutableLaunch();
    launch.geometry = null;
    const validated = try terminal_launch.inspect(std.testing.allocator, launch);
    var creation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x15),
        validated,
    );
    defer creation.deinit();
    try std.testing.expect(!creation.startsImmediately());
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&creation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const pending = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.awaiting_start, pending.phase());
    try std.testing.expect(pending.maybeObservedGeometry() == null);
    try std.testing.expectEqual(@as(usize, 0), aggregate.runtimeCount());

    const geometry = @import("terminal-host-model").geometry.Geometry{
        .cell_pixel_height = 19,
        .cell_pixel_width = 11,
        .columns = 101,
        .rows = 37,
    };
    var start = try aggregate.prepareStartTerminal(connection_id, terminal_id, geometry);
    try std.testing.expect(start.requiresEffect());
    try std.testing.expectError(
        error.RuntimeSpawnFailed,
        aggregate.commitPreparedTerminalStart(&start),
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.awaiting_start,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    try std.testing.expectEqual(@as(usize, 0), aggregate.runtimeCount());

    var retry = try aggregate.prepareStartTerminal(connection_id, terminal_id, geometry);
    try std.testing.expect(retry.requiresEffect());
    try std.testing.expectError(
        error.RuntimeSpawnFailed,
        aggregate.commitPreparedTerminalStart(&retry),
    );
    _ = try aggregate.disposeTerminal(connection_id, terminal_id);
}
test "deferred terminal starts at the exact supplied geometry and replays only that geometry" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x16),
    );
    var validated = try validatedLongRunningLaunch();
    validated.launch.geometry = null;
    validated.launch_digest = (try terminal_launch.inspect(
        std.testing.allocator,
        validated.launch,
    )).launch_digest;
    var creation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x17),
        validated,
    );
    defer creation.deinit();
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&creation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const geometry = @import("terminal-host-model").geometry.Geometry{
        .cell_pixel_height = 21,
        .cell_pixel_width = 12,
        .columns = 113,
        .rows = 41,
    };
    var start = try aggregate.prepareStartTerminal(connection_id, terminal_id, geometry);
    const started = try aggregate.commitPreparedTerminalStart(&start);
    try std.testing.expectEqual(subject.StartTransition.started, started.transition);
    try std.testing.expectEqualDeep(geometry, started.geometry);
    const runtime_geometry = aggregate.runtimeFor(terminal_id).?.geometry();
    try std.testing.expectEqual(geometry.columns, runtime_geometry.columns);
    try std.testing.expectEqual(geometry.rows, runtime_geometry.rows);
    try std.testing.expectEqual(geometry.cell_pixel_width, runtime_geometry.cell_pixel_width);
    try std.testing.expectEqual(geometry.cell_pixel_height, runtime_geometry.cell_pixel_height);
    try std.testing.expectEqualDeep(
        geometry,
        aggregate.core().findTerminal(terminal_id).?.observedGeometry(),
    );

    var replay = try aggregate.prepareStartTerminal(connection_id, terminal_id, geometry);
    try std.testing.expect(!replay.requiresEffect());
    try std.testing.expectEqual(
        subject.StartTransition.already_started,
        (try aggregate.commitPreparedTerminalStart(&replay)).transition,
    );
    var conflict_geometry = geometry;
    conflict_geometry.columns += 1;
    try std.testing.expectError(
        error.StartConflict,
        aggregate.prepareStartTerminal(connection_id, terminal_id, conflict_geometry),
    );
}
test "creation replay returns retained identity without preparing a new runtime" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const creation_id = testId(core_module.CreationId, 0x21);
    const terminal_id = testId(core_module.TerminalId, 0x22);
    const validated = try terminal_launch.inspect(
        std.testing.allocator,
        validMissingExecutableLaunch(),
    );
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x20),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        creation_id,
        terminal_id,
        validated.launch_digest,
        80,
        24,
    );

    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        creation_id,
        validated,
    );
    defer preparation.deinit();
    try std.testing.expect(!preparation.requiresEffect());
    const outcome = try aggregate.commitPreparedTerminal(&preparation);
    switch (outcome) {
        .replayed => |identity| {
            try std.testing.expect(identity.creation_id.eql(creation_id));
            try std.testing.expect(identity.terminal_id.eql(terminal_id));
        },
        .created => return error.ExpectedCreationReplay,
    }
    try std.testing.expectEqual(@as(usize, 0), aggregate.runtimeCount());

    _ = aggregate.core().disconnect(connection_id);
}
test "dispose preserves an exited Core record when its runtime is missing" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x32);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x30),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x31),
        terminal_id,
        [_]u8{0x33} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try aggregate.core().ptyDrained(terminal_id);
    try publishPendingOutcome(&aggregate, terminal_id);

    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.disposeTerminal(connection_id, terminal_id),
    );
    try std.testing.expect(aggregate.core().findTerminal(terminal_id) != null);

    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.core().disconnect(connection_id);
}
test "missing runtime enters retryable worker-integrity containment" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x72);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x70),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x71),
        terminal_id,
        [_]u8{0x73} ** 32,
        80,
        24,
    );

    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.requestTermination(connection_id, terminal_id, .graceful),
    );
    try std.testing.expectEqual(
        core_module.TerminationRequest.force,
        aggregate.core().findTerminal(terminal_id).?.terminationRequest(),
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.containing_failure,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    try std.testing.expectEqual(
        core_module.RuntimeFailureKind.worker_integrity,
        aggregate.core().findTerminal(terminal_id).?.runtimeFailure().?.kind(),
    );

    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.requestTermination(connection_id, terminal_id, .force),
    );
    try std.testing.expectEqual(
        core_module.TerminationRequest.force,
        aggregate.core().findTerminal(terminal_id).?.terminationRequest(),
    );
    const containment = aggregate.core().findTerminal(terminal_id).?.failureContainment() orelse
        return error.TestUnexpectedResult;
    switch (containment) {
        .blocked => {},
        .stopping => return error.TestUnexpectedResult,
    }
    _ = aggregate.disconnect(connection_id);
}
test "termination dispatch failure fences input before Core publication and cleanup retry" {
    const builtin = @import("builtin");
    const FailingDispatcher = struct {
        calls: usize = 0,

        pub fn dispatch(
            self: *@This(),
            _: *terminal_runtime.TerminalRuntime,
            _: subject.TerminationMode,
        ) !void {
            self.calls += 1;
            return error.InjectedTerminationDispatchFailure;
        }
    };
    const ObservingFailureFencer = struct {
        calls: usize = 0,
        core: *core_module.HostCore,
        core_failure_unpublished_before_fence: bool = false,
        fence_closed_before_return: bool = false,
        result: ?terminal_runtime.HostFailureFenceResult = null,
        terminal_id: core_module.TerminalId,

        pub fn fence(
            self: *@This(),
            runtime: *terminal_runtime.TerminalRuntime,
        ) terminal_runtime.HostFailureFenceResult {
            self.calls += 1;
            const terminal = self.core.findTerminal(self.terminal_id) orelse unreachable;
            self.core_failure_unpublished_before_fence = terminal.runtimeFailure() == null;
            const result = runtime.fenceHostFailure();
            self.result = result;
            self.fence_closed_before_return = terminal_runtime.testing.failureFenced(runtime);
            return result;
        }
    };
    const ScriptedContainment = struct {
        calls: usize = 0,
        core_failure_visible_after_fence: bool = false,
        prepared_input_rejected: bool = false,
        preparation: *input_admission.Preparation,
        retry_kept_fence_closed: bool = false,

        pub fn contain(
            self: *@This(),
            core: *core_module.HostCore,
            terminal_id: core_module.TerminalId,
            runtime: *terminal_runtime.TerminalRuntime,
        ) !void {
            self.calls += 1;
            const terminal = core.findTerminal(terminal_id) orelse unreachable;
            if (self.calls == 1) {
                self.core_failure_visible_after_fence =
                    terminal_runtime.testing.failureFenced(runtime) and
                    terminal.phase() == .containing_failure and
                    terminal.runtimeFailure().?.stage() == .dispatch_termination;
                self.prepared_input_rejected = switch (self.preparation.commit()) {
                    .core_rejected => |err| err == error.InvalidPhase,
                    else => false,
                };
                _ = core.failureContainmentBlocked(
                    terminal_id,
                    .release_output,
                    0,
                ) catch unreachable;
                return error.InjectedOutputReleaseFailure;
            }

            self.retry_kept_fence_closed = terminal_runtime.testing.failureFenced(runtime);
            _ = core.synchronizeRevision(
                terminal_id,
                runtime.completion().final_revision,
            ) catch unreachable;
            _ = core.failureContained(
                terminal_id,
                .{ .unavailable = .observe_process },
            ) catch unreachable;
        }
    };

    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const creation_id = testId(core_module.CreationId, 0x75);
    const attachment_id = testId(core_module.AttachmentId, 0x76);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x74),
    );
    var launch = validMissingExecutableLaunch();
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const windows_arguments = [_][]const u8{
        "/D",
        "/S",
        "/C",
        "more",
    };
    launch.executable = if (windows_executable) |value| value else "/bin/cat";
    launch.arguments = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &.{};
    const validated = try terminal_launch.inspect(std.testing.allocator, launch);
    var create_preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        creation_id,
        validated,
    );
    defer create_preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&create_preparation);
    const terminal_id = switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );
    const runtime = aggregate.runtimeFor(terminal_id).?;
    var input_preparation = aggregate.prepareRawInput(
        connection_id,
        attachment_id,
        1,
        "prepared-before-dispatch",
    );
    defer input_preparation.deinit();
    try std.testing.expect(input_preparation.accepted() != null);

    var dispatcher: FailingDispatcher = .{};
    var failure_fencer: ObservingFailureFencer = .{
        .core = aggregate.core(),
        .terminal_id = terminal_id,
    };
    var containment: ScriptedContainment = .{ .preparation = &input_preparation };
    const requestTerminationUsing = subject.testing.requestTerminationUsingFn;
    try std.testing.expectError(
        error.TerminationDispatchFailed,
        requestTerminationUsing(
            &aggregate,
            connection_id,
            terminal_id,
            .graceful,
            &dispatcher,
            &failure_fencer,
            &containment,
        ),
    );

    try std.testing.expect(failure_fencer.core_failure_unpublished_before_fence);
    try std.testing.expect(failure_fencer.fence_closed_before_return);
    switch (failure_fencer.result.?) {
        .host_won => {},
        .engine_won => return error.ExpectedHostFenceWinner,
    }
    try std.testing.expect(containment.core_failure_visible_after_fence);
    try std.testing.expect(containment.prepared_input_rejected);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    const blocked = aggregate.core().findTerminal(terminal_id).?.failureContainment() orelse
        return error.ExpectedBlockedContainment;
    switch (blocked) {
        .blocked => |state| try std.testing.expectEqual(
            core_module.RuntimeCleanupFailure.release_output,
            state.cleanup_failure,
        ),
        .stopping => return error.ExpectedBlockedContainment,
    }
    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.prepareRawInput("closed"),
    );

    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        try requestTerminationUsing(
            &aggregate,
            connection_id,
            terminal_id,
            .force,
            &dispatcher,
            &failure_fencer,
            &containment,
        ),
    );
    try std.testing.expect(containment.retry_kept_fence_closed);
    try std.testing.expectEqual(@as(usize, 1), dispatcher.calls);
    try std.testing.expectEqual(@as(usize, 1), failure_fencer.calls);
    try std.testing.expectEqual(@as(usize, 2), containment.calls);
    try std.testing.expectEqual(
        core_module.TerminalPhase.failed,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.prepareRawInput("still-closed"),
    );
}
test "engine failure between dispatch and Host fencing remains the Core primary cause" {
    const builtin = @import("builtin");
    const FailingDispatcher = struct {
        pub fn dispatch(
            _: *@This(),
            _: *terminal_runtime.TerminalRuntime,
            _: subject.TerminationMode,
        ) !void {
            return error.InjectedTerminationDispatchFailure;
        }
    };
    const EngineWinningFencer = struct {
        result: ?terminal_runtime.HostFailureFenceResult = null,

        pub fn fence(
            self: *@This(),
            runtime: *terminal_runtime.TerminalRuntime,
        ) terminal_runtime.HostFailureFenceResult {
            // This capability is the exact synchronization seam after the
            // failed dispatch and before the Host attempts its bounded fence.
            recordReaderFailure(runtime);
            const result = runtime.fenceHostFailure();
            self.result = result;
            return result;
        }
    };
    const ObservingContainment = struct {
        calls: usize = 0,
        retained_engine_cause: bool = false,

        pub fn contain(
            self: *@This(),
            core: *core_module.HostCore,
            terminal_id: core_module.TerminalId,
            _: *terminal_runtime.TerminalRuntime,
        ) !void {
            self.calls += 1;
            const failure = core.findTerminal(terminal_id).?.runtimeFailure().?;
            self.retained_engine_cause = failure.kind() == .pty_read and
                failure.stage() == .read_output;
            return error.InjectedContainmentObservationStop;
        }
    };

    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x78),
    );
    var launch = validMissingExecutableLaunch();
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "more" };
    launch.executable = if (windows_executable) |value| value else "/bin/cat";
    launch.arguments = if (builtin.os.tag == .windows) &windows_arguments else &.{};
    const validated = try terminal_launch.inspect(std.testing.allocator, launch);
    var creation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x79),
        validated,
    );
    defer creation.deinit();
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&creation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };

    var dispatcher: FailingDispatcher = .{};
    var fencer: EngineWinningFencer = .{};
    var containment: ObservingContainment = .{};
    try std.testing.expectError(
        error.TerminationDispatchFailed,
        subject.testing.requestTerminationUsingFn(
            &aggregate,
            connection_id,
            terminal_id,
            .force,
            &dispatcher,
            &fencer,
            &containment,
        ),
    );

    switch (fencer.result.?) {
        .engine_won => |failure| try std.testing.expectEqual(
            terminal_runtime.RuntimeFailureKind.pty_read,
            failure,
        ),
        .host_won => return error.ExpectedEngineFenceWinner,
    }
    try std.testing.expect(containment.retained_engine_cause);
    try std.testing.expectEqual(@as(usize, 1), containment.calls);
    const retained = aggregate.core().findTerminal(terminal_id).?.runtimeFailure().?;
    try std.testing.expectEqual(core_module.RuntimeFailureKind.pty_read, retained.kind());
    try std.testing.expectEqual(core_module.RuntimeFailureStage.read_output, retained.stage());
}
test "termination of a skipped settled target is replay-only without dispatch" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x21),
    );
    const validated = try validatedImmediateExitLaunch();
    const terminal_id = try createSkippedTarget(&aggregate, connection_id, validated);
    const completion = try aggregate.runtimeFor(terminal_id).?.waitForCompletion();
    try std.testing.expectEqual(
        terminal_runtime.CompletionState.normal,
        completion.classification,
    );
    // The engine has published normal completion, but the bounded background
    // scan has not visited this target, so Core still observes running.
    try std.testing.expectEqual(
        core_module.TerminalPhase.running,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );

    var dispatcher: CountingTerminationDispatcher = .{};
    var fencer: SkippedTargetObservingFencer = .{};
    var containment: SkippedTargetObservingContainment = .{};
    const requestTerminationUsing = subject.testing.requestTerminationUsingFn;

    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        try requestTerminationUsing(
            &aggregate,
            connection_id,
            terminal_id,
            .graceful,
            &dispatcher,
            &fencer,
            &containment,
        ),
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    try std.testing.expectEqual(
        core_module.TerminationRequest.none,
        aggregate.core().findTerminal(terminal_id).?.terminationRequest(),
    );
    try std.testing.expectEqual(@as(usize, 0), dispatcher.calls);
    try std.testing.expectEqual(@as(usize, 0), fencer.calls);
    try std.testing.expectEqual(@as(usize, 0), containment.calls);

    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        try requestTerminationUsing(
            &aggregate,
            connection_id,
            terminal_id,
            .force,
            &dispatcher,
            &fencer,
            &containment,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), dispatcher.calls);
    try std.testing.expectEqual(
        core_module.TerminationRequest.none,
        aggregate.core().findTerminal(terminal_id).?.terminationRequest(),
    );
    _ = aggregate.disconnect(connection_id);
}
test "exact pass retains a skipped target engine failure without dispatch" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x31),
    );
    const validated = try validatedLongRunningLaunch();
    const terminal_id = try createSkippedTarget(&aggregate, connection_id, validated);
    const runtime = aggregate.runtimeFor(terminal_id).?;
    // Publish a deterministic engine failure without HostService.
    recordReaderFailure(runtime);
    try std.testing.expectEqual(
        terminal_runtime.CompletionState{ .failed = .pty_read },
        runtime.completion().classification,
    );

    var dispatcher: FailingTerminationDispatcher = .{};
    var fencer: SkippedTargetObservingFencer = .{};
    var containment: SkippedTargetObservingContainment = .{};
    const requestTerminationUsing = subject.testing.requestTerminationUsingFn;
    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        try requestTerminationUsing(
            &aggregate,
            connection_id,
            terminal_id,
            .force,
            &dispatcher,
            &fencer,
            &containment,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), dispatcher.calls);
    try std.testing.expectEqual(@as(usize, 0), fencer.calls);
    try std.testing.expectEqual(@as(usize, 0), containment.calls);
    const retained = aggregate.core().findTerminal(terminal_id).?.runtimeFailure().?;
    try std.testing.expectEqual(core_module.RuntimeFailureKind.pty_read, retained.kind());
    try std.testing.expectEqual(core_module.RuntimeFailureStage.read_output, retained.stage());
    try std.testing.expectEqual(
        core_module.TerminalPhase.failed,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    _ = aggregate.disconnect(connection_id);
}
test "runtime completion synchronization emits one typed exit notice" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const creation_id = testId(core_module.CreationId, 0x81);
    const terminal_id = testId(core_module.TerminalId, 0x82);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x80),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        creation_id,
        terminal_id,
        [_]u8{0x83} ** 32,
        80,
        24,
    );
    const synchronizeRuntimeCompletion = subject.testing.synchronizeRuntimeCompletionFn;

    const drained: terminal_runtime.Completion = .{
        .child_exited = false,
        .exit_result = null,
        .final_revision = 3,
        .pty_drained = true,
    };
    try std.testing.expect(synchronizeRuntimeCompletion(
        aggregate.core(),
        terminal_id,
        drained,
    ) == null);
    try std.testing.expectEqual(
        core_module.TerminalPhase.finishing,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    try std.testing.expectEqual(
        @as(u64, 3),
        aggregate.core().findTerminal(terminal_id).?.observedRevision(),
    );

    var exited = drained;
    exited.child_exited = true;
    exited.exit_result = .{ .exit_code = 7 };
    exited.final_revision = 4;
    try std.testing.expect(synchronizeRuntimeCompletion(
        aggregate.core(),
        terminal_id,
        exited,
    ) == null);
    try std.testing.expectEqual(
        core_module.TerminalPhase.finishing,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    exited.classification = .normal;
    exited.writer_stopped = true;
    const notice = synchronizeRuntimeCompletion(
        aggregate.core(),
        terminal_id,
        exited,
    ).?;
    try std.testing.expect(notice.creation_id.eql(creation_id));
    try std.testing.expect(notice.terminal_id.eql(terminal_id));
    try std.testing.expectEqual(@as(u64, 4), notice.final_revision);
    switch (notice.result) {
        .exit_code => |code| try std.testing.expectEqual(@as(u32, 7), code),
        .signal, .unknown => return error.ExpectedExitCode,
    }
    try std.testing.expect(synchronizeRuntimeCompletion(
        aggregate.core(),
        terminal_id,
        exited,
    ) == null);

    try publishPendingOutcome(&aggregate, terminal_id);
    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.disconnect(connection_id);
}
test "maintenance synchronizes terminal-driven geometry without projection demand" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x84),
    );
    const validated = try validatedLongRunningLaunch();
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x85),
        validated,
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const runtime = aggregate.runtimeFor(terminal_id).?;
    try std.testing.expectEqual(
        @as(u16, 80),
        aggregate.core().findTerminal(terminal_id).?.observedGeometry().columns,
    );

    try std.testing.expectEqual(
        terminal_runtime.testing.ReaderBatchOutcomeType.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    const runtime_geometry = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 132), runtime_geometry.columns);
    try std.testing.expectEqual(
        @as(u16, 80),
        aggregate.core().findTerminal(terminal_id).?.observedGeometry().columns,
    );

    aggregate.synchronizeRuntimes();

    const observed = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(runtime_geometry.columns, observed.observedGeometry().columns);
    try std.testing.expectEqual(runtime_geometry.rows, observed.observedGeometry().rows);
    try std.testing.expectEqual(
        runtime_geometry.cell_pixel_width,
        observed.observedGeometry().cell_pixel_width,
    );
    try std.testing.expectEqual(
        runtime_geometry.cell_pixel_height,
        observed.observedGeometry().cell_pixel_height,
    );
    try std.testing.expectEqual(runtime_geometry.revision, observed.observedRevision());
}
test "application terminal controls require input capability and synchronize one exact Core observation" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xb0),
    );
    const validated = try validatedLongRunningLaunch();
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0xb1),
        validated,
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const input_attachment_id = testId(core_module.AttachmentId, 0xb2);
    const read_attachment_id = testId(core_module.AttachmentId, 0xb3);
    _ = try aggregate.attach(
        connection_id,
        input_attachment_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = false },
        false,
    );
    _ = try aggregate.attach(
        connection_id,
        read_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    const runtime = aggregate.runtimeFor(terminal_id).?;
    const before = runtime.geometry();

    try std.testing.expectError(
        error.MissingResource,
        aggregate.applyTerminalControl(connection_id, read_attachment_id, .clear),
    );
    try std.testing.expectEqualDeep(before, runtime.geometry());
    try std.testing.expectEqual(
        before.revision,
        aggregate.core().findTerminal(terminal_id).?.observedRevision(),
    );

    const cleared = try aggregate.applyTerminalControl(
        connection_id,
        input_attachment_id,
        .clear,
    );
    const after_clear = runtime.geometry();
    try std.testing.expect(cleared.attachment_id.eql(input_attachment_id));
    try std.testing.expect(cleared.terminal_id.eql(terminal_id));
    try std.testing.expectEqual(terminal_runtime.TerminalControlOperation.clear, cleared.operation);
    try std.testing.expectEqual(before.generation + 1, cleared.generation);
    try std.testing.expectEqual(before.revision + 1, cleared.revision);
    try std.testing.expectEqual(cleared.generation, after_clear.generation);
    try std.testing.expectEqual(cleared.revision, after_clear.revision);
    try std.testing.expectEqual(
        cleared.revision,
        aggregate.core().findTerminal(terminal_id).?.observedRevision(),
    );

    const reset = try aggregate.applyTerminalControl(
        connection_id,
        input_attachment_id,
        .reset,
    );
    const after_reset = runtime.geometry();
    try std.testing.expectEqual(terminal_runtime.TerminalControlOperation.reset, reset.operation);
    try std.testing.expectEqual(cleared.generation + 1, reset.generation);
    try std.testing.expectEqual(cleared.revision + 1, reset.revision);
    try std.testing.expectEqual(reset.generation, after_reset.generation);
    try std.testing.expectEqual(reset.revision, after_reset.revision);
    try std.testing.expectEqual(
        reset.revision,
        aggregate.core().findTerminal(terminal_id).?.observedRevision(),
    );
    try std.testing.expectEqual(@as(usize, 2), aggregate.core().attachmentCountForTerminal(terminal_id));
}
test "configuration update preserves an unsynchronized terminal-driven geometry" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x87),
    );
    const validated = try validatedLongRunningLaunch();
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x88),
        validated,
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const runtime = aggregate.runtimeFor(terminal_id).?;
    try std.testing.expectEqual(
        terminal_runtime.testing.ReaderBatchOutcomeType.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    const terminal_driven = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 132), terminal_driven.columns);
    try std.testing.expectEqual(
        @as(u16, 80),
        aggregate.core().findTerminal(terminal_id).?.observedGeometry().columns,
    );

    const update = try aggregate.updateTerminalConfiguration(
        connection_id,
        terminal_id,
        .{ .scrollback_bytes = 4_096 },
    );

    const final = runtime.geometry();
    const observed = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(@as(u16, 132), final.columns);
    try std.testing.expectEqual(final.columns, observed.observedGeometry().columns);
    try std.testing.expectEqual(final.rows, observed.observedGeometry().rows);
    try std.testing.expectEqual(final.cell_pixel_width, observed.observedGeometry().cell_pixel_width);
    try std.testing.expectEqual(
        final.cell_pixel_height,
        observed.observedGeometry().cell_pixel_height,
    );
    try std.testing.expectEqual(terminal_driven.revision, final.revision);
    try std.testing.expectEqual(final.revision, update.revision);
    try std.testing.expectEqual(final.revision, observed.observedRevision());
}
test "Windows exit code 259 synchronizes as one TerminalExited with code 259 (THCR-041)" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x51),
    );
    const windows_executable = try std.process.getEnvVarOwned(
        std.testing.allocator,
        "ComSpec",
    );
    defer std.testing.allocator.free(windows_executable);
    var launch = validMissingExecutableLaunch();
    launch.executable = windows_executable;
    launch.arguments = &.{ "/D", "/Q", "/C", "exit 259" };
    const validated = try terminal_launch.inspect(std.testing.allocator, launch);
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x52),
        validated,
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = try switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => error.ExpectedCreatedTerminal,
    };

    const completion = try aggregate.runtimeFor(terminal_id).?.waitForCompletion();
    try std.testing.expectEqual(
        terminal_runtime.CompletionState.normal,
        completion.classification,
    );
    try std.testing.expectEqual(
        pty_process.ExitResult{ .exit_code = 259 },
        completion.exit_result.?,
    );

    // The maintenance pass copies that normal engine completion into Core.
    aggregate.synchronizeRuntimes();

    const terminal = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.exited, terminal.phase());
    const pending = terminal.pendingOutcome() orelse
        return error.ExpectedExitedOutcome;
    switch (pending) {
        .exited => |outcome| {
            try std.testing.expect(outcome.terminal_id.eql(terminal_id));
            switch (outcome.result) {
                .exit_code => |code| try std.testing.expectEqual(@as(u32, 259), code),
                .signal, .unknown => return error.ExpectedExitCode,
            }
        },
        .failed => return error.ExpectedExitedOutcome,
    }

    // Exactly one outcome exists: publishing it is applied once and idempotent
    // thereafter, and a later synchronization publishes nothing new.
    try publishPendingOutcome(&aggregate, terminal_id);
    try std.testing.expect(
        aggregate.core().findTerminal(terminal_id).?.pendingOutcome() == null,
    );
    aggregate.synchronizeRuntimes();
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        aggregate.core().findTerminal(terminal_id).?.phase(),
    );

    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.disconnect(connection_id);
}
test "process observation failure maps to a distinct runtime failure" {
    const runtimeFailureFromObservation = subject.testing.runtimeFailureFromObservationFn;
    const failure = runtimeFailureFromObservation(.{
        .child_exited = false,
        .classification = .{ .failed = .process_observation },
        .exit_result = null,
        .final_revision = 7,
        .pty_drained = false,
    }).?;
    try std.testing.expectEqual(core_module.RuntimeFailureKind.process_observation, failure.kind());
    try std.testing.expectEqual(core_module.RuntimeFailureStage.observe_process, failure.stage());
}
test "worker integrity failure maps to a distinct runtime failure" {
    const runtimeFailureFromObservation = subject.testing.runtimeFailureFromObservationFn;
    const failure = runtimeFailureFromObservation(.{
        .child_exited = false,
        .classification = .{ .failed = .worker_integrity },
        .exit_result = null,
        .final_revision = 7,
        .pty_drained = false,
    }).?;
    try std.testing.expectEqual(core_module.RuntimeFailureKind.worker_integrity, failure.kind());
    try std.testing.expectEqual(core_module.RuntimeFailureStage.synchronize_worker, failure.stage());
}
test "the engine primary classification is the only runtime failure input" {
    const runtimeFailureFromObservation = subject.testing.runtimeFailureFromObservationFn;
    const failure = runtimeFailureFromObservation(.{
        .child_exited = false,
        .classification = .{ .failed = .pty_write },
        .exit_result = null,
        .final_revision = 7,
        .pty_drained = false,
    }).?;
    try std.testing.expectEqual(core_module.RuntimeFailureKind.pty_write, failure.kind());
    try std.testing.expectEqual(core_module.RuntimeFailureStage.write_input, failure.stage());
}
test "pending and normal engine completion do not map to runtime failures" {
    const runtimeFailureFromObservation = subject.testing.runtimeFailureFromObservationFn;
    const pending: terminal_runtime.Completion = .{
        .child_exited = false,
        .exit_result = null,
        .final_revision = 6,
        .pty_drained = false,
    };
    try std.testing.expect(runtimeFailureFromObservation(pending) == null);
    try std.testing.expect(runtimeFailureFromObservation(.{
        .child_exited = true,
        .classification = .normal,
        .exit_result = .{ .exit_code = 0 },
        .final_revision = 7,
        .pty_drained = true,
        .writer_stopped = true,
    }) == null);
}
