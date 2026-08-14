const std = @import("std");

const control_wire = @import("wire.zig");

pub const Error = control_wire.DecodeError || error{
    CommandInspectionOutOfMemory,
    InvalidField,
    InvalidIdentifier,
};

pub fn Decoded(comptime Wire: type, comptime Command: type) type {
    return struct {
        parsed: control_wire.Parsed(Wire),
        value: Command,

        pub fn deinit(self: *@This()) void {
            self.parsed.deinit();
            self.* = undefined;
        }
    };
}

pub fn decodeEmpty(
    allocator: std.mem.Allocator,
    payload: []const u8,
) Error!Decoded(control_wire.Empty, void) {
    const parsed = try control_wire.decode(control_wire.Empty, allocator, payload);
    return .{ .parsed = parsed, .value = {} };
}

pub fn parseCanonicalU64(value: []const u8) error{InvalidField}!u64 {
    if (value.len == 0 or (value.len > 1 and value[0] == '0')) return error.InvalidField;
    for (value) |byte| if (byte < '0' or byte > '9') return error.InvalidField;
    return std.fmt.parseInt(u64, value, 10) catch error.InvalidField;
}

pub fn parsePositiveCanonicalU64(value: []const u8) error{InvalidField}!u64 {
    const parsed = try parseCanonicalU64(value);
    if (parsed == 0) return error.InvalidField;
    return parsed;
}
