//! The `handover` capability's requests on `worker-host`. Terminal and creation ids are 32 lowercase
//! hexadecimal digits and a process is `{pid, creationTime}`, as `keeper` names it. A
//! handover-record payload is binary: a 32-byte header, then the chunk.

const std = @import("std");
const identifiers = @import("terminal-host-model").identifiers;
const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const decode_keeper = @import("decode_keeper.zig");

const TerminalId = identifiers.TerminalId;
const CreationId = identifiers.CreationId;

/// A record is at most 128 MiB; the `obhr` envelope enforces the same bound.
pub const max_record_bytes: u32 = 128 * 1024 * 1024;
pub const max_chunk_bytes: u32 = 4 * 1024 * 1024;
pub const record_header_bytes = 32;

pub const Quiesce = struct {
    terminal_id: TerminalId,
    /// The Host the mark names.
    target: decode_keeper.KeeperProcess,
    deadline_seconds: u8,
};

pub const ReadRecord = struct {
    terminal_id: TerminalId,
    offset: u32,
};

pub const Import = struct {
    creation_id: CreationId,
    record_bytes: u32,
    deadline_seconds: u8,
};

pub const Discard = union(enum) {
    import: CreationId,
    terminal: TerminalId,
};

/// One chunk of a record. `chunk` borrows the payload.
pub const RecordChunk = struct {
    /// The creation id of an import, or the terminal id of an export.
    id: [16]u8,
    offset: u32,
    chunk: []const u8,
};

const WireQuiesce = struct {
    terminalId: []const u8,
    target: decode_keeper.WireProcess,
    deadlineSeconds: u8,
};

const WireReadRecord = struct {
    terminalId: []const u8,
    offset: u32,
};

const WireImport = struct {
    creationId: []const u8,
    recordBytes: u32,
    deadlineSeconds: u8,
};

const WireCreation = struct {
    creationId: []const u8,
};

const WireDiscard = struct {
    creationId: ?[]const u8 = null,
    terminalId: ?[]const u8 = null,
};

pub fn decodeQuiesce(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireQuiesce, Quiesce) {
    const parsed = try control_wire.decode(WireQuiesce, allocator, payload);
    errdefer parsed.deinit();
    return .{ .parsed = parsed, .value = .{
        .terminal_id = try identifiers.parseLowerHex(TerminalId, parsed.value.terminalId),
        .target = try decode_keeper.process(parsed.value.target),
        .deadline_seconds = try deadline(parsed.value.deadlineSeconds),
    } };
}

pub fn decodeReadRecord(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireReadRecord, ReadRecord) {
    const parsed = try control_wire.decode(WireReadRecord, allocator, payload);
    errdefer parsed.deinit();
    if (parsed.value.offset >= max_record_bytes) return error.InvalidField;
    return .{ .parsed = parsed, .value = .{
        .terminal_id = try identifiers.parseLowerHex(TerminalId, parsed.value.terminalId),
        .offset = parsed.value.offset,
    } };
}

pub fn decodeImport(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireImport, Import) {
    const parsed = try control_wire.decode(WireImport, allocator, payload);
    errdefer parsed.deinit();
    const record_bytes = parsed.value.recordBytes;
    if (record_bytes == 0 or record_bytes > max_record_bytes) return error.InvalidField;
    return .{ .parsed = parsed, .value = .{
        .creation_id = try identifiers.parseLowerHex(CreationId, parsed.value.creationId),
        .record_bytes = record_bytes,
        .deadline_seconds = try deadline(parsed.value.deadlineSeconds),
    } };
}

/// `RestoreTerminal`'s payload.
pub fn decodeCreation(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireCreation, CreationId) {
    const parsed = try control_wire.decode(WireCreation, allocator, payload);
    errdefer parsed.deinit();
    return .{ .parsed = parsed, .value = try identifiers.parseLowerHex(CreationId, parsed.value.creationId) };
}

/// `DiscardImport` names exactly one of an import's creation id or a restored terminal's id.
pub fn decodeDiscard(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireDiscard, Discard) {
    const parsed = try control_wire.decode(WireDiscard, allocator, payload);
    errdefer parsed.deinit();
    const value: Discard = if (parsed.value.creationId) |creation| blk: {
        if (parsed.value.terminalId != null) return error.InvalidField;
        break :blk .{ .import = try identifiers.parseLowerHex(CreationId, creation) };
    } else if (parsed.value.terminalId) |terminal|
        .{ .terminal = try identifiers.parseLowerHex(TerminalId, terminal) }
    else
        return error.InvalidField;
    return .{ .parsed = parsed, .value = value };
}

/// Checks a handover-record payload's header: the id, the offset, a length from 1 to 4 MiB that
/// the chunk fills exactly and that ends inside 128 MiB, and the reserved bytes zero.
pub fn decodeRecordChunk(payload: []const u8) error{InvalidField}!RecordChunk {
    if (payload.len < record_header_bytes) return error.InvalidField;
    const offset_value = std.mem.readInt(u64, payload[16..24], .little);
    const length = std.mem.readInt(u32, payload[24..28], .little);
    if (std.mem.readInt(u32, payload[28..32], .little) != 0) return error.InvalidField;
    if (length == 0 or length > max_chunk_bytes or payload.len - record_header_bytes != length) {
        return error.InvalidField;
    }
    if (offset_value > max_record_bytes - length) return error.InvalidField;
    return .{
        .id = payload[0..16].*,
        .offset = @intCast(offset_value),
        .chunk = payload[record_header_bytes..],
    };
}

fn deadline(seconds: u8) error{InvalidField}!u8 {
    if (seconds < 1 or seconds > 30) return error.InvalidField;
    return seconds;
}
