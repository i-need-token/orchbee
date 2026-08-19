const std = @import("std");

pub const IdentifierBytes = [16]u8;

pub const AttachmentId = struct {
    bytes: IdentifierBytes,

    pub fn fromBytes(bytes: IdentifierBytes) @This() {
        return .{ .bytes = bytes };
    }

    pub fn eql(self: @This(), other: @This()) bool {
        return equalBytes(self.bytes, other.bytes);
    }
};

pub const ClientId = struct {
    bytes: IdentifierBytes,

    pub fn fromBytes(bytes: IdentifierBytes) @This() {
        return .{ .bytes = bytes };
    }

    pub fn eql(self: @This(), other: @This()) bool {
        return equalBytes(self.bytes, other.bytes);
    }
};

pub const CreationId = struct {
    bytes: IdentifierBytes,

    pub fn fromBytes(bytes: IdentifierBytes) @This() {
        return .{ .bytes = bytes };
    }

    pub fn eql(self: @This(), other: @This()) bool {
        return equalBytes(self.bytes, other.bytes);
    }
};

pub const HostEpoch = struct {
    bytes: IdentifierBytes,

    pub fn fromBytes(bytes: IdentifierBytes) @This() {
        return .{ .bytes = bytes };
    }

    pub fn random() @This() {
        return randomNonZero(@This());
    }

    pub fn eql(self: @This(), other: @This()) bool {
        return equalBytes(self.bytes, other.bytes);
    }
};

pub const TerminalId = struct {
    bytes: IdentifierBytes,

    pub fn fromBytes(bytes: IdentifierBytes) @This() {
        return .{ .bytes = bytes };
    }

    pub fn random() @This() {
        return randomNonZero(@This());
    }

    pub fn eql(self: @This(), other: @This()) bool {
        return equalBytes(self.bytes, other.bytes);
    }
};

pub const TerminalThemeOperationId = struct {
    bytes: IdentifierBytes,

    pub fn fromBytes(bytes: IdentifierBytes) @This() {
        return .{ .bytes = bytes };
    }

    pub fn eql(self: @This(), other: @This()) bool {
        return equalBytes(self.bytes, other.bytes);
    }
};

pub fn formatLowerHex(identifier: anytype, buffer: *[32]u8) []const u8 {
    const digits = "0123456789abcdef";
    for (identifier.bytes, 0..) |byte, index| {
        buffer[index * 2] = digits[byte >> 4];
        buffer[index * 2 + 1] = digits[byte & 0x0f];
    }
    return buffer;
}

pub fn parseLowerHex(comptime Id: type, value: []const u8) error{InvalidIdentifier}!Id {
    if (value.len != 32) return error.InvalidIdentifier;
    var result: IdentifierBytes = undefined;
    var non_zero = false;
    for (&result, 0..) |*byte, index| {
        const high = std.fmt.charToDigit(value[index * 2], 16) catch
            return error.InvalidIdentifier;
        const low = std.fmt.charToDigit(value[index * 2 + 1], 16) catch
            return error.InvalidIdentifier;
        if ((value[index * 2] >= 'A' and value[index * 2] <= 'F') or
            (value[index * 2 + 1] >= 'A' and value[index * 2 + 1] <= 'F'))
        {
            return error.InvalidIdentifier;
        }
        byte.* = @intCast(high * 16 + low);
        non_zero = non_zero or byte.* != 0;
    }
    if (!non_zero) return error.InvalidIdentifier;
    return Id.fromBytes(result);
}

fn equalBytes(left: IdentifierBytes, right: IdentifierBytes) bool {
    return std.mem.eql(u8, &left, &right);
}

fn randomNonZero(comptime Id: type) Id {
    while (true) {
        var bytes: IdentifierBytes = undefined;
        std.crypto.random.bytes(&bytes);
        for (bytes) |byte| {
            if (byte != 0) return Id.fromBytes(bytes);
        }
    }
}
