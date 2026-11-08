//! The Host's acceptor side of the `OBHS` handshake on its broker endpoint. Every connection's
//! first frame is judged here before any OBTP byte: an admitted Worker offer continues as the
//! single Worker session, and a control request gets its one reply. The decision reads only the
//! build's identity and an explicit `Facts` snapshot, never `HostService`, so the dispatch thread
//! and the control lanes share it.

const std = @import("std");
const builtin = @import("builtin");
const build_options = @import("build-options");
const handshake = @import("terminal-host-handshake");

const message = handshake.message;

pub const product_version: []const u8 = build_options.host_version;
pub const environment: []const u8 = build_options.environment;
const build: message.Build = .{ .commit = build_options.source_commit };

/// The Host lists only the connection kind it accepts; it takes part in no delivery.
const connections = [_]message.ConnectionEntry{.{
    .kind = handshake.registry.worker_host.name,
    .generations = handshake.registry.worker_host.generations,
    .capabilities = handshake.registry.worker_host.capabilities,
}};

/// This build's `worker-host` acceptor profile.
pub fn profile() handshake.admission.Profile {
    return handshake.registry.workerHostProfile(product_version, build, environment);
}

/// What a reply reports about the Host when it is composed.
pub const Facts = struct {
    terminals: u32,
    state: message.ProcessState,
};

/// The dispatch thread's latest `Facts`, published for the control lanes, which never touch
/// `HostService`. A reply may lag the live count by one dispatch iteration.
pub const SharedFacts = struct {
    terminals: std.atomic.Value(u32) = .init(0),
    stopping: std.atomic.Value(bool) = .init(false),

    pub fn publish(self: *SharedFacts, facts: Facts) void {
        self.terminals.store(facts.terminals, .release);
        self.stopping.store(facts.state != .ready, .release);
    }

    pub fn load(self: *const SharedFacts) Facts {
        return .{
            .terminals = self.terminals.load(.acquire),
            .state = if (self.stopping.load(.acquire)) .stopping else .ready,
        };
    }
};

pub const Reply = struct {
    /// The caller-owned frame to send.
    frame: []u8,
    kind: Kind,

    pub const Kind = enum {
        /// The connection continues as the Worker session.
        session,
        /// An offer, or a payload that decodes as no message, was rejected; close after `frame`.
        offer_refused,
        /// A control request got its one reply; close after `frame`.
        request_answered,
    };
};

pub const RespondError = error{ OutOfMemory, InvalidReply };

/// Judges one candidate payload. Only the dispatch thread, while no Worker session exists, passes
/// `session_slot_free`; a control lane is itself the free control slot. A known request other than
/// `query` is answered `unsupported` until the Host can drain or end on request.
pub fn respond(
    allocator: std.mem.Allocator,
    payload: []const u8,
    facts: Facts,
    session_slot_free: bool,
) RespondError!Reply {
    var arena_state = std.heap.ArenaAllocator.init(allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const host = profile();
    const candidate = handshake.decode.initiatorPayload(arena, payload) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        error.Encoding, error.Message => return .{
            .frame = try rejection(allocator, .malformed),
            .kind = .offer_refused,
        },
    };
    const decision = try host.decide(arena, candidate, .{
        .session_slot_free = session_slot_free,
        .control_slot_free = true,
        .state = facts.state,
    });
    const processes = [_]message.ProcessEntry{.{
        .role = .terminal_host,
        .product_version = product_version,
        .build = build,
        .environment = environment,
        .pid = processId(),
        .state = facts.state,
        .terminals = facts.terminals,
        .connections = &connections,
    }};
    const reply: message.AcceptorMessage = switch (decision) {
        .accept => |answer| .{ .answer = answer },
        .reject => |refusal| .{ .rejection = refusal },
        .unsupported => |unsupported| .{ .unsupported = unsupported },
        .serve => |request| switch (request) {
            .query => .{ .query_result = .{ .processes = &processes, .restart_keeps_terminals = null } },
            .drain, .exit => .{ .unsupported = .{ .request = request.name() } },
        },
    };
    const kind: Reply.Kind = switch (decision) {
        .accept => .session,
        else => switch (candidate) {
            .offer => .offer_refused,
            .request, .unknown_request => .request_answered,
        },
    };
    return .{ .frame = try encodeReply(allocator, reply), .kind = kind };
}

/// The rejection for a candidate that never produced a decodable frame: `malformed` for a foreign
/// protocol or an out-of-bounds length, `timeout` when the candidate deadline passed.
pub fn rejection(allocator: std.mem.Allocator, reason: message.RejectionReason) RespondError![]u8 {
    return encodeReply(allocator, .{ .rejection = profile().reject(reason) });
}

fn encodeReply(allocator: std.mem.Allocator, reply: message.AcceptorMessage) RespondError![]u8 {
    return handshake.encode.acceptorFrame(allocator, reply) catch |err| switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        // Build options and decoded names already satisfy every grammar; a refusal is a bug that
        // closes this connection instead of ending the Host.
        error.InvalidMessage, error.PayloadTooLarge => error.InvalidReply,
    };
}

fn processId() u32 {
    return switch (builtin.os.tag) {
        .windows => std.os.windows.GetCurrentProcessId(),
        else => @intCast(std.c.getpid()),
    };
}
