const std = @import("std");
const identifiers = @import("terminal-host-model").identifiers;
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const prepared_attachment = @import("prepared_attachment.zig");
const failure_ops = @import("failure.zig");
const input_admission_ops = @import("input_admission.zig");

pub fn prepareAttachment(
    self: *state.State,
    attachment_id: identifiers.AttachmentId,
    capabilities: contracts.Capabilities,
) !prepared_attachment.Internal {
    std.debug.assert(capabilities.input or capabilities.read or capabilities.resize);
    self.terminal_mutex.lock();
    errdefer self.terminal_mutex.unlock();
    if (capabilities.input or capabilities.resize) {
        try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    } else if (capabilities.read) {
        try failure_ops.ensureReadableStateAvailableLocked(self);
    }
    if (capabilities.input) {
        self.input_mutex.lock();
        self.input_ownership.ensureAttachmentCapacity() catch |err| {
            self.input_mutex.unlock();
            return err;
        };
        self.input_mutex.unlock();
    }
    if (capabilities.read) try self.history.reserveAttachment(attachment_id);
    return .{
        .attachment_id = attachment_id,
        .bell_sequence = self.bell_sequence,
        .history_reserved = capabilities.read,
        .input_state_reserved = capabilities.input,
        .runtime = self,
    };
}

pub fn detachInput(self: *state.State, attachment_id: identifiers.AttachmentId) void {
    if (!input_admission_ops.hasAttachmentInputOwnership(self, attachment_id)) return;
    var repair = input_admission_ops.prepareAttachmentInputRepair(self, attachment_id) catch |err| {
        input_admission_ops.failAttachmentInputRepair(self, attachment_id, err, "preparation failed");
        return;
    };
    defer repair.deinit();
    _ = repair.commit() catch |err| {
        input_admission_ops.failAttachmentInputRepair(self, attachment_id, err, "commit was rejected");
    };
}

pub fn detachHistory(self: *state.State, attachment_id: identifiers.AttachmentId) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    self.history.detach(&self.terminal, attachment_id);
}
