const std = @import("std");
const identifiers = @import("terminal-host-model").identifiers;
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const prepared_attachment = @import("prepared_attachment.zig");
const attachment_input_admission_ops = @import("attachment_input_admission.zig");
const failure_ops = @import("failure.zig");

const focus_mode_retry_limit = 3;

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
    if (!attachment_input_admission_ops.hasOwnership(self, attachment_id)) return;
    if (deferWhileMoving(self, attachment_id, true)) return;
    for (0..focus_mode_retry_limit) |_| {
        var repair = attachment_input_admission_ops.prepareRepair(self, attachment_id) catch |err| {
            attachment_input_admission_ops.failRepair(self, attachment_id, err, "preparation failed");
            return;
        };
        const committed = repair.commit();
        repair.deinit();
        _ = committed catch |err| {
            if (err == error.FocusModeChanged) continue;
            attachment_input_admission_ops.failRepair(self, attachment_id, err, "commit was rejected");
            return;
        };
        return;
    }
    attachment_input_admission_ops.failRepair(
        self,
        attachment_id,
        error.FocusModeChanged,
        "focus mode did not stabilize",
    );
}

pub fn resetInput(self: *state.State, attachment_id: identifiers.AttachmentId) void {
    if (!attachment_input_admission_ops.hasOwnership(self, attachment_id)) return;
    if (deferWhileMoving(self, attachment_id, false)) return;
    for (0..focus_mode_retry_limit) |_| {
        var repair = attachment_input_admission_ops.prepareReset(self, attachment_id) catch |err| {
            attachment_input_admission_ops.failReset(self, attachment_id, err, "preparation failed");
            return;
        };
        const committed = repair.commit();
        repair.deinit();
        _ = committed catch |err| {
            if (err == error.FocusModeChanged) continue;
            attachment_input_admission_ops.failReset(self, attachment_id, err, "commit was rejected");
            return;
        };
        return;
    }
    attachment_input_admission_ops.failReset(
        self,
        attachment_id,
        error.FocusModeChanged,
        "focus mode did not stabilize",
    );
}

/// While the terminal moves, an attachment's input cleanup waits for the move's end, since it
/// writes to the program; until then its held input stays in the ledger, where an export carries
/// it. A cleanup that cannot be recorded forgets the held input instead. True when it waits.
fn deferWhileMoving(self: *state.State, attachment_id: identifiers.AttachmentId, detach: bool) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    if (self.move == .none) return false;
    self.deferred_input.append(self.allocator, .{ .attachment_id = attachment_id, .detach = detach }) catch {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        if (detach) self.input_ownership.discard(attachment_id) else self.input_ownership.resetRetainingAttachment(attachment_id);
    };
    return true;
}

pub fn detachHistory(self: *state.State, attachment_id: identifiers.AttachmentId) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    self.history.detach(&self.terminal, attachment_id);
}
