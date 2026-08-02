const std = @import("std");

pub const Identifier = [16]u8;
pub const LaunchDigest = [32]u8;
pub const ConnectionId = u64;

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
    force_terminate_requested: bool = false,
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
    connection_id: ConnectionId,
    demand: ProjectionDemand = .paused,
    force_snapshot: bool = true,
    in_flight: ?InFlightFrame = null,
    last_abandoned_sequence: u64 = 0,
    next_sequence: u64 = 1,
    pending_dirty: bool = false,
    resize_lease: bool,
    terminal_id: Identifier,
};

pub const CoreError = error{
    AttachmentConflict,
    ConnectionConflict,
    CreationConflict,
    InputBackpressure,
    InvalidFrameAcknowledgement,
    InvalidPhase,
    InventoryGenerationOverflow,
    MissingConnection,
    MissingResource,
    RevisionOverflow,
    ResizeLeaseDenied,
    SequenceOverflow,
    TerminalConflict,
};

pub const Connection = struct {
    client_id: Identifier,
};

pub const RequiredCapability = enum {
    none,
    input,
    read,
    resize,
};

pub const HostCore = struct {
    allocator: std.mem.Allocator,
    attachments: std.AutoArrayHashMap(Identifier, Attachment),
    connections: std.AutoArrayHashMap(ConnectionId, Connection),
    creations: std.AutoArrayHashMap(Identifier, Identifier),
    epoch: Identifier,
    inventory_generation: u64 = 1,
    terminals: std.AutoArrayHashMap(Identifier, Terminal),

    pub fn init(allocator: std.mem.Allocator, epoch: Identifier) HostCore {
        return .{
            .allocator = allocator,
            .attachments = .init(allocator),
            .connections = .init(allocator),
            .creations = .init(allocator),
            .epoch = epoch,
            .terminals = .init(allocator),
        };
    }

    pub fn deinit(self: *HostCore) void {
        self.attachments.deinit();
        self.connections.deinit();
        self.creations.deinit();
        self.terminals.deinit();
        self.* = undefined;
    }

    pub fn connect(
        self: *HostCore,
        connection_id: ConnectionId,
        client_id: Identifier,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        if (self.connections.get(connection_id)) |connection| {
            return if (equalIdentifier(connection.client_id, client_id))
                .idempotent
            else
                error.ConnectionConflict;
        }
        try self.connections.putNoClobber(connection_id, .{ .client_id = client_id });
        return .applied;
    }

    pub fn disconnect(self: *HostCore, connection_id: ConnectionId) TransitionOutcome {
        if (!self.connections.orderedRemove(connection_id)) return .idempotent;

        var index: usize = self.attachments.count();
        while (index > 0) {
            index -= 1;
            if (self.attachments.values()[index].connection_id == connection_id) {
                self.attachments.orderedRemoveAt(index);
            }
        }
        return .applied;
    }

    pub fn resolveCreation(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: Identifier,
        launch_digest: LaunchDigest,
    ) CoreError!?Identifier {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal_id = self.creations.get(creation_id) orelse return null;
        const terminal = self.terminals.get(terminal_id) orelse unreachable;
        if (!std.mem.eql(u8, &terminal.launch_digest, &launch_digest)) {
            return error.CreationConflict;
        }
        return terminal_id;
    }

    pub fn reserveTerminal(self: *HostCore) (CoreError || std.mem.Allocator.Error)!void {
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
        try self.terminals.ensureUnusedCapacity(1);
        try self.creations.ensureUnusedCapacity(1);
    }

    pub fn commitTerminal(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: Identifier,
        terminal_id: Identifier,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
    ) CoreError!void {
        if (!self.connected(connection_id)) return error.MissingConnection;
        if (self.creations.contains(creation_id)) return error.CreationConflict;
        if (self.terminals.contains(terminal_id)) return error.TerminalConflict;
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
        self.terminals.putAssumeCapacityNoClobber(terminal_id, .{
            .columns = columns,
            .creation_id = creation_id,
            .launch_digest = launch_digest,
            .rows = rows,
            .terminal_id = terminal_id,
        });
        self.creations.putAssumeCapacityNoClobber(creation_id, terminal_id);
        self.inventory_generation += 1;
    }

    pub fn createTerminal(
        self: *HostCore,
        connection_id: ConnectionId,
        creation_id: Identifier,
        terminal_id: Identifier,
        launch_digest: LaunchDigest,
        columns: u16,
        rows: u16,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        if (try self.resolveCreation(connection_id, creation_id, launch_digest)) |_| {
            return .idempotent;
        }
        try self.reserveTerminal();
        try self.commitTerminal(
            connection_id,
            creation_id,
            terminal_id,
            launch_digest,
            columns,
            rows,
        );
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
        const terminal = self.terminals.get(terminal_id) orelse return .idempotent;
        if (terminal.phase != .starting) return error.InvalidPhase;
        self.removeTerminalAttachments(terminal_id);
        _ = self.creations.orderedRemove(terminal.creation_id);
        _ = self.terminals.orderedRemove(terminal_id);
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
        self.inventory_generation += 1;
        return .applied;
    }

    pub fn requestTerminate(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.terminate_requested) return .idempotent;
        if (terminal.phase != .running) return error.InvalidPhase;
        terminal.terminate_requested = true;
        return .applied;
    }

    pub fn requestForceTerminate(
        self: *HostCore,
        connection_id: ConnectionId,
        terminal_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal = try self.requireTerminal(terminal_id);
        if (terminal.force_terminate_requested or terminal.phase == .exited) return .idempotent;
        if (!terminal.terminate_requested or
            (terminal.phase != .running and terminal.phase != .finishing))
        {
            return error.InvalidPhase;
        }
        terminal.force_terminate_requested = true;
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
        connection_id: ConnectionId,
        terminal_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const terminal = self.terminals.get(terminal_id) orelse return .idempotent;
        if (terminal.phase != .exited) return error.InvalidPhase;
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
        self.removeTerminalAttachments(terminal_id);
        _ = self.creations.orderedRemove(terminal.creation_id);
        _ = self.terminals.orderedRemove(terminal_id);
        self.inventory_generation += 1;
        return .applied;
    }

    pub fn attach(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: Identifier,
        terminal_id: Identifier,
        capabilities: Capabilities,
        request_resize_lease: bool,
    ) (CoreError || std.mem.Allocator.Error)!TransitionOutcome {
        if (!self.connected(connection_id)) return error.MissingConnection;
        _ = try self.requireTerminal(terminal_id);

        if (self.findAttachment(attachment_id)) |existing| {
            const equivalent = existing.connection_id == connection_id and
                equalIdentifier(existing.terminal_id, terminal_id) and
                existing.capabilities == capabilities and
                existing.resize_lease == (request_resize_lease and capabilities.resize);
            return if (equivalent) .idempotent else error.AttachmentConflict;
        }
        if (request_resize_lease and !capabilities.resize) return error.ResizeLeaseDenied;
        if (request_resize_lease) {
            for (self.attachments.values()) |attachment| {
                if (equalIdentifier(attachment.terminal_id, terminal_id) and attachment.resize_lease) {
                    return error.ResizeLeaseDenied;
                }
            }
        }

        try self.attachments.putNoClobber(attachment_id, .{
            .attachment_id = attachment_id,
            .capabilities = capabilities,
            .connection_id = connection_id,
            .resize_lease = request_resize_lease,
            .terminal_id = terminal_id,
        });
        return .applied;
    }

    pub fn detach(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: Identifier,
    ) CoreError!TransitionOutcome {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return .idempotent;
        if (attachment.connection_id != connection_id) {
            return error.MissingResource;
        }
        _ = self.attachments.orderedRemove(attachment_id);
        return .applied;
    }

    pub fn acceptInput(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: Identifier,
        byte_count: usize,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveAttachment(connection_id, attachment_id, .input);
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
        connection_id: ConnectionId,
        attachment_id: Identifier,
        columns: u16,
        rows: u16,
    ) CoreError!TransitionOutcome {
        const attachment = self.resolveAttachment(connection_id, attachment_id, .resize) catch |err| {
            return if (err == error.MissingResource) error.ResizeLeaseDenied else err;
        };
        if (!attachment.resize_lease) {
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
        for (self.attachments.values()) |*attachment| {
            if (equalIdentifier(attachment.terminal_id, terminal_id) and
                attachment.capabilities.read and attachment.demand == .active)
            {
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
        for (self.attachments.values()) |*attachment| {
            if (equalIdentifier(attachment.terminal_id, terminal_id) and
                attachment.capabilities.read and attachment.demand == .active)
            {
                if (geometry_changed) attachment.force_snapshot = true;
                attachment.pending_dirty = true;
                if (geometry_changed) attachment.blocked = null;
            }
        }
    }

    pub fn setProjectionDemand(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: Identifier,
        demand: ProjectionDemand,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveAttachment(connection_id, attachment_id, .read);
        if (attachment.demand == demand) return .idempotent;
        attachment.demand = demand;
        if (demand == .active) {
            attachment.blocked = null;
            attachment.force_snapshot = true;
            attachment.pending_dirty = true;
        } else {
            if (attachment.in_flight) |flight| {
                attachment.last_abandoned_sequence = @max(
                    attachment.last_abandoned_sequence,
                    flight.sequence,
                );
            }
            attachment.acknowledged_generation = null;
            attachment.acknowledged_revision = null;
            attachment.blocked = null;
            attachment.force_snapshot = true;
            attachment.in_flight = null;
            attachment.pending_dirty = false;
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
        connection_id: ConnectionId,
        attachment_id: Identifier,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveAttachment(connection_id, attachment_id, .read);
        if (sequence != 0 and sequence <= attachment.last_abandoned_sequence) return .idempotent;
        if (attachment.in_flight) |flight| {
            if (flight.sequence != sequence or flight.revision != revision or flight.generation != generation) {
                return error.InvalidFrameAcknowledgement;
            }
            attachment.acknowledged_revision = revision;
            attachment.acknowledged_generation = generation;
            attachment.in_flight = null;
            attachment.blocked = null;
            const terminal = try self.requireTerminal(attachment.terminal_id);
            attachment.pending_dirty = terminal.revision > revision;
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
        connection_id: ConnectionId,
        attachment_id: Identifier,
        sequence: u64,
    ) CoreError!TransitionOutcome {
        const attachment = try self.resolveAttachment(connection_id, attachment_id, .read);
        if (sequence != 0 and sequence <= attachment.last_abandoned_sequence) return .idempotent;
        const flight = attachment.in_flight orelse return error.InvalidFrameAcknowledgement;
        if (flight.sequence != sequence) return error.InvalidFrameAcknowledgement;
        attachment.in_flight = null;
        attachment.last_abandoned_sequence = @max(attachment.last_abandoned_sequence, sequence);
        attachment.force_snapshot = true;
        attachment.pending_dirty = true;
        attachment.blocked = null;
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
        return self.attachments.getPtr(attachment_id);
    }

    pub fn findTerminal(self: *HostCore, terminal_id: Identifier) ?*Terminal {
        return self.terminals.getPtr(terminal_id);
    }

    pub fn resolveAttachment(
        self: *HostCore,
        connection_id: ConnectionId,
        attachment_id: Identifier,
        required: RequiredCapability,
    ) CoreError!*Attachment {
        if (!self.connected(connection_id)) return error.MissingConnection;
        const attachment = self.findAttachment(attachment_id) orelse return error.MissingResource;
        if (attachment.connection_id != connection_id) return error.MissingResource;
        const permitted = switch (required) {
            .none => true,
            .input => attachment.capabilities.input,
            .read => attachment.capabilities.read,
            .resize => attachment.capabilities.resize,
        };
        if (!permitted) return error.MissingResource;
        _ = try self.requireTerminal(attachment.terminal_id);
        return attachment;
    }

    pub fn hasActiveProjectionDemand(self: *const HostCore, terminal_id: Identifier) bool {
        for (self.attachments.values()) |attachment| {
            if (equalIdentifier(attachment.terminal_id, terminal_id) and
                attachment.capabilities.read and attachment.demand == .active)
            {
                return true;
            }
        }
        return false;
    }

    pub fn terminalCount(self: *const HostCore) usize {
        return self.terminals.count();
    }

    pub fn connectionCount(self: *const HostCore) usize {
        return self.connections.count();
    }

    pub fn connectionClientId(self: *const HostCore, connection_id: ConnectionId) ?Identifier {
        const connection = self.connections.get(connection_id) orelse return null;
        return connection.client_id;
    }

    fn connected(self: *const HostCore, connection_id: ConnectionId) bool {
        return self.connections.contains(connection_id);
    }

    fn requireTerminal(self: *HostCore, terminal_id: Identifier) CoreError!*Terminal {
        return self.terminals.getPtr(terminal_id) orelse error.MissingResource;
    }

    fn removeTerminalAttachments(self: *HostCore, terminal_id: Identifier) void {
        var index: usize = self.attachments.count();
        while (index > 0) {
            index -= 1;
            if (equalIdentifier(self.attachments.values()[index].terminal_id, terminal_id)) {
                self.attachments.orderedRemoveAt(index);
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

fn identifier(byte: u8) Identifier {
    return [_]u8{byte} ** 16;
}

test "multi-terminal creation is idempotent without a product count cap" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const creation = identifier(3);
    const terminal = identifier(4);
    const digest = [_]u8{5} ** 32;

    try std.testing.expectEqual(.applied, try core.connect(connection, identifier(2)));
    try std.testing.expectEqual(
        .applied,
        try core.createTerminal(connection, creation, terminal, digest, 80, 24),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.createTerminal(connection, creation, identifier(9), digest, 80, 24),
    );
    try std.testing.expectError(
        error.CreationConflict,
        core.createTerminal(connection, creation, terminal, [_]u8{6} ** 32, 80, 24),
    );
    try std.testing.expectEqual(
        .applied,
        try core.createTerminal(connection, identifier(7), identifier(8), digest, 100, 30),
    );
    try std.testing.expectEqual(@as(usize, 2), core.terminalCount());
    try std.testing.expectEqual(@as(u16, 100), core.findTerminal(identifier(8)).?.columns);
}

test "disconnect removes only that physical connection's attachments" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const client = identifier(2);
    const first_connection: ConnectionId = 2;
    const second_connection: ConnectionId = 7;
    const terminal_id = identifier(4);
    _ = try core.connect(first_connection, client);
    _ = try core.connect(second_connection, client);
    _ = try core.createTerminal(first_connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(first_connection, identifier(6), terminal_id, .{
        .input = true,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.attach(second_connection, identifier(8), terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);

    try std.testing.expectEqual(.applied, core.disconnect(first_connection));
    try std.testing.expectEqual(@as(usize, 1), core.attachments.count());
    try std.testing.expect(core.findAttachment(identifier(6)) == null);
    try std.testing.expect(core.findAttachment(identifier(8)) != null);
    try std.testing.expectEqual(TerminalPhase.running, core.findTerminal(terminal_id).?.phase);
    try std.testing.expect(!core.findTerminal(terminal_id).?.terminate_requested);
}

test "attachment routing and terminal disposal are exact across registries" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const first_connection: ConnectionId = 2;
    const second_connection: ConnectionId = 3;
    const first_terminal = identifier(4);
    const second_terminal = identifier(5);
    const first_attachment = identifier(6);
    const second_attachment = identifier(7);
    _ = try core.connect(first_connection, identifier(8));
    _ = try core.connect(second_connection, identifier(8));
    _ = try core.createTerminal(first_connection, identifier(9), first_terminal, [_]u8{10} ** 32, 80, 24);
    _ = try core.createTerminal(second_connection, identifier(11), second_terminal, [_]u8{12} ** 32, 100, 30);
    _ = try core.spawnSucceeded(first_terminal);
    _ = try core.spawnSucceeded(second_terminal);
    _ = try core.attach(first_connection, first_attachment, first_terminal, .{
        .input = true,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.attach(second_connection, second_attachment, second_terminal, .{
        .input = true,
        .read = true,
        .resize = false,
    }, false);

    try std.testing.expectEqualSlices(
        u8,
        &first_terminal,
        &(try core.resolveAttachment(first_connection, first_attachment, .input)).terminal_id,
    );
    try std.testing.expectEqualSlices(
        u8,
        &second_terminal,
        &(try core.resolveAttachment(second_connection, second_attachment, .read)).terminal_id,
    );
    try std.testing.expectError(
        error.MissingResource,
        core.resolveAttachment(first_connection, second_attachment, .none),
    );

    _ = try core.childExited(first_terminal, .{ .exit_code = 0 });
    _ = try core.ptyDrained(first_terminal);
    _ = try core.disposeTerminal(first_connection, first_terminal);
    try std.testing.expect(core.findTerminal(first_terminal) == null);
    try std.testing.expect(core.findAttachment(first_attachment) == null);
    try std.testing.expect(core.findTerminal(second_terminal) != null);
    try std.testing.expect(core.findAttachment(second_attachment) != null);
}

test "inventory generation exhaustion leaves membership unchanged" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    _ = try core.connect(connection, identifier(3));
    core.inventory_generation = std.math.maxInt(u64);

    try std.testing.expectError(
        error.InventoryGenerationOverflow,
        core.createTerminal(connection, identifier(4), identifier(5), [_]u8{6} ** 32, 80, 24),
    );
    try std.testing.expectEqual(@as(usize, 0), core.terminalCount());
    try std.testing.expectEqual(@as(usize, 0), core.creations.count());
}

test "input, resize, and reordered completion preserve bounds" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(connection, identifier(2));
    _ = try core.createTerminal(connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = true,
        .read = true,
        .resize = true,
    }, true);

    _ = try core.acceptInput(connection, attachment_id, max_pending_input_bytes);
    try std.testing.expectError(error.InputBackpressure, core.acceptInput(connection, attachment_id, 1));
    _ = try core.inputWritten(terminal_id, max_pending_input_bytes);
    _ = try core.resize(connection, attachment_id, 120, 40);
    try std.testing.expectEqual(@as(u64, 0), core.findTerminal(terminal_id).?.revision);

    try std.testing.expectError(
        error.InvalidPhase,
        core.requestForceTerminate(connection, terminal_id),
    );
    _ = try core.requestTerminate(connection, terminal_id);
    try std.testing.expectEqual(.idempotent, try core.requestTerminate(connection, terminal_id));
    try std.testing.expectEqual(.applied, try core.requestForceTerminate(connection, terminal_id));
    try std.testing.expectEqual(.idempotent, try core.requestForceTerminate(connection, terminal_id));
    try std.testing.expect(core.findTerminal(terminal_id).?.force_terminate_requested);
    _ = try core.ptyDrained(terminal_id);
    try std.testing.expectEqual(TerminalPhase.finishing, core.findTerminal(terminal_id).?.phase);
    _ = try core.childExited(terminal_id, .{ .exit_code = 0 });
    try std.testing.expectEqual(TerminalPhase.exited, core.findTerminal(terminal_id).?.phase);
    try std.testing.expectEqual(.idempotent, try core.disposeTerminal(connection, identifier(9)));
    _ = try core.disposeTerminal(connection, terminal_id);
    try std.testing.expect(core.findTerminal(terminal_id) == null);
}

test "runtime revision synchronization rejects regression" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(connection, identifier(2));
    _ = try core.createTerminal(connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = true,
    }, true);
    _ = try core.synchronizeRevision(terminal_id, 8);
    try std.testing.expectEqual(@as(u64, 8), core.findTerminal(terminal_id).?.revision);
    try std.testing.expectError(error.InvalidPhase, core.synchronizeRevision(terminal_id, 7));
}

test "projection demand gates one in-flight frame and coalesces later revisions" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(connection, identifier(2));
    _ = try core.createTerminal(connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
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
        try core.setProjectionDemand(connection, attachment_id, .active),
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
        core.frameApplied(connection, attachment_id, 1, 8, 1),
    );
    try std.testing.expectEqual(
        .applied,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    const second = try core.commitFrame(attachment_id, 1, 9, 1, .delta);
    try std.testing.expectEqual(@as(u64, 2), second.sequence);
    try std.testing.expectEqual(@as(?u64, 1), second.base_revision);
    _ = try core.frameApplied(connection, attachment_id, 2, 9, 1);
    _ = try core.setProjectionDemand(connection, attachment_id, .paused);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    const resumed = core.findAttachment(attachment_id).?;
    try std.testing.expect(resumed.force_snapshot);
    try std.testing.expect(resumed.pending_dirty);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, 9, 9, 1, .delta),
    );
    _ = try core.commitFrame(attachment_id, null, 9, 1, .snapshot);
}

test "pause abandons delivery state and keeps hidden revisions attachment lazy" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(connection, identifier(2));
    _ = try core.createTerminal(connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);

    _ = try core.synchronizeRevision(terminal_id, 1);
    try std.testing.expect(!core.findAttachment(attachment_id).?.pending_dirty);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    _ = try core.commitFrame(attachment_id, null, 1, 1, .snapshot);
    _ = try core.synchronizeRevision(terminal_id, 2);
    _ = try core.setProjectionDemand(connection, attachment_id, .paused);

    const paused = core.findAttachment(attachment_id).?;
    try std.testing.expect(paused.in_flight == null);
    try std.testing.expect(paused.acknowledged_revision == null);
    try std.testing.expectEqual(@as(u64, 1), paused.last_abandoned_sequence);
    try std.testing.expect(!paused.pending_dirty);
    try std.testing.expect(paused.force_snapshot);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_id));

    _ = try core.synchronizeRevision(terminal_id, 3);
    try std.testing.expect(!core.findAttachment(attachment_id).?.pending_dirty);
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.needSnapshot(connection, attachment_id, 1),
    );

    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    const resumed = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);
    try std.testing.expectEqual(@as(u64, 2), resumed.sequence);
    try std.testing.expectEqual(
        .idempotent,
        try core.frameApplied(connection, attachment_id, 1, 1, 1),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.needSnapshot(connection, attachment_id, 1),
    );
    try std.testing.expectEqual(
        @as(u64, 2),
        core.findAttachment(attachment_id).?.in_flight.?.sequence,
    );
}

