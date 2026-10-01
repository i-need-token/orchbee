const std = @import("std");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const contracts = @import("contracts.zig");

const TerminalControl = contracts.TerminalControl;
const TerminalControlError = contracts.TerminalControlError;

pub fn applyTerminalControl(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    operation: terminal_runtime.TerminalControlOperation,
) TerminalControlError!TerminalControl {
    const attachment = try self.core_state.authorizeCurrentInput(connection_id, attachment_id);
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
    _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
        return error.RuntimeStateDiverged;
    const update = runtime.applyTerminalControl(operation) catch |err| return switch (err) {
        error.RevisionOverflow => error.RevisionOverflow,
        error.TerminalClearAlternateScreen => error.TerminalClearAlternateScreen,
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
        .operation = if (operation == .clear_prompt) .clear else operation,
        .revision = update.revision,
        .terminal_id = terminal_id,
    };
}
