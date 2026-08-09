const std = @import("std");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const service_boundary = @import("terminal-host-service");

const core_module = core_boundary.state;
const input_admission = service_boundary.testing.input_admission;
const subject = service_boundary.testing.runtime_aggregate;
const terminal_launch = engine_boundary.launch;
const terminal_runtime = engine_boundary.runtime;

const fenceResizeFailure = terminal_runtime.testing.fenceResizeFailure;
const recordReaderFailure = terminal_runtime.testing.recordReaderFailureFn;
const recordUnsafeResizeFailure = subject.testing.recordUnsafeResizeFailureFn;

const features = [_]u16{22};
const palette = [_]u32{0} ** 256;

fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
}

fn publishPendingOutcome(
    aggregate: *subject.HostRuntimeAggregate,
    terminal_id: core_module.TerminalId,
) !void {
    const terminal = aggregate.core().findTerminal(terminal_id) orelse
        return error.ExpectedTerminal;
    const pending = terminal.pendingOutcome() orelse return error.ExpectedPendingOutcome;
    const token = switch (pending) {
        .exited => |outcome| outcome.token,
        .failed => |outcome| outcome.token,
    };
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        aggregate.core().markOutcomePublished(token),
    );
    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        aggregate.core().markOutcomePublished(token),
    );
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
        input_admission.Rejection.encoding_failed,
        input_admission.fromRuntimeError(error.WriteFailed),
    );
}

fn validMissingExecutableLaunch() terminal_launch.Launch {
    return .{
        .arguments = &.{},
        .configuration = null,
        .current_directory = null,
        .environment = &.{},
        .executable = "orchbee-terminal-host-deliberately-missing-executable",
        .geometry = .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 80,
            .rows = 24,
        },
        .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &features,
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &palette,
            .version_response = "Orchbee test",
        },
    };
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
        .{},
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
    _ = try aggregate.core().createTerminal(
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
        .{},
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
    _ = try aggregate.core().createTerminal(
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
    _ = try aggregate.core().createTerminal(
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
                    .runtime_rejected => |rejection| rejection == .terminal_runtime_unavailable,
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
        .{},
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
        .{},
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
    _ = try aggregate.core().createTerminal(
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
    _ = try aggregate.core().createTerminal(
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
    _ = try aggregate.core().createTerminal(
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
    _ = try aggregate.core().createTerminal(
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

    switch (aggregate.acceptRawInput(connection_id, attachment_id, "x")) {
        .runtime_missing => {},
        else => return error.ExpectedMissingRuntime,
    }
    switch (aggregate.acceptSemanticInput(
        connection_id,
        attachment_id,
        .{ .text = .{ .text = "x" } },
        1024,
    )) {
        .runtime_missing => {},
        else => return error.ExpectedMissingRuntime,
    }
    switch (aggregate.acceptRawInput(connection_id + 1, attachment_id, "x")) {
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
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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
        .{},
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
        "abc",
    );
    try std.testing.expect(input_preparation.accepted() != null);
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    input_preparation.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
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

test "unsafe resize failure is retained as an observable disposable terminal" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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
        .{},
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
    _ = try aggregate.core().createTerminal(
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
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        try aggregate.detach(owner_connection_id, attachment_id),
    );
    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        try aggregate.detach(owner_connection_id, attachment_id),
    );

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
    _ = try aggregate.core().createTerminal(
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

        pub fn takeLastOwnedRuntime(self: *@This()) ?*FakeRuntime {
            self.count -= 1;
            const engine = self.engines[self.count];
            self.engines[self.count] = null;
            return engine;
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
    _ = try aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, 0x94),
        first_terminal_id,
        [_]u8{0x95} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().createTerminal(
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
