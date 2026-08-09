const std = @import("std");

const attachment_projection = @import("attachment_projection.zig");
const identifiers = @import("terminal-host-model").identifiers;

pub const AttachmentId = identifiers.AttachmentId;
pub const ClientId = identifiers.ClientId;
pub const TerminalId = identifiers.TerminalId;
pub const ConnectionId = u64;

pub const Admission = enum {
    create,
    replay,
};

pub const Error = error{
    AttachmentConflict,
    InvalidCapabilities,
    ResizeLeaseDenied,
};

pub const Capabilities = packed struct {
    input: bool,
    read: bool,
    resize: bool,
};

pub const Attachment = struct {
    attachment_id: AttachmentId,
    capabilities: Capabilities,
    projection: attachment_projection.State = .{},
    resize_lease: bool,
    terminal_attachment_index: usize,
    terminal_id: TerminalId,

    pub fn projectionDemand(self: Attachment) attachment_projection.Demand {
        return self.projection.demand();
    }

    pub fn projectionReady(self: Attachment) ?attachment_projection.Ready {
        return self.projection.ready();
    }

    pub fn projectionInFlight(self: Attachment) ?attachment_projection.Flight {
        return self.projection.inFlight();
    }

    pub fn projectionBlocked(self: Attachment) ?attachment_projection.Blocked {
        return self.projection.blocked();
    }

    pub fn nextProjectionSequence(self: Attachment) u64 {
        return self.projection.next_sequence;
    }
};

