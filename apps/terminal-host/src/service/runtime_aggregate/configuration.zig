const std = @import("std");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const model = @import("terminal-host-model");
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");
const contracts = @import("contracts.zig");

const ConfigurationUpdateError = contracts.ConfigurationUpdateError;
const ResizeError = contracts.ResizeError;
const TerminalConfigurationUpdate = contracts.TerminalConfigurationUpdate;
const TerminalControl = contracts.TerminalControl;
const TerminalControlError = contracts.TerminalControlError;
const TerminalResize = contracts.TerminalResize;

pub fn prepareTerminalDefaultsUpdate(
    self: anytype,
    configuration: terminal_configuration.Configuration,
) core_module.CoreError!core_module.PreparedTerminalDefaultsUpdate {
    return self.core_state.prepareTerminalDefaultsUpdate(configuration);
}

pub fn commitTerminalDefaultsUpdate(
    self: anytype,
    update: core_module.PreparedTerminalDefaultsUpdate,
) void {
    self.core_state.commitTerminalDefaultsUpdate(update);
}

pub fn updateTerminalConfiguration(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    configuration: terminal_configuration.Configuration,
) ConfigurationUpdateError!TerminalConfigurationUpdate {
    if (!terminal_configuration.valid(configuration)) return error.InvalidConfiguration;
    if (self.core_state.connectionClientId(connection_id) == null) return error.MissingConnection;
    const terminal = self.core_state.findTerminal(terminal_id) orelse return error.MissingResource;
    if (!terminal.acceptsConfigurationUpdate()) return error.InvalidPhase;
    if (terminal.effectiveConfiguration().scrollback_bytes != configuration.scrollback_bytes and
        terminal.configurationRevision() == std.math.maxInt(u64))
    {
        return error.ConfigurationRevisionOverflow;
    }
    const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
    if (runtime.configuration().scrollback_bytes !=
        terminal.effectiveConfiguration().scrollback_bytes)
    {
        return error.RuntimeStateDiverged;
    }
    _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
        return error.RuntimeStateDiverged;
    const runtime_update = runtime.updateConfiguration(configuration) catch |err| {
        return switch (err) {
            error.InvalidConfiguration => error.InvalidConfiguration,
            error.RevisionOverflow => error.TerminalRevisionOverflow,
            error.TerminalResizeFailed => error.TerminalRuntimeUnavailable,
            error.TerminalRuntimeUnavailable => error.TerminalRuntimeUnavailable,
        };
    };

    // All Core checks happen before the runtime mutation. HostService is
    // single-owner, so these commits cannot fail without an invariant
    // violation after the runtime has accepted the update.
    _ = self.core_state.updateTerminalConfiguration(
        connection_id,
        terminal_id,
        configuration,
    ) catch unreachable;
    const runtime_geometry = self.synchronizeRuntimeObservation(
        terminal_id,
        runtime,
    ) catch unreachable;
    std.debug.assert(runtime_geometry.revision == runtime_update.revision);
    const updated_terminal = self.core_state.findTerminal(terminal_id).?;
    return .{
        .configuration = updated_terminal.effectiveConfiguration(),
        .configuration_revision = updated_terminal.configurationRevision(),
        .generation = runtime_update.generation,
        .history_rows_evicted = runtime_update.history_rows_evicted,
        .revision = runtime_update.revision,
        .terminal_id = terminal_id,
    };
}

pub fn resizeTerminal(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    geometry: terminal_geometry.Geometry,
) ResizeError!TerminalResize {
    if (!terminal_geometry.isValid(geometry)) return error.InvalidGeometry;
    const attachment = self.core_state.resolveAttachment(
        connection_id,
        attachment_id,
        .resize,
    ) catch |err| return err;
    if (!attachment.resize_lease) return error.ResizeLeaseDenied;
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
    _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
        return error.RuntimeStateDiverged;
    _ = try self.core_state.resize(
        connection_id,
        attachment_id,
        geometry.columns,
        geometry.rows,
    );
    const revision = runtime.resizeGeometry(
        geometry.columns,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    ) catch |err| {
        if (err == error.RevisionOverflow) return error.TerminalRevisionOverflow;
        if (err == error.TerminalRuntimeUnavailable) return error.TerminalRuntimeUnavailable;
        if (!runtime.isResizeFenced()) return error.RuntimeResizeFailed;
        const failure = runtime.primaryFailure() orelse .resize;
        _ = self.core_state.runtimeFailed(
            terminal_id,
            host_runtime_maintenance.runtimeFailureFromEngineKind(failure),
        ) catch unreachable;
        self.maintenance.containRuntimeFailure(
            &self.core_state,
            terminal_id,
            runtime,
        ) catch {};
        return error.UnsafeRuntimeAfterResizeFailure;
    };
    const runtime_geometry = self.synchronizeRuntimeObservation(
        terminal_id,
        runtime,
    ) catch unreachable;
    std.debug.assert(runtime_geometry.columns == geometry.columns);
    std.debug.assert(runtime_geometry.rows == geometry.rows);
    std.debug.assert(runtime_geometry.cell_pixel_width == geometry.cell_pixel_width);
    std.debug.assert(runtime_geometry.cell_pixel_height == geometry.cell_pixel_height);
    std.debug.assert(runtime_geometry.revision == revision);
    return .{
        .attachment_id = attachment_id,
        .geometry = geometry,
        .revision = revision,
        .terminal_id = terminal_id,
    };
}

pub fn applyTerminalControl(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    operation: terminal_runtime.TerminalControlOperation,
) TerminalControlError!TerminalControl {
    const attachment = try self.core_state.authorizeInput(connection_id, attachment_id);
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
    _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
        return error.RuntimeStateDiverged;
    const update = runtime.applyTerminalControl(operation) catch |err| return switch (err) {
        error.RevisionOverflow => error.RevisionOverflow,
        error.TerminalExited,
        error.TerminalResizeFailed,
        error.TerminalRuntimeUnavailable,
        => error.InvalidPhase,
    };
    const observation = self.synchronizeRuntimeObservation(terminal_id, runtime) catch unreachable;
    std.debug.assert(observation.generation == update.generation);
    std.debug.assert(observation.revision == update.revision);
    return .{
        .attachment_id = attachment_id,
        .generation = update.generation,
        .operation = operation,
        .revision = update.revision,
        .terminal_id = terminal_id,
    };
}
