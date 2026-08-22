pub const std = @import("std");
pub const core_boundary = @import("terminal-host-core");
pub const engine_boundary = @import("terminal-host-engine");
pub const service_boundary = @import("terminal-host-service");

pub const core_module = core_boundary.state;
pub const input_admission = service_boundary.testing.input_admission;
pub const pty_process = @import("terminal-host-platform").process;
pub const subject = service_boundary.testing.runtime_aggregate;
pub const terminal_launch = engine_boundary.launch;
pub const terminal_runtime = engine_boundary.runtime;

pub const commitReaderBatch = terminal_runtime.testing.commitReaderBatchFn;
pub const createModel = core_module.testing.createTerminal;
pub const fenceResizeFailure = terminal_runtime.testing.fenceResizeFailure;
pub const recordReaderFailure = terminal_runtime.testing.recordReaderFailureFn;
pub const recordUnsafeResizeFailure = subject.testing.recordUnsafeResizeFailureFn;

// Must match `cleanup_retry_inspection_budget` in
// src/service/runtime_maintenance/. The cursor-advance assertions below
// validate the bounded per-pass scan; a drift in either constant fails them.
pub const cleanup_retry_inspection_budget = 16;

pub const features = [_]u16{22};
pub const palette = [_]u32{0} ** 256;

pub fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
}

