const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");

const Attachment = contracts.Attachment;
const AttachmentId = contracts.AttachmentId;
const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const RequiredCapability = contracts.RequiredCapability;
const TerminalId = contracts.TerminalId;
const TerminalSession = contracts.TerminalSession;

pub fn mutableAttachment(self: anytype, attachment_id: AttachmentId) ?*Attachment {
    const attachment = rawAttachment(self, attachment_id) orelse return null;
    refreshAttachmentProjectionState(self, attachment);
    return attachment;
}

pub fn resolveMutableAttachment(
    self: anytype,
    connection_id: ConnectionId,
    attachment_id: AttachmentId,
    required: RequiredCapability,
) CoreError!*Attachment {
    if (!connected(self, connection_id)) return error.MissingConnection;
    const attachment = mutableAttachment(self, attachment_id) orelse return error.MissingResource;
    const permitted = switch (required) {
        .none => true,
        .input => attachment.capabilities.input,
        .read => attachment.capabilities.read,
        .resize => attachment.capabilities.resize,
    };
    if (!permitted) return error.MissingResource;
    _ = try requireTerminal(self, attachment.terminal_id);
    return attachment;
}

pub fn connected(self: anytype, connection_id: ConnectionId) bool {
    const broker = self.broker orelse return false;
    return broker.owns(connection_id);
}

pub fn rawAttachment(self: anytype, attachment_id: AttachmentId) ?*Attachment {
    const broker = if (self.broker) |*active| active else return null;
    return broker.find(attachment_id);
}

pub fn requireTerminal(self: anytype, terminal_id: TerminalId) CoreError!*TerminalSession {
    return self.terminal_registry.find(terminal_id) orelse error.MissingResource;
}

pub fn refreshAttachmentProjectionState(self: anytype, attachment: *Attachment) void {
    if (!attachment.capabilities.read) return;
    const terminal = self.terminal_registry.get(attachment.terminal_id) orelse return;
    attachment.projection.refresh(
        terminal.observedRevision(),
        terminal.observedGeometryRevision(),
    );
}

pub fn removeTerminalAttachments(self: anytype, terminal_id: TerminalId) void {
    const broker = if (self.broker) |*active| active else return;
    broker.removeTerminalAttachments(terminal_id);
}

pub fn ownsTestingTerminalStorage(
    self: anytype,
    session: *const TerminalSession,
) bool {
    if (comptime !target_builtin.is_test) return false;
    var storage = self.testing_state.terminal_storage;
    while (storage) |candidate| : (storage = candidate.next) {
        if (&candidate.session == session) return true;
    }
    return false;
}

pub fn destroyTestingTerminalStorage(
    self: anytype,
    session: *TerminalSession,
) void {
    if (comptime !target_builtin.is_test) unreachable;
    var link = &self.testing_state.terminal_storage;
    while (link.*) |storage| {
        if (&storage.session == session) {
            link.* = storage.next;
            self.allocator.destroy(storage);
            return;
        }
        link = &storage.next;
    }
    unreachable;
}
