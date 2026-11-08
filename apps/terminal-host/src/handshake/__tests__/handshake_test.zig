//! The Zig binding against every `schemas/connection` fixture, the registry mirror, and the bounds
//! too large for a fixture file. The Rust binding runs the same fixtures, so both decode, refuse,
//! and admit identically.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const fixtures = @import("handshake-fixtures");

const admission = handshake.admission;
const decode = handshake.decode;
const encode = handshake.encode;
const frame = handshake.frame;
const message = handshake.message;
const registry = handshake.registry;

const Direction = enum { initiator, acceptor };

const Decoded = union(Direction) {
    initiator: message.InitiatorMessage,
    acceptor: message.AcceptorMessage,
};

fn decodeFrame(arena: std.mem.Allocator, direction: Direction, bytes: []const u8) decode.FrameError!Decoded {
    return switch (direction) {
        .initiator => .{ .initiator = try decode.initiatorFrame(arena, bytes) },
        .acceptor => .{ .acceptor = try decode.acceptorFrame(arena, bytes) },
    };
}

fn encodeFrame(arena: std.mem.Allocator, decoded: Decoded) ![]u8 {
    return switch (decoded) {
        .initiator => |value| encode.initiatorFrame(arena, value),
        .acceptor => |value| encode.acceptorFrame(arena, value),
    };
}

fn opposite(direction: Direction) Direction {
    return switch (direction) {
        .initiator => .acceptor,
        .acceptor => .initiator,
    };
}

fn parse(arena: std.mem.Allocator, source: []const u8) !std.json.Value {
    return std.json.parseFromSliceLeaky(std.json.Value, arena, source, .{});
}

fn field(value: std.json.Value, name: []const u8) std.json.Value {
    return value.object.get(name) orelse std.debug.panic("fixture member {s} is missing", .{name});
}

fn text(value: std.json.Value, name: []const u8) []const u8 {
    return field(value, name).string;
}

fn caseDirection(case: std.json.Value) Direction {
    return std.meta.stringToEnum(Direction, text(case, "direction")) orelse
        std.debug.panic("unknown fixture direction", .{});
}

test "valid and boundary payloads decode only in their direction and round-trip" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    var cases_seen: usize = 0;
    for ([_][]const u8{ fixtures.valid, fixtures.boundary }) |source| {
        for (field(try parse(arena, source), "cases").array.items) |case| {
            cases_seen += 1;
            const name = text(case, "name");
            const payload = text(case, "payload");
            const bytes = try frame.encode(arena, payload);
            const decoded = decodeFrame(arena, caseDirection(case), bytes) catch |err| {
                std.debug.print("{s} must decode: {s}\n", .{ name, @errorName(err) });
                return err;
            };
            if (decodeFrame(arena, opposite(caseDirection(case)), bytes)) |_| {
                std.debug.print("{s} must be refused from the other direction\n", .{name});
                return error.TestUnexpectedResult;
            } else |_| {}
            const encoded = try encodeFrame(arena, decoded);
            const redecoded = try decodeFrame(arena, caseDirection(case), encoded);
            std.testing.expectEqualDeep(decoded, redecoded) catch |err| {
                std.debug.print("{s} must round-trip\n", .{name});
                return err;
            };
            const again = try encodeFrame(arena, redecoded);
            try std.testing.expectEqualSlices(u8, encoded, again);
        }
    }
    try std.testing.expect(cases_seen > 0);
}

/// Hands out a fixed byte stream and records how far a reader consumed it.
const SliceSource = struct {
    bytes: []const u8,
    position: usize = 0,

    pub fn read(self: *SliceSource, buffer: []u8) !usize {
        const count = @min(buffer.len, self.bytes.len - self.position);
        @memcpy(buffer[0..count], self.bytes[self.position..][0..count]);
        self.position += count;
        return count;
    }
};