test "active projection demand is aggregated across attachments" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = identifier(4);
    const first_attachment = identifier(6);
    const second_attachment = identifier(7);
    _ = try core.connect(connection, identifier(2));
    _ = try core.createTerminal(connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    for ([_]Identifier{ first_attachment, second_attachment }) |attachment_id| {
        _ = try core.attach(connection, attachment_id, terminal_id, .{
            .input = false,
            .read = true,
            .resize = false,
        }, false);
        _ = try core.setProjectionDemand(connection, attachment_id, .active);
    }

    try std.testing.expect(core.hasActiveProjectionDemand(terminal_id));
    _ = try core.setProjectionDemand(connection, first_attachment, .paused);
    try std.testing.expect(core.hasActiveProjectionDemand(terminal_id));
    _ = try core.detach(connection, second_attachment);
    try std.testing.expect(!core.hasActiveProjectionDemand(terminal_id));
}

test "recovery is exact and blocked projection requires explicit pause resume" {
    var core = HostCore.init(std.testing.allocator, identifier(1));
    defer core.deinit();
    const connection: ConnectionId = 2;
    const terminal_id = identifier(4);
    const attachment_id = identifier(6);
    _ = try core.connect(connection, identifier(2));
    _ = try core.createTerminal(connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(connection, attachment_id, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 3);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
    _ = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);

    try std.testing.expectError(
        error.InvalidFrameAcknowledgement,
        core.needSnapshot(connection, attachment_id, 2),
    );
    try std.testing.expectEqual(.applied, try core.needSnapshot(connection, attachment_id, 1));
    try std.testing.expectEqual(.idempotent, try core.needSnapshot(connection, attachment_id, 1));
    try std.testing.expect(core.findAttachment(attachment_id).?.force_snapshot);
    _ = try core.commitFrame(attachment_id, null, 3, 1, .snapshot);
    _ = try core.frameApplied(connection, attachment_id, 2, 3, 1);

    try core.blockProjection(attachment_id, .capture_allocation_failed);
    try std.testing.expectError(
        error.InvalidPhase,
        core.commitFrame(attachment_id, null, 3, 1, .snapshot),
    );
    try std.testing.expectEqual(
        .idempotent,
        try core.setProjectionDemand(connection, attachment_id, .active),
    );
    try std.testing.expect(core.findAttachment(attachment_id).?.blocked != null);
    _ = try core.setProjectionDemand(connection, attachment_id, .paused);
    _ = try core.setProjectionDemand(connection, attachment_id, .active);
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
    const first_connection: ConnectionId = 2;
    const second_connection: ConnectionId = 7;
    const terminal_id = identifier(4);
    const first_attachment = identifier(6);
    const second_attachment = identifier(8);
    _ = try core.connect(first_connection, first_client);
    _ = try core.connect(second_connection, second_client);
    _ = try core.createTerminal(first_connection, identifier(3), terminal_id, [_]u8{5} ** 32, 80, 24);
    _ = try core.spawnSucceeded(terminal_id);
    _ = try core.attach(first_connection, first_attachment, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.attach(second_connection, second_attachment, terminal_id, .{
        .input = false,
        .read = true,
        .resize = false,
    }, false);
    _ = try core.synchronizeRevision(terminal_id, 4);
    _ = try core.setProjectionDemand(first_connection, first_attachment, .active);
    _ = try core.setProjectionDemand(second_connection, second_attachment, .active);
    _ = try core.commitFrame(first_attachment, null, 4, 1, .snapshot);
    _ = try core.commitFrame(second_attachment, null, 4, 1, .snapshot);
    _ = try core.frameApplied(first_connection, first_attachment, 1, 4, 1);

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
