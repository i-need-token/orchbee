const std = @import("std");
const subject = @import("terminal-host-model").identifiers;

test "semantic identifiers retain bytes without sharing a type" {
    const bytes = [_]u8{1} ** 16;
    const attachment_id = subject.AttachmentId.fromBytes(bytes);
    const client_id = subject.ClientId.fromBytes(bytes);
    const creation_id = subject.CreationId.fromBytes(bytes);
    const host_epoch = subject.HostEpoch.fromBytes(bytes);
    const terminal_id = subject.TerminalId.fromBytes(bytes);

    try std.testing.expect(@TypeOf(attachment_id) != @TypeOf(client_id));
    try std.testing.expect(@TypeOf(creation_id) != @TypeOf(terminal_id));
    try std.testing.expect(@TypeOf(host_epoch) != @TypeOf(terminal_id));
    try std.testing.expectEqualSlices(u8, &bytes, &attachment_id.bytes);
    try std.testing.expect(terminal_id.eql(subject.TerminalId.fromBytes(bytes)));
}

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
