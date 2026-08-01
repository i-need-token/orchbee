const std = @import("std");

pub const Identifier = [16]u8;
pub const LaunchDigest = [32]u8;

pub const max_pending_input_bytes = 4 * 1024 * 1024;

pub const TerminalPhase = enum {
    starting,
    running,
    finishing,
    exited,
};

pub const TransitionOutcome = enum {
    applied,
    idempotent,
};

pub const ExitResult = union(enum) {
    exit_code: u32,
    signal: u32,
    unknown: u32,
};

pub const Terminal = struct {
    cell_pixel_height: u16 = 1,
    cell_pixel_width: u16 = 1,
    child_exited: bool = false,
    columns: u16,
    creation_id: Identifier,
    exit_result: ?ExitResult = null,
    launch_digest: LaunchDigest,
    pending_input_bytes: usize = 0,
    phase: TerminalPhase = .starting,
    pty_drained: bool = false,
    revision: u64 = 0,
    rows: u16,
    terminal_id: Identifier,
    terminate_requested: bool = false,
};

pub const ProjectionDemand = enum {
    active,
    paused,
};

pub const ProjectionBlock = enum {
    capture_allocation_failed,
    projection_too_large,
};

pub const ProjectionFrameKind = enum {
    snapshot,
    delta,
};

pub const InFlightFrame = struct {
    base_revision: ?u64,
    generation: u64,
    kind: ProjectionFrameKind,
    revision: u64,
    sequence: u64,
};

pub const Capabilities = packed struct {
    input: bool,
    read: bool,
    resize: bool,
};

pub const Attachment = struct {
    acknowledged_generation: ?u64 = null,
    acknowledged_revision: ?u64 = null,
    attachment_id: Identifier,
    blocked: ?ProjectionBlock = null,
    capabilities: Capabilities,
    client_id: Identifier,
    demand: ProjectionDemand = .paused,
    force_snapshot: bool = true,
    in_flight: ?InFlightFrame = null,
    last_recovery_sequence: u64 = 0,
    next_sequence: u64 = 1,
    pending_dirty: bool = true,
    resize_lease: bool,
    terminal_id: Identifier,
};

pub const CoreError = error{
    AttachmentConflict,
    CapacityExceeded,
    CreationConflict,
    InputBackpressure,
    InvalidFrameAcknowledgement,
    InvalidPhase,
    MissingConnection,
    MissingResource,
    RevisionOverflow,
    ResizeLeaseDenied,
    SequenceOverflow,
};

