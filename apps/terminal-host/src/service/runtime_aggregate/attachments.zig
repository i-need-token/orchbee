const std = @import("std");
const core_module = @import("terminal-host-core").state;
const terminal_input = @import("terminal-input");
const contracts = @import("contracts.zig");
const projection_ops = @import("projection.zig");

const AttachError = contracts.AttachError;
const AttachOutcome = contracts.AttachOutcome;
const AttachmentBinding = contracts.AttachmentBinding;
const InputAdmission = contracts.InputAdmission;
const InputPreparation = contracts.InputPreparation;
const ReadableAttachment = contracts.ReadableAttachment;
const ReadableAttachmentError = contracts.ReadableAttachmentError;

pub fn readableAttachment(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) ReadableAttachmentError!ReadableAttachment {
    const attachment = self.core_state.resolveAttachment(
        connection_id,
        attachment_id,
        .read,
    ) catch return error.ReadableAttachmentUnavailable;
    const terminal = self.core_state.findTerminal(attachment.terminal_id) orelse
        return error.ReadableAttachmentUnavailable;
    const runtime = self.runtimeFor(attachment.terminal_id) orelse
        return error.TerminalRuntimeMissing;
    if (!terminal.hasReadableRuntimeState() or !runtime.readableStateAvailable()) {
        return error.ReadableAttachmentUnavailable;
    }
    return ReadableAttachment.init(attachment_id, runtime);
}

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

pub fn acceptRawInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    bytes: []const u8,
) InputAdmission {
    var preparation = self.prepareRawInput(connection_id, attachment_id, bytes);
    defer preparation.deinit();
    return preparation.commit();
}

pub fn prepareRawInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    bytes: []const u8,
) InputPreparation {
    const attachment = self.core_state.authorizeInput(
        connection_id,
        attachment_id,
    ) catch |err| return InputPreparation.initCoreRejected(err);
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse
        return InputPreparation.initRuntimeMissing();

    const runtime_input = runtime.prepareRawInput(bytes) catch |err|
        return InputPreparation.initRuntimeRejected(err);
    return InputPreparation.initPrepared(attachment_id, runtime_input);
}

pub fn acceptSemanticInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    input: terminal_input.SemanticInput,
    maximum_bytes: usize,
) InputAdmission {
    var preparation = self.prepareSemanticInput(
        connection_id,
        attachment_id,
        input,
        maximum_bytes,
    );
    defer preparation.deinit();
    return preparation.commit();
}

pub fn prepareSemanticInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    input: terminal_input.SemanticInput,
    maximum_bytes: usize,
) InputPreparation {
    const attachment = self.core_state.authorizeInput(
        connection_id,
        attachment_id,
    ) catch |err| return InputPreparation.initCoreRejected(err);
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse
        return InputPreparation.initRuntimeMissing();

    const runtime_input = runtime.prepareAttachmentSemanticInput(
        attachment_id,
        input,
        maximum_bytes,
    ) catch |err| return InputPreparation.initRuntimeRejected(err);
    return InputPreparation.initPrepared(attachment_id, runtime_input);
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
        .resize_lease = attachment.resize_lease,
        .terminal_id = attachment.terminal_id,
    };
}
