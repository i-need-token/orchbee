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
    if (try self.core_state.resolveCreationWithOverlay(
        connection_id,
        creation_id,
        validated_launch.launch_digest,
        if (validated_launch.environment_overlay.entries.len == 0)
            null
        else
            validated_launch.environment_overlay.digest,
    )) |terminal_id| {
        const terminal = self.core_state.findTerminal(terminal_id) orelse unreachable;
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
        .launch_digest = validated_launch.launch_digest,
        .runtime_preparation = prepared_launch,
        .terminal_id = terminal_id,
    } };
}

pub fn commitPreparedTerminal(
    self: anytype,
    preparation: *CreatePreparation,
) CreateCommitError!CreateOutcome {
    switch (preparation.*) {
        .replayed => |identity| return .{ .replayed = identity },
        .consumed => unreachable,
        .pending => |*pending| {
            const retained = self.allocator.create(RetainedTerminal) catch
                return error.RuntimeAllocationOutOfMemory;
            errdefer self.allocator.destroy(retained);
            const identity: TerminalIdentity = .{
                .creation_id = pending.creation_id,
                .terminal_id = pending.terminal_id,
            };
            if (pending.initial_geometry) |geometry| {
                const runtime = terminal_runtime.TerminalRuntime.createWithOptions(
                    self.allocator,
                    pending.runtime_preparation.processLaunch(geometry),
                    pending.runtime_preparation.runtimeOptions(geometry),
                ) catch |err| {
                    return if (err == error.OutOfMemory)
                        error.RuntimeAllocationOutOfMemory
                    else
                        error.RuntimeSpawnFailed;
                };
                retained.* = .{
                    .runtime = runtime,
                    .session = undefined,
                    .start_preparation = null,
                };
                // All domain capacity and the complete engine exist before
                // this infallible commit. The caller has already encoded and
                // reserved the records that make the commit observable.
                self.core_state.commitRunningTerminalWithOverlay(
                    pending.connection_id,
                    pending.creation_id,
                    pending.terminal_id,
                    pending.launch_digest,
                    pending.environment_overlay_digest,
                    geometry.columns,
                    geometry.rows,
                    geometry.cell_pixel_width,
                    geometry.cell_pixel_height,
                    pending.runtime_preparation.configuration,
                    pending.runtime_preparation.themeDefaults(),
                    &retained.session,
                );
                pending.runtime_preparation.deinit(self.allocator);
            } else {
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
            }
            preparation.* = .consumed;
            return .{ .created = identity };
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
