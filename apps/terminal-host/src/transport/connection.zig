const std = @import("std");

const handshake = @import("terminal-host-handshake");
const host_service = @import("terminal-host-service").service;
const diagnostic_log = @import("terminal-host-platform").diagnostic_log;
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
        .host = .{ .service = service },
        .session_slot_free = true,
        .deadline_ns = candidate_deadline_ns,
    })) {
        .session => {},
        .offer_refused => return .worker,
        // The dispatch thread applies a request itself, so it never leaves one unanswered.
        .request_answered, .unanswered => return .control_request,
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
        // Control requests take effect here, between two Worker requests.
        lanes.serve(service);
        if (service.controlExitDue()) {
            // The Host ends this session itself; let the Worker read what was already written.
            candidate.awaitPeerClose(transport, service);
            return;
        }
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
            try writeSession(transport, outgoing.records(), service, lanes);
            if (connection.shouldClose()) return;
            continue;
        }

        var events = events: {
            var turn = TurnClock.start();
            defer turn.finish(.events);
            break :events try service.pollEvents(&connection);
        };
        defer events.deinit();
        try writeSession(transport, events.records(), service, lanes);

        if (!try transport.waitReadable()) continue;
        switch (try readSessionRecord(service.allocator, transport, service, lanes)) {
            .record => |record| {
                defer service.allocator.free(record);
                var turn = TurnClock.start();
                const handled = service.handle(&connection, record);
                turn.finish(.{ .request = host_service.requestKind(record) orelse 0 });
                var outgoing = handled catch |err| {
                    try answerFailedRequest(transport, service, &connection, record, err, lanes);
                    continue;
                };
                defer outgoing.deinit();
                try writeSession(transport, outgoing.records(), service, lanes);
            },
            .discarded => |discarded| try answerFailedRequest(
                transport,
                service,
                &connection,
                &discarded.header,
                discarded.reason,
                lanes,
            ),
        }
        if (connection.shouldClose()) return;
    }
}

/// Answers one request that could not be carried out on its own correlation id, so the session
/// serves the next request; a failure that cannot be answered closes the connection.
fn answerFailedRequest(
    transport: anytype,
    service: *host_service.HostService,
    connection: *const host_service.ConnectionState,
    header: []const u8,
    err: anyerror,
    lanes: anytype,
) !void {
    var storage: [host_service.failed_request_answer_bytes]u8 = undefined;
    const answer = service.answerFailedRequest(connection, header, err, &storage) orelse
        return switch (err) {
            error.InvalidTransportRecord, error.UnknownPayloadEncoding => error.ConnectionClosed,
            else => err,
        };
    std.log.warn("answering a terminal Host request that failed: {s}", .{@errorName(err)});
    try writeSession(transport, &.{answer}, service, lanes);
}

/// Times one dispatch turn for the diagnostic log; a missing clock records nothing.
const TurnClock = struct {
    timer: ?monotonic.Timer,

    fn start() TurnClock {
        return .{ .timer = monotonic.Timer.start() catch null };
    }

    fn finish(self: *TurnClock, turn: diagnostic_log.Turn) void {
        const timer = if (self.timer) |*value| value else return;
        diagnostic_log.slowTurn(turn, timer.read());
    }
};

/// Writes an active session's records. The session has no write deadline: while the Worker is
/// not reading, the Host keeps maintaining terminals and applying control requests, and closes
/// only when the pipe breaks, a write fails, or an `exit` becomes due, which cancels the write
/// partway and so ends the stream.
fn writeSession(
    transport: anytype,
    records: []const []const u8,
    service: *host_service.HostService,
    lanes: anytype,
) !void {
    if (!@hasDecl(@TypeOf(transport.*), "writeSome")) return writeOutgoing(transport, records);
    for (records) |record| {
        errdefer transport.cancelPendingWrite();
        var offset: usize = 0;
        while (offset < record.len) {
            const written = try transport.writeSome(record[offset..], poll_interval_ms);
            if (written != 0) {
                offset += written;
                continue;
            }
            lanes.serve(service);
            service.maintain();
            if (service.controlExitDue()) return error.ConnectionClosed;
        }
    }
}

const SessionRecord = union(enum) {
    record: []u8,
    /// A whole framed record the Host read and discarded, keeping only its header to answer it.
    discarded: struct {
        header: [protocol.transport_envelope_header_bytes]u8,
        /// `OutOfMemory` for a record too large to hold, or `UnknownPayloadEncoding`.
        reason: anyerror,
    },
};

/// Reads one record of an active session. Like its writes, the read has no deadline once the
/// header began: the Host keeps maintaining terminals and applying control requests meanwhile.
fn readSessionRecord(
    allocator: std.mem.Allocator,
    transport: anytype,
    service: *host_service.HostService,
    lanes: anytype,
) !SessionRecord {
    var header: [protocol.transport_envelope_header_bytes]u8 = undefined;
    try readSessionBytes(transport, &header, service, lanes);
    const total_length = protocol.transportRecordLength(&header) catch |err| switch (err) {
        // Framing passed and only the payload encoding is unknown, so the record can be stepped
        // over and answered.
        error.InvalidField => {
            const framed_length = protocol.transportRecordFramedLength(&header) catch
                return error.ConnectionClosed;
            try discardSessionBytes(transport, framed_length - header.len, service, lanes);
            return .{ .discarded = .{ .header = header, .reason = error.UnknownPayloadEncoding } };
        },
        else => return error.ConnectionClosed,
    };
    const record = allocator.alloc(u8, total_length) catch {
        // The stream stays framed: discard the body so the next record starts where it should.
        try discardSessionBytes(transport, total_length - header.len, service, lanes);
        return .{ .discarded = .{ .header = header, .reason = error.OutOfMemory } };
    };
    errdefer allocator.free(record);
    @memcpy(record[0..header.len], &header);
    try readSessionBytes(transport, record[header.len..], service, lanes);
    return .{ .record = record };
}

fn discardSessionBytes(
    transport: anytype,
    length: usize,
    service: *host_service.HostService,
    lanes: anytype,
) !void {
    var discard: [4096]u8 = undefined;
    var remaining = length;
    while (remaining != 0) {
        const step = @min(remaining, discard.len);
        try readSessionBytes(transport, discard[0..step], service, lanes);
        remaining -= step;
    }
}

fn readSessionBytes(
    transport: anytype,
    bytes: []u8,
    service: *host_service.HostService,
    lanes: anytype,
) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        service.maintain();
        if (!try transport.waitReadable()) {
            lanes.serve(service);
            if (service.controlExitDue()) return error.ConnectionClosed;
            continue;
        }
        const length = transport.read(bytes[offset..]) catch return error.ConnectionClosed;
        if (length == 0) return error.ConnectionClosed;
        offset += length;
    }
}

pub fn serveAcceptedConnection(
    transport: anytype,
    service: *host_service.HostService,
    lanes: anytype,
) Served {
    return serveWithLanes(transport, service, record_read_timeout_ns, lanes) catch |err| {
        if (!candidate.isSilentClose(err)) {
            std.log.warn("closing terminal Host connection after dispatch failure: {s}", .{@errorName(err)});
        }
        return .worker;
    };
}

/// Writes records with one deadline for the batch: the bound for a connection that is not yet,
/// or never becomes, the Worker's session.
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
