//! The acceptor's ordered admission checks: a pure decision over explicit facts. The endpoint owns
//! reading, slot counting, lifecycle, and the effects of a served request.

const std = @import("std");

const message = @import("message.zig");

/// What an acceptor knows about itself when it judges a frame.
pub const Profile = struct {
    connection: []const u8,
    generations: message.GenerationRange,
    capabilities: []const []const u8,
    product_version: []const u8,
    build: message.Build,
    environment: []const u8,
    /// Present exactly when `environment` is not `orchbee`.
    fingerprint: ?[]const u8,

    /// The rejection carrying this acceptor's facts. Use it directly for `malformed`, when the
    /// frame does not decode, and for `timeout`.
    pub fn reject(self: Profile, reason: message.RejectionReason) message.Rejection {
        return .{
            .reason = reason,
            .generations = self.generations,
            .product_version = self.product_version,
            .environment = self.environment,
        };
    }

    /// Runs the checks after decoding, in order, and stops at the first failure: connection kind,
    /// environment, then in `orchbee` the generation ranges or elsewhere the fingerprint, then the
    /// slot, then the lifecycle. A control request checks only its environment and a control slot;
    /// it is served while draining or stopping. `scratch` holds the answer's capability list.
    pub fn decide(
        self: Profile,
        scratch: std.mem.Allocator,
        initiator: message.InitiatorMessage,
        load: Load,
    ) error{OutOfMemory}!Decision {
        return switch (initiator) {
            .offer => |offer| self.decideOffer(scratch, offer, load),
            .request => |request| if (self.checkRequest(request.environment, load)) |reason|
                .{ .reject = self.reject(reason) }
            else
                .{ .serve = request.kind },
            .unknown_request => |request| if (self.checkRequest(request.environment, load)) |reason|
                .{ .reject = self.reject(reason) }
            else
                .{ .unsupported = .{ .request = request.request } },
        };
    }

    fn decideOffer(
        self: Profile,
        scratch: std.mem.Allocator,
        offer: message.Offer,
        load: Load,
    ) error{OutOfMemory}!Decision {
        if (!std.mem.eql(u8, offer.connection, self.connection)) {
            return .{ .reject = self.reject(.connection_unsupported) };
        }
        if (!std.mem.eql(u8, offer.environment, self.environment)) {
            return .{ .reject = self.reject(.environment_mismatch) };
        }
        const ours = self.generations;
        const theirs = offer.generations;
        const generation = if (self.fingerprint) |fingerprint| fingerprint: {
            // Equal fingerprints imply equal registries; disjoint ranges disprove that.
            const equal = if (offer.fingerprint) |offered| std.mem.eql(u8, offered, fingerprint) else false;
            const common = if (equal) theirs.highestCommon(ours) else null;
            break :fingerprint common orelse return .{ .reject = self.reject(.fingerprint_mismatch) };
        } else release: {
            if (theirs.highest < ours.lowest) return .{ .reject = self.reject(.initiator_too_old) };
            if (theirs.lowest > ours.highest) return .{ .reject = self.reject(.initiator_too_new) };
            // Both checks passed, so the ranges overlap and the lower highest is common.
            break :release @min(theirs.highest, ours.highest);
        };
        if (!load.session_slot_free) return .{ .reject = self.reject(.capacity) };
        if (load.state != .ready) return .{ .reject = self.reject(.stopping) };
        return .{ .accept = .{
            .generation = generation,
            .capabilities = try intersection(scratch, offer.capabilities, self.capabilities),
            .product_version = self.product_version,
            .build = self.build,
        } };
    }

    fn checkRequest(self: Profile, environment: []const u8, load: Load) ?message.RejectionReason {
        if (!std.mem.eql(u8, environment, self.environment)) return .environment_mismatch;
        if (!load.control_slot_free) return .capacity;
        return null;
    }
};

/// The acceptor's load when a frame arrives.
pub const Load = struct {
    session_slot_free: bool,
    control_slot_free: bool,
    /// Any state but `ready` refuses offers with `stopping`.
    state: message.ProcessState,
};

/// What the acceptor does with a decoded frame.
pub const Decision = union(enum) {
    /// Send the answer; the connection continues at its generation.
    accept: message.Answer,
    /// Perform the request, send its one reply, and close.
    serve: message.RequestKind,
    /// Send this reply to an unknown request type and close.
    unsupported: message.Unsupported,
    /// Send the rejection and close.
    reject: message.Rejection,
};

/// The names both ascending lists contain, in ascending order.
fn intersection(
    scratch: std.mem.Allocator,
    left: []const []const u8,
    right: []const []const u8,
) error{OutOfMemory}![]const []const u8 {
    var shared: std.ArrayList([]const u8) = .empty;
    var left_index: usize = 0;
    var right_index: usize = 0;
    while (left_index < left.len and right_index < right.len) {
        switch (std.mem.order(u8, left[left_index], right[right_index])) {
            .lt => left_index += 1,
            .gt => right_index += 1,
            .eq => {
                try shared.append(scratch, left[left_index]);
                left_index += 1;
                right_index += 1;
            },
        }
    }
    return shared.toOwnedSlice(scratch);
}
