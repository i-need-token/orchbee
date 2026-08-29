const std = @import("std");

const json_wire = @import("ssh-json-wire");

pub const header_bytes = 16;
pub const maximum_frame_bytes = 1024 * 1024;
pub const initial_channel_credit_bytes = 65_536;
pub const project_root_maximum_message_bytes = 16 * 1024;
pub const project_root_maximum_path_bytes = 4_096;
pub const project_root_service_id = "project-root-v1";
pub const heartbeat_interval_ms = 15_000;
pub const heartbeat_miss_limit = 3;
pub const protocol_major = 0;
pub const protocol_minor = 1;
pub const protocol_patch = 0;

const maximum_payload_bytes = maximum_frame_bytes - header_bytes;
const record_read_timeout_ns = 15 * std.time.ns_per_s;
const magic = "OBSE";

pub const Kind = enum(u8) {
    hello = 1,
    hello_accepted = 2,
    heartbeat = 3,
    heartbeat_acknowledged = 4,
    failure = 5,
    channel_open = 16,
    channel_opened = 17,
    channel_data = 18,
    channel_credit = 19,
    channel_close = 20,
    channel_closed = 21,
};

pub const ProtocolVersion = struct {
    major: u8,
    minor: u8,
    patch: u8,
};

pub const Hello = struct {
    protocolVersion: ProtocolVersion,
    runtimeInstanceId: []const u8,
    clientNonce: []const u8,
    maxFrameBytes: u32,
    initialChannelCreditBytes: u32,
};

pub const Heartbeat = struct {
    runtimeEpoch: []const u8,
    sequence: u32,
};

pub const ChannelOpen = struct {
    runtimeEpoch: []const u8,
    channelId: u32,
    serviceId: []const u8,
    initialCreditBytes: u32,
};

pub const ChannelClose = struct {
    runtimeEpoch: []const u8,
    channelId: u32,
    reason: []const u8,
};

pub const ChannelData = struct {
    channel_id: u32,
    data: []const u8,
};

pub const ChannelCredit = struct {
    channel_id: u32,
    bytes: u32,
};

pub const Frame = struct {
    allocator: std.mem.Allocator,
    kind: Kind,
    payload: []u8,

    pub fn deinit(self: *Frame) void {
        self.allocator.free(self.payload);
        self.* = undefined;
    }
};

pub fn readFrame(
    io: std.Io,
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
) !Frame {
    var header: [header_bytes]u8 = undefined;
    try readExactly(io, socket, &header);
    if (!std.mem.eql(u8, header[0..4], magic)) return error.InvalidFrame;
    if (header[4] != protocol_major or header[5] != protocol_minor or
        header[6] != protocol_patch)
    {
        return error.UnsupportedProtocolVersion;
    }
    if (header[12] != 0 or header[13] != 0 or header[14] != 0 or header[15] != 0) {
        return error.InvalidFrame;
    }
    const kind = std.enums.fromInt(Kind, header[7]) orelse return error.UnsupportedFrameKind;
    const payload_length = std.mem.readInt(u32, header[8..12], .big);
    if (payload_length > maximum_payload_bytes) return error.FrameTooLarge;
    const payload = try allocator.alloc(u8, payload_length);
    errdefer allocator.free(payload);
    try readExactly(io, socket, payload);
    return .{ .allocator = allocator, .kind = kind, .payload = payload };
}

pub fn decodeHello(allocator: std.mem.Allocator, payload: []const u8) !std.json.Parsed(Hello) {
    var parsed = try json_wire.decode(Hello, allocator, payload);
    errdefer parsed.deinit();
    if (parsed.value.protocolVersion.major != protocol_major or
        parsed.value.protocolVersion.minor != protocol_minor or
        parsed.value.protocolVersion.patch != protocol_patch)
    {
        return error.UnsupportedProtocolVersion;
    }
    if (!validIdentity(parsed.value.runtimeInstanceId) or
        !validIdentity(parsed.value.clientNonce) or
        parsed.value.maxFrameBytes != maximum_frame_bytes or
        parsed.value.initialChannelCreditBytes == 0 or
        parsed.value.initialChannelCreditBytes > 16 * 1024 * 1024)
    {
        return error.InvalidHello;
    }
    return parsed;
}

