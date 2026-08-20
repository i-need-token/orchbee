const std = @import("std");
const core_module = @import("terminal-host-core").state;
const contracts = @import("contracts.zig");
const projection_ops = @import("projection.zig");

const AttachError = contracts.AttachError;
const AttachOutcome = contracts.AttachOutcome;
const AttachmentBinding = contracts.AttachmentBinding;
const CapabilityUpdateError = contracts.CapabilityUpdateError;

pub fn attach(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    terminal_id: core_module.TerminalId,
    capabilities: core_module.Capabilities,
    request_resize_lease: bool,
) AttachError!AttachOutcome {
    const admission = try self.core_state.validateAttachmentAdmission(
        connection_id,
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease,
    );
    if (admission == .replay) {
        return .{
            .binding = attachmentBinding(self.core_state.findAttachment(attachment_id).?),
            .transition = .idempotent,
        };
    }

    const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
    var runtime_preparation = runtime.prepareAttachment(attachment_id, capabilities) catch |err| return switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => error.TerminalRuntimeUnavailable,
    };
    defer runtime_preparation.deinit();
    const transition = try self.core_state.attachAtBellSequence(
        connection_id,
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease,
        runtime_preparation.bellSequence(),
    );
    std.debug.assert(transition == .applied);
    runtime_preparation.commit();
    const attachment = self.core_state.findAttachment(attachment_id) orelse unreachable;
    return .{
        .binding = attachmentBinding(attachment),
        .transition = transition,
    };
}

pub fn detach(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) core_module.CoreError!core_module.AttachmentDetach {
    if (self.core_state.findAttachment(attachment_id) == null) {
        return self.core_state.detach(connection_id, attachment_id);
    }
    const terminal_id = (try self.core_state.resolveAttachment(
        connection_id,
        attachment_id,
        .none,
    )).terminal_id;
    // resolveAttachment proved the only fallible Core precondition. Repair
    // attachment-owned input while its capability is still authoritative,
    // then remove broker authority and the remaining derived state.
    if (self.runtimeFor(terminal_id)) |runtime| runtime.detachInput(attachment_id);
    const detached = self.core_state.detach(connection_id, attachment_id) catch unreachable;
    if (self.runtimeFor(terminal_id)) |runtime| runtime.detachHistory(attachment_id);
    projection_ops.releaseProjectionIfIdle(self, terminal_id);
    return detached;
}

pub fn updateAttachmentInputCapability(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    capability_revision: u64,
    input: bool,
) CapabilityUpdateError!AttachmentBinding {
    const admission = try self.core_state.validateAttachmentInputCapabilityUpdate(
        connection_id,
        attachment_id,
        capability_revision,
        input,
    );
    if (admission == .replay) {
        return attachmentBinding(self.core_state.findAttachment(attachment_id).?);
    }

    const attachment = self.core_state.findAttachment(attachment_id) orelse unreachable;
    const runtime = self.runtimeFor(attachment.terminal_id) orelse
        return error.TerminalRuntimeMissing;
    if (input) {
        var runtime_preparation = runtime.prepareAttachment(attachment_id, .{
            .input = true,
            .read = false,
            .resize = false,
        }) catch |err| return switch (err) {
            error.OutOfMemory => error.OutOfMemory,
            error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => error.TerminalRuntimeUnavailable,
        };
        defer runtime_preparation.deinit();
        self.core_state.commitAttachmentInputCapabilityUpdate(
            attachment_id,
            capability_revision,
            true,
        );
        runtime_preparation.commit();
    } else {
        // Repair and release only this attachment's held key/mouse/focus ownership
        // before revoking its Core authority. Already accepted queue records
        // remain once-admitted and an upgrade creates a fresh empty ledger.
        runtime.detachInput(attachment_id);
        self.core_state.commitAttachmentInputCapabilityUpdate(
            attachment_id,
            capability_revision,
            false,
        );
    }
    return attachmentBinding(self.core_state.findAttachment(attachment_id).?);
}

pub fn disconnect(
    self: anytype,
    connection_id: core_module.ConnectionId,
) core_module.TransitionOutcome {
    // Bulk one-pass teardown: detach every attachment exactly once, then
    // release projection resources at most once per affected terminal
    // (when its last attachment is removed). The O(1) active-demand
    // aggregate keeps each release decision constant-cost, so an
    // N-attachment disconnect is linear instead of quadratic.
    while (self.core_state.attachmentsForConnection(connection_id).len > 0) {
        const attachment_ids = self.core_state.attachmentsForConnection(connection_id);
        const attachment_id = attachment_ids[attachment_ids.len - 1];
        const terminal_id = self.core_state.findAttachment(attachment_id).?.terminal_id;
        if (self.runtimeFor(terminal_id)) |runtime| {
            runtime.detachInput(attachment_id);
        }
        _ = self.core_state.detach(connection_id, attachment_id) catch unreachable;
        if (self.runtimeFor(terminal_id)) |runtime| {
            runtime.detachHistory(attachment_id);
        }
        if (self.core_state.attachmentCountForTerminal(terminal_id) == 0) {
            projection_ops.releaseProjectionIfIdle(self, terminal_id);
        }
    }
    return self.core_state.disconnect(connection_id);
}

fn attachmentBinding(attachment: *const core_module.Attachment) AttachmentBinding {
    return .{
        .attachment_id = attachment.attachment_id,
        .capabilities = attachment.capabilities,
        .capability_revision = attachment.capability_revision,
        .resize_lease = attachment.resize_lease,
        .terminal_id = attachment.terminal_id,
    };
}
