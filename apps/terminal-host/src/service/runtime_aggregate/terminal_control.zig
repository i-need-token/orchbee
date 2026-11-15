const std = @import("std");
const target_builtin = @import("builtin");
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
    if (comptime target_builtin.is_test) {
        if (self.testing_state.after_terminal_control_commit) |hook| {
            hook(self.testing_state.after_terminal_control_commit_context, runtime);
        }
    }
    // The PTY reader keeps committing output after the control releases the terminal, so the
    // observation may already be newer than the control's own commit; Core adopts that newer
    // state. Both counters only grow under the terminal mutex, so it is never older.
    const observation = self.synchronizeRuntimeObservation(terminal_id, runtime) catch unreachable;
    std.debug.assert(observation.generation >= update.generation);
    std.debug.assert(observation.revision >= update.revision);
    return .{
        .attachment_id = attachment_id,
        .generation = update.generation,
        .operation = if (operation == .clear_prompt) .clear else operation,
        .revision = update.revision,
        .terminal_id = terminal_id,
    };
}
