//! Reads a connection's first `OBHS` frame within the candidate deadline and sends the Host's one
//! reply. Every broker-endpoint connection starts here; only an admitted Worker offer continues
//! with OBTP.

const std = @import("std");

const handshake = @import("terminal-host-handshake");
const monotonic = @import("terminal-host-platform").monotonic;
const service_boundary = @import("terminal-host-service");
const connection = @import("connection.zig");

const candidate_admission = service_boundary.candidate_admission;
const host_lifecycle = service_boundary.host_lifecycle;
const host_service = service_boundary.service;

/// How long a connection whose final reply is written waits for its peer to close.
pub const reply_linger_ns = std.time.ns_per_s;
/// How long a control lane waits for the dispatch thread to apply a posted `drain` or `exit`. The
/// dispatch thread applies posted requests between two Worker requests, and reading one record and
/// writing one reply each stop at `connection.record_read_timeout_ns`.
pub const request_apply_deadline_ns = 2 * connection.record_read_timeout_ns;

pub const Outcome = enum {
    /// The offer was answered; the connection continues as the Worker session.
    session,
    /// The offer, or a payload that decodes as no message, got a rejection.
    offer_refused,
    /// A control request got its one reply.
    request_answered,
    /// No complete frame arrived; a best-effort rejection was sent where one applies.
    refused,
    /// A control lane's `drain` or `exit` was not applied before its deadline. The connection
    /// closes without a reply, and the request stays posted for the dispatch thread.
    unanswered,
};

/// How a candidate reaches the Host: the dispatch thread reads and changes `HostService`
/// directly, and a control lane reads the facts the dispatch thread last published and posts a
/// `drain` or `exit` for it to apply.
pub const HostAccess = union(enum) {
    service: *host_service.HostService,
    lane: *candidate_admission.LaneHandoff,

    fn facts(self: HostAccess) candidate_admission.Facts {
        return switch (self) {
            .service => |service| service.candidateFacts(),
            .lane => |handoff| handoff.load(),
        };
    }

    /// Applies `request` and returns a state that shows it applied, or null when the dispatch
    /// thread did not apply a lane's request before `request_apply_deadline_ns`.
    fn perform(self: HostAccess, request: host_lifecycle.Request) ?host_lifecycle.State {
        const handoff = switch (self) {
            .service => |service| return service.applyControlRequest(request),
            .lane => |handoff| handoff,
        };
        handoff.post(request);
        var timer = monotonic.Timer.start() catch return null;
        while (true) {
            const state = handoff.load().state;
            if (host_lifecycle.applied(state, request)) return state;
            if (timer.read() >= request_apply_deadline_ns) return null;
            monotonic.sleep(std.time.ns_per_ms);
        }
    }
};

pub const Options = struct {
    host: HostAccess,
    /// Only the dispatch thread, while no Worker session exists, offers the session slot.
    session_slot_free: bool,
    deadline_ns: u64 = handshake.frame.candidate_deadline_ns,
};

/// Judges one candidate. `transport` reads, waits, and writes as the connection loop requires.
/// While the dispatch thread waits, it keeps maintaining terminals.
pub fn admit(allocator: std.mem.Allocator, transport: anytype, options: Options) !Outcome {
    var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
    var deadline = try connection.Deadline.init(&timer, options.deadline_ns);
    var source: Source(@TypeOf(transport)) = .{
        .transport = transport,
        .deadline = &deadline,
        .service = switch (options.host) {
            .service => |service| service,
            .lane => null,
        },
    };
    const payload = handshake.frame.readPayload(allocator, &source) catch |err| switch (err) {
        error.CandidateTimeout => return refuse(allocator, transport, .timeout),
        error.ForeignProtocol, error.Malformed => return refuse(allocator, transport, .malformed),
        error.Closed, error.ConnectionClosed => return .refused,
        else => return err,
    };
    defer allocator.free(payload);
    const reply = switch (try candidate_admission.respond(
        allocator,
        payload,
        options.host.facts(),
        options.session_slot_free,
    )) {
        .reply => |reply| reply,
        .perform => |request| performed: {
            const state = options.host.perform(request) orelse {
                std.log.warn("terminal Host {s} request was not applied before its deadline", .{@tagName(request)});
                return .unanswered;
            };
            break :performed try candidate_admission.performed(allocator, request, state);
        },
    };
    defer allocator.free(reply.frame);
    try connection.writeOutgoing(transport, &.{reply.frame});
    if (reply.kind != .session) awaitPeerClose(transport, source.service);
    return switch (reply.kind) {
        .session => .session,
        .offer_refused => .offer_refused,
        .request_answered => .request_answered,
    };
}

/// Whether a connection ended without a dispatch failure worth logging: its peer left, or its
/// first byte showed another user, which is dropped as silently.
pub fn isSilentClose(err: anyerror) bool {
    return err == error.ConnectionClosed or err == error.PeerIdentityMismatch;
}

/// Keeps a connection open after its final reply until the peer closes it, because closing the
/// Host's end first can discard a reply the peer has not read: a Windows pipe disconnect drops
/// unread bytes. A peer that sends more or stays past the linger ends it.
pub fn awaitPeerClose(transport: anytype, service: ?*host_service.HostService) void {
    var timer = monotonic.Timer.start() catch return;
    var deadline = connection.Deadline.init(&timer, reply_linger_ns) catch return;
    while (!deadline.expired()) {
        if (service) |running| running.maintain();
        if (transport.waitReadable() catch return) return;
    }
}

/// Sends a rejection for a candidate that never produced a decodable frame; the peer may already
/// be gone, so a failed write changes nothing.
fn refuse(
    allocator: std.mem.Allocator,
    transport: anytype,
    reason: handshake.message.RejectionReason,
) error{OutOfMemory}!Outcome {
    const frame = candidate_admission.rejection(allocator, reason) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        error.InvalidReply => return .refused,
    };
    defer allocator.free(frame);
    connection.writeOutgoing(transport, &.{frame}) catch {};
    return .refused;
}

const Interrupt = error{ CandidateTimeout, ConnectionClosed };

/// The errors a transport's readiness wait reports, kept distinct so a dispatch failure is logged.
fn WaitError(comptime Transport: type) type {
    const wait = @typeInfo(@TypeOf(std.meta.Child(Transport).waitReadable)).@"fn";
    return @typeInfo(wait.return_type.?).error_union.error_set;
}

/// The frame reader's byte source: it waits for readable bytes until the deadline and keeps the
/// dispatch thread's maintenance running.
fn Source(comptime Transport: type) type {
    return struct {
        transport: Transport,
        deadline: *connection.Deadline,
        service: ?*host_service.HostService,

        pub fn read(self: *@This(), bytes: []u8) (Interrupt || WaitError(Transport))!usize {
            while (true) {
                if (self.service) |service| service.maintain();
                if (self.deadline.expired()) return error.CandidateTimeout;
                if (!try self.transport.waitReadable()) continue;
                return self.transport.read(bytes) catch return error.ConnectionClosed;
            }
        }
    };
}