pub const HostCore = struct {
    allocator: std.mem.Allocator,
    attachments: std.ArrayList(Attachment) = .empty,
    connections: std.ArrayList(Identifier) = .empty,
    epoch: Identifier,
    terminal: ?Terminal = null,

    pub fn init(allocator: std.mem.Allocator, epoch: Identifier) HostCore {
        return .{ .allocator = allocator, .epoch = epoch };
    }

    pub fn deinit(self: *HostCore) void {
        self.attachments.deinit(self.allocator);
        self.connections.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn connect(self: *HostCore, client_id: Identifier) std.mem.Allocator.Error!TransitionOutcome {
        if (findIdentifier(self.connections.items, client_id) != null) return .idempotent;
        try self.connections.append(self.allocator, client_id);
        return .applied;
    }

    pub fn disconnect(self: *HostCore, client_id: Identifier) TransitionOutcome {
        const connection_index = findIdentifier(self.connections.items, client_id) orelse return .idempotent;
        _ = self.connections.orderedRemove(connection_index);

        var index: usize = self.attachments.items.len;
        while (index > 0) {
            index -= 1;
            if (equalIdentifier(self.attachments.items[index].client_id, client_id)) {
                _ = self.attachments.orderedRemove(index);
            }
        }
        return .applied;
    }

    pub fn createTerminal(
        self: *HostCore,
        client_id: Identifier,
        creation_id: Identifier,
        terminal_id: Identifier,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        if (self.terminal) |terminal| {
            if (!equalIdentifier(terminal.creation_id, creation_id)) return error.CapacityExceeded;
            if (!std.mem.eql(u8, &terminal.launch_digest, &launch_digest)) {
                return error.CreationConflict;
            }
            return .idempotent;
        }

        self.terminal = .{
            .columns = columns,
            .creation_id = creation_id,
            .launch_digest = launch_digest,
            .rows = rows,
            .terminal_id = terminal_id,
        };
        return .applied;
    }

    pub fn spawnSucceeded(self: *HostCore, terminal_id: Identifier) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.phase == .running) return .idempotent;
        if (terminal.phase != .starting) return error.InvalidPhase;
        terminal.phase = .running;
        return .applied;
    }

    pub fn spawnFailed(self: *HostCore, terminal_id: Identifier) CoreError!TransitionOutcome {
        const terminal = self.terminal orelse return .idempotent;
        if (!equalIdentifier(terminal.terminal_id, terminal_id)) return error.MissingResource;
        if (terminal.phase != .starting) return error.InvalidPhase;
        self.removeTerminalAttachments(terminal_id);
        self.terminal = null;
        return .applied;
    }

    pub fn requestTerminate(
        self: *HostCore,
        client_id: Identifier,
        terminal_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.terminate_requested) return .idempotent;
        if (terminal.phase != .running) return error.InvalidPhase;
        terminal.terminate_requested = true;
        return .applied;
    }

    pub fn childExited(
        self: *HostCore,
        terminal_id: Identifier,
        result: ExitResult,
    ) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.child_exited) return .idempotent;
        if (terminal.phase != .running and terminal.phase != .finishing) return error.InvalidPhase;
        terminal.child_exited = true;
        terminal.exit_result = result;
        updateCompletionPhase(terminal);
        return .applied;
    }

    pub fn ptyDrained(self: *HostCore, terminal_id: Identifier) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.pty_drained) return .idempotent;
        if (terminal.phase != .running and terminal.phase != .finishing) return error.InvalidPhase;
        terminal.pty_drained = true;
        updateCompletionPhase(terminal);
        return .applied;
    }

    pub fn disposeTerminal(
        self: *HostCore,
        client_id: Identifier,
        terminal_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const terminal = self.terminal orelse return .idempotent;
        if (!equalIdentifier(terminal.terminal_id, terminal_id)) return error.MissingResource;
        if (terminal.phase != .exited) return error.InvalidPhase;
        self.removeTerminalAttachments(terminal_id);
        self.terminal = null;
        return .applied;
    }

    pub fn attach(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
        terminal_id: Identifier,
        capabilities: Capabilities,
        request_resize_lease: bool,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        _ = try self.requireTerminal(terminal_id);

        if (self.findAttachment(attachment_id)) |existing| {
            const equivalent = equalIdentifier(existing.client_id, client_id) and
                equalIdentifier(existing.terminal_id, terminal_id) and
                existing.capabilities == capabilities and
                existing.resize_lease == (request_resize_lease and capabilities.resize);
            return if (equivalent) .idempotent else error.AttachmentConflict;
        }
        if (request_resize_lease and !capabilities.resize) return error.ResizeLeaseDenied;
        if (request_resize_lease) {
            for (self.attachments.items) |attachment| {
                if (equalIdentifier(attachment.terminal_id, terminal_id) and attachment.resize_lease) {
                    return error.ResizeLeaseDenied;
                }
            }
        }

        try self.attachments.append(self.allocator, .{
            .attachment_id = attachment_id,
            .capabilities = capabilities,
            .client_id = client_id,
            .resize_lease = request_resize_lease,
            .terminal_id = terminal_id,
        });
        return .applied;
    }

    pub fn detach(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const index = self.findAttachmentIndex(attachment_id) orelse return .idempotent;
        if (!equalIdentifier(self.attachments.items[index].client_id, client_id)) {
            return error.MissingResource;
        }
        _ = self.attachments.orderedRemove(index);
        return .applied;
    }

    pub fn acceptInput(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
        byte_count: usize,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (!equalIdentifier(attachment.client_id, client_id) or !attachment.capabilities.input) {
            return error.MissingResource;
        }
        const terminal = try self.requireTerminal(attachment.terminal_id);
        if (terminal.phase != .running) return error.InvalidPhase;
        if (byte_count > max_pending_input_bytes - terminal.pending_input_bytes) {
            return error.InputBackpressure;
        }
        terminal.pending_input_bytes += byte_count;
        return .applied;
    }

    pub fn inputWritten(
        self: *HostCore,
        terminal_id: Identifier,
        byte_count: usize,
    ) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        if (byte_count > terminal.pending_input_bytes) return error.InvalidPhase;
        terminal.pending_input_bytes -= byte_count;
        return .applied;
    }

    pub fn resize(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
        columns: u16,
        rows: u16,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (!equalIdentifier(attachment.client_id, client_id) or
            !attachment.capabilities.resize or
            !attachment.resize_lease)
        {
            return error.ResizeLeaseDenied;
        }
        const terminal = try self.requireTerminal(attachment.terminal_id);
        if (terminal.phase != .running) return error.InvalidPhase;
        if (terminal.columns == columns and terminal.rows == rows) return .idempotent;
        return .applied;
    }

    pub fn synchronizeRevision(self: *HostCore, terminal_id: Identifier, revision: u64) CoreError!TransitionOutcome {
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.phase != .running and terminal.phase != .finishing) return error.InvalidPhase;
        if (revision < terminal.revision) return error.InvalidPhase;
        if (revision == terminal.revision) return .idempotent;
        terminal.revision = revision;
        for (self.attachments.items) |*attachment| {
            if (equalIdentifier(attachment.terminal_id, terminal_id) and attachment.capabilities.read) {
                attachment.pending_dirty = true;
            }
        }
        return .applied;
    }

    pub fn synchronizeGeometry(
        self: *HostCore,
        terminal_id: Identifier,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        revision: u64,
    ) CoreError!void {
        const terminal = try self.requireTerminal(terminal_id);
        if (revision < terminal.revision) return error.InvalidPhase;
        const geometry_changed = terminal.columns != columns or terminal.rows != rows or
            terminal.cell_pixel_width != cell_pixel_width or
            terminal.cell_pixel_height != cell_pixel_height;
        const revision_changed = terminal.revision != revision;
        if (!geometry_changed and !revision_changed) return;
        terminal.columns = columns;
        terminal.rows = rows;
        terminal.cell_pixel_width = cell_pixel_width;
        terminal.cell_pixel_height = cell_pixel_height;
        terminal.revision = revision;
        for (self.attachments.items) |*attachment| {
            if (equalIdentifier(attachment.terminal_id, terminal_id) and attachment.capabilities.read) {
                if (geometry_changed) attachment.force_snapshot = true;
                attachment.pending_dirty = true;
                if (geometry_changed) attachment.blocked = null;
            }
        }
    }

    pub fn setProjectionDemand(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
        demand: ProjectionDemand,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (!equalIdentifier(attachment.client_id, client_id) or !attachment.capabilities.read) {
            return error.MissingResource;
        }
        if (attachment.demand == demand) return .idempotent;
        attachment.demand = demand;
        if (demand == .active) {
            if (attachment.blocked != null) {
                attachment.blocked = null;
                attachment.force_snapshot = true;
            }
            attachment.pending_dirty = true;
        }
        return .applied;
    }

    pub fn commitFrame(
        self: *HostCore,
        attachment_id: Identifier,
        base_revision: ?u64,
        revision: u64,
        generation: u64,
        kind: ProjectionFrameKind,
    ) CoreError!InFlightFrame {
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (!attachment.capabilities.read or attachment.demand != .active or
            attachment.in_flight != null or attachment.blocked != null)
        {
            return error.InvalidPhase;
        }
        if (generation == 0) return error.InvalidPhase;
        if (kind == .snapshot) {
            if (base_revision != null) return error.InvalidPhase;
        } else {
            const acknowledged_revision = attachment.acknowledged_revision orelse return error.InvalidPhase;
            const acknowledged_generation = attachment.acknowledged_generation orelse return error.InvalidPhase;
            if (attachment.force_snapshot or base_revision != acknowledged_revision or
                acknowledged_generation != generation or revision < acknowledged_revision)
            {
                return error.InvalidPhase;
            }
        }
        const flight: InFlightFrame = .{
            .base_revision = base_revision,
            .generation = generation,
            .kind = kind,
            .revision = revision,
            .sequence = attachment.next_sequence,
        };
        attachment.next_sequence = std.math.add(u64, attachment.next_sequence, 1) catch
            return error.SequenceOverflow;
        attachment.in_flight = flight;
        attachment.force_snapshot = false;
        attachment.pending_dirty = false;
        return flight;
    }

    pub fn frameApplied(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (!equalIdentifier(attachment.client_id, client_id) or !attachment.capabilities.read) {
            return error.MissingResource;
        }
        if (attachment.in_flight) |flight| {
            if (flight.sequence != sequence or flight.revision != revision or flight.generation != generation) {
                return error.InvalidFrameAcknowledgement;
            }
            attachment.acknowledged_revision = revision;
            attachment.acknowledged_generation = generation;
            attachment.in_flight = null;
            attachment.blocked = null;
            if (self.terminal) |terminal| {
                attachment.pending_dirty = terminal.revision > revision;
            }
            return .applied;
        }
        if (attachment.acknowledged_revision == revision and
            attachment.acknowledged_generation == generation and
            attachment.next_sequence == sequence +| 1)
        {
            return .idempotent;
        }
        return error.InvalidFrameAcknowledgement;
    }

    pub fn needSnapshot(
        self: *HostCore,
        client_id: Identifier,
        attachment_id: Identifier,
        sequence: u64,
    ) CoreError!TransitionOutcome {
        if (!self.connected(client_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (!equalIdentifier(attachment.client_id, client_id) or !attachment.capabilities.read) {
            return error.MissingResource;
        }
        if (sequence <= attachment.last_recovery_sequence) return .idempotent;
        const flight = attachment.in_flight orelse return error.InvalidFrameAcknowledgement;
        if (flight.sequence != sequence) return error.InvalidFrameAcknowledgement;
        attachment.in_flight = null;
        attachment.force_snapshot = true;
        attachment.pending_dirty = true;
        attachment.blocked = null;
        attachment.last_recovery_sequence = sequence;
        return .applied;
    }

    pub fn blockProjection(
        self: *HostCore,
        attachment_id: Identifier,
        reason: ProjectionBlock,
    ) CoreError!void {
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        attachment.blocked = reason;
        attachment.pending_dirty = false;
    }

    pub fn projectionCaughtUp(
        self: *HostCore,
        attachment_id: Identifier,
    ) CoreError!void {
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (attachment.in_flight != null or attachment.force_snapshot) return error.InvalidPhase;
        attachment.pending_dirty = false;
    }

    pub fn findAttachment(self: *HostCore, attachment_id: Identifier) ?*Attachment {
        const index = self.findAttachmentIndex(attachment_id) orelse return null;
        return &self.attachments.items[index];
    }

    fn findAttachmentIndex(self: *const HostCore, attachment_id: Identifier) ?usize {
        for (self.attachments.items, 0..) |attachment, index| {
            if (equalIdentifier(attachment.attachment_id, attachment_id)) return index;
        }
        return null;
    }

    fn connected(self: *const HostCore, client_id: Identifier) bool {
        return findIdentifier(self.connections.items, client_id) != null;
    }

    fn requireTerminal(self: *HostCore, terminal_id: Identifier) CoreError!*Terminal {
        const terminal = if (self.terminal) |*value| value else return error.MissingResource;
        if (!equalIdentifier(terminal.terminal_id, terminal_id)) return error.MissingResource;
        return terminal;
    }

    fn removeTerminalAttachments(self: *HostCore, terminal_id: Identifier) void {
        var index: usize = self.attachments.items.len;
        while (index > 0) {
            index -= 1;
            if (equalIdentifier(self.attachments.items[index].terminal_id, terminal_id)) {
                _ = self.attachments.orderedRemove(index);
            }
        }
    }
};

fn updateCompletionPhase(terminal: *Terminal) void {
    terminal.phase = if (terminal.child_exited and terminal.pty_drained) .exited else .finishing;
}

fn equalIdentifier(left: Identifier, right: Identifier) bool {
    return std.mem.eql(u8, &left, &right);
}

fn findIdentifier(values: []const Identifier, value: Identifier) ?usize {
    for (values, 0..) |candidate, index| {
        if (equalIdentifier(candidate, value)) return index;
    }
    return null;
}

fn identifier(byte: u8) Identifier {
    return [_]u8{byte} ** 16;
}

test "one-terminal creation is idempotent and capacity bounded" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const creation = identifier(3);
    const terminal = identifier(4);
    const digest = [_]u8{5} ** 32;

    try std.testing.expectEqual(.applied, try core.connect(client));
    try std.testing.expectEqual(
        .applied,
        try core.createTerminal(client, creation, terminal, digest, 80, 24),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.createTerminal(client, creation, identifier(9), digest, 80, 24),
    );
    try std.testing.expectError(
        error.CreationConflict,
        core.createTerminal(client, creation, terminal, [_]u8{6} ** 32, 80, 24),
    );
    try std.testing.expectError(
        error.CapacityExceeded,
        core.createTerminal(client, identifier(7), identifier(8), digest, 80, 24),
    );
}

