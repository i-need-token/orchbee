//! The `handover` capability's answers on `worker-host`. Members appear in bytewise name order, as
//! every control encoder writes them. `HandoverRecordChunk` is binary: a 32-byte header naming the
//! export's terminal, the chunk's offset and length, then the chunk.

const std = @import("std");
const common = @import("encode_common.zig");
const decode_handover = @import("decode_handover.zig");

pub const ResumeOutcome = enum {
    resumed,
    claimed_elsewhere,

    fn wire(self: ResumeOutcome) []const u8 {
        return switch (self) {
            .resumed => "resumed",
            .claimed_elsewhere => "claimed-elsewhere",
        };
    }
};

pub const DiscardOutcome = enum { discarded, claimed };

/// `QuiesceTerminal`'s answer: the exported record's size.
pub fn quiesced(allocator: std.mem.Allocator, terminal_id: anytype, record_bytes: u32) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"recordBytes\":{d},\"terminalId\":\"{s}\"}}", .{
        record_bytes,
        common.identifierText(terminal_id, &terminal_buffer),
    });
}

pub fn resumed(allocator: std.mem.Allocator, terminal_id: anytype, outcome: ResumeOutcome) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"outcome\":\"{s}\",\"terminalId\":\"{s}\"}}", .{
        outcome.wire(),
        common.identifierText(terminal_id, &terminal_buffer),
    });
}

/// The answer of `DropTerminal` and `ClaimTerminal`.
pub fn terminalNamed(allocator: std.mem.Allocator, terminal_id: anytype) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"terminalId\":\"{s}\"}}", .{common.identifierText(terminal_id, &terminal_buffer)});
}

pub fn imported(allocator: std.mem.Allocator, creation_id: anytype) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"creationId\":\"{s}\"}}", .{common.identifierText(creation_id, &creation_buffer)});
}

pub fn recordWritten(allocator: std.mem.Allocator, creation_id: anytype, received_bytes: u32) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"creationId\":\"{s}\",\"receivedBytes\":{d}}}", .{
        common.identifierText(creation_id, &creation_buffer),
        received_bytes,
    });
}

pub fn discarded(outcome: DiscardOutcome) []const u8 {
    return switch (outcome) {
        .discarded => "{\"outcome\":\"discarded\"}",
        .claimed => "{\"outcome\":\"claimed\"}",
    };
}

/// One `HandoverRecordChunk`: the header for `chunk` at `offset` of the record of the terminal
/// `terminal_id`, then the chunk. `chunk` holds 1 byte to 4 MiB.
pub fn recordChunk(allocator: std.mem.Allocator, terminal_id: [16]u8, offset: u32, chunk: []const u8) ![]u8 {
    std.debug.assert(chunk.len > 0 and chunk.len <= decode_handover.max_chunk_bytes);
    const payload = try allocator.alloc(u8, decode_handover.record_header_bytes + chunk.len);
    payload[0..16].* = terminal_id;
    std.mem.writeInt(u64, payload[16..24], offset, .little);
    std.mem.writeInt(u32, payload[24..28], @intCast(chunk.len), .little);
    std.mem.writeInt(u32, payload[28..32], 0, .little);
    @memcpy(payload[decode_handover.record_header_bytes..], chunk);
    return payload;
}
