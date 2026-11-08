//! The Host's acceptor side of the `OBHS` handshake on its broker endpoint. Every connection's
//! first frame is judged here before any OBTP byte: an admitted Worker offer continues as the
//! single Worker session, and a control request gets its one reply. The decision reads only the
//! build's identity and an explicit `Facts` snapshot, never `HostService`, so the dispatch thread
//! and the control lanes share it.

const std = @import("std");
const builtin = @import("builtin");
const build_options = @import("build-options");
const handshake = @import("terminal-host-handshake");

const host_lifecycle = @import("host_lifecycle.zig");

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
    state: host_lifecycle.State,
};

/// TH-003 handoff between the control lanes and the dispatch thread. The dispatch thread
/// publishes its latest `Facts`, and a lane replies from them, never from `HostService`, so a
/// reply may lag the live terminal count by one dispatch iteration. A lane posts `drain` or `exit`
/// instead of performing it: the dispatch thread applies posted requests between two Worker
/// requests and publishes the resulting state, and the lane answers once that state shows its
/// request applied. States only move forward, so the lane cannot miss that moment.
pub const LaneHandoff = struct {
    terminals: std.atomic.Value(u32) = .init(0),
    state: std.atomic.Value(host_lifecycle.State) = .init(.ready),
    /// One bit per request; lanes set them and only the dispatch thread clears them.
    posted: std.atomic.Value(u8) = .init(0),

    /// Called only by the dispatch thread.
    pub fn publish(self: *LaneHandoff, facts: Facts) void {
        self.terminals.store(facts.terminals, .release);
        self.state.store(facts.state, .release);
    }

    pub fn load(self: *const LaneHandoff) Facts {
        return .{
            .terminals = self.terminals.load(.acquire),
            .state = self.state.load(.acquire),
        };
    }

    /// Posts `request` for the dispatch thread; posting a pending request again changes nothing.
    pub fn post(self: *LaneHandoff, request: host_lifecycle.Request) void {
        _ = self.posted.fetchOr(requestBit(request), .acq_rel);
    }

    /// Takes every posted request. Called only by the dispatch thread.
    pub fn take(self: *LaneHandoff) Posted {
        return .{ .bits = self.posted.swap(0, .acq_rel) };
    }
};

pub const Posted = struct {
    bits: u8,

    pub fn contains(self: Posted, request: host_lifecycle.Request) bool {
        return self.bits & requestBit(request) != 0;
    }
};

fn requestBit(request: host_lifecycle.Request) u8 {
    return @as(u8, 1) << @intFromEnum(request);
}

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

/// How to answer one candidate.
pub const Judgment = union(enum) {
    /// Send this reply.
    reply: Reply,
    /// A `drain` or `exit` request passed admission. Have the dispatch thread apply it, then send
    /// `performed` with the state that shows it applied.
    perform: host_lifecycle.Request,
};

pub const RespondError = error{ OutOfMemory, InvalidReply };

/// Judges one candidate payload. Only the dispatch thread, while no Worker session exists, passes
/// `session_slot_free`; a control lane is itself the free control slot.
pub fn respond(
    allocator: std.mem.Allocator,
    payload: []const u8,
    facts: Facts,
    session_slot_free: bool,
) RespondError!Judgment {
    var arena_state = std.heap.ArenaAllocator.init(allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const host = profile();
    const candidate = handshake.decode.initiatorPayload(arena, payload) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        error.Encoding, error.Message => return .{ .reply = .{
            .frame = try rejection(allocator, .malformed),
            .kind = .offer_refused,
        } },
    };
    const state = facts.state.processState();
    const decision = try host.decide(arena, candidate, .{
        .session_slot_free = session_slot_free,
        .control_slot_free = true,
        .state = state,
    });
    const processes = [_]message.ProcessEntry{.{
        .role = .terminal_host,
        .product_version = product_version,
        .build = build,
        .environment = environment,
        .pid = processId(),
        .state = state,
        .terminals = facts.terminals,
        .connections = &connections,
    }};
    const reply: message.AcceptorMessage = switch (decision) {
        .accept => |answer| .{ .answer = answer },
        .reject => |refusal| .{ .rejection = refusal },
        .unsupported => |unsupported| .{ .unsupported = unsupported },
        .serve => |request| switch (request) {
            .query => .{ .query_result = .{ .processes = &processes, .restart_keeps_terminals = null } },
            .drain => return .{ .perform = .drain },
            .exit => return .{ .perform = .exit },
        },
    };
    const kind: Reply.Kind = switch (decision) {
        .accept => .session,
        else => switch (candidate) {
            .offer => .offer_refused,
            .request, .unknown_request => .request_answered,
        },
    };
    return .{ .reply = .{ .frame = try encodeReply(allocator, reply), .kind = kind } };
}

/// The one reply to a `drain` or `exit` request, once `state` shows it applied.
pub fn performed(
    allocator: std.mem.Allocator,
    request: host_lifecycle.Request,
    state: host_lifecycle.State,
) RespondError!Reply {
    std.debug.assert(host_lifecycle.applied(state, request));
    const reply: message.AcceptorMessage = switch (request) {
        .drain => .{ .drain_result = .{ .state = state.processState() } },
        .exit => .{ .exit_result = .{} },
    };
    return .{ .frame = try encodeReply(allocator, reply), .kind = .request_answered };
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
