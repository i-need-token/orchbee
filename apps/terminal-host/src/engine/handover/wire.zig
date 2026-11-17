//! Little-endian fields of the handover record. A reader refuses a field that runs past its
//! section; the section codecs judge the values.
const std = @import("std");

pub const Error = error{InvalidRecord};

pub const Writer = struct {
    allocator: std.mem.Allocator,
    list: *std.ArrayList(u8),

    pub fn int(self: Writer, comptime T: type, value: T) std.mem.Allocator.Error!void {
        var raw: [@sizeOf(T)]u8 = undefined;
        std.mem.writeInt(T, &raw, value, .little);
        try self.list.appendSlice(self.allocator, &raw);
    }

    pub fn flag(self: Writer, value: bool) std.mem.Allocator.Error!void {
        try self.int(u8, @intFromBool(value));
    }

    pub fn bytes(self: Writer, value: []const u8) std.mem.Allocator.Error!void {
        try self.list.appendSlice(self.allocator, value);
    }

    /// The exporting Host's own state never exceeds a field's bound; a value that does is a
    /// defect the export refuses rather than truncates.
    pub fn counted(self: Writer, comptime Length: type, value: []const u8) (Error || std.mem.Allocator.Error)!void {
        try self.int(Length, std.math.cast(Length, value.len) orelse return error.InvalidRecord);
        try self.bytes(value);
    }

    pub fn optionalInt(self: Writer, comptime T: type, value: ?T) std.mem.Allocator.Error!void {
        try self.flag(value != null);
        if (value) |present| try self.int(T, present);
    }
};

pub const Reader = struct {
    source: []const u8,
    offset: usize = 0,

    pub fn init(source: []const u8) Reader {
        return .{ .source = source };
    }

    pub fn take(self: *Reader, length: usize) Error![]const u8 {
        if (length > self.source.len - self.offset) return error.InvalidRecord;
        const result = self.source[self.offset..][0..length];
        self.offset += length;
        return result;
    }

    pub fn array(self: *Reader, comptime length: usize) Error![length]u8 {
        return (try self.take(length))[0..length].*;
    }

    pub fn int(self: *Reader, comptime T: type) Error!T {
        return std.mem.readInt(T, &(try self.array(@sizeOf(T))), .little);
    }

    pub fn flag(self: *Reader) Error!bool {
        return switch (try self.int(u8)) {
            0 => false,
            1 => true,
            else => error.InvalidRecord,
        };
    }

    pub fn counted(self: *Reader, comptime Length: type, maximum: usize) Error![]const u8 {
        const length = try self.int(Length);
        if (length > maximum) return error.InvalidRecord;
        return self.take(length);
    }

    pub fn optionalInt(self: *Reader, comptime T: type) Error!?T {
        return if (try self.flag()) try self.int(T) else null;
    }

    /// Bounds a count by the bytes left, so a forged count cannot size an allocation beyond
    /// what the section could hold at `minimum_item_bytes` per item.
    pub fn count(self: *Reader, comptime Length: type, maximum: usize, minimum_item_bytes: usize) Error!usize {
        const value = try self.int(Length);
        if (value > maximum or value > (self.source.len - self.offset) / minimum_item_bytes) {
            return error.InvalidRecord;
        }
        return value;
    }

    pub fn finish(self: *const Reader) Error!void {
        if (self.offset != self.source.len) return error.InvalidRecord;
    }
};

/// Wire values of an enum, listed once so encoder and decoder cannot disagree. Values start at
/// one; zero is never a member.
pub fn Code(comptime T: type, comptime members: []const T) type {
    comptime if (members.len != @typeInfo(T).@"enum".fields.len) {
        @compileError("every member of " ++ @typeName(T) ++ " needs a wire value");
    };
    return struct {
        pub fn encode(value: T) u8 {
            inline for (members, 1..) |member, code| if (value == member) return code;
            unreachable;
        }

        pub fn decode(code: u8) Error!T {
            if (code == 0 or code > members.len) return error.InvalidRecord;
            return members[code - 1];
        }
    };
}
