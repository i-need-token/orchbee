const std = @import("std");

const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
pub const ProtocolVersion = struct {
    major: u16,
    minor: u16,
    patch: u16,
};

pub const Hello = struct {
    client_id: identifiers.ClientId,
    frame_bytes: usize,
    input_bytes: usize,
    protocol_version: ProtocolVersion,
    row_window_bytes: usize,
};

pub const InventoryCursor = struct {
    generation: u64,
    offset: usize,
};

pub const ListTerminals = struct {
    cursor: ?InventoryCursor,
    maximum_terminals: usize,
};
pub fn decodeHello(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.Hello, Hello) {
    const parsed = try control_wire.decode(control_wire.Hello, allocator, payload);
    errdefer parsed.deinit();
    if (!validHello(parsed.value)) return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .client_id = try identifiers.parseLowerHex(identifiers.ClientId, parsed.value.clientId),
            .frame_bytes = parsed.value.frameBytes,
            .input_bytes = parsed.value.inputBytes,
            .protocol_version = .{
                .major = parsed.value.protocol.major,
                .minor = parsed.value.protocol.minor,
                .patch = parsed.value.protocol.patch,
            },
            .row_window_bytes = parsed.value.rowWindowBytes,
        },
    };
}

pub fn decodeListTerminals(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.ListTerminals, ListTerminals) {
    const parsed = try control_wire.decode(control_wire.ListTerminals, allocator, payload);
    errdefer parsed.deinit();
    if (parsed.value.maximumTerminals == 0 or
        parsed.value.maximumTerminals > protocol.Limits.inventory_page_terminals)
    {
        return error.InvalidField;
    }
    const cursor: ?InventoryCursor = if (parsed.value.cursor) |wire_cursor| .{
        .generation = try decode_common.parsePositiveCanonicalU64(wire_cursor.inventoryGeneration),
        .offset = std.math.cast(
            usize,
            try decode_common.parseCanonicalU64(wire_cursor.offset),
        ) orelse return error.InvalidField,
    } else null;
    return .{
        .parsed = parsed,
        .value = .{
            .cursor = cursor,
            .maximum_terminals = parsed.value.maximumTerminals,
        },
    };
}
pub fn validHello(hello: control_wire.Hello) bool {
    if (hello.frameBytes == 0 or hello.frameBytes > protocol.Limits.frame_bytes or
        hello.inputBytes == 0 or hello.inputBytes > protocol.Limits.input_bytes or
        hello.rowWindowBytes == 0 or hello.rowWindowBytes > protocol.Limits.row_window_bytes or
        hello.supportedOptionalFeatures.len > protocol.Limits.optional_features)
    {
        return false;
    }
    for (hello.supportedOptionalFeatures) |feature| {
        if (feature.len == 0 or feature.len > protocol.Limits.optional_feature_name_bytes) {
            return false;
        }
    }
    return true;
}
