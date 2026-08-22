const std = @import("std");
const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const model = @import("terminal-host-model");
const terminal_configuration = model.configuration;
const contracts = @import("contracts.zig");

const ConfigurationUpdateError = contracts.ConfigurationUpdateError;
const TerminalConfigurationUpdate = contracts.TerminalConfigurationUpdate;

pub fn configurationObservationIsCompatible(
    committed_revision: u64,
    observed_revision: u64,
) bool {
    return observed_revision >= committed_revision;
}

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
    if (comptime target_builtin.is_test) {
        if (self.testing_state.after_configuration_commit) |hook| {
            hook(self.testing_state.after_configuration_commit_context, runtime);
        }
    }

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
    std.debug.assert(configurationObservationIsCompatible(
        runtime_update.revision,
        runtime_geometry.revision,
    ));
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