test "disconnect removes only attachment state" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const terminal_id = identifier(4);
    _ = try core.connect(client);
    _ = try core.createTerminal(client, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(client, identifier(6), terminal_id, .{
        .input = true,
        .read = true,
        .resize = true,
    }, true);

    try std.testing.expectEqual(.applied, core.disconnect(client));
    try std.testing.expectEqual(@as(usize, 0), core.attachments.items.len);
    try std.testing.expect(core.terminal != null);
    try std.testing.expectEqual(TerminalPhase.running, core.terminal.?.phase);
    try std.testing.expect(!core.terminal.?.terminate_requested);
}

test "input, resize, and reordered completion preserve bounds" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(client);
    _ = try core.createTerminal(client, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(client, attachment_id, terminal_id, .{
        .input = true,
        .read = true,
        .resize = true,
    }, true);

    _ = try core.acceptInput(client, attachment_id, max_pending_input_bytes);
    try std.testing.expectError(error.InputBackpressure, core.acceptInput(client, attachment_id, 1));
    _ = try core.inputWritten(terminal_id, max_pending_input_bytes);
    _ = try core.resize(client, attachment_id, 120, 40);
    try std.testing.expectEqual(@as(u64, 0), core.terminal.?.revision);

    _ = try core.requestTerminate(client, terminal_id);
    try std.testing.expectEqual(.idempotent, try core.requestTerminate(client, terminal_id));
    _ = try core.ptyDrained(terminal_id);
    try std.testing.expectEqual(TerminalPhase.finishing, core.terminal.?.phase);
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    try std.testing.expectEqual(TerminalPhase.exited, core.terminal.?.phase);
    try std.testing.expectError(
        error.MissingResource,
        core.disposeTerminal(client, identifier(9)),
    );
    _ = try core.disposeTerminal(client, terminal_id);
    try std.testing.expect(core.terminal == null);
}

