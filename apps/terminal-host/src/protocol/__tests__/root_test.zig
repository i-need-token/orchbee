const std = @import("std");
const subject = @import("terminal-protocol");

const PayloadEncoding = subject.PayloadEncoding;
const Reader = subject.testing.ReaderType;
const encodeTransportEnvelope = subject.encodeTransportEnvelope;
const finalizeTransportEnvelope = subject.finalizeTransportEnvelope;
const isKnownMessageKind = subject.isKnownMessageKind;
const max_control_json_bytes = subject.Limits.control_json_bytes;
const optionalIdentifier = subject.testing.optionalIdentifierFn;
const terminal_frame_header_bytes = subject.terminal_frame_header_bytes;
const transportRecordLength = subject.transportRecordLength;
const transport_envelope_header_bytes = subject.transport_envelope_header_bytes;
const validateTerminalFrame = subject.validateTerminalFrame;

test "frame validator rejects zero required identifiers" {
    var bytes = [_]u8{0} ** terminal_frame_header_bytes;
    @memcpy(bytes[0..4], "OBTF");
    try std.testing.expectError(
        error.InvalidIdentifier,
        validateTerminalFrame(std.testing.allocator, &bytes),
    );
}

test "reader rejects an impossible starting offset even for an empty read" {
    const empty = [_]u8{};
    var reader = Reader.init(&empty, 1);
    try std.testing.expectError(error.Truncated, reader.readBytes(0));
}

test "optional and encoded identifiers preserve the invalid identifier error" {
    const zero_identifier = [_]u8{0} ** 16;
    try std.testing.expectError(error.InvalidIdentifier, optionalIdentifier(&.{ 1, 2, 3 }));
    try std.testing.expectError(
        error.InvalidIdentifier,
        encodeTransportEnvelope(
            std.testing.allocator,
            4,
            .control_json,
            "{}",
            zero_identifier,
            null,
            false,
            false,
        ),
    );
    try std.testing.expectError(
        error.InvalidIdentifier,
        encodeTransportEnvelope(
            std.testing.allocator,
            4,
            .control_json,
            "{}",
            null,
            zero_identifier,
            false,
            false,
        ),
    );
}

test "required envelope encoder rejects unknown message kinds before allocation" {
    try std.testing.expectError(
        error.UnknownRequiredMessage,
        encodeTransportEnvelope(
            std.testing.allocator,
            200,
            .control_json,
            "{}",
            null,
            null,
            false,
            false,
        ),
    );
}

test "required envelope encoder rejects a registered kind with the wrong role or encoding" {
    try std.testing.expectError(
        error.InvalidField,
        encodeTransportEnvelope(
            std.testing.allocator,
            subject.MessageKind.terminal_started,
            .control_json,
            "{}",
            null,
            [_]u8{0x33} ** 16,
            false,
            false,
        ),
    );
    try std.testing.expectError(
        error.InvalidField,
        encodeTransportEnvelope(
            std.testing.allocator,
            subject.MessageKind.write_input,
            .control_json,
            "{}",
            [_]u8{0x22} ** 16,
            [_]u8{0x33} ** 16,
            false,
            false,
        ),
    );
}

test "transport record length applies the encoding bound before allocation" {
    var header = [_]u8{0} ** transport_envelope_header_bytes;
    @memcpy(header[0..4], "OBTP");
    std.mem.writeInt(u16, header[4..6], transport_envelope_header_bytes, .little);
    header[22] = @intFromEnum(PayloadEncoding.control_json);
    const oversized_control_bytes: u32 = max_control_json_bytes + 1;
    std.mem.writeInt(
        u32,
        header[8..12],
        transport_envelope_header_bytes + oversized_control_bytes,
        .little,
    );
    std.mem.writeInt(u32, header[12..16], oversized_control_bytes, .little);
    try std.testing.expectError(error.LimitExceeded, transportRecordLength(&header));

    std.mem.writeInt(u32, header[8..12], transport_envelope_header_bytes, .little);
    std.mem.writeInt(u32, header[12..16], 0, .little);
    header[22] = 0xff;
    try std.testing.expectError(error.InvalidField, transportRecordLength(&header));
}

