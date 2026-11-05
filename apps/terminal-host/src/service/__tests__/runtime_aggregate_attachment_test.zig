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
const protocol = @import("terminal-protocol");
const terminal_geometry = @import("terminal-host-model").geometry;
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

const ResizeCommitProbe = struct {
    calls: usize = 0,
    revision: ?u64 = null,

    fn advanceRuntime(
        context: ?*anyopaque,
        runtime: *terminal_runtime.TerminalRuntime,
    ) void {
        const self: *ResizeCommitProbe = @ptrCast(@alignCast(context.?));
        const update = runtime.applyTerminalControl(.clear) catch unreachable;
        self.calls += 1;
        self.revision = update.revision;
    }
};

const ConfigurationCommitProbe = struct {
    calls: usize = 0,
    revision: ?u64 = null,

    fn commitReaderOutput(
        context: ?*anyopaque,
        runtime: *terminal_runtime.TerminalRuntime,
    ) void {
        const self: *ConfigurationCommitProbe = @ptrCast(@alignCast(context.?));
        std.debug.assert(commitReaderBatch(runtime, "after configuration") == .committed);
        self.calls += 1;
        self.revision = runtime.geometry().revision;
    }
};

test "configuration observation accepts the commit revision or a newer runtime revision" {
    try std.testing.expect(subject.testing.configurationObservationIsCompatibleFn(11, 11));
    try std.testing.expect(subject.testing.configurationObservationIsCompatibleFn(11, 12));
    try std.testing.expect(!subject.testing.configurationObservationIsCompatibleFn(11, 10));
}

test "configuration synchronization adopts runtime output committed after the update" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x75),
    );
    var launch_fixture = try validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x76),
        validated,
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const configuration_revision_before =
        aggregate.core().findTerminal(terminal_id).?.configurationRevision();

    var probe: ConfigurationCommitProbe = .{};
    subject.testing.setAfterConfigurationCommitHook(
        &aggregate,
        ConfigurationCommitProbe.commitReaderOutput,
        &probe,
    );
    defer subject.testing.setAfterConfigurationCommitHook(&aggregate, null, null);
    const update = try aggregate.updateTerminalConfiguration(
        connection_id,
        terminal_id,
        .{ .scrollback_bytes = 4_096 },
    );

    try std.testing.expectEqual(@as(usize, 1), probe.calls);
    try std.testing.expectEqual(update.revision + 1, probe.revision.?);
    try std.testing.expectEqual(
        configuration_revision_before + 1,
        update.configuration_revision,
    );
    try std.testing.expectEqual(@as(usize, 0), update.history_rows_evicted);
    const runtime_geometry = aggregate.runtimeFor(terminal_id).?.geometry();
    const observed = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(runtime_geometry.revision, observed.observedRevision());
    try std.testing.expect(runtime_geometry.revision > update.revision);
}

test "resize observation accepts the exact commit or a newer canonical runtime state" {
    const geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    };
    const exact: terminal_runtime.Geometry = .{
        .cell_pixel_height = geometry.cell_pixel_height,
        .cell_pixel_width = geometry.cell_pixel_width,
        .columns = geometry.columns,
        .generation = 7,
        .revision = 11,
        .rows = geometry.rows,
    };
    try std.testing.expect(subject.testing.resizeObservationIsCompatibleFn(geometry, 11, exact));

    var mismatched = exact;
    mismatched.columns += 1;
    try std.testing.expect(!subject.testing.resizeObservationIsCompatibleFn(geometry, 11, mismatched));
    mismatched.revision = 12;
    try std.testing.expect(subject.testing.resizeObservationIsCompatibleFn(geometry, 11, mismatched));
    mismatched.revision = 10;
    try std.testing.expect(!subject.testing.resizeObservationIsCompatibleFn(geometry, 11, mismatched));
}