test "runtime revision synchronization rejects regression" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(client);
    _ = try core.createTerminal(client, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(client, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = true,
    }, true);
    _ = try core.synchronizeRevision(terminal_id, 8);
    try std.testing.expectEqual(@as(u64, 8), core.terminal.?.revision);
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 7));
}

test "projection demand gates one in-flight frame and coalesces later revisions" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(client);
    _ = try core.createTerminal(client, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(client, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 1);

    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 1, 1, .snapshot),
    );
    try std.testing.expectEqual(
        .applied,
        try core.setProjectionDemand(client, attachment_id, .active),
    );
    const first = try core.commitFrame(attachment_id, null, 1, 1, .snapshot);
    try std.testing.expectEqual(@as(u64, 1), first.sequence);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 1, 1, .snapshot),
    );

    _ = try core.synchronizeRevision(terminal_id, 9);
    try std.testing.expect(core.findAttachment(attachment_id).?.pending_dirty);
    try std.testing.expectError(
        error.InvalidFrameAcknowledgement,
        core.frameApplied(client, attachment_id, 1, 8, 1),
    );
    try std.testing.expectEqual(
        .applied,
        try core.frameApplied(client, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(client, attachment_id, 1, 1, 1),
    );
    const second = try core.commitFrame(attachment_id, 1, 9, 1, .delta);
    try std.testing.expectEqual(@as(u64, 2), second.sequence);
    try std.testing.expectEqual(@as(?u64, 1), second.base_revision);
}

