const std = @import("std");

const handshake = @import("terminal-host-handshake");
const host_service = @import("terminal-host-service").service;
const monotonic = @import("terminal-host-platform").monotonic;
const protocol = @import("terminal-protocol");
const candidate = @import("candidate.zig");
const control_lanes = @import("control_lanes.zig");

const endpoint_id_bytes = 32;
pub const poll_interval_ms = 20;
pub const record_read_timeout_ns = 5 * std.time.ns_per_s;

pub const Deadline = struct {
    timer: *monotonic.Timer,
    timeout_ns: u64,

    pub fn init(timer: *monotonic.Timer, timeout_ns: i128) !Deadline {
        if (timeout_ns < 0) return error.ConnectionClosed;
        return .{
            .timer = timer,
            .timeout_ns = std.math.cast(u64, timeout_ns) orelse return error.ConnectionClosed,
        };
    }

    pub fn expired(self: *Deadline) bool {
        return self.timer.read() >= self.timeout_ns;
    }

    pub fn remainingNs(self: *Deadline) u64 {
        const elapsed = self.timer.read();
        return self.timeout_ns -| elapsed;
    }
};

pub fn validEndpointId(endpoint_id: []const u8) bool {
    if (endpoint_id.len != endpoint_id_bytes) return false;
    for (endpoint_id) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return false,
    };
    return true;
}

/// What one accepted connection was, for the wait for a Host's first connection.
pub const Served = enum {
    /// A connection that could have been the Worker: its session, a refused offer, or a candidate
    /// that never produced a decodable frame.
    worker,
    /// A control request, answered and closed; the Host still waits for its Worker.
    control_request,
};

pub fn serveConnection(transport: anytype, service: *host_service.HostService) !void {
    return serveConnectionWithHandshakeTimeout(
        transport,
        service,
        record_read_timeout_ns,
    );
}

/// Serves one connection without control lanes; `handshake_timeout_ns` bounds both the
/// `OBHS` candidate, together with its own deadline, and the OBTP `Hello` that follows.
pub fn serveConnectionWithHandshakeTimeout(
    transport: anytype,
    service: *host_service.HostService,
    handshake_timeout_ns: i128,
) !void {
    var lanes: control_lanes.None = .{};
    _ = try serveWithLanes(transport, service, handshake_timeout_ns, &lanes);
}

/// Admits the connection's `OBHS` candidate first. An admitted Worker offer becomes the session:
/// `lanes` answer control connections until it ends, and they are joined before this returns.
fn serveWithLanes(
    transport: anytype,
    service: *host_service.HostService,
    handshake_timeout_ns: i128,
    lanes: anytype,
) !Served {
    if (handshake_timeout_ns < 0) return error.ConnectionClosed;
    const candidate_deadline_ns = @min(
        handshake.frame.candidate_deadline_ns,
        std.math.cast(u64, handshake_timeout_ns) orelse std.math.maxInt(u64),
    );
    switch (try candidate.admit(service.allocator, transport, .{
        .facts = .{ .service = service },
        .session_slot_free = true,
        .deadline_ns = candidate_deadline_ns,
    })) {
        .session => {},
        .offer_refused => return .worker,
        .request_answered => return .control_request,
        .refused => return error.ConnectionClosed,
    }
    lanes.start(service.candidateFacts());
    defer lanes.stopAndJoin(service);
    try serveSession(transport, service, handshake_timeout_ns, lanes);
    return .worker;
}

fn serveSession(
    transport: anytype,
    service: *host_service.HostService,
    handshake_timeout_ns: i128,
    lanes: anytype,
) !void {
    var connection: host_service.ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    var handshake_timer = monotonic.Timer.start() catch return error.ConnectionClosed;
    var handshake_deadline = Deadline.init(&handshake_timer, handshake_timeout_ns) catch return error.ConnectionClosed;

    while (true) {
        if (connection.shouldClose()) return;
        if (connection.isAwaitingHello()) {
            if (handshake_deadline.expired()) return error.ConnectionClosed;
            if (!try transport.waitReadable()) {
                service.maintain();
                continue;
            }
            const record = try readRecordBeforeMaintaining(
                service.allocator,
                transport,
                &handshake_deadline,
                service,
            );
            defer service.allocator.free(record);
            var outgoing = service.handle(&connection, record) catch |err| switch (err) {
                error.InvalidTransportRecord => return error.ConnectionClosed,
                else => return err,
            };
            defer outgoing.deinit();
            try writeOutgoing(transport, outgoing.records());
            if (connection.shouldClose()) return;
            continue;
        }

        lanes.publish(service.candidateFacts());
        var events = try service.pollEvents(&connection);
        defer events.deinit();
        try writeOutgoing(transport, events.records());

        if (!try transport.waitReadable()) continue;
        const record = try readRecordMaintaining(service.allocator, transport, service);
        defer service.allocator.free(record);
        var outgoing = service.handle(&connection, record) catch |err| switch (err) {
            error.InvalidTransportRecord => return error.ConnectionClosed,
            else => return err,
        };
        defer outgoing.deinit();
        try writeOutgoing(transport, outgoing.records());
        if (connection.shouldClose()) return;
    }
}

