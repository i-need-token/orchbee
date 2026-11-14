const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const model = @import("terminal-host-model");
const terminal_geometry = model.geometry;
const terminal_runtime = terminal_engine.runtime;
const host_retained_terminal = @import("../retained_terminal.zig");
const contracts = @import("contracts.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const StartCommitError = contracts.StartCommitError;
const StartOutcome = contracts.StartOutcome;
const StartPreparation = contracts.StartPreparation;
const StartPrepareError = contracts.StartPrepareError;

pub fn prepareStartTerminal(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    geometry: terminal_geometry.Geometry,
) StartPrepareError!StartPreparation {
    const admission = try self.core_state.prepareTerminalStart(
        connection_id,
        terminal_id,
        geometry,
    );
    const terminal = self.core_state.findTerminal(terminal_id) orelse unreachable;
    const identity: contracts.TerminalIdentity = .{
        .creation_id = terminal.creation_id,
        .terminal_id = terminal_id,
    };
    return switch (admission) {
        .replay => .{ .replayed = .{
            .geometry = terminal.startGeometry().?,
            .identity = identity,
            .transition = .already_started,
        } },
        .start => .{ .pending = .{
            .connection_id = connection_id,
            .geometry = geometry,
            .identity = identity,
        } },
    };
}

pub fn commitPreparedTerminalStart(
    self: anytype,
    preparation: *StartPreparation,
) StartCommitError!StartOutcome {
    switch (preparation.*) {
        .replayed => |outcome| return outcome,
        .consumed => unreachable,
        .pending => |pending| {
            const session = self.core_state.findTerminal(pending.identity.terminal_id) orelse
                return error.TerminalRuntimeMissing;
            const retained = RetainedTerminal.fromMutableSession(@constCast(session));
            const launch_preparation = if (retained.start_preparation) |*value|
                value
            else
                return error.TerminalRuntimeMissing;
            std.debug.assert(retained.runtime == null);
            const runtime = terminal_runtime.TerminalRuntime.createWithOptions(
                self.allocator,
                launch_preparation.processLaunch(pending.geometry),
                launch_preparation.runtimeOptions(pending.geometry),
            ) catch |err| {
                return if (err == error.OutOfMemory)
                    error.RuntimeAllocationOutOfMemory
                else
                    error.RuntimeSpawnFailed;
            };
            retained.runtime = runtime;
            retained.deinitStartPreparation(self.allocator);
            self.core_state.commitTerminalStart(
                pending.identity.terminal_id,
                pending.geometry,
            );
            preparation.* = .consumed;
            return .{
                .geometry = pending.geometry,
                .identity = pending.identity,
                .transition = .started,
            };
        },
    }
}
