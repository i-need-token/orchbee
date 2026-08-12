const std = @import("std");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const service_boundary = @import("terminal-host-service");

const core_module = core_boundary.state;
const input_admission = service_boundary.testing.input_admission;
const pty_process = @import("terminal-host-platform").process;
const subject = service_boundary.testing.runtime_aggregate;
const terminal_launch = engine_boundary.launch;
const terminal_runtime = engine_boundary.runtime;

const commitReaderBatch = terminal_runtime.testing.commitReaderBatchFn;
const fenceResizeFailure = terminal_runtime.testing.fenceResizeFailure;
const recordReaderFailure = terminal_runtime.testing.recordReaderFailureFn;
const recordUnsafeResizeFailure = subject.testing.recordUnsafeResizeFailureFn;

// Must match `cleanup_retry_inspection_budget` in
// src/service/runtime_maintenance.zig. The cursor-advance assertions below
// validate the bounded per-pass scan; a drift in either constant fails them.
const cleanup_retry_inspection_budget = 16;

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

/// THCR-036 native launch that exits immediately with status zero. The
/// resulting engine publishes a normal completion without any HostService
/// interaction.
fn validatedImmediateExitLaunch() !terminal_launch.Validated {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "exit 0" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "exit /B 0" };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    var launch = validMissingExecutableLaunch();
    launch.executable = executable;
    launch.arguments = arguments;
    return terminal_launch.inspect(std.testing.allocator, launch);
}

/// THCR-036 native launch that keeps running until containment stops it, so a
/// deterministic engine failure can be published before the child exits.
fn validatedLongRunningLaunch() !terminal_launch.Validated {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/cat";
    const posix_arguments = [_][]const u8{};
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "more" };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    var launch = validMissingExecutableLaunch();
    launch.executable = executable;
    launch.arguments = arguments;
    return terminal_launch.inspect(std.testing.allocator, launch);
}

/// THCR-036 setup: 64 model-only filler sessions followed by one native
/// target. Registry keys are insertion-ordered, so the target sits at index 64
/// and stays outside every 64-entry fair background slice while the cursor
/// starts at zero.
fn createSkippedTarget(
    aggregate: *subject.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    validated: terminal_launch.Validated,
) !core_module.TerminalId {
    for (0..64) |offset| {
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, @intCast(0x10 + offset)),
            testId(core_module.TerminalId, @intCast(0x50 + offset)),
            [_]u8{0x80} ** 32,
            80,
            24,
        );
    }
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x90),
        validated,
        .{},
    );
    defer preparation.deinit();
    return switch (try aggregate.commitPreparedTerminal(&preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => error.ExpectedCreatedTerminal,
    };
}

const CountingTerminationDispatcher = struct {
    calls: usize = 0,

    pub fn dispatch(
        self: *@This(),
        _: *terminal_runtime.TerminalRuntime,
        _: subject.TerminationMode,
    ) !void {
        self.calls += 1;
    }
};

