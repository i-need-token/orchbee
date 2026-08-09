const std = @import("std");

const contract = @import("contract.zig");
const wire = @import("wire.zig");

pub const Summary = struct {
    correlation_id: ?[16]u8,
    event: bool,
    host_epoch: ?[16]u8,
    kind: u16,
    optional_message: bool,
    payload: []const u8,
    payload_encoding: u8,
    protocol_major: u16,
    protocol_minor: u16,
    protocol_patch: u16,
    response: bool,
};

pub fn validateTransportEnvelope(bytes: []const u8) contract.ProtocolError!Summary {
    if (bytes.len < contract.transport_envelope_header_bytes) return error.Truncated;
    if (bytes.len > contract.Limits.transport_record_bytes) return error.LimitExceeded;
    try wire.requireMagic(bytes, "OBTP");

    var reader = wire.Reader.init(bytes, 4);
    const header_length = try reader.readInt(u16);
    const flags = try reader.readInt(u16);
    const total_length = try reader.readInt(u32);
    const payload_length = try reader.readInt(u32);
    const protocol_major = try reader.readInt(u16);
    const protocol_minor = try reader.readInt(u16);
    const kind = try reader.readInt(u16);
    const payload_encoding = try reader.readInt(u8);
    const reserved_byte = try reader.readInt(u8);
    const correlation_id = try wire.optionalIdentifier(try reader.readBytes(16));
    const host_epoch = try wire.optionalIdentifier(try reader.readBytes(16));
    const protocol_patch = try reader.readInt(u16);
    const reserved = try reader.readBytes(6);

    if (header_length != contract.transport_envelope_header_bytes) return error.UnsupportedVersion;
    if (total_length != bytes.len or payload_length != bytes.len - header_length) {
        return error.LengthMismatch;
    }
    if (reserved_byte != 0 or flags & 0xff00 != 0) return error.UnknownRequiredFlag;
    for (reserved) |byte| if (byte != 0) return error.UnknownRequiredFlag;
    const event = flags & 4 != 0;
    const response = flags & 2 != 0;
    if (event and response) return error.InvalidField;
    const optional_message = flags & 1 != 0;
    if (!contract.isKnownMessageKind(kind) and !optional_message) return error.UnknownRequiredMessage;
    if (payload_length > try contract.maximumPayloadBytes(payload_encoding)) return error.LimitExceeded;

    return .{
        .correlation_id = correlation_id,
        .event = event,
        .host_epoch = host_epoch,
        .kind = kind,
        .optional_message = optional_message,
        .payload = bytes[header_length..],
        .payload_encoding = payload_encoding,
        .protocol_major = protocol_major,
        .protocol_minor = protocol_minor,
        .protocol_patch = protocol_patch,
        .response = response,
    };
}

pub fn transportRecordLength(header: []const u8) contract.ProtocolError!usize {
    if (header.len < contract.transport_envelope_header_bytes) return error.Truncated;
    try wire.requireMagic(header, "OBTP");
    const header_length = std.mem.readInt(u16, header[4..6], .little);
    const total_length = std.mem.readInt(u32, header[8..12], .little);
    const payload_length = std.mem.readInt(u32, header[12..16], .little);
    if (header_length != contract.transport_envelope_header_bytes) return error.UnsupportedVersion;
    if (total_length < contract.transport_envelope_header_bytes or
        payload_length != total_length - contract.transport_envelope_header_bytes)
    {
        return error.LengthMismatch;
    }
    if (total_length > contract.Limits.transport_record_bytes) return error.LimitExceeded;
    if (payload_length > try contract.maximumPayloadBytes(header[22])) return error.LimitExceeded;
    return total_length;
}