test "recovery is exact and blocked projection requires explicit pause resume" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(client);
    _ = try core.createTerminal(client, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(client, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 3);
    _ = try core.setProjectionDemand(client, attachment_id, .active);
    _ = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);

    try std.testing.expectError(
        error.InvalidFrameAcknowledgement,
        core.needSnapshot(client, attachment_id, 2),
    );
    try std.testing.expectEqual(.applied, try core.needSnapshot(client, attachment_id, 1));
    try std.testing.expectEqual(.idempotent, try core.needSnapshot(client, attachment_id, 1));
    try std.testing.expect(core.findAttachment(attachment_id).?.force_snapshot);
    _ = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);
    _ = try core.frameApplied(client, attachment_id, 2, 3, 1);

    try core.blockProjection(attachment_id, .capture_allocation_failed);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 3, 1, .snapshot),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.setProjectionDemand(client, attachment_id, .active),
    );
    try std.testing.expect(core.findAttachment(attachment_id).?.blocked != null);
    _ = try core.setProjectionDemand(client, attachment_id, .paused);
    _ = try core.setProjectionDemand(client, attachment_id, .active);
    const attachment = core.findAttachment(attachment_id).?;
    try std.testing.expect(attachment.blocked == null);
    try std.testing.expect(attachment.force_snapshot);
    try std.testing.expect(attachment.pending_dirty);
}