const FailingTerminationDispatcher = struct {
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

const SkippedTargetObservingFencer = struct {
    calls: usize = 0,
    result: ?terminal_runtime.HostFailureFenceResult = null,

    pub fn fence(
        self: *@This(),
        runtime: *terminal_runtime.TerminalRuntime,
    ) terminal_runtime.HostFailureFenceResult {
        self.calls += 1;
        const result = runtime.fenceHostFailure();
        self.result = result;
        return result;
    }
};

const SkippedTargetObservingContainment = struct {
    calls: usize = 0,

    pub fn contain(
        self: *@This(),
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !void {
        self.calls += 1;
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

// THCR-036 regression: the bounded background scan visits at most 64 registry
// positions per poll, so a target past that slice can publish a normal engine
// completion that Core has not yet linearized. Graceful and force termination
// used to evaluate the stale running lifecycle, dispatch a platform effect,
// and commit a termination intent. The exact-terminal synchronization pass
// must copy the already-published normal classification first, so every
// request is replay-only, Core is exited immediately, and no dispatch occurs.
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

// THCR-036 regression: an engine failure published on a skipped target must be
// copied into Core by the exact pass before any dispatch decision. The failing
// termination dispatcher must never be reached, and dispatch_termination can
// never become the primary outcome.
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
        .{},
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
        .{},
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
        .{},
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

// THCR-041 regression: a Windows child that terminates with exit status 259
// (STILL_ACTIVE's numeric value) must synchronize through the service
// lifecycle into exactly one TerminalExited outcome carrying code 259, with no
// TerminalFailed observe-process/unavailable outcome. Native Windows execution
// required; compiled for `x86_64-windows-gnu` on non-Windows.
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
        .{},
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

// THCR-026 regression: disconnect used to re-scan the shrinking per-terminal
// attachment list after every removal, so N input-only attachments on one
// terminal caused N(N-1)/2 active-demand list visits and N projection-release
// decisions. The repair keeps the demand query O(1) and performs a single-pass
// teardown that releases projection resources at most once per terminal.
test "disconnect teardown is linear for many input-only attachments" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x40),
    );
    const terminal_id = testId(core_module.TerminalId, 0x41);
    _ = try aggregate.core().createTerminal(
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

// THDD-001: a retained runtime whose process tree is not confirmed contained
// must keep its cleanup ownership after a failed fatal-teardown pass. The
// probe injects session-observation/signaling failures into the engine's
// bounded containment attempt; when it stops failing it force-terminates the
// real child session so the subsequent identity release completes.
const RetainedCleanupProbe = struct {
    failures_remaining: usize,
    contains: usize = 0,
    runtime: ?*terminal_runtime.TerminalRuntime = null,

    fn fromContext(context: *anyopaque) *RetainedCleanupProbe {
        return @ptrCast(@alignCast(context));
    }

    fn contain(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.contains += 1;
        if (self.failures_remaining > 0) {
            self.failures_remaining -= 1;
            return error.InjectedSessionObservationFailure;
        }
        // Successful containment: actually terminate the real child session so
        // the subsequent process-identity release completes deterministically.
        try self.runtime.?.forceTerminate();
    }

    fn hooks(self: *RetainedCleanupProbe) terminal_runtime.testing.RuntimeTestingHooksType {
        return .{
            .contain_process_tree = contain,
            .context = self,
        };
    }
};

const HookedThreadSpawner = struct {
    hooks: terminal_runtime.testing.RuntimeTestingHooksType,

    pub fn terminalRuntimeTestingHooks(self: *HookedThreadSpawner) terminal_runtime.testing.RuntimeTestingHooksType {
        return self.hooks;
    }

    pub fn spawn(
        _: *HookedThreadSpawner,
        comptime thread_function: fn (*terminal_runtime.TerminalRuntime) void,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

fn longRunningProcessLaunch() !pty_process.Launch {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    return .{
        .arguments = arguments,
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    };
}

fn commitHookedRuntime(
    aggregate: *subject.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    probe: *RetainedCleanupProbe,
) !*terminal_runtime.TerminalRuntime {
    const process_launch = try longRunningProcessLaunch();
    var spawner: HookedThreadSpawner = .{ .hooks = probe.hooks() };
    const runtime = try terminal_runtime.testing.createWithOptionsUsingSpawnerFn(
        std.testing.allocator,
        process_launch,
        .{ .cell_pixel_height = 16, .cell_pixel_width = 8 },
        &spawner,
    );
    probe.runtime = runtime;
    try aggregate.core().reserveTerminal();
    aggregate.core().commitRunningTerminal(
        connection_id,
        testId(core_module.CreationId, 0x42),
        terminal_id,
        [_]u8{0x43} ** 32,
        80,
        24,
        8,
        16,
        .{},
        runtime,
    );
    return runtime;
}

// THDD-001 controllable logical clock for the maintenance cleanup scheduler.
// Production samples real monotonic time; tests substitute this so the bounded
// cleanup-retry deadline can be advanced deterministically without sleeping.
const LogicalCleanupClock = struct {
    now_ns: i128 = 0,
};

fn readLogicalCleanupNs(clock: *subject.testing.CleanupClockType) i128 {
    return @as(*LogicalCleanupClock, @ptrCast(@alignCast(clock.context.?))).now_ns;
}

// THDD-001 regression (base RED): a fatal-teardown pass whose containment
// window fails must NOT abandon the unconfirmed process tree after one failed
// pass. The terminal is retained with an explicit blocked-cleanup containment
// (retained cleanup ownership), so the bounded cleanup-retry scheduler can
// retry it instead of silently orphaning the tree.
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

// THDD-001 deterministic evidence: fatal teardown retains the cleanup
// ownership for an unconfirmed process tree, retries containment on the bounded
// backoff schedule (driven by the controllable cleanup-retry clock), and lets
// the release owner release only after a later successful containment
// observation completes cleanup.
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

// THDD-001 boundary evidence: after the bounded retry budget is exhausted the
// tree is never silently orphaned and never released while unconfirmed; it is
// in an explicitly retained/observable failed-cleanup state.
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

// THDD-001 observability evidence: the retained-cleanup state is surfaced as
// an observable status (active, attempts, exhausted) across the window.
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

// THCR-027 regression: DisposeTerminal on an already-blocked cleanup phase
// flattened the exact RuntimeCleanupFailure into a payload-free
// FailureContainmentIncomplete error, so the aggregate could never produce
// DisposeResult.cleanup_blocked for a phase blocked before this request. Each
// retained variant must round-trip through disposal unchanged and leave the
// blocked terminal and its attachments fully retained for the independent
// maintenance retry.
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
        _ = try aggregate.core().createTerminal(
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

test "cleanup retry deadlines live in a monotonic scheduling domain" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0xa1),
    );
    const terminal_id = testId(core_module.TerminalId, 0xa2);
    _ = try aggregate.core().createTerminal(
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
    _ = try aggregate.core().createTerminal(
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
    _ = try aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, 0xac),
        first_terminal_id,
        [_]u8{0xad} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().createTerminal(
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
        _ = try aggregate.core().createTerminal(
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
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, @intCast(0xc0 + offset)),
            testId(core_module.TerminalId, @intCast(0xd0 + offset)),
            [_]u8{0xe0} ** 32,
            80,
            24,
        );
    }
    const due_terminal_id = testId(core_module.TerminalId, 0xf0);
    _ = try aggregate.core().createTerminal(
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
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, @intCast(0x10 + offset)),
            testId(core_module.TerminalId, @intCast(0x30 + offset)),
            [_]u8{0x40} ** 32,
            80,
            24,
        );
    }
    const first_due_id = testId(core_module.TerminalId, 0x50);
    _ = try aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, 0x51),
        first_due_id,
        [_]u8{0x52} ** 32,
        80,
        24,
    );
    for (0..cleanup_retry_inspection_budget) |offset| {
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, @intCast(0x20 + offset)),
            testId(core_module.TerminalId, @intCast(0x60 + offset)),
            [_]u8{0x70} ** 32,
            80,
            24,
        );
    }
    const second_due_id = testId(core_module.TerminalId, 0x80);
    _ = try aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, 0x81),
        second_due_id,
        [_]u8{0x82} ** 32,
        80,
        24,
    );
    for (0..cleanup_retry_inspection_budget) |offset| {
        _ = try aggregate.core().createTerminal(
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
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, @intCast(0xd0 + offset)),
            terminal_id,
            [_]u8{0xe0} ** 32,
            80,
            24,
        );
    }
    const due_terminal_id = testId(core_module.TerminalId, 0xf0);
    _ = try aggregate.core().createTerminal(
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