test "resize synchronization adopts a runtime revision that advances after commit" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x73);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x70),
    );
    var launch_fixture = try validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x71),
        validated,
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = switch (created) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );

    var probe: ResizeCommitProbe = .{};
    subject.testing.setAfterResizeCommitHook(
        &aggregate,
        ResizeCommitProbe.advanceRuntime,
        &probe,
    );
    defer subject.testing.setAfterResizeCommitHook(&aggregate, null, null);
    const resized = try aggregate.resizeTerminal(connection_id, attachment_id, .{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
        .columns = 100,
        .rows = 30,
    });

    try std.testing.expectEqual(@as(usize, 1), probe.calls);
    try std.testing.expectEqual(resized.revision + 1, probe.revision.?);
    const runtime_geometry = aggregate.runtimeFor(terminal_id).?.geometry();
    const observed = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(runtime_geometry.revision, observed.observedRevision());
    try std.testing.expectEqualDeep(
        terminal_geometry.Geometry{
            .cell_pixel_height = runtime_geometry.cell_pixel_height,
            .cell_pixel_width = runtime_geometry.cell_pixel_width,
            .columns = runtime_geometry.columns,
            .rows = runtime_geometry.rows,
        },
        observed.observedGeometry(),
    );
    try std.testing.expect(runtime_geometry.revision > resized.revision);
}