test "attachments advance independently and geometry invalidates every delta base" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const first_client = identifier(2);
    const second_client = identifier(7);
    const terminal_id = identifier(4);
    const first_attachment = identifier(6);
    const second_attachment = identifier(8);
    _ = try core.connect(first_client);
    _ = try core.connect(second_client);
    _ = try core.createTerminal(first_client, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(first_client, first_attachment, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.attach(second_client, second_attachment, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 4);
    _ = try core.setProjectionDemand(first_client, first_attachment, .active);
    _ = try core.setProjectionDemand(second_client, second_attachment, .active);
    _ = try core.commitFrame(first_attachment, null, 4, 1, .snapshot);
    _ = try core.commitFrame(second_attachment, null, 4, 1, .snapshot);
    _ = try core.frameApplied(first_client, first_attachment, 1, 4, 1);

    _ = try core.synchronizeRevision(terminal_id, 5);
    try std.testing.expect(core.findAttachment(first_attachment).?.pending_dirty);
    try std.testing.expect(core.findAttachment(second_attachment).?.in_flight != null);
    const first_delta = try core.commitFrame(first_attachment, 4, 5, 1, .delta);
    try std.testing.expectEqual(@as(u64, 5), first_delta.revision);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(second_attachment, 4, 5, 1, .delta),
    );

    try core.synchronizeGeometry(terminal_id, 120, 40, 10, 20, 6);
    try std.testing.expect(core.findAttachment(first_attachment).?.force_snapshot);
    try std.testing.expect(core.findAttachment(second_attachment).?.force_snapshot);
}