pub fn decodeHeartbeat(
    allocator: std.mem.Allocator,
    payload: []const u8,
) !std.json.Parsed(Heartbeat) {
    var parsed = try json_wire.decode(Heartbeat, allocator, payload);
    errdefer parsed.deinit();
    if (!validIdentity(parsed.value.runtimeEpoch)) return error.InvalidHeartbeat;
    return parsed;
}

pub fn decodeChannelOpen(
    allocator: std.mem.Allocator,
    payload: []const u8,
) !std.json.Parsed(ChannelOpen) {
    var parsed = try json_wire.decode(ChannelOpen, allocator, payload);
    errdefer parsed.deinit();
    if (!validIdentity(parsed.value.runtimeEpoch) or
        parsed.value.channelId == 0 or
        !std.mem.eql(u8, parsed.value.serviceId, project_root_service_id) or
        parsed.value.initialCreditBytes != initial_channel_credit_bytes)
    {
        return error.InvalidChannelOpen;
    }
    return parsed;
}

pub fn decodeChannelClose(
    allocator: std.mem.Allocator,
    payload: []const u8,
) !std.json.Parsed(ChannelClose) {
    var parsed = try json_wire.decode(ChannelClose, allocator, payload);
    errdefer parsed.deinit();
    if (!validIdentity(parsed.value.runtimeEpoch) or parsed.value.channelId == 0 or
        (!std.mem.eql(u8, parsed.value.reason, "completed") and
            !std.mem.eql(u8, parsed.value.reason, "cancelled") and
            !std.mem.eql(u8, parsed.value.reason, "attachment-lost") and
            !std.mem.eql(u8, parsed.value.reason, "protocol-failure")))
    {
        return error.InvalidChannelClose;
    }
    return parsed;
}

pub fn decodeChannelData(payload: []const u8) !ChannelData {
    if (payload.len < @sizeOf(u32)) return error.InvalidChannelData;
    const channel_id = std.mem.readInt(u32, payload[0..4], .big);
    if (channel_id == 0) return error.InvalidChannelData;
    return .{ .channel_id = channel_id, .data = payload[4..] };
}

pub fn decodeChannelCredit(payload: []const u8) !ChannelCredit {
    if (payload.len != @sizeOf(u32) * 2) return error.InvalidChannelCredit;
    const channel_id = std.mem.readInt(u32, payload[0..4], .big);
    const bytes = std.mem.readInt(u32, payload[4..8], .big);
    if (channel_id == 0 or bytes == 0 or bytes > 16 * 1024 * 1024) {
        return error.InvalidChannelCredit;
    }
    return .{ .channel_id = channel_id, .bytes = bytes };
}

pub fn writeHelloAccepted(
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
    runtime_instance_id: []const u8,
    runtime_epoch: []const u8,
    runtime_version: []const u8,
    build_digest: []const u8,
    platform: []const u8,
) !void {
    var payload: std.Io.Writer.Allocating = .init(allocator);
    defer payload.deinit();
    try std.json.Stringify.value(.{
        .protocolVersion = .{
            .major = protocol_major,
            .minor = protocol_minor,
            .patch = protocol_patch,
        },
        .runtimeInstanceId = runtime_instance_id,
        .runtimeEpoch = runtime_epoch,
        .runtimeVersion = runtime_version,
        .buildDigest = build_digest,
        .platform = platform,
        .capabilities = [_][]const u8{project_root_service_id},
        .heartbeatIntervalMs = heartbeat_interval_ms,
        .heartbeatMissLimit = heartbeat_miss_limit,
        .maxFrameBytes = maximum_frame_bytes,
        .initialChannelCreditBytes = initial_channel_credit_bytes,
    }, .{}, &payload.writer);
    try writeFrame(socket, .hello_accepted, payload.written());
}

pub fn writeChannelOpened(
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
    runtime_epoch: []const u8,
    channel_id: u32,
) !void {
    var payload: std.Io.Writer.Allocating = .init(allocator);
    defer payload.deinit();
    try std.json.Stringify.value(.{
        .runtimeEpoch = runtime_epoch,
        .channelId = channel_id,
        .initialCreditBytes = initial_channel_credit_bytes,
    }, .{}, &payload.writer);
    try writeFrame(socket, .channel_opened, payload.written());
}

