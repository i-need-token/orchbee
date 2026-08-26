const std = @import("std");

const identifiers = @import("terminal-host-model").identifiers;
const terminal_session = @import("terminal_session.zig");

pub const CreationId = identifiers.CreationId;
pub const TerminalId = identifiers.TerminalId;
pub const LaunchDigest = terminal_session.LaunchDigest;
pub const Session = terminal_session.Session;

pub const Error = error{
    CreationConflict,
    InventoryGenerationOverflow,
    MissingResource,
    TerminalConflict,
};

/// Owns terminal membership, the creation-identity binding index, and the
/// inventory generation as one consistency boundary. Session allocations stay
/// with the caller: an admitted pointer must remain stable until removal, and
/// the returned pointer becomes reclaimable only after both indexes commit its
/// removal. Attachment indexes are intentionally not part of this owner: they
/// belong to the active broker.
pub const Registry = struct {
    allocator: std.mem.Allocator,
    creations: std.AutoArrayHashMapUnmanaged(CreationId, TerminalId),
    inventory_generation: u64 = 1,
    terminals: std.AutoArrayHashMapUnmanaged(TerminalId, *Session),

    pub fn init(allocator: std.mem.Allocator) Registry {
        return .{
            .allocator = allocator,
            .creations = .empty,
            .terminals = .empty,
        };
    }

    pub fn deinit(self: *Registry) void {
        self.creations.deinit(self.allocator);
        self.terminals.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn generation(self: *const Registry) u64 {
        return self.inventory_generation;
    }

    pub fn count(self: *const Registry) usize {
        return self.terminals.count();
    }

    pub fn keys(self: *const Registry) []const TerminalId {
        return self.terminals.keys();
    }

    pub fn values(self: *const Registry) []const *const Session {
        return self.terminals.values();
    }

    pub fn find(self: *Registry, terminal_id: TerminalId) ?*Session {
        return self.terminals.get(terminal_id);
    }

    pub fn get(self: *const Registry, terminal_id: TerminalId) ?*const Session {
        return self.terminals.get(terminal_id);
    }

    pub fn resolveCreation(
        self: *const Registry,
        creation_id: CreationId,
        launch_digest: LaunchDigest,
    ) Error!?TerminalId {
        const terminal_id = self.creations.get(creation_id) orelse return null;
        const terminal = self.terminals.get(terminal_id) orelse return error.MissingResource;
        if (!std.mem.eql(u8, &terminal.launch_digest, &launch_digest)) {
            return error.CreationConflict;
        }
        return terminal_id;
    }

    pub fn reserveAdmission(
        self: *Registry,
    ) (Error || std.mem.Allocator.Error)!void {
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
        try self.terminals.ensureUnusedCapacity(self.allocator, 1);
        try self.creations.ensureUnusedCapacity(self.allocator, 1);
    }

    pub fn validateAdmission(
        self: *const Registry,
        creation_id: CreationId,
        terminal_id: TerminalId,
    ) Error!void {
        if (self.creations.contains(creation_id)) return error.CreationConflict;
        if (self.terminals.contains(terminal_id)) return error.TerminalConflict;
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
    }

    /// Commits both registry indexes and their generation without allocation.
    /// The caller must have completed `reserveAdmission`, validated that the
    /// single-threaded Host owner has not changed the admission facts, and
    /// supplied storage whose address remains stable through removal.
    pub fn commitAssumeCapacity(self: *Registry, session: *Session) void {
        std.debug.assert(!self.creations.contains(session.creation_id));
        std.debug.assert(!self.terminals.contains(session.terminal_id));
        std.debug.assert(self.inventory_generation != std.math.maxInt(u64));
        self.terminals.putAssumeCapacityNoClobber(session.terminal_id, session);
        self.creations.putAssumeCapacityNoClobber(session.creation_id, session.terminal_id);
        self.inventory_generation += 1;
    }

    pub fn requireGenerationAdvance(self: *const Registry) Error!void {
        if (self.inventory_generation == std.math.maxInt(u64)) {
            return error.InventoryGenerationOverflow;
        }
    }

    /// Removes a retained session and its creation binding as one infallible
    /// commit. Observable removal may swap the final index slots into the
    /// removed positions before advancing the inventory generation. Inventory
    /// generation is intentionally unchanged during fatal Host teardown
    /// because no later inventory can be observed.
    pub fn removeAssumePresent(
        self: *Registry,
        terminal_id: TerminalId,
        update_inventory_generation: bool,
    ) *Session {
        const terminal = self.terminals.get(terminal_id).?;
        const creation_terminal_id = self.creations.get(terminal.creation_id).?;
        std.debug.assert(creation_terminal_id.eql(terminal_id));
        if (update_inventory_generation) {
            std.debug.assert(self.inventory_generation != std.math.maxInt(u64));
        }
        _ = self.creations.swapRemove(terminal.creation_id);
        _ = self.terminals.swapRemove(terminal_id);
        if (update_inventory_generation) self.inventory_generation += 1;
        return terminal;
    }
};

pub const testing = if (@import("builtin").is_test) struct {
    pub fn creationCount(registry: *const Registry) usize {
        return registry.creations.count();
    }

    pub fn creationKeys(registry: *const Registry) []const CreationId {
        return registry.creations.keys();
    }

    pub fn creationValues(registry: *const Registry) []const TerminalId {
        return registry.creations.values();
    }

    pub fn creationTerminal(registry: *const Registry, creation_id: CreationId) ?TerminalId {
        return registry.creations.get(creation_id);
    }

    pub fn containsCreation(registry: *const Registry, creation_id: CreationId) bool {
        return registry.creations.contains(creation_id);
    }

    pub fn injectCreationBinding(
        registry: *Registry,
        creation_id: CreationId,
        terminal_id: TerminalId,
    ) !void {
        try registry.creations.putNoClobber(registry.allocator, creation_id, terminal_id);
    }

    pub fn setInventoryGeneration(registry: *Registry, generation: u64) void {
        registry.inventory_generation = generation;
    }
} else struct {};
