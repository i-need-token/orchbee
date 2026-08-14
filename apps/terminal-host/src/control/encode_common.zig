const identifiers = @import("terminal-host-model").identifiers;

pub const empty_control = "{}";

pub fn identifierText(identifier: anytype, buffer: *[32]u8) []const u8 {
    return identifiers.formatLowerHex(identifier, buffer);
}
