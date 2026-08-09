const std = @import("std");

const contract = @import("contract.zig");

pub const Reader = struct {
    bytes: []const u8,
    offset: usize,

    pub fn init(bytes: []const u8, offset: usize) Reader {
        return .{ .bytes = bytes, .offset = offset };
    }

    pub fn readBytes(self: *Reader, length: usize) contract.ProtocolError![]const u8 {
        if (self.offset > self.bytes.len) return error.Truncated;
        if (length > self.bytes.len -| self.offset) return error.Truncated;
        const value = self.bytes[self.offset .. self.offset + length];
        self.offset += length;
        return value;
    }

    pub fn readInt(self: *Reader, comptime Int: type) contract.ProtocolError!Int {
        const bytes = try self.readBytes(@sizeOf(Int));
        return std.mem.readInt(Int, bytes[0..@sizeOf(Int)], .little);
    }
};

pub fn copyIdentifier(bytes: []const u8) contract.ProtocolError![16]u8 {
    if (bytes.len != 16) return error.InvalidIdentifier;
    var result: [16]u8 = undefined;
    @memcpy(&result, bytes);
    var non_zero = false;
    for (result) |byte| non_zero = non_zero or byte != 0;
    if (!non_zero) return error.InvalidIdentifier;
    return result;
}

pub fn optionalIdentifier(bytes: []const u8) contract.ProtocolError!?[16]u8 {
    if (bytes.len != 16) return error.InvalidIdentifier;
    var non_zero = false;
    for (bytes) |byte| non_zero = non_zero or byte != 0;
    return if (non_zero) try copyIdentifier(bytes) else null;
}

pub fn requireMagic(bytes: []const u8, expected: *const [4]u8) contract.ProtocolError!void {
    if (bytes.len < expected.len) return error.Truncated;
    if (!std.mem.eql(u8, bytes[0..4], expected)) return error.InvalidMagic;
}