pub fn serveAcceptedConnection(
    transport: anytype,
    service: *host_service.HostService,
    lanes: anytype,
) Served {
    return serveWithLanes(transport, service, record_read_timeout_ns, lanes) catch |err| {
        switch (err) {
            error.ConnectionClosed => {},
            else => std.log.warn("closing terminal Host connection after dispatch failure: {s}", .{@errorName(err)}),
        }
        return .worker;
    };
}

pub fn serveUntilIdleLoop(
    server: anytype,
    service: anytype,
    comptime recoverable_failure: fn (anyerror) bool,
) !void {
    var completed_accept = false;
    while (true) {
        if (!completed_accept) {
            // A control request answered before the Worker arrives keeps the Host waiting.
            const served: Served = server.serveOne(service) catch |err| served: {
                if (!recoverable_failure(err)) return err;
                break :served .worker;
            };
            completed_accept = served != .control_request;
            continue;
        }
        service.maintain();
        if (service.canStopWhenIdle()) return;
        const served = server.serveOneBefore(service, poll_interval_ms) catch |err| {
            if (!recoverable_failure(err)) return err;
            continue;
        };
        _ = served;
    }
}

pub fn writeOutgoing(transport: anytype, records: []const []const u8) !void {
    var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
    var deadline = try Deadline.init(&timer, record_read_timeout_ns);
    for (records) |record| {
        if (@hasDecl(@TypeOf(transport.*), "writeAllBefore")) {
            try transport.writeAllBefore(record, &deadline);
        } else {
            try transport.writeAll(record);
        }
    }
}

pub fn readRecord(allocator: std.mem.Allocator, transport: anytype) ![]u8 {
    return readRecordWithTimeout(allocator, transport, record_read_timeout_ns);
}

fn readRecordMaintaining(
    allocator: std.mem.Allocator,
    transport: anytype,
    service: *host_service.HostService,
) ![]u8 {
    var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
    var deadline = try Deadline.init(&timer, record_read_timeout_ns);
    return readRecordBeforeMaintaining(allocator, transport, &deadline, service);
}

pub fn readRecordWithTimeout(
    allocator: std.mem.Allocator,
    transport: anytype,
    timeout_ns: i128,
) ![]u8 {
    var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
    var deadline = try Deadline.init(&timer, timeout_ns);
    return readRecordBefore(allocator, transport, &deadline);
}

fn readRecordBefore(
    allocator: std.mem.Allocator,
    transport: anytype,
    deadline: *Deadline,
) ![]u8 {
    return readRecordBeforeWithMaintenance(allocator, transport, deadline, null);
}

fn readRecordBeforeMaintaining(
    allocator: std.mem.Allocator,
    transport: anytype,
    deadline: *Deadline,
    service: *host_service.HostService,
) ![]u8 {
    return readRecordBeforeWithMaintenance(allocator, transport, deadline, service);
}

fn readRecordBeforeWithMaintenance(
    allocator: std.mem.Allocator,
    transport: anytype,
    deadline: *Deadline,
    service: ?*host_service.HostService,
) ![]u8 {
    var header: [protocol.transport_envelope_header_bytes]u8 = undefined;
    try readExactlyBeforeWithMaintenance(transport, &header, deadline, service);
    const total_length = protocol.transportRecordLength(&header) catch return error.ConnectionClosed;
    const record = try allocator.alloc(u8, total_length);
    errdefer allocator.free(record);
    @memcpy(record[0..header.len], &header);
    try readExactlyBeforeWithMaintenance(transport, record[header.len..], deadline, service);
    return record;
}

fn readExactlyBefore(transport: anytype, bytes: []u8, deadline: *Deadline) !void {
    return readExactlyBeforeWithMaintenance(transport, bytes, deadline, null);
}

fn readExactlyBeforeWithMaintenance(
    transport: anytype,
    bytes: []u8,
    deadline: *Deadline,
    service: ?*host_service.HostService,
) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        if (service) |active_service| active_service.maintain();
        if (deadline.expired()) return error.ConnectionClosed;
        if (!try transport.waitReadable()) continue;
        const length = transport.read(bytes[offset..]) catch return error.ConnectionClosed;
        if (length == 0) return error.ConnectionClosed;
        offset += length;
    }
}