test "transport envelope finalizes a payload already written into reserved storage" {
    var storage: [transport_envelope_header_bytes + 64]u8 = undefined;
    const payload = "{\"reserved\":true}";
    @memcpy(storage[transport_envelope_header_bytes..][0..payload.len], payload);
    const record = try finalizeTransportEnvelope(
        &storage,
        subject.MessageKind.ping,
        .control_json,
        payload.len,
        [_]u8{0x22} ** 16,
        [_]u8{0x33} ** 16,
        true,
        false,
    );
    const envelope = try subject.validateTransportEnvelope(record);
    try std.testing.expectEqualStrings(payload, envelope.payload);
    try std.testing.expectEqual(
        transport_envelope_header_bytes + payload.len,
        record.len,
    );
}

test "message kind registry contains exactly the current declared kinds" {
    var declared = [_]bool{false} ** 65_536;
    inline for (@typeInfo(subject.MessageKind).@"struct".decls) |declaration| {
        const kind: u16 = @field(subject.MessageKind, declaration.name);
        try std.testing.expect(!declared[kind]);
        declared[kind] = true;
        try std.testing.expect(isKnownMessageKind(kind));
    }
    for (0..65_536) |value| {
        const kind: u16 = @intCast(value);
        try std.testing.expectEqual(
            declared[value],
            isKnownMessageKind(kind),
        );
    }
}

test "every native request dispatch operation is owned by the registry" {
    const operation_count = @typeInfo(subject.RequestOperation).@"enum".fields.len;
    var seen = [_]bool{false} ** operation_count;
    inline for (@typeInfo(subject.MessageKind).@"struct".decls) |declaration| {
        const kind: u16 = @field(subject.MessageKind, declaration.name);
        const contract = subject.messageContract(kind).?;
        if (contract.request) |request| {
            seen[@intFromEnum(request.operation)] = true;
        }
    }
    for (seen) |registered| try std.testing.expect(registered);

    const semantic_kinds = [_]u16{
        subject.MessageKind.focus_input,
        subject.MessageKind.key_input,
        subject.MessageKind.mouse_input,
        subject.MessageKind.paste_input,
        subject.MessageKind.text_input,
    };
    for (semantic_kinds) |kind| {
        try std.testing.expectEqual(
            subject.RequestOperation.semantic_input,
            subject.messageContract(kind).?.request.?.operation,
        );
    }
}

test "semantic input preserves U+FEFF and rejects a BOM-prefixed key" {
    var text_payload = [_]u8{0x22} ** 31;
    std.mem.writeInt(u64, text_payload[16..24], 7, .little);
    std.mem.writeInt(u32, text_payload[24..28], 3, .little);
    @memcpy(text_payload[28..31], "\xef\xbb\xbf");

    const decoded_text = try subject.validateCommittedTextInput(&text_payload);
    const text = switch (decoded_text.input) {
        .text => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expectEqualSlices(u8, "\xef\xbb\xbf", text.text);

    var paste_payload = [_]u8{0} ** 35;
    @memset(paste_payload[0..16], 0x22);
    std.mem.writeInt(u64, paste_payload[16..24], 7, .little);
    paste_payload[24] = 1;
    std.mem.writeInt(u32, paste_payload[28..32], 3, .little);
    @memcpy(paste_payload[32..35], "\xef\xbb\xbf");
    const decoded_paste = try subject.validatePasteInput(&paste_payload);
    const paste = switch (decoded_paste.input) {
        .paste => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expect(paste.bracketed);
    try std.testing.expectEqualSlices(u8, "\xef\xbb\xbf", paste.text);

    var key_payload = [_]u8{0} ** 45;
    @memset(key_payload[0..16], 0x22);
    std.mem.writeInt(u64, key_payload[16..24], 7, .little);
    key_payload[24] = 1;
    std.mem.writeInt(u16, key_payload[30..32], 5, .little);
    @memcpy(key_payload[40..45], "\xef\xbb\xbfF1");
    try std.testing.expectError(error.InvalidField, subject.validateKeyInput(&key_payload));
}

test "mouse input admits the exact finite float32 coordinate boundary" {
    var payload = [_]u8{0} ** 40;
    @memset(payload[0..16], 0x22);
    std.mem.writeInt(u64, payload[16..24], 7, .little);
    payload[24] = 1;
    payload[25] = 1;
    std.mem.writeInt(u32, payload[28..32], 0x7f7fffff, .little);
    std.mem.writeInt(u32, payload[32..36], 0xff7fffff, .little);

    const decoded = try subject.validateMouseInput(&payload);
    const mouse = switch (decoded.input) {
        .mouse => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expectEqual(@as(u32, 0x7f7fffff), @as(u32, @bitCast(mouse.x)));
    try std.testing.expectEqual(@as(u32, 0xff7fffff), @as(u32, @bitCast(mouse.y)));
}
