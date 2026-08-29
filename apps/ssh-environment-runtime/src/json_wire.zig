const std = @import("std");

pub const DecodeError = error{
    InvalidField,
    InvalidJson,
    InvalidUtf8,
    OutOfMemory,
};

const max_safe_integer: f64 = 9_007_199_254_740_991.0;

pub fn decode(
    comptime Wire: type,
    allocator: std.mem.Allocator,
    payload: []const u8,
) DecodeError!std.json.Parsed(Wire) {
    if (!std.unicode.utf8ValidateSlice(payload)) return error.InvalidUtf8;

    var dynamic = std.json.parseFromSlice(
        std.json.Value,
        allocator,
        payload,
        .{ .parse_numbers = false },
    ) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.InvalidJson,
    };
    errdefer dynamic.deinit();

    refineValue(Wire, &dynamic.value) catch return error.InvalidField;
    const value = std.json.parseFromValueLeaky(
        Wire,
        dynamic.arena.allocator(),
        dynamic.value,
        .{},
    ) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.InvalidField,
    };
    return .{ .arena = dynamic.arena, .value = value };
}

fn refineValue(comptime T: type, value: *std.json.Value) error{InvalidField}!void {
    switch (@typeInfo(T)) {
        .int => try refineInteger(T, value),
        .@"struct" => |struct_info| {
            if (struct_info.is_tuple or value.* != .object) return error.InvalidField;
            var fields_seen = [_]bool{false} ** struct_info.fields.len;
            var iterator = value.object.iterator();
            while (iterator.next()) |entry| {
                inline for (struct_info.fields, 0..) |field, index| {
                    if (std.mem.eql(u8, field.name, entry.key_ptr.*)) {
                        try refineValue(field.type, entry.value_ptr);
                        fields_seen[index] = true;
                        break;
                    }
                } else return error.InvalidField;
            }
            inline for (struct_info.fields, fields_seen) |field, seen| {
                if (!seen and field.default_value_ptr == null) return error.InvalidField;
            }
        },
        .array => |array| {
            if (value.* != .array or value.array.items.len != array.len) {
                return error.InvalidField;
            }
            for (value.array.items) |*item| try refineValue(array.child, item);
        },
        .pointer => |pointer| switch (pointer.size) {
            .slice => {
                if (pointer.child == u8) {
                    if (value.* != .string) return error.InvalidField;
                    return;
                }
                if (value.* != .array) return error.InvalidField;
                for (value.array.items) |*item| try refineValue(pointer.child, item);
            },
            else => return error.InvalidField,
        },
        else => return error.InvalidField,
    }
}

fn refineInteger(comptime T: type, value: *std.json.Value) error{InvalidField}!void {
    const lexeme = switch (value.*) {
        .number_string => |number| number,
        else => return error.InvalidField,
    };
    const number = std.fmt.parseFloat(f64, lexeme) catch return error.InvalidField;
    if (!std.math.isFinite(number) or @trunc(number) != number or
        number < 0 or number > max_safe_integer)
    {
        return error.InvalidField;
    }
    const maximum: f64 = @floatFromInt(std.math.maxInt(T));
    if (number > maximum) return error.InvalidField;
    value.* = .{ .integer = @intFromFloat(number) };
}
