//! The `OBHS` frame: the bytes `OBHS`, a big-endian `u32` payload length from 1 to 1,048,576, and
//! that many payload bytes. Deadlines and the transport stay with the caller.

const std = @import("std");

pub const magic = "OBHS";
pub const header_bytes = 8;
pub const maximum_payload_bytes = 1_048_576;
/// How long an acceptor waits for a candidate's complete frame before a best-effort `timeout`
/// rejection.
pub const candidate_deadline_ns: u64 = 2 * std.time.ns_per_s;

pub const DecodeError = error{ Magic, Length, Truncated, TrailingBytes };
pub const EncodeError = error{ PayloadTooLarge, OutOfMemory };
/// `Closed` is an end of stream before a complete frame, `ForeignProtocol` a received byte that
/// differs from `OBHS`, and `Malformed` an out-of-bounds length.
pub const ReadError = error{ Closed, ForeignProtocol, Malformed };

pub fn payloadLength(length_bytes: [4]u8) ?usize {
    const length = std.mem.readInt(u32, &length_bytes, .big);
    if (length < 1 or length > maximum_payload_bytes) return null;
    return length;
}

/// Returns the payload of exactly one complete in-memory frame.
pub fn completePayload(bytes: []const u8) DecodeError![]const u8 {
    const prefix = bytes[0..@min(bytes.len, magic.len)];
    if (!std.mem.eql(u8, prefix, magic[0..prefix.len])) return error.Magic;
    if (bytes.len < header_bytes) return error.Truncated;
    const length = payloadLength(bytes[magic.len..header_bytes].*) orelse return error.Length;
    const payload = bytes[header_bytes..];
    if (payload.len < length) return error.Truncated;
    if (payload.len > length) return error.TrailingBytes;
    return payload;
}

/// Frames one payload.
pub fn encode(allocator: std.mem.Allocator, payload: []const u8) EncodeError![]u8 {
    if (payload.len < 1 or payload.len > maximum_payload_bytes) return error.PayloadTooLarge;
    const bytes = try allocator.alloc(u8, header_bytes + payload.len);
    @memcpy(bytes[0..magic.len], magic);
    std.mem.writeInt(u32, bytes[magic.len..header_bytes], @intCast(payload.len), .big);
    @memcpy(bytes[header_bytes..], payload);
    return bytes;
}

/// Reads exactly one frame and returns its caller-owned payload, never reading past it. `source`
/// has `read(bytes: []u8) !usize`, which returns at least one byte or 0 at the end of the stream.
/// A foreign byte is reported as soon as it arrives, and an out-of-bounds length before any
/// payload is read or allocated.
pub fn readPayload(allocator: std.mem.Allocator, source: anytype) ![]u8 {
    var header: [header_bytes]u8 = undefined;
    var filled: usize = 0;
    while (filled < header_bytes) {
        filled += try readSome(source, header[filled..]);
        const received = @min(filled, magic.len);
        if (!std.mem.eql(u8, header[0..received], magic[0..received])) return error.ForeignProtocol;
    }
    const length = payloadLength(header[magic.len..header_bytes].*) orelse return error.Malformed;
    const payload = try allocator.alloc(u8, length);
    errdefer allocator.free(payload);
    filled = 0;
    while (filled < length) filled += try readSome(source, payload[filled..]);
    return payload;
}

fn readSome(source: anytype, bytes: []u8) !usize {
    const received = try source.read(bytes);
    if (received == 0) return error.Closed;
    return @min(received, bytes.len);
}