/// Yields one byte per read and fails once its bytes run out, so a reader that waits for bytes
/// it does not need surfaces as an error.
const Trickle = struct {
    bytes: []const u8,
    position: usize = 0,

    pub fn read(self: *Trickle, buffer: []u8) !usize {
        if (self.position == self.bytes.len) return error.TestReadPastFrame;
        buffer[0] = self.bytes[self.position];
        self.position += 1;
        return 1;
    }
};

const ReaderOutcome = enum { closed, foreign, malformed, message };

fn readOutcome(arena: std.mem.Allocator, case_direction: Direction, bytes: []const u8) !ReaderOutcome {
    var source: SliceSource = .{ .bytes = bytes };
    const payload = frame.readPayload(arena, &source) catch |err| return switch (err) {
        error.Closed => .closed,
        error.ForeignProtocol => .foreign,
        error.Malformed => .malformed,
        else => err,
    };
    const decoded = switch (case_direction) {
        .initiator => if (decode.initiatorPayload(arena, payload)) |_| true else |_| false,
        .acceptor => if (decode.acceptorPayload(arena, payload)) |_| true else |_| false,
    };
    if (!decoded) return .malformed;
    try std.testing.expect(source.position < bytes.len);
    return .message;
}

test "every invalid case is refused by its first layer" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const cases = field(try parse(arena, fixtures.invalid), "cases").array.items;
    try std.testing.expect(cases.len > 0);
    for (cases) |case| {
        const name = text(case, "name");
        const layer = text(case, "layer");
        errdefer std.debug.print("invalid case {s} failed\n", .{name});
        if (std.mem.eql(u8, layer, "frame")) {
            const hex = text(case, "frameHex");
            const bytes = try arena.alloc(u8, hex.len / 2);
            _ = try std.fmt.hexToBytes(bytes, hex);
            if (decodeFrame(arena, caseDirection(case), bytes)) |_| return error.TestUnexpectedResult else |_| {}
            const expected = std.meta.stringToEnum(ReaderOutcome, text(case, "reader")).?;
            try std.testing.expectEqual(expected, try readOutcome(arena, caseDirection(case), bytes));
            continue;
        }
        const bytes = try frame.encode(arena, text(case, "payload"));
        const expected: anyerror = if (std.mem.eql(u8, layer, "encoding")) error.Encoding else error.Message;
        if (decodeFrame(arena, caseDirection(case), bytes)) |_| {
            return error.TestUnexpectedResult;
        } else |err| try std.testing.expectEqual(expected, err);
    }
}

fn generations(value: std.json.Value) message.GenerationRange {
    return .{
        .lowest = @intCast(field(value, "lowest").integer),
        .highest = @intCast(field(value, "highest").integer),
    };
}

fn names(arena: std.mem.Allocator, value: std.json.Value) ![]const []const u8 {
    const list = try arena.alloc([]const u8, value.array.items.len);
    for (value.array.items, list) |item, *name| name.* = item.string;
    return list;
}

fn profile(arena: std.mem.Allocator, value: std.json.Value) !admission.Profile {
    const build = field(value, "build");
    return .{
        .connection = text(value, "connection"),
        .generations = generations(field(value, "generations")),
        .capabilities = try names(arena, field(value, "capabilities")),
        .product_version = text(value, "productVersion"),
        .build = .{
            .commit = if (build.object.get("commit")) |commit| commit.string else null,
            .cohort = if (build.object.get("cohort")) |cohort| cohort.string else null,
        },
        .environment = text(value, "environment"),
        .fingerprint = if (value.object.get("fingerprint")) |fingerprint| fingerprint.string else null,
    };
}

fn expectSameNames(expected: []const []const u8, actual: []const []const u8) !void {
    try std.testing.expectEqual(expected.len, actual.len);
    for (expected, actual) |left, right| try std.testing.expectEqualStrings(left, right);
}

