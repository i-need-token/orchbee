const std = @import("std");
const subject = @import("terminal-host-model").identifiers;

test "semantic identifiers share one canonical lowercase wire rendering" {
    const terminal_id = subject.TerminalId.fromBytes(.{
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
        0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
    });
    var buffer: [32]u8 = undefined;
    try std.testing.expectEqualStrings(
        "0123456789abcdeffedcba9876543210",
        subject.formatLowerHex(terminal_id, &buffer),
    );
}

test "wire identifiers require canonical non-zero lowercase hexadecimal text" {
    const parsed = try subject.parseLowerHex(
        subject.TerminalId,
        "00112233445566778899aabbccddeeff",
    );
    var buffer: [32]u8 = undefined;
    try std.testing.expectEqualStrings(
        "00112233445566778899aabbccddeeff",
        subject.formatLowerHex(parsed, &buffer),
    );
    try std.testing.expectError(
        error.InvalidIdentifier,
        subject.parseLowerHex(subject.TerminalId, "00112233445566778899AABBCCDDEEFF"),
    );
    try std.testing.expectError(
        error.InvalidIdentifier,
        subject.parseLowerHex(subject.TerminalId, "00000000000000000000000000000000"),
    );
}
