const std = @import("std");
const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_launch = terminal_engine.launch;
const terminal_runtime = terminal_engine.runtime;
const host_retained_terminal = @import("../retained_terminal.zig");
const contracts = @import("contracts.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const CreateBuild = contracts.CreateBuild;
const CreateCommitError = contracts.CreateCommitError;
const CreateOutcome = contracts.CreateOutcome;
const CreatePreparation = contracts.CreatePreparation;
const CreatePrepareError = contracts.CreatePrepareError;
const TerminalIdentity = contracts.TerminalIdentity;

pub fn prepareCreateTerminal(
    self: anytype,
    connection_id: core_module.ConnectionId,
    creation_id: core_module.CreationId,
    validated_launch: terminal_launch.Validated,
) CreatePrepareError!CreatePreparation {
    // The next creation on this connection always empties the staged-output slot.
    var seed = self.staged_output.take(self.allocator, connection_id, creation_id);
    errdefer if (seed) |bytes| self.allocator.free(bytes);
    const digest = validated_launch.launch_digest;
    if (try self.core_state.resolveCreationWithOverlay(
        connection_id,
        creation_id,
        digest,
        if (validated_launch.environment_overlay.entries.len == 0)
            null
        else
            validated_launch.environment_overlay.digest,
    )) |terminal_id| {
        const terminal = self.core_state.findTerminal(terminal_id) orelse unreachable;
        if (seed) |bytes| self.allocator.free(bytes);
        return .{ .replayed = .{
            .creation_id = terminal.creation_id,
            .terminal_id = terminal_id,
        } };
    }

    var prepared_launch = terminal_launch.prepareRuntime(
        validated_launch,
        self.allocator,
        self.core_state.terminalDefaults(),
    ) catch return error.LaunchPreparationOutOfMemory;
    errdefer prepared_launch.deinit(self.allocator);
    prepared_launch.output_seed = seed;
    seed = null;

    try self.core_state.reserveTerminal();

    var terminal_id = identifiers.TerminalId.random();
    while (self.core_state.findTerminal(terminal_id) != null) {
        terminal_id = identifiers.TerminalId.random();
    }

    return .{ .pending = .{
        .allocator = self.allocator,
        .connection_id = connection_id,
        .creation_id = creation_id,
        .environment_overlay_digest = validated_launch.environment_overlay.digest,
        .initial_geometry = validated_launch.launch.geometry,
        .launch_digest = digest,
        .runtime_preparation = prepared_launch,
        .terminal_id = terminal_id,
    } };
}

/// Commits a creation without geometry as an awaiting terminal; it builds nothing.
pub fn commitAwaitingStartTerminal(
    self: anytype,
    preparation: *CreatePreparation,
) error{RuntimeAllocationOutOfMemory}!TerminalIdentity {
    const pending = &preparation.pending;
    std.debug.assert(pending.initial_geometry == null);
    const retained = self.allocator.create(RetainedTerminal) catch
        return error.RuntimeAllocationOutOfMemory;
    retained.* = .{
        .runtime = null,
        .session = undefined,
        .start_preparation = pending.runtime_preparation,
    };
    self.core_state.commitAwaitingStartTerminalWithOverlay(
        pending.connection_id,
        pending.creation_id,
        pending.terminal_id,
        pending.launch_digest,
        pending.environment_overlay_digest,
        retained.start_preparation.?.configuration,
        retained.start_preparation.?.themeDefaults(),
        &retained.session,
    );
    const identity: TerminalIdentity = .{
        .creation_id = pending.creation_id,
        .terminal_id = pending.terminal_id,
    };
    preparation.* = .consumed;
    return identity;
}

/// Takes a creation with geometry out of its preparation for a background build. On error the
/// preparation is unchanged.
pub fn beginCreateBuild(
    self: anytype,
    preparation: *CreatePreparation,
) error{RuntimeAllocationOutOfMemory}!CreateBuild {
    const pending = preparation.pending;
    const retained = self.allocator.create(RetainedTerminal) catch
        return error.RuntimeAllocationOutOfMemory;
    preparation.* = .consumed;
    return .{ .geometry = pending.initial_geometry.?, .pending = pending, .retained = retained };
}

/// Commits a creation whose build returned `runtime`. Every request that could change the
/// admission facts checked at prepare waited for this creation, so the commit stays infallible;
/// the requester may have disconnected meanwhile, which the retained terminal outlives.
pub fn commitCreateBuild(
    self: anytype,
    build: *CreateBuild,
    runtime: *terminal_runtime.TerminalRuntime,
) TerminalIdentity {
    const pending = &build.pending;
    build.retained.* = .{
        .runtime = runtime,
        .session = undefined,
        .start_preparation = null,
    };
    self.core_state.commitRunningTerminalWithOverlay(
        pending.connection_id,
        pending.creation_id,
        pending.terminal_id,
        pending.launch_digest,
        pending.environment_overlay_digest,
        build.geometry.columns,
        build.geometry.rows,
        build.geometry.cell_pixel_width,
        build.geometry.cell_pixel_height,
        pending.runtime_preparation.configuration,
        pending.runtime_preparation.themeDefaults(),
        &build.retained.session,
    );
    pending.runtime_preparation.deinit(self.allocator);
    return build.identity();
}

/// Releases what a failed or cancelled creation build owns. Its runtime, if the build made one,
/// is the caller's to shut down first.
pub fn abandonCreateBuild(self: anytype, build: *CreateBuild) void {
    self.allocator.destroy(build.retained);
    build.pending.runtime_preparation.deinit(self.allocator);
    build.* = undefined;
}

/// The synchronous composition of the creation stages that aggregate tests drive directly. A
/// failed build leaves the preparation pending, as it was.
pub fn commitPreparedTerminal(
    self: anytype,
    preparation: *CreatePreparation,
) CreateCommitError!CreateOutcome {
    if (comptime !target_builtin.is_test) {
        @compileError("synchronous creation is available only to service tests");
    }
    switch (preparation.*) {
        .replayed => |identity| return .{ .replayed = identity },
        .consumed => unreachable,
        .pending => |pending| {
            if (pending.initial_geometry == null) {
                return .{ .created = try commitAwaitingStartTerminal(self, preparation) };
            }
            var build = try beginCreateBuild(self, preparation);
            const runtime = terminal_runtime.TerminalRuntime.createWithOptions(
                self.allocator,
                build.processLaunch(),
                build.runtimeOptions(),
            ) catch |err| {
                self.allocator.destroy(build.retained);
                preparation.* = .{ .pending = build.pending };
                return if (err == error.OutOfMemory)
                    error.RuntimeAllocationOutOfMemory
                else
                    error.RuntimeSpawnFailed;
            };
            return .{ .created = commitCreateBuild(self, &build, runtime) };
        },
    }
}

pub fn adoptRunningTerminalForTesting(
    self: anytype,
    connection_id: core_module.ConnectionId,
    creation_id: core_module.CreationId,
    terminal_id: core_module.TerminalId,
    launch_digest: terminal_launch.Digest,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    configuration: terminal_configuration.Configuration,
    runtime: *terminal_runtime.TerminalRuntime,
) std.mem.Allocator.Error!void {
    if (comptime !target_builtin.is_test) {
        @compileError("custom runtime adoption is available only to service tests");
    }
    const retained = try self.allocator.create(RetainedTerminal);
    retained.* = .{
        .runtime = runtime,
        .session = undefined,
        .start_preparation = null,
    };
    self.core_state.commitRunningTerminal(
        connection_id,
        creation_id,
        terminal_id,
        launch_digest,
        columns,
        rows,
        cell_pixel_width,
        cell_pixel_height,
        configuration,
        runtime.themeDefaults(),
        &retained.session,
    );
}