test "admission decides every fixture case and its reply encodes" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const document = try parse(arena, fixtures.admission);
    const acceptors = field(document, "acceptors");
    const cases = field(document, "cases").array.items;
    try std.testing.expect(cases.len > 0);
    for (cases) |case| {
        errdefer std.debug.print("admission case {s} failed\n", .{text(case, "name")});
        const acceptor = try profile(arena, field(acceptors, text(case, "acceptor")));
        const load_value = field(case, "load");
        const load: admission.Load = .{
            .session_slot_free = field(load_value, "sessionSlotFree").bool,
            .control_slot_free = field(load_value, "controlSlotFree").bool,
            .state = message.ProcessState.fromName(text(load_value, "state")),
        };
        const decision: admission.Decision = if (decode.initiatorPayload(arena, text(case, "payload"))) |initiator|
            try acceptor.decide(arena, initiator, load)
        else |_|
            .{ .reject = acceptor.reject(.malformed) };
        const expected = field(case, "decision");
        const kind = text(expected, "kind");
        const reply: ?message.AcceptorMessage = switch (decision) {
            .accept => |answer| accept: {
                try std.testing.expectEqualStrings("accept", kind);
                try std.testing.expectEqual(field(expected, "generation").integer, answer.generation);
                try expectSameNames(try names(arena, field(expected, "capabilities")), answer.capabilities);
                break :accept .{ .answer = answer };
            },
            .reject => |rejection| reject: {
                try std.testing.expectEqualStrings("reject", kind);
                try std.testing.expectEqualStrings(text(expected, "reason"), rejection.reason.name());
                break :reject .{ .rejection = rejection };
            },
            .serve => |request| serve: {
                try std.testing.expectEqualStrings("serve", kind);
                try std.testing.expectEqualStrings(text(expected, "request"), request.name());
                break :serve null;
            },
            .unsupported => |unsupported| unsupported: {
                try std.testing.expectEqualStrings("unsupported", kind);
                try std.testing.expectEqualStrings(text(expected, "request"), unsupported.request);
                break :unsupported .{ .unsupported = unsupported };
            },
        };
        if (reply) |value| _ = try decode.acceptorFrame(arena, try encode.acceptorFrame(arena, value));
    }
}

test "every fixture reply decodes as an acceptor message" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const document = try parse(arena, fixtures.admission);
    for ([_][]const u8{ "replies", "requestReplies" }) |group| {
        const cases = field(document, group).array.items;
        try std.testing.expect(cases.len > 0);
        for (cases) |case| {
            errdefer std.debug.print("{s} case {s} failed\n", .{ group, text(case, "name") });
            _ = try decode.acceptorPayload(arena, text(case, "reply"));
        }
    }
}

test "worker-host constants mirror the registry and the fingerprint is configured" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const entry = field(field(try parse(arena, fixtures.registry), "negotiatedConnections"), registry.worker_host.name);
    try std.testing.expectEqual(generations(field(entry, "generations")), registry.worker_host.generations);
    const declared = field(entry, "capabilities").array.items;
    try std.testing.expectEqual(declared.len, registry.worker_host.capabilities.len);
    for (declared, registry.worker_host.capabilities) |capability, name| {
        try std.testing.expectEqualStrings(text(capability, "name"), name);
    }
    var inputs_include_binding = false;
    for (field(entry, "fingerprintInputs").array.items) |input| {
        if (std.mem.eql(u8, input.string, "apps/terminal-host/src/handshake")) inputs_include_binding = true;
    }
    try std.testing.expect(inputs_include_binding);
    try std.testing.expect(message.isLowercaseHex(registry.worker_host.development_fingerprint, 64));
}

fn paddedOffer(arena: std.mem.Allocator, padding: usize) !message.InitiatorMessage {
    const filler = try arena.alloc(u8, padding);
    @memset(filler, 'x');
    const extensions = try arena.dupe(handshake.json.Member, &.{.{ .name = "padding", .value = .{ .string = filler } }});
    return .{ .offer = .{
        .connection = registry.worker_host.name,
        .generations = registry.worker_host.generations,
        .capabilities = &.{},
        .product_version = "1.6.0",
        .build = .{},
        .environment = message.release_environment,
        .fingerprint = null,
        .extensions = extensions,
    } };
}

