const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const contracts = @import("contracts.zig");

const HistoryAccess = contracts.HistoryAccess;
const InteractionAccessError = contracts.InteractionAccessError;
const SelectionAccess = contracts.SelectionAccess;

const AccessBinding = struct {
    attachment_id: core_module.AttachmentId,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
    theme_revision: u64,
};

fn resolveAccess(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) InteractionAccessError!AccessBinding {
    const attachment = self.core_state.resolveAttachment(
        connection_id,
        attachment_id,
        .read,
    ) catch return error.InteractionAccessUnavailable;
    const terminal = self.core_state.findTerminal(attachment.terminal_id) orelse
        return error.InteractionAccessUnavailable;
    const runtime = self.runtimeFor(attachment.terminal_id) orelse
        return error.TerminalRuntimeMissing;
    if (!terminal.hasReadableRuntimeState() or !runtime.readableStateAvailable()) {
        return error.InteractionAccessUnavailable;
    }
    return .{
        .attachment_id = attachment_id,
        .terminal_id = attachment.terminal_id,
        .runtime = runtime,
        .theme_revision = terminal.themeState().revision,
    };
}

pub fn sampleTerminalProcesses(
    self: anytype,
    allocator: @import("std").mem.Allocator,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) !contracts.TerminalProcessSample {
    const binding = try resolveAccess(self, connection_id, attachment_id);
    return .{
        .terminal_id = binding.terminal_id,
        .sample = try binding.runtime.sampleProcesses(allocator),
    };
}

pub fn historyAccess(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) InteractionAccessError!HistoryAccess {
    const binding = try resolveAccess(self, connection_id, attachment_id);
    return HistoryAccess.init(binding.attachment_id, binding.runtime, binding.theme_revision);
}

pub fn selectionAccess(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) InteractionAccessError!SelectionAccess {
    const binding = try resolveAccess(self, connection_id, attachment_id);
    return SelectionAccess.init(binding.attachment_id, binding.runtime);
}
