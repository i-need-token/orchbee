const std = @import("std");
const target_builtin = @import("builtin");

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
    bell_cursor: u64,
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
    /// O(1) per-terminal count of read-capable attachments whose projection
    /// demand is active. Maintained exactly on add, demand transition, and
    /// remove; keeps the broker-loss teardown linear instead of quadratic.
    active_read_demand_by_terminal: std.AutoHashMap(TerminalId, usize),
    client_id: ClientId,
    connection_id: ConnectionId,
    /// Test-only teardown-cost sensor: total attachment-list entries examined
    /// by active-projection-demand queries since the last reset. The O(1)
    /// aggregate examines none; the pre-fix walk examined the whole remaining
    /// per-terminal list per query, making an N-attachment disconnect visit
    /// N(N-1)/2 entries. Only meaningful in test builds.
    active_read_demand_scans: usize = 0,

    pub fn init(
        allocator: std.mem.Allocator,
        connection_id: ConnectionId,
        client_id: ClientId,
    ) Session {
        return .{
            .allocator = allocator,
            .attachments = .init(allocator),
            .attachments_by_terminal = .init(allocator),
            .active_read_demand_by_terminal = .init(allocator),
            .client_id = client_id,
            .connection_id = connection_id,
        };
    }

    pub fn deinit(self: *Session) void {
        var attachment_lists = self.attachments_by_terminal.valueIterator();
        while (attachment_lists.next()) |list| list.deinit(self.allocator);
        self.attachments.deinit();
        self.attachments_by_terminal.deinit();
        self.active_read_demand_by_terminal.deinit();
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
        bell_sequence: u64,
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

        // A new attachment starts with paused projection demand, so the
        // active-read-demand aggregate holds a zero entry for its terminal.
        // Ensuring it here makes demand transitions allocation-free and
        // infallible during the owner's commit.
        var demand_entry_created = false;
        if (!self.active_read_demand_by_terminal.contains(terminal_id)) {
            try self.active_read_demand_by_terminal.putNoClobber(terminal_id, 0);
            demand_entry_created = true;
        }
        errdefer if (demand_entry_created) {
            _ = self.active_read_demand_by_terminal.fetchRemove(terminal_id);
        };

        try self.attachments.putNoClobber(attachment_id, .{
            .attachment_id = attachment_id,
            .bell_cursor = bell_sequence,
            .capabilities = capabilities,
            .resize_lease = resize_lease,
            .terminal_attachment_index = terminal_list.items.len - 1,
            .terminal_id = terminal_id,
        });
    }

    pub fn remove(self: *Session, attachment_id: AttachmentId) ?Attachment {
        const attachment = self.attachments.get(attachment_id) orelse return null;
        const terminal_id = attachment.terminal_id;
        const terminal_list = self.attachments_by_terminal.getPtr(terminal_id).?;
        const terminal_last = terminal_list.pop().?;
        const terminal_becomes_empty = terminal_list.items.len == 0;
        if (!terminal_last.eql(attachment.attachment_id)) {
            terminal_list.items[attachment.terminal_attachment_index] = terminal_last;
            self.attachments.getPtr(terminal_last).?.terminal_attachment_index =
                attachment.terminal_attachment_index;
        }
        if (terminal_becomes_empty) {
            var removed = self.attachments_by_terminal.fetchRemove(terminal_id).?;
            removed.value.deinit(self.allocator);
        }
        _ = self.attachments.swapRemove(attachment.attachment_id);
        if (attachment.capabilities.read and attachment.projectionDemand() == .active) {
            const demand_count = self.active_read_demand_by_terminal.getPtr(terminal_id).?;
            std.debug.assert(demand_count.* > 0);
            demand_count.* -= 1;
        }
        if (terminal_becomes_empty) {
            _ = self.active_read_demand_by_terminal.fetchRemove(terminal_id);
        }
        return attachment;
    }

    pub fn removeTerminalAttachments(self: *Session, terminal_id: TerminalId) void {
        while (self.idsForTerminal(terminal_id)) |attachment_ids| {
            if (attachment_ids.len == 0) break;
            _ = self.remove(attachment_ids[attachment_ids.len - 1]);
        }
    }

    pub fn attachmentCountForTerminal(self: *const Session, terminal_id: TerminalId) usize {
        const attachment_ids = self.idsForTerminal(terminal_id) orelse return 0;
        return attachment_ids.len;
    }

    /// Applies a projection demand transition and keeps the per-terminal
    /// active-read-demand aggregate exact. Callers must have proven the
    /// attachment is read-capable and that the transition is not idempotent;
    /// the aggregate entry was ensured at attach time, so this commit is
    /// allocation-free and infallible.
    pub fn applyProjectionDemand(
        self: *Session,
        attachment_id: AttachmentId,
        demand: attachment_projection.Demand,
    ) void {
        const attachment = self.attachments.getPtr(attachment_id).?;
        std.debug.assert(attachment.capabilities.read);
        std.debug.assert(attachment.projectionDemand() != demand);
        const terminal_id = attachment.terminal_id;
        if (demand == .active) {
            attachment.projection.activate();
            const demand_count = self.active_read_demand_by_terminal.getPtr(terminal_id).?;
            demand_count.* += 1;
        } else {
            attachment.projection.pause();
            const demand_count = self.active_read_demand_by_terminal.getPtr(terminal_id).?;
            std.debug.assert(demand_count.* > 0);
            demand_count.* -= 1;
        }
    }

    pub fn hasActiveProjectionDemand(
        self: *Session,
        terminal_id: TerminalId,
    ) bool {
        const demand_count = self.active_read_demand_by_terminal.get(terminal_id) orelse 0;
        if (target_builtin.is_test) {
            // Test-only teardown-cost sensor. The O(1) aggregate consults no
            // attachment-list entries, so every query records zero scan work;
            // the pre-fix walk examined the whole remaining list per query.
            self.active_read_demand_scans += 0;
        }
        return demand_count > 0;
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
    pub fn activeReadDemandScans(session: *const Session) usize {
        return session.active_read_demand_scans;
    }

    pub fn resetActiveReadDemandScans(session: *Session) void {
        session.active_read_demand_scans = 0;
    }

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