pub fn encodeTransportEnvelope(
    allocator: std.mem.Allocator,
    kind: u16,
    encoding: contract.PayloadEncoding,
    payload: []const u8,
    correlation_id: ?[16]u8,
    host_epoch: ?[16]u8,
    response: bool,
    event: bool,
) (contract.ProtocolError || std.mem.Allocator.Error)![]u8 {
    const total_length = try transportEnvelopeLength(
        kind,
        encoding,
        payload.len,
        correlation_id,
        host_epoch,
        response,
        event,
    );
    const bytes = try allocator.alloc(u8, total_length);
    errdefer allocator.free(bytes);
    @memcpy(bytes[contract.transport_envelope_header_bytes..], payload);
    return finalizeTransportEnvelope(
        bytes,
        kind,
        encoding,
        payload.len,
        correlation_id,
        host_epoch,
        response,
        event,
    );
}

pub fn transportEnvelopeLength(
    kind: u16,
    encoding: contract.PayloadEncoding,
    payload_length: usize,
    correlation_id: ?[16]u8,
    host_epoch: ?[16]u8,
    response: bool,
    event: bool,
) contract.ProtocolError!usize {
    if (event and response) return error.InvalidField;
    // This encoder is the required-message production path. Optional
    // extensions must use a flag-aware codec so a caller cannot construct a
    // record that the validator will reject as an unknown required kind.
    if (!contract.isKnownMessageKind(kind)) return error.UnknownRequiredMessage;
    const expected_encoding = contract.messagePayloadEncoding(kind, response, event) orelse
        return error.InvalidField;
    if (encoding != expected_encoding) return error.InvalidField;
    if (payload_length > try contract.maximumPayloadBytes(@intFromEnum(encoding))) return error.LimitExceeded;
    return transportEnvelopeAllocationLength(payload_length, correlation_id, host_epoch);
}

pub fn transportEnvelopeAllocationLength(
    payload_length: usize,
    correlation_id: ?[16]u8,
    host_epoch: ?[16]u8,
) contract.ProtocolError!usize {
    if (correlation_id) |identifier| _ = try wire.copyIdentifier(&identifier);
    if (host_epoch) |identifier| _ = try wire.copyIdentifier(&identifier);
    const total_length = std.math.add(
        usize,
        contract.transport_envelope_header_bytes,
        payload_length,
    ) catch return error.LimitExceeded;
    if (total_length > contract.Limits.transport_record_bytes) return error.LimitExceeded;
    return total_length;
}

/// Finalizes an envelope whose payload already occupies the bytes immediately
/// after the fixed header. `storage` may be larger than the resulting record;
/// callers retain ownership of the complete allocation and use the returned
/// prefix as the wire record.
pub fn finalizeTransportEnvelope(
    storage: []u8,
    kind: u16,
    encoding: contract.PayloadEncoding,
    payload_length: usize,
    correlation_id: ?[16]u8,
    host_epoch: ?[16]u8,
    response: bool,
    event: bool,
) contract.ProtocolError![]u8 {
    const total_length = try transportEnvelopeLength(
        kind,
        encoding,
        payload_length,
        correlation_id,
        host_epoch,
        response,
        event,
    );
    if (storage.len < total_length) return error.Truncated;
    const bytes = storage[0..total_length];
    @memset(bytes[0..contract.transport_envelope_header_bytes], 0);
    @memcpy(bytes[0..4], "OBTP");
    std.mem.writeInt(u16, bytes[4..6], contract.transport_envelope_header_bytes, .little);
    var flags: u16 = 0;
    if (response) flags |= 2;
    if (event) flags |= 4;
    std.mem.writeInt(u16, bytes[6..8], flags, .little);
    std.mem.writeInt(u32, bytes[8..12], @intCast(total_length), .little);
    std.mem.writeInt(u32, bytes[12..16], @intCast(payload_length), .little);
    std.mem.writeInt(u16, bytes[16..18], contract.current_protocol_major, .little);
    std.mem.writeInt(u16, bytes[18..20], contract.current_protocol_minor, .little);
    std.mem.writeInt(u16, bytes[20..22], kind, .little);
    bytes[22] = @intFromEnum(encoding);
    if (correlation_id) |identifier| @memcpy(bytes[24..40], &identifier);
    if (host_epoch) |identifier| @memcpy(bytes[40..56], &identifier);
    std.mem.writeInt(u16, bytes[56..58], contract.current_protocol_patch, .little);
    return bytes;
}