test "payload bound is exact" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const unpadded = (try encode.initiatorFrame(arena, try paddedOffer(arena, 0))).len - frame.header_bytes;
    const largest = try encode.initiatorFrame(arena, try paddedOffer(arena, frame.maximum_payload_bytes - unpadded));
    try std.testing.expectEqual(frame.header_bytes + frame.maximum_payload_bytes, largest.len);
    _ = try decode.initiatorFrame(arena, largest);
    var source: SliceSource = .{ .bytes = largest };
    _ = try decode.initiatorPayload(arena, try frame.readPayload(arena, &source));
    try std.testing.expectError(
        error.PayloadTooLarge,
        encode.initiatorFrame(arena, try paddedOffer(arena, frame.maximum_payload_bytes - unpadded + 1)),
    );
}

test "query result lists at most 258 processes" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const entry: message.ProcessEntry = .{
        .role = .terminal_host,
        .product_version = "1.6.0",
        .build = .{},
        .environment = message.release_environment,
        .pid = 1,
        .state = .ready,
        .terminals = 0,
        .connections = &.{.{ .kind = registry.worker_host.name, .generations = registry.worker_host.generations, .capabilities = null }},
    };
    const processes = try arena.alloc(message.ProcessEntry, message.maximum_query_processes + 1);
    @memset(processes, entry);
    processes[0].role = .server;
    processes[0].connections = &.{.{ .kind = registry.worker_host.name, .generations = registry.worker_host.generations, .capabilities = &.{} }};
    processes[1].role = .worker;
    const largest: message.AcceptorMessage = .{ .query_result = .{
        .processes = processes[0..message.maximum_query_processes],
        .restart_keeps_terminals = true,
    } };
    const encoded = try encode.acceptorFrame(arena, largest);
    const decoded = try decode.acceptorFrame(arena, encoded);
    try std.testing.expectEqual(message.maximum_query_processes, decoded.query_result.processes.len);

    const oversized: message.AcceptorMessage = .{ .query_result = .{ .processes = processes, .restart_keeps_terminals = true } };
    try std.testing.expectError(error.InvalidMessage, encode.acceptorFrame(arena, oversized));
    const payload = try frame.completePayload(encoded);
    const marker = "],\"restartKeepsTerminals\"";
    const split = std.mem.lastIndexOf(u8, payload, marker).?;
    const peer = payload[std.mem.lastIndexOf(u8, payload[0..split], ",{\"role\"").?..split];
    const grown = try std.mem.concat(arena, u8, &.{ payload[0..split], peer, payload[split..] });
    try std.testing.expectError(error.Message, decode.acceptorPayload(arena, grown));
}

test "reader decides without waiting for bytes it does not need" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    for ([_][]const u8{ "X", "OX", "OBX", "OBHX" }) |prefix| {
        var trickle: Trickle = .{ .bytes = prefix };
        try std.testing.expectError(error.ForeignProtocol, frame.readPayload(arena, &trickle));
    }
    for ([_]u32{ 0, frame.maximum_payload_bytes + 1, std.math.maxInt(u32) }) |length| {
        var header: [frame.header_bytes]u8 = undefined;
        @memcpy(header[0..frame.magic.len], frame.magic);
        std.mem.writeInt(u32, header[frame.magic.len..], length, .big);
        var trickle: Trickle = .{ .bytes = &header };
        try std.testing.expectError(error.Malformed, frame.readPayload(arena, &trickle));
    }
    const exit = try encode.acceptorFrame(arena, .{ .exit_result = .{} });
    var trickle: Trickle = .{ .bytes = exit };
    _ = try decode.acceptorPayload(arena, try frame.readPayload(arena, &trickle));
}