test "configuration and resize preflight preserve Core without a runtime" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x92);
    const attachment_id = testId(core_module.AttachmentId, 0x93);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x90),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x91),
        terminal_id,
        [_]u8{0x94} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = false, .read = false, .resize = true },
        true,
    );
    const initial = aggregate.core().findTerminal(terminal_id).?.*;

    try std.testing.expectError(
        error.InvalidConfiguration,
        aggregate.updateTerminalConfiguration(
            connection_id,
            terminal_id,
            .{ .scrollback_bytes = std.math.maxInt(usize) },
        ),
    );
    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.updateTerminalConfiguration(
            connection_id,
            terminal_id,
            .{ .scrollback_bytes = 0 },
        ),
    );
    try std.testing.expectEqual(
        initial.effectiveConfiguration().scrollback_bytes,
        aggregate.core().findTerminal(terminal_id).?.effectiveConfiguration().scrollback_bytes,
    );
    try std.testing.expectEqual(
        initial.configurationRevision(),
        aggregate.core().findTerminal(terminal_id).?.configurationRevision(),
    );

    try std.testing.expectError(
        error.InvalidGeometry,
        aggregate.resizeTerminal(connection_id, attachment_id, .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 0,
            .rows = 24,
        }),
    );
    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.resizeTerminal(connection_id, attachment_id, .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 100,
            .rows = 30,
        }),
    );
    const unchanged = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqualDeep(initial.observedGeometry(), unchanged.observedGeometry());
    try std.testing.expectEqual(initial.observedRevision(), unchanged.observedRevision());

    core_module.testing.setConfigurationRevision(
        aggregate.core(),
        terminal_id,
        std.math.maxInt(u64),
    );
    try std.testing.expectError(
        error.ConfigurationRevisionOverflow,
        aggregate.updateTerminalConfiguration(
            connection_id,
            terminal_id,
            .{ .scrollback_bytes = 0 },
        ),
    );

    _ = try aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try aggregate.core().ptyDrained(terminal_id);
    try publishPendingOutcome(&aggregate, terminal_id);
    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.disconnect(connection_id);
}
test "read attachment is not committed when its terminal runtime is missing" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x42);
    const attachment_id = testId(core_module.AttachmentId, 0x43);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x40),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x41),
        terminal_id,
        [_]u8{0x44} ** 32,
        80,
        24,
    );

    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        aggregate.attach(
            connection_id,
            attachment_id,
            terminal_id,
            .{ .input = false, .read = true, .resize = false },
            false,
        ),
    );
    try std.testing.expect(aggregate.core().findAttachment(attachment_id) == null);

    _ = aggregate.disconnect(connection_id);
}
test "input admission identifies its failing owner without retaining Core credit" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x62);
    const attachment_id = testId(core_module.AttachmentId, 0x63);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x60),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x61),
        terminal_id,
        [_]u8{0x64} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = false },
        false,
    );

    switch (aggregate.acceptRawInput(connection_id, attachment_id, 1, "x")) {
        .runtime_missing => {},
        else => return error.ExpectedMissingRuntime,
    }
    switch (aggregate.acceptSemanticInput(
        connection_id,
        attachment_id,
        1,
        .{ .text = .{ .text = "x" } },
        1024,
    )) {
        .runtime_missing => {},
        else => return error.ExpectedMissingRuntime,
    }
    switch (aggregate.acceptRawInput(connection_id + 1, attachment_id, 1, "x")) {
        .core_rejected => |err| try std.testing.expectEqual(error.MissingConnection, err),
        else => return error.ExpectedCoreRejection,
    }
    _ = try aggregate.detach(connection_id, attachment_id);
    _ = try aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try aggregate.core().ptyDrained(terminal_id);
    try publishPendingOutcome(&aggregate, terminal_id);
    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.disconnect(connection_id);
}
test "discarding prepared input releases the engine-owned queue reservation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;

    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const creation_id = testId(core_module.CreationId, 0xa1);
    const attachment_id = testId(core_module.AttachmentId, 0xa2);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xa0),
    );
    var launch = validMissingExecutableLaunch();
    launch.executable = executable;
    launch.arguments = arguments;
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
        .{ .input = true, .read = true, .resize = false },
        false,
    );
    const runtime = aggregate.runtimeFor(terminal_id).?;

    var input_preparation = aggregate.prepareRawInput(
        connection_id,
        attachment_id,
        1,
        "abc",
    );
    try std.testing.expect(input_preparation.accepted() != null);
    try std.testing.expectEqual(@as(usize, 3), terminal_runtime.testing.inputQueueSnapshot(runtime).client_charge.bytes);
    input_preparation.deinit();
    try std.testing.expectEqual(@as(usize, 0), terminal_runtime.testing.inputQueueSnapshot(runtime).client_charge.bytes);
    const replayed = try aggregate.attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = false },
        false,
    );
    try std.testing.expectEqual(core_module.TransitionOutcome.idempotent, replayed.transition);
    _ = try aggregate.detach(connection_id, attachment_id);
    try std.testing.expectError(
        error.MissingAttachment,
        runtime.setHistoryDemand(attachment_id, .viewing_history),
    );
    _ = aggregate.disconnect(connection_id);
}
test "prepared input rejects a replacement attachment incarnation" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0xa3);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xa4),
    );

    var first_launch_fixture = try validatedLongRunningLaunch();
    defer first_launch_fixture.deinit();
    const first_validated = first_launch_fixture.validated;
    var first_preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0xa5),
        first_validated,
    );
    defer first_preparation.deinit();
    const first_terminal_id = switch (try aggregate.commitPreparedTerminal(&first_preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };

    var second_launch_fixture = try validatedLongRunningLaunch();
    defer second_launch_fixture.deinit();
    const second_validated = second_launch_fixture.validated;
    var second_preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0xa6),
        second_validated,
    );
    defer second_preparation.deinit();
    const second_terminal_id = switch (try aggregate.commitPreparedTerminal(&second_preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };

    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        first_terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );
    const first_runtime = aggregate.runtimeFor(first_terminal_id).?;
    const second_runtime = aggregate.runtimeFor(second_terminal_id).?;

    var semantic_preparation = aggregate.prepareSemanticInput(
        connection_id,
        attachment_id,
        1,
        .{ .text = .{ .text = "same" } },
        std.math.maxInt(usize),
    );
    defer semantic_preparation.deinit();
    try std.testing.expect(semantic_preparation.accepted() != null);
    // ConPTY may enqueue terminal replies independently of this uncommitted
    // client reservation. Inspect the client bucket, not the aggregate queue.
    try std.testing.expectEqual(@as(usize, 4), terminal_runtime.testing.inputQueueSnapshot(first_runtime).client_charge.bytes);
    _ = try aggregate.detach(connection_id, attachment_id);
    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        first_terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );
    switch (semantic_preparation.commit()) {
        .core_rejected => |err| try std.testing.expectEqual(error.MissingResource, err),
        .accepted, .runtime_missing, .runtime_rejected => return error.ExpectedCoreRejection,
    }
    try std.testing.expectEqual(@as(usize, 0), terminal_runtime.testing.inputQueueSnapshot(first_runtime).client_charge.bytes);

    var input_preparation = aggregate.prepareRawInput(
        connection_id,
        attachment_id,
        1,
        "stale",
    );
    defer input_preparation.deinit();
    try std.testing.expect(input_preparation.accepted() != null);
    try std.testing.expectEqual(@as(usize, 5), terminal_runtime.testing.inputQueueSnapshot(first_runtime).client_charge.bytes);

    _ = try aggregate.detach(connection_id, attachment_id);
    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        second_terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );
    switch (input_preparation.commit()) {
        .core_rejected => |err| try std.testing.expectEqual(error.MissingResource, err),
        .accepted, .runtime_missing, .runtime_rejected => return error.ExpectedCoreRejection,
    }
    try std.testing.expectEqual(@as(usize, 0), terminal_runtime.testing.inputQueueSnapshot(first_runtime).client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), terminal_runtime.testing.inputQueueSnapshot(second_runtime).client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), terminal_runtime.testing.inputQueueSnapshot(first_runtime).published_client_entries);
    try std.testing.expectEqual(@as(usize, 0), terminal_runtime.testing.inputQueueSnapshot(second_runtime).published_client_entries);
    try std.testing.expect(
        aggregate.core().findAttachment(attachment_id).?.terminal_id.eql(second_terminal_id),
    );
}
test "unsafe resize failure is retained as an observable disposable terminal" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;

    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const creation_id = testId(core_module.CreationId, 0xb1);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xb0),
    );
    var launch = validMissingExecutableLaunch();
    launch.executable = executable;
    launch.arguments = arguments;
    const validated = try terminal_launch.inspect(std.testing.allocator, launch);
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        creation_id,
        validated,
    );
    defer preparation.deinit();
    const creation = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = switch (creation) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const runtime = aggregate.runtimeFor(terminal_id).?;

    fenceResizeFailure(runtime);
    recordUnsafeResizeFailure(&aggregate, terminal_id, runtime);

    const retained = aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.failed, retained.phase());
    try std.testing.expectEqual(core_module.RuntimeFailureKind.resize, retained.runtimeFailure().?.kind());
    try std.testing.expect(!runtime.readableStateAvailable());
    const pending = retained.pendingOutcome() orelse return error.ExpectedFailedOutcome;
    const token = switch (pending) {
        .failed => |outcome| failed: {
            try std.testing.expect(outcome.terminal_id.eql(terminal_id));
            try std.testing.expectEqual(core_module.RuntimeFailureKind.resize, outcome.failure.kind());
            break :failed outcome.token;
        },
        .exited => return error.ExpectedFailedOutcome,
    };

    try std.testing.expectError(
        error.OutcomePublicationPending,
        aggregate.disposeTerminal(connection_id, terminal_id),
    );
    try std.testing.expect(aggregate.runtimeFor(terminal_id) == runtime);
    try std.testing.expectEqual(@as(usize, 1), aggregate.runtimeCount());
    try std.testing.expect(aggregate.core().findTerminal(terminal_id).?.pendingOutcome() != null);
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        aggregate.core().markOutcomePublished(token),
    );
    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        aggregate.core().markOutcomePublished(token),
    );

    switch (try aggregate.disposeTerminal(connection_id, terminal_id)) {
        .transition => |transition| try std.testing.expectEqual(
            core_module.TransitionOutcome.applied,
            transition,
        ),
        .cleanup_blocked => return error.ExpectedSuccessfulDisposal,
    }
    try std.testing.expect(aggregate.core().findTerminal(terminal_id) == null);
    switch (try aggregate.disposeTerminal(connection_id, terminal_id)) {
        .transition => |transition| try std.testing.expectEqual(
            core_module.TransitionOutcome.idempotent,
            transition,
        ),
        .cleanup_blocked => return error.ExpectedIdempotentDisposal,
    }
    _ = aggregate.disconnect(connection_id);
}
test "attachment transitions preserve idempotency and broker ownership" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const owner_connection_id: core_module.ConnectionId = 1;
    const other_connection_id: core_module.ConnectionId = 2;
    const terminal_id = testId(core_module.TerminalId, 0x52);
    const attachment_id = testId(core_module.AttachmentId, 0x53);
    _ = try aggregate.core().connect(
        owner_connection_id,
        testId(core_module.ClientId, 0x50),
    );
    _ = try createModel(
        aggregate.core(),
        owner_connection_id,
        testId(core_module.CreationId, 0x54),
        terminal_id,
        [_]u8{0x55} ** 32,
        80,
        24,
    );
    const capabilities: core_module.Capabilities = .{
        .input = true,
        .read = false,
        .resize = false,
    };

    const attached_transition = try aggregate.core().attach(
        owner_connection_id,
        attachment_id,
        terminal_id,
        capabilities,
        false,
    );
    try std.testing.expectEqual(core_module.TransitionOutcome.applied, attached_transition);
    const attached = aggregate.core().findAttachment(attachment_id).?;
    try std.testing.expect(attached.attachment_id.eql(attachment_id));
    try std.testing.expect(attached.terminal_id.eql(terminal_id));
    try std.testing.expect(std.meta.eql(capabilities, attached.capabilities));

    const replayed = try aggregate.core().attach(
        owner_connection_id,
        attachment_id,
        terminal_id,
        capabilities,
        false,
    );
    try std.testing.expectEqual(core_module.TransitionOutcome.idempotent, replayed);
    try std.testing.expectError(
        error.MissingConnection,
        aggregate.detach(other_connection_id, attachment_id),
    );
    try std.testing.expect(aggregate.core().findAttachment(attachment_id) != null);
    const detached = try aggregate.detach(owner_connection_id, attachment_id);
    try std.testing.expectEqual(core_module.TransitionOutcome.applied, detached.outcome);
    try std.testing.expect(detached.resize_lease_change == null);
    const replayed_detach = try aggregate.detach(owner_connection_id, attachment_id);
    try std.testing.expectEqual(core_module.TransitionOutcome.idempotent, replayed_detach.outcome);
    try std.testing.expect(replayed_detach.resize_lease_change == null);

    _ = aggregate.disconnect(owner_connection_id);
    _ = aggregate.disconnect(other_connection_id);
}
test "disconnect removes all broker attachments without removing terminals" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const first_connection_id: core_module.ConnectionId = 1;
    const second_connection_id: core_module.ConnectionId = 2;
    const terminal_id = testId(core_module.TerminalId, 0x62);
    const first_attachment_id = testId(core_module.AttachmentId, 0x63);
    const second_attachment_id = testId(core_module.AttachmentId, 0x64);
    _ = try aggregate.core().connect(
        first_connection_id,
        testId(core_module.ClientId, 0x60),
    );
    _ = try createModel(
        aggregate.core(),
        first_connection_id,
        testId(core_module.CreationId, 0x65),
        terminal_id,
        [_]u8{0x66} ** 32,
        80,
        24,
    );
    const capabilities: core_module.Capabilities = .{
        .input = false,
        .read = true,
        .resize = false,
    };
    _ = try aggregate.core().attach(
        first_connection_id,
        first_attachment_id,
        terminal_id,
        capabilities,
        false,
    );
    _ = try aggregate.core().attach(
        first_connection_id,
        second_attachment_id,
        terminal_id,
        capabilities,
        false,
    );

    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        aggregate.disconnect(first_connection_id),
    );
    try std.testing.expect(aggregate.core().findAttachment(first_attachment_id) == null);
    try std.testing.expect(aggregate.core().findAttachment(second_attachment_id) == null);
    try std.testing.expect(aggregate.core().findTerminal(terminal_id) != null);

    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        aggregate.disconnect(second_connection_id),
    );
}
test "disconnect teardown is linear for many input-only attachments" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x40),
    );
    const terminal_id = testId(core_module.TerminalId, 0x41);
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x42),
        terminal_id,
        [_]u8{0x43} ** 32,
        80,
        24,
    );
    const attachment_count = 256;
    for (0..attachment_count) |index| {
        _ = try aggregate.core().attach(
            connection_id,
            testId(core_module.AttachmentId, @intCast(index)),
            terminal_id,
            .{ .input = true, .read = false, .resize = false },
            false,
        );
    }
    try std.testing.expectEqual(attachment_count, aggregate.core().attachmentCount());

    core_module.testing.resetActiveReadDemandScans(aggregate.core());
    subject.testing.resetProjectionReleaseDecisions(&aggregate);
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        aggregate.disconnect(connection_id),
    );

    // Every active-demand query is O(1) and examines zero attachment entries.
    // The pre-fix walk examined 255+254+...+0 = 32,640 entries.
    try std.testing.expectEqual(
        @as(usize, 0),
        core_module.testing.activeReadDemandScans(aggregate.core()),
    );
    // Projection resources are released at most once per affected terminal.
    try std.testing.expect(
        subject.testing.projectionReleaseDecisions(&aggregate) <= 1,
    );
    try std.testing.expectEqual(@as(usize, 0), aggregate.core().attachmentCount());
    try std.testing.expect(aggregate.core().findTerminal(terminal_id) != null);
    try std.testing.expectEqual(@as(usize, 0), aggregate.core().connectionCount());
}