/// Owns the active physical broker identity, every broker-scoped attachment,
/// and the rebuildable terminal-to-attachment index. No retained terminal
/// stores a second copy of attachment authority or projection demand.
pub const Session = struct {
    allocator: std.mem.Allocator,
    attachments: std.AutoArrayHashMap(AttachmentId, Attachment),
    attachments_by_terminal: std.AutoHashMap(TerminalId, std.ArrayList(AttachmentId)),
    client_id: ClientId,
    connection_id: ConnectionId,

    pub fn init(
        allocator: std.mem.Allocator,
        connection_id: ConnectionId,
        client_id: ClientId,
    ) Session {
        return .{
            .allocator = allocator,
            .attachments = .init(allocator),
            .attachments_by_terminal = .init(allocator),
            .client_id = client_id,
            .connection_id = connection_id,
        };
    }

    pub fn deinit(self: *Session) void {
        var attachment_lists = self.attachments_by_terminal.valueIterator();
        while (attachment_lists.next()) |list| list.deinit(self.allocator);
        self.attachments.deinit();
        self.attachments_by_terminal.deinit();
        self.* = undefined;
    }

    pub fn matches(
        self: *const Session,
        connection_id: ConnectionId,
        client_id: ClientId,
    ) bool {
        return self.connection_id == connection_id and self.client_id.eql(client_id);
    }

    pub fn owns(self: *const Session, connection_id: ConnectionId) bool {
        return self.connection_id == connection_id;
    }

    pub fn clientId(self: *const Session) ClientId {
        return self.client_id;
    }

    pub fn count(self: *const Session) usize {
        return self.attachments.count();
    }

    pub fn keys(self: *const Session) []const AttachmentId {
        return self.attachments.keys();
    }

    pub fn values(self: *const Session) []const Attachment {
        return self.attachments.values();
    }

    pub fn find(self: *Session, attachment_id: AttachmentId) ?*Attachment {
        return self.attachments.getPtr(attachment_id);
    }

    pub fn get(self: *const Session, attachment_id: AttachmentId) ?Attachment {
        return self.attachments.get(attachment_id);
    }

    pub fn idsForTerminal(
        self: *const Session,
        terminal_id: TerminalId,
    ) ?[]const AttachmentId {
        const list = self.attachments_by_terminal.get(terminal_id) orelse return null;
        return list.items;
    }

    pub fn validateAdmission(
        self: *const Session,
        attachment_id: AttachmentId,
        terminal_id: TerminalId,
        capabilities: Capabilities,
        request_resize_lease: bool,
    ) Error!Admission {
        if (!capabilities.input and !capabilities.read and !capabilities.resize) {
            return error.InvalidCapabilities;
        }
        if (request_resize_lease and !capabilities.resize) return error.ResizeLeaseDenied;

        if (self.attachments.get(attachment_id)) |existing| {
            const equivalent = existing.terminal_id.eql(terminal_id) and
                existing.capabilities == capabilities and
                existing.resize_lease == request_resize_lease;
            return if (equivalent) .replay else error.AttachmentConflict;
        }
        return .create;
    }

    /// Adds both broker indexes as one allocation-safe transaction. The Host
    /// validates terminal lifecycle before calling this owner-local commit.
    pub fn add(
        self: *Session,
        attachment_id: AttachmentId,
        terminal_id: TerminalId,
        capabilities: Capabilities,
        resize_lease: bool,
    ) std.mem.Allocator.Error!void {
        std.debug.assert(!self.attachments.contains(attachment_id));
        std.debug.assert(!resize_lease or capabilities.resize);
        std.debug.assert(!resize_lease or !self.hasResizeLease(terminal_id));

        var terminal_list_created = false;
        if (!self.attachments_by_terminal.contains(terminal_id)) {
            try self.attachments_by_terminal.putNoClobber(terminal_id, .empty);
            terminal_list_created = true;
        }
        errdefer if (terminal_list_created) {
            var removed = self.attachments_by_terminal.fetchRemove(terminal_id).?;
            removed.value.deinit(self.allocator);
        };
        const terminal_list = self.attachments_by_terminal.getPtr(terminal_id).?;
        try terminal_list.append(self.allocator, attachment_id);
        errdefer _ = terminal_list.pop();

        try self.attachments.putNoClobber(attachment_id, .{
            .attachment_id = attachment_id,
            .capabilities = capabilities,
            .resize_lease = resize_lease,
            .terminal_attachment_index = terminal_list.items.len - 1,
            .terminal_id = terminal_id,
        });
    }

    pub fn remove(self: *Session, attachment_id: AttachmentId) ?Attachment {
        const attachment = self.attachments.get(attachment_id) orelse return null;
        const terminal_list = self.attachments_by_terminal.getPtr(attachment.terminal_id).?;
        const terminal_last = terminal_list.pop().?;
        if (!terminal_last.eql(attachment.attachment_id)) {
            terminal_list.items[attachment.terminal_attachment_index] = terminal_last;
            self.attachments.getPtr(terminal_last).?.terminal_attachment_index =
                attachment.terminal_attachment_index;
        }
        if (terminal_list.items.len == 0) {
            var removed = self.attachments_by_terminal.fetchRemove(attachment.terminal_id).?;
            removed.value.deinit(self.allocator);
        }
        _ = self.attachments.swapRemove(attachment.attachment_id);
        return attachment;
    }

    pub fn removeTerminalAttachments(self: *Session, terminal_id: TerminalId) void {
        while (self.idsForTerminal(terminal_id)) |attachment_ids| {
            if (attachment_ids.len == 0) break;
            _ = self.remove(attachment_ids[attachment_ids.len - 1]);
        }
    }

    pub fn hasActiveProjectionDemand(
        self: *const Session,
        terminal_id: TerminalId,
    ) bool {
        const attachment_ids = self.idsForTerminal(terminal_id) orelse return false;
        for (attachment_ids) |attachment_id| {
            const attachment = self.attachments.get(attachment_id).?;
            if (attachment.capabilities.read and attachment.projectionDemand() == .active) {
                return true;
            }
        }
        return false;
    }

    pub fn hasResizeLease(self: *const Session, terminal_id: TerminalId) bool {
        const attachment_ids = self.idsForTerminal(terminal_id) orelse return false;
        for (attachment_ids) |attachment_id| {
            if (self.attachments.get(attachment_id).?.resize_lease) return true;
        }
        return false;
    }
};

pub const testing = if (@import("builtin").is_test) struct {
    pub fn indexedAttachmentCount(session: *const Session) usize {
        var count: usize = 0;
        var lists = session.attachments_by_terminal.valueIterator();
        while (lists.next()) |list| count += list.items.len;
        return count;
    }

    pub fn indexReferencesOnly(
        session: *const Session,
        retained_terminal_ids: []const TerminalId,
    ) bool {
        var iterator = session.attachments_by_terminal.iterator();
        while (iterator.next()) |entry| {
            if (entry.value_ptr.items.len == 0) return false;
            var retained = false;
            for (retained_terminal_ids) |terminal_id| {
                if (terminal_id.eql(entry.key_ptr.*)) {
                    retained = true;
                    break;
                }
            }
            if (!retained) return false;
        }
        return true;
    }
} else struct {};