pub fn writeChannelData(
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
    channel_id: u32,
    data: []const u8,
) !void {
    if (channel_id == 0 or data.len > project_root_maximum_message_bytes) {
        return error.InvalidChannelData;
    }
    const payload = try allocator.alloc(u8, @sizeOf(u32) + data.len);
    defer allocator.free(payload);
    std.mem.writeInt(u32, payload[0..4], channel_id, .big);
    @memcpy(payload[4..], data);
    try writeFrame(socket, .channel_data, payload);
}

pub fn writeChannelClosed(
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
    runtime_epoch: []const u8,
    channel_id: u32,
    reason: []const u8,
) !void {
    var payload: std.Io.Writer.Allocating = .init(allocator);
    defer payload.deinit();
    try std.json.Stringify.value(.{
        .runtimeEpoch = runtime_epoch,
        .channelId = channel_id,
        .reason = reason,
    }, .{}, &payload.writer);
    try writeFrame(socket, .channel_closed, payload.written());
}

pub fn writeHeartbeatAcknowledged(
    allocator: std.mem.Allocator,
    socket: std.posix.socket_t,
    runtime_epoch: []const u8,
    sequence: u32,
) !void {
    var payload: std.Io.Writer.Allocating = .init(allocator);
    defer payload.deinit();
    try std.json.Stringify.value(.{
        .runtimeEpoch = runtime_epoch,
        .sequence = sequence,
    }, .{}, &payload.writer);
    try writeFrame(socket, .heartbeat_acknowledged, payload.written());
}

pub fn writeFailure(socket: std.posix.socket_t, code: []const u8) !void {
    var buffer: [160]u8 = undefined;
    const payload = try std.fmt.bufPrint(&buffer, "{{\"code\":\"{s}\"}}", .{code});
    try writeFrame(socket, .failure, payload);
}

pub fn writeFrame(socket: std.posix.socket_t, kind: Kind, payload: []const u8) !void {
    if (payload.len > maximum_payload_bytes) return error.FrameTooLarge;
    var header = [_]u8{0} ** header_bytes;
    @memcpy(header[0..4], magic);
    header[4] = protocol_major;
    header[5] = protocol_minor;
    header[6] = protocol_patch;
    header[7] = @intFromEnum(kind);
    std.mem.writeInt(u32, header[8..12], @intCast(payload.len), .big);
    try sendAll(socket, &header);
    try sendAll(socket, payload);
}

pub fn validIdentity(value: []const u8) bool {
    if (value.len != 32) return false;
    for (value) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return false,
    };
    return true;
}

fn readExactly(io: std.Io, socket: std.posix.socket_t, destination: []u8) !void {
    const started_at: std.Io.Timestamp = .now(io, .awake);
    var offset: usize = 0;
    while (offset < destination.len) {
        const elapsed = started_at.durationTo(.now(io, .awake));
        if (elapsed.nanoseconds > record_read_timeout_ns) return error.ConnectionTimedOut;
        var descriptors = [_]std.posix.pollfd{.{
            .fd = socket,
            .events = std.posix.POLL.IN,
            .revents = 0,
        }};
        if (try std.posix.poll(&descriptors, 1000) == 0) continue;
        if (descriptors[0].revents & std.posix.POLL.IN != 0) {
            const read_length = std.posix.read(socket, destination[offset..]) catch
                return error.ConnectionClosed;
            if (read_length == 0) return error.ConnectionClosed;
            offset += read_length;
        }
        if (descriptors[0].revents &
            (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0 and
            offset < destination.len)
        {
            return error.ConnectionClosed;
        }
    }
}

fn sendAll(socket: std.posix.socket_t, bytes: []const u8) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        const sent = std.c.send(
            socket,
            bytes[offset..].ptr,
            bytes.len - offset,
            std.posix.MSG.NOSIGNAL,
        );
        if (sent <= 0) return error.ConnectionClosed;
        offset += @intCast(sent);
    }
}