pub fn publishPendingOutcome(
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

pub fn validMissingExecutableLaunch() terminal_launch.Launch {
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

const immediate_exit_posix_arguments = [_][]const u8{ "-c", "exit 0" };
const immediate_exit_windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "exit /B 0" };
const long_running_posix_arguments = [_][]const u8{};
const long_running_windows_arguments = [_][]const u8{ "/D", "/S", "/C", "more" };
const long_running_process_posix_arguments = [_][]const u8{ "-c", "sleep 30" };
const long_running_process_windows_arguments = [_][]const u8{
    "/D",
    "/S",
    "/C",
    "ping -n 31 127.0.0.1 >nul",
};

pub const ValidatedLaunchFixture = struct {
    allocator: std.mem.Allocator,
    validated: terminal_launch.Validated,
    windows_executable: ?[]u8,

    pub fn deinit(self: *@This()) void {
        if (self.windows_executable) |value| self.allocator.free(value);
        self.* = undefined;
    }
};

fn validatedLaunchFixture(
    posix_executable: []const u8,
    posix_arguments: []const []const u8,
    windows_arguments: []const []const u8,
) !ValidatedLaunchFixture {
    const builtin = @import("builtin");
    const allocator = std.testing.allocator;
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(allocator, "ComSpec")
    else
        null;
    errdefer if (windows_executable) |value| allocator.free(value);
    const executable = if (windows_executable) |value| value else posix_executable;
    const arguments = if (builtin.os.tag == .windows)
        windows_arguments
    else
        posix_arguments;
    var launch = validMissingExecutableLaunch();
    launch.executable = executable;
    launch.arguments = arguments;
    return .{
        .allocator = allocator,
        .validated = try terminal_launch.inspect(allocator, launch),
        .windows_executable = windows_executable,
    };
}

/// THCR-036 native launch that exits immediately with status zero. The
/// resulting engine publishes a normal completion without any HostService
/// interaction.
pub fn validatedImmediateExitLaunch() !ValidatedLaunchFixture {
    return validatedLaunchFixture(
        "/bin/sh",
        &immediate_exit_posix_arguments,
        &immediate_exit_windows_arguments,
    );
}

/// THCR-036 native launch that keeps running until containment stops it, so a
/// deterministic engine failure can be published before the child exits.
pub fn validatedLongRunningLaunch() !ValidatedLaunchFixture {
    return validatedLaunchFixture(
        "/bin/cat",
        &long_running_posix_arguments,
        &long_running_windows_arguments,
    );
}

/// THCR-036 setup: 64 model-only filler sessions followed by one native
/// target. Registry keys are insertion-ordered, so the target sits at index 64
/// and stays outside every 64-entry fair background slice while the cursor
/// starts at zero.
pub fn createSkippedTarget(
    aggregate: *subject.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    validated: terminal_launch.Validated,
) !core_module.TerminalId {
    for (0..64) |offset| {
        _ = try createModel(
            aggregate.core(),
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
    );
    defer preparation.deinit();
    return switch (try aggregate.commitPreparedTerminal(&preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => error.ExpectedCreatedTerminal,
    };
}

pub const CountingTerminationDispatcher = struct {
    calls: usize = 0,

    pub fn dispatch(
        self: *@This(),
        _: *terminal_runtime.TerminalRuntime,
        _: subject.TerminationMode,
    ) !void {
        self.calls += 1;
    }
};

pub const FailingTerminationDispatcher = struct {
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

pub const SkippedTargetObservingFencer = struct {
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

pub const SkippedTargetObservingContainment = struct {
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

// THCR-036 regression: the bounded background scan visits at most 64 registry
// positions per poll, so a target past that slice can publish a normal engine
// completion that Core has not yet linearized. Graceful and force termination
// used to evaluate the stale running lifecycle, dispatch a platform effect,
// and commit a termination intent. The exact-terminal synchronization pass
// must copy the already-published normal classification first, so every
// request is replay-only, Core is exited immediately, and no dispatch occurs.

// THCR-036 regression: an engine failure published on a skipped target must be
// copied into Core by the exact pass before any dispatch decision. The failing
// termination dispatcher must never be reached, and dispatch_termination can
// never become the primary outcome.

// THCR-041 regression: a Windows child that terminates with exit status 259
// (STILL_ACTIVE's numeric value) must synchronize through the service
// lifecycle into exactly one TerminalExited outcome carrying code 259, with no
// TerminalFailed observe-process/unavailable outcome. Native Windows execution
// required; compiled for `x86_64-windows-gnu` on non-Windows.

// THCR-026 regression: disconnect used to re-scan the shrinking per-terminal
// attachment list after every removal, so N input-only attachments on one
// terminal caused N(N-1)/2 active-demand list visits and N projection-release
// decisions. The repair keeps the demand query O(1) and performs a single-pass
// teardown that releases projection resources at most once per terminal.

// THDD-001: a retained runtime whose process tree is not confirmed contained
// must keep its cleanup ownership after a failed fatal-teardown pass. The
// probe injects session-observation/signaling failures into the engine's
// bounded containment attempt; when it stops failing it force-terminates the
// real child session so the subsequent identity release completes.
pub const RetainedCleanupProbe = struct {
    failures_remaining: usize,
    contains: usize = 0,
    runtime: ?*terminal_runtime.TerminalRuntime = null,

    pub fn fromContext(context: *anyopaque) *RetainedCleanupProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn contain(context: *anyopaque) anyerror!void {
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

    pub fn hooks(self: *RetainedCleanupProbe) terminal_runtime.testing.RuntimeTestingHooksType {
        return .{
            .contain_process_tree = contain,
            .context = self,
        };
    }
};

pub const HookedThreadSpawner = struct {
    hooks: terminal_runtime.testing.RuntimeTestingHooksType,

    pub fn terminalRuntimeTestingHooks(self: *HookedThreadSpawner) terminal_runtime.testing.RuntimeTestingHooksType {
        return self.hooks;
    }

    pub fn spawn(
        _: *HookedThreadSpawner,
        comptime thread_function: anytype,
        runtime: anytype,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

pub const ProcessLaunchFixture = struct {
    allocator: std.mem.Allocator,
    launch: pty_process.Launch,
    windows_executable: ?[]u8,

    pub fn deinit(self: *@This()) void {
        if (self.windows_executable) |value| self.allocator.free(value);
        self.* = undefined;
    }
};

pub fn longRunningProcessLaunch() !ProcessLaunchFixture {
    const builtin = @import("builtin");
    const allocator = std.testing.allocator;
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(allocator, "ComSpec")
    else
        null;
    errdefer if (windows_executable) |value| allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &long_running_process_windows_arguments
    else
        &long_running_process_posix_arguments;
    return .{
        .allocator = allocator,
        .launch = .{
            .arguments = arguments,
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = executable,
            .rows = 24,
        },
        .windows_executable = windows_executable,
    };
}

test "launch fixtures retain every borrowed Windows value until deinit" {
    const builtin = @import("builtin");

    var validated_fixture = try validatedLongRunningLaunch();
    defer validated_fixture.deinit();
    const expected_validated_arguments = if (builtin.os.tag == .windows)
        &long_running_windows_arguments
    else
        &long_running_posix_arguments;
    try std.testing.expect(
        validated_fixture.validated.launch.arguments.ptr == expected_validated_arguments.ptr,
    );

    var process_fixture = try longRunningProcessLaunch();
    defer process_fixture.deinit();
    const expected_process_arguments = if (builtin.os.tag == .windows)
        &long_running_process_windows_arguments
    else
        &long_running_process_posix_arguments;
    try std.testing.expect(
        process_fixture.launch.arguments.ptr == expected_process_arguments.ptr,
    );

    if (builtin.os.tag == .windows) {
        const validated_executable = validated_fixture.windows_executable orelse
            return error.ExpectedOwnedWindowsExecutable;
        try std.testing.expect(
            validated_fixture.validated.launch.executable.ptr == validated_executable.ptr,
        );
        const process_executable = process_fixture.windows_executable orelse
            return error.ExpectedOwnedWindowsExecutable;
        try std.testing.expect(
            process_fixture.launch.executable.ptr == process_executable.ptr,
        );
    } else {
        try std.testing.expectEqual(@as(?[]u8, null), validated_fixture.windows_executable);
        try std.testing.expectEqual(@as(?[]u8, null), process_fixture.windows_executable);
    }
}

/// POSIX launch whose root exits normally after starting a descendant that
/// closes every PTY descriptor but remains in the retained session.
const normal_exit_with_retained_descendant_arguments = [_][]const u8{
    "-c",
    "trap '' HUP; sleep 30 </dev/null >/dev/null 2>&1 & exit 0",
};

pub fn normalExitWithRetainedDescendantProcessLaunch() pty_process.Launch {
    return .{
        .arguments = &normal_exit_with_retained_descendant_arguments,
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    };
}

pub fn commitHookedRuntime(
    aggregate: *subject.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    probe: *RetainedCleanupProbe,
) !*terminal_runtime.TerminalRuntime {
    var process_launch = try longRunningProcessLaunch();
    defer process_launch.deinit();
    return commitHookedRuntimeWithLaunch(
        aggregate,
        connection_id,
        terminal_id,
        probe,
        process_launch.launch,
    );
}

pub fn commitHookedRuntimeWithLaunch(
    aggregate: *subject.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    probe: *RetainedCleanupProbe,
    process_launch: pty_process.Launch,
) !*terminal_runtime.TerminalRuntime {
    var spawner: HookedThreadSpawner = .{ .hooks = probe.hooks() };
    const runtime = try terminal_runtime.testing.createWithOptionsUsingSpawnerFn(
        std.testing.allocator,
        process_launch,
        .{ .cell_pixel_height = 16, .cell_pixel_width = 8 },
        &spawner,
    );
    probe.runtime = runtime;
    try aggregate.core().reserveTerminal();
    try subject.testing.adoptRunningTerminalFn(
        aggregate,
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
pub const LogicalCleanupClock = struct {
    now_ns: i128 = 0,
};

pub fn readLogicalCleanupNs(clock: *subject.testing.CleanupClockType) i128 {
    return @as(*LogicalCleanupClock, @ptrCast(@alignCast(clock.context.?))).now_ns;
}

// THDD-001 regression (base RED): a fatal-teardown pass whose containment
// window fails must NOT abandon the unconfirmed process tree after one failed
// pass. The terminal is retained with an explicit blocked-cleanup containment
// (retained cleanup ownership), so the bounded cleanup-retry scheduler can
// retry it instead of silently orphaning the tree.

// THDD-001 deterministic evidence: fatal teardown retains the cleanup
// ownership for an unconfirmed process tree, retries containment on the bounded
// backoff schedule (driven by the controllable cleanup-retry clock), and lets
// the release owner release only after a later successful containment
// observation completes cleanup.

// THDD-001 boundary evidence: after the bounded retry budget is exhausted the
// tree is never silently orphaned and never released while unconfirmed; it is
// in an explicitly retained/observable failed-cleanup state.

// THDD-001 observability evidence: the retained-cleanup state is surfaced as
// an observable status (active, attempts, exhausted) across the window.

// THCR-027 regression: DisposeTerminal on an already-blocked cleanup phase
// flattened the exact RuntimeCleanupFailure into a payload-free
// FailureContainmentIncomplete error, so the aggregate could never produce
// DisposeResult.cleanup_blocked for a phase blocked before this request. Each
// retained variant must round-trip through disposal unchanged and leave the
// blocked terminal and its attachments fully retained for the independent
// maintenance retry.
