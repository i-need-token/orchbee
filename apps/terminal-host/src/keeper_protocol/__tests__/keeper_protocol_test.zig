//! The Zig binding against every `schemas/keeper` fixture and the bounds too large for a fixture
//! file. The sequence fixtures' state outcomes are the Keeper's witnesses; here every request and
//! reply they carry decodes in its direction and pairs.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const keeper = @import("terminal-host-keeper-protocol");
const fixtures = @import("keeper-fixtures");

const decode = keeper.decode;
const encode = keeper.encode;
const frame = keeper.frame;
const message = keeper.message;

const Direction = enum { initiator, acceptor };

const Decoded = union(Direction) {
    initiator: message.InitiatorMessage,
    acceptor: message.AcceptorMessage,
};

const message_files = [_][]const u8{ fixtures.owner, fixtures.claimant, fixtures.worker };
const sequence_files = [_][]const u8{ fixtures.owner_sequences, fixtures.claimant_sequences };

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

/// Decodes `bytes` in `direction`, refuses them from the other, and proves the round trip.
fn expectRoundTrip(arena: std.mem.Allocator, name: []const u8, direction: Direction, bytes: []const u8) !Decoded {
    const decoded = decodeFrame(arena, direction, bytes) catch |err| {
        std.debug.print("{s} must decode: {s}\n", .{ name, @errorName(err) });
        return err;
    };
    if (decodeFrame(arena, opposite(direction), bytes)) |_| {
        std.debug.print("{s} must be refused from the other direction\n", .{name});
        return error.TestUnexpectedResult;
    } else |_| {}
    const encoded = try encodeFrame(arena, decoded);
    const redecoded = try decodeFrame(arena, direction, encoded);
    std.testing.expectEqualDeep(decoded, redecoded) catch |err| {
        std.debug.print("{s} must round-trip\n", .{name});
        return err;
    };
    try std.testing.expectEqualSlices(u8, encoded, try encodeFrame(arena, redecoded));
    return decoded;
}

test "valid and boundary payloads decode only in their direction and round-trip" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    var cases_seen: usize = 0;
    for (message_files) |source| {
        const document = try parse(arena, source);
        for ([_][]const u8{ "valid", "boundary" }) |group| {
            for (field(document, group).array.items) |case| {
                cases_seen += 1;
                const bytes = try frame.encode(arena, text(case, "payload"));
                _ = try expectRoundTrip(arena, text(case, "name"), caseDirection(case), bytes);
            }
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
    var cases_seen: usize = 0;
    for (message_files) |source| {
        for (field(try parse(arena, source), "invalid").array.items) |case| {
            cases_seen += 1;
            const layer = text(case, "layer");
            errdefer std.debug.print("invalid case {s} failed\n", .{text(case, "name")});
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
    try std.testing.expect(cases_seen > 0);
}

test "every sequence request and reply decodes in its direction and pairs" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    var steps_seen: usize = 0;
    for (sequence_files) |source| {
        for (field(try parse(arena, source), "sequences").array.items) |sequence| {
            const name = text(sequence, "name");
            for (field(sequence, "steps").array.items) |step| {
                const request_value = step.object.get("request") orelse continue;
                steps_seen += 1;
                errdefer std.debug.print("sequence {s} step {d} failed\n", .{ name, steps_seen });
                const request_text = try std.json.Stringify.valueAlloc(arena, request_value, .{});
                const reply_text = try std.json.Stringify.valueAlloc(arena, field(step, "reply"), .{});
                const request = (try expectRoundTrip(arena, name, .initiator, try frame.encode(arena, request_text))).initiator;
                const reply = (try expectRoundTrip(arena, name, .acceptor, try frame.encode(arena, reply_text))).acceptor.reply;
                switch (request) {
                    .request => |known| {
                        try std.testing.expectEqual(known.id, reply.id);
                        try std.testing.expect(reply.body.answers(std.meta.activeTag(known.body)));
                    },
                    .unknown_request => |unknown| {
                        try std.testing.expectEqual(unknown.id, reply.id);
                        try std.testing.expect(reply.body == .unsupported);
                        try std.testing.expectEqualStrings(unknown.request, reply.body.unsupported);
                    },
                }
            }
        }
    }
    try std.testing.expect(steps_seen > 0);
}

fn paddedRelease(arena: std.mem.Allocator, padding: usize) !message.InitiatorMessage {
    const filler = try arena.alloc(u8, padding);
    @memset(filler, 'x');
    const extensions = try arena.dupe(handshake.json.Member, &.{
        .{ .name = "padding", .value = .{ .string = filler } },
    });
    return .{ .request = .{
        .id = 1,
        .body = .{ .release = [_]u8{0x5f} ** 16 },
        .extensions = extensions,
    } };
}

test "payload bound is exact" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const unpadded = (try encode.initiatorFrame(arena, try paddedRelease(arena, 0))).len - frame.header_bytes;
    const largest = try encode.initiatorFrame(arena, try paddedRelease(arena, frame.maximum_payload_bytes - unpadded));
    try std.testing.expectEqual(frame.header_bytes + frame.maximum_payload_bytes, largest.len);
    _ = try decode.initiatorFrame(arena, largest);
    var source: SliceSource = .{ .bytes = largest };
    _ = try decode.initiatorPayload(arena, try frame.readPayload(arena, &source));
    try std.testing.expectError(
        error.PayloadTooLarge,
        encode.initiatorFrame(arena, try paddedRelease(arena, frame.maximum_payload_bytes - unpadded + 1)),
    );
}

test "reader decides without waiting for bytes it does not need" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    for ([_][]const u8{ "X", "OX", "OBX", "OBKX", "OBHS" }) |prefix| {
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
    const reply = try encode.acceptorFrame(arena, .{ .reply = .{ .id = 1, .body = .hello } });
    var trickle: Trickle = .{ .bytes = reply };
    _ = try decode.acceptorPayload(arena, try frame.readPayload(arena, &trickle));
}
