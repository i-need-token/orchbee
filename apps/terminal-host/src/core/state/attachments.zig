const std = @import("std");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const Attachment = contracts.Attachment;
const AttachmentAdmission = contracts.AttachmentAdmission;
const AttachmentDetach = contracts.AttachmentDetach;
const AttachmentId = contracts.AttachmentId;
const BellDeliveryToken = contracts.BellDeliveryToken;
const Capabilities = contracts.Capabilities;
const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const RequiredCapability = contracts.RequiredCapability;
const TerminalId = contracts.TerminalId;
const TransitionOutcome = contracts.TransitionOutcome;

pub fn attach(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    terminal_id: TerminalId,
    capabilities: Capabilities,
    request_resize_lease: bool,
) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
    return self.attachAtBellSequence(
        connection_id,
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease,
        0,
    );
}

pub fn attachAtBellSequence(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    terminal_id: TerminalId,
    capabilities: Capabilities,
    request_resize_lease: bool,
    bell_sequence: u64,
) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
    switch (try self.validateAttachmentAdmission(
        connection_id,
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease,
    )) {
        .replay => return .idempotent,
        .create => {},
    }

    const broker = if (self.broker) |*active| active else unreachable;
    try broker.add(
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease,
        bell_sequence,
    );
    return .applied;
}

pub fn prepareBellDelivery(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    observed_sequence: u64,
) CoreError!?BellDeliveryToken {
    const attachment = try storage.resolveMutableAttachment(
        self,
        connection_id,
        attachment_id,
        .read,
    );
    std.debug.assert(attachment.bell_cursor <= observed_sequence);
    if (attachment.bell_cursor == observed_sequence) return null;
    return .{
        .attachment_id = attachment_id,
        .sequence = attachment.bell_cursor + 1,
    };
}

pub fn commitBellDelivery(self: anytype, token: BellDeliveryToken) void {
    const attachment = storage.rawAttachment(self, token.attachment_id) orelse unreachable;
    std.debug.assert(attachment.capabilities.read);
    std.debug.assert(attachment.bell_cursor < std.math.maxInt(u64));
    std.debug.assert(attachment.bell_cursor + 1 == token.sequence);
    attachment.bell_cursor = token.sequence;
}

pub fn validateAttachmentAdmission(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    terminal_id: TerminalId,
    capabilities: Capabilities,
    request_resize_lease: bool,
) CoreError!AttachmentAdmission {
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const broker = if (self.broker) |*active| active else unreachable;
    const admission = try broker.validateAdmission(
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease,
    );
    if (admission == .replay) return .replay;

    const terminal = try storage.requireTerminal(self, terminal_id);
    if ((capabilities.input or capabilities.resize) and !terminal.acceptsRuntimeWork()) {
        return error.InvalidPhase;
    }
    if (capabilities.read and !terminal.hasReadableRuntimeState()) {
        return error.InvalidPhase;
    }
    if (request_resize_lease and broker.hasResizeLease(terminal_id)) {
        return error.ResizeLeaseDenied;
    }
    return .create;
}

pub fn detach(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
) CoreError!AttachmentDetach {
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const broker = if (self.broker) |*active| active else unreachable;
    const attachment = broker.remove(attachment_id) orelse return .{
        .outcome = .idempotent,
        .resize_lease_change = null,
    };
    const terminal = self.findTerminal(attachment.terminal_id);
    const promoted_attachment_id = if (attachment.resize_lease and
        terminal != null and terminal.?.acceptsRuntimeWork())
        broker.promoteResizeLease(attachment.terminal_id)
    else
        null;
    return .{
        .outcome = .applied,
        .resize_lease_change = if (promoted_attachment_id) |promoted| .{
            .attachment_id = promoted,
            .resize_lease = true,
            .terminal_id = attachment.terminal_id,
        } else null,
    };
}

pub fn authorizeInput(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
) CoreError!*const Attachment {
    const attachment = try self.resolveAttachment(connection_id, attachment_id, .input);
    const terminal = try storage.requireTerminal(self, attachment.terminal_id);
    if (!terminal.acceptsRuntimeWork()) return error.InvalidPhase;
    return attachment;
}

pub fn resize(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    columns: u16,
    rows: u16,
) CoreError!TransitionOutcome {
    const attachment = self.resolveAttachment(connection_id, attachment_id, .resize) catch |err| {
        return if (err == error.MissingResource) error.ResizeLeaseDenied else err;
    };
    if (!attachment.resize_lease) {
        return error.ResizeLeaseDenied;
    }
    const terminal = try storage.requireTerminal(self, attachment.terminal_id);
    if (!terminal.acceptsRuntimeWork()) return error.InvalidPhase;
    const geometry = terminal.observedGeometry();
    if (geometry.columns == columns and geometry.rows == rows) return .idempotent;
    return .applied;
}

pub fn findAttachment(self: anytype, attachment_id: AttachmentId) ?*const Attachment {
    return storage.mutableAttachment(self, attachment_id);
}

pub fn attachmentCount(self: anytype) usize {
    const broker = if (self.broker) |*active| active else return 0;
    return broker.count();
}

pub fn attachmentKeys(self: anytype) []const AttachmentId {
    const broker = if (self.broker) |*active| active else return &.{};
    return broker.keys();
}

pub fn attachmentValues(self: anytype) []const Attachment {
    const broker = if (self.broker) |*active| active else return &.{};
    return broker.values();
}

pub fn resolveAttachment(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    required: RequiredCapability,
) CoreError!*const Attachment {
    return storage.resolveMutableAttachment(self, connection_id, attachment_id, required);
}

pub fn hasActiveProjectionDemand(self: anytype, terminal_id: TerminalId) bool {
    if (self.terminal_registry.get(terminal_id) == null) return false;
    const broker = if (self.broker) |*active| active else return false;
    return broker.hasActiveProjectionDemand(terminal_id);
}

pub fn attachmentCountForTerminal(self: anytype, terminal_id: TerminalId) usize {
    const broker = if (self.broker) |*active| active else return 0;
    return broker.attachmentCountForTerminal(terminal_id);
}
