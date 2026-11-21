//! The requests held behind a creation in progress: which requests depend on it, and the order in
//! which they are released once it settles.
//!
//! Every request that names the terminal a creation creates or starts, every other creation, and
//! every request that changes what a creation is admitted with is held in arrival order. Each is
//! released once nothing before it holds it, so it meets the state the creation left.
const std = @import("std");

const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const protocol = @import("terminal-protocol");

/// Requests held behind a creation before the session stops reading more of them.
pub const held_request_limit = 16;

pub const HoldError = error{ HeldRequestLimit, OutOfMemory };

/// What a request depends on, from its kind and the terminal its payload names.
const Dependence = struct {
    /// It creates or starts a terminal, or changes what a later creation is admitted with.
    creation: bool = false,
    terminal: ?core_module.TerminalId = null,
};

const HeldRequest = struct {
    record: []u8,
    dependence: Dependence,
};

/// Owned by the dispatch thread. `creating` names the terminal of the creation in progress, or is
/// null when none is.
pub const Holds = struct {
    held: std.ArrayList(HeldRequest) = .empty,

    /// Whether the session may read another request: false while the held-request limit is
    /// reached, until the creation they wait for settles.
    pub fn acceptsRequests(self: *const Holds) bool {
        return self.held.items.len < held_request_limit;
    }

    /// Holds `record` when it depends on the creation in progress or on a request already held,
    /// and returns whether it did.
    pub fn holdIfDependent(
        self: *Holds,
        allocator: std.mem.Allocator,
        creating: ?core_module.TerminalId,
        record: []const u8,
    ) HoldError!bool {
        if (creating == null and self.held.items.len == 0) return false;
        const dependence = dependenceOf(allocator, record);
        if (!dependsOnEarlier(dependence, creating, self.held.items)) return false;
        if (self.held.items.len >= held_request_limit) return error.HeldRequestLimit;
        try self.held.ensureUnusedCapacity(allocator, 1);
        const copy = try allocator.dupe(u8, record);
        self.held.appendAssumeCapacity(.{ .record = copy, .dependence = dependence });
        return true;
    }

    /// The first held request that nothing before it holds any longer. The caller owns the
    /// record and dispatches it without holding it again.
    pub fn takeReleased(self: *Holds, creating: ?core_module.TerminalId) ?[]u8 {
        for (self.held.items, 0..) |held, index| {
            if (dependsOnEarlier(held.dependence, creating, self.held.items[0..index])) continue;
            _ = self.held.orderedRemove(index);
            return held.record;
        }
        return null;
    }

    /// Drops every held request unanswered, as a closed session's requests are.
    pub fn drop(self: *Holds, allocator: std.mem.Allocator) void {
        for (self.held.items) |held| allocator.free(held.record);
        self.held.clearRetainingCapacity();
    }

    pub fn deinit(self: *Holds, allocator: std.mem.Allocator) void {
        self.drop(allocator);
        self.held.deinit(allocator);
        self.* = .{};
    }
};

fn dependsOnEarlier(
    dependence: Dependence,
    creating: ?core_module.TerminalId,
    earlier: []const HeldRequest,
) bool {
    if (creating) |creating_id| {
        if (dependence.creation) return true;
        if (dependence.terminal) |terminal_id| {
            if (terminal_id.eql(creating_id)) return true;
        }
    }
    for (earlier) |held| {
        if (dependence.creation and held.dependence.creation) return true;
        const terminal_id = dependence.terminal orelse continue;
        const held_terminal_id = held.dependence.terminal orelse continue;
        if (terminal_id.eql(held_terminal_id)) return true;
    }
    return false;
}

fn dependenceOf(allocator: std.mem.Allocator, record: []const u8) Dependence {
    const envelope = protocol.validateTransportEnvelope(record) catch return .{};
    if (envelope.event or envelope.response) return .{};
    const contract = protocol.messageContract(envelope.kind) orelse return .{};
    const request = contract.request orelse return .{};
    return switch (request.operation) {
        .create_terminal, .stage_terminal_output, .set_terminal_defaults, .restore_terminal => .{ .creation = true },
        .start_terminal => .{
            .creation = true,
            .terminal = control.decode.namedTerminal(allocator, envelope.payload),
        },
        .terminate_terminal,
        .force_terminate_terminal,
        .dispose_terminal,
        .attach_terminal,
        .update_terminal_configuration,
        .apply_terminal_theme,
        .quiesce_terminal,
        .resume_terminal,
        .drop_terminal,
        .claim_terminal,
        // Only a `DiscardImport` that names the restored terminal; the import's moves owner holds
        // one that names the import until the restore's build ends.
        .discard_import,
        => .{ .terminal = control.decode.namedTerminal(allocator, envelope.payload) },
        // Attachment-scoped requests cannot name a creation: no attachment admits a terminal
        // that is not yet running.
        else => .{},
    };
}
