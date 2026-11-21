//! The `keeper` capability's requests on `worker-host`. A process is `{pid, creationTime}`: a
//! positive uint32 and its `FILETIME` as positive canonical decimal text. A Keeper endpoint id is 32
//! lowercase hexadecimal digits.

const std = @import("std");
const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");

pub const KeeperProcess = struct {
    pid: u32,
    /// The process's creation time as a `FILETIME`, never zero.
    created: u64,
};

pub const AttachKeeper = struct {
    keeper: KeeperProcess,
    endpoint_id: [32]u8,
};

pub const WireProcess = struct {
    pid: u32,
    creationTime: []const u8,
};

const WireAttachKeeper = struct {
    keeper: WireProcess,
    endpointId: []const u8,
};

const WireDetachKeeper = struct {
    keeper: WireProcess,
};

pub fn decodeAttachKeeper(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireAttachKeeper, AttachKeeper) {
    const parsed = try control_wire.decode(WireAttachKeeper, allocator, payload);
    errdefer parsed.deinit();
    return .{ .parsed = parsed, .value = .{
        .keeper = try process(parsed.value.keeper),
        .endpoint_id = try endpointId(parsed.value.endpointId),
    } };
}

pub fn decodeDetachKeeper(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireDetachKeeper, KeeperProcess) {
    const parsed = try control_wire.decode(WireDetachKeeper, allocator, payload);
    errdefer parsed.deinit();
    return .{ .parsed = parsed, .value = try process(parsed.value.keeper) };
}

pub fn process(wire: WireProcess) error{InvalidField}!KeeperProcess {
    if (wire.pid == 0) return error.InvalidField;
    return .{ .pid = wire.pid, .created = try decode_common.parsePositiveCanonicalU64(wire.creationTime) };
}

fn endpointId(text: []const u8) error{InvalidField}![32]u8 {
    if (text.len != 32) return error.InvalidField;
    for (text) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return error.InvalidField,
    };
    return text[0..32].*;
}
