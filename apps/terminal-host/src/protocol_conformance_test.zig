const std = @import("std");
const protocol = @import("terminal-protocol");
const options = @import("conformance-options");

const frame_fixture = options.frame_fixture;
const envelope_fixture = options.envelope_fixture;
const input_fixture = options.input_fixture;
const key_input_fixture = options.key_input_fixture;
const text_input_fixture = options.text_input_fixture;
const mouse_input_fixture = options.mouse_input_fixture;
const focus_input_fixture = options.focus_input_fixture;
const paste_input_fixture = options.paste_input_fixture;

test "canonical TypeScript terminal frame fixture conforms in Zig" {
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame_fixture);
    try std.testing.expectEqual(protocol.FrameKind.delta, summary.kind);
    try std.testing.expectEqual(@as(u64, 7), summary.sequence);
    try std.testing.expectEqual(@as(?u64, 41), summary.base_revision);
    try std.testing.expectEqual(@as(u64, 42), summary.revision);
    try std.testing.expectEqual(@as(u64, 3), summary.generation);
    try std.testing.expectEqual(@as(u16, 6), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.rows);
    try std.testing.expectEqual(@as(usize, 12), summary.cell_count);
    try std.testing.expectEqual(@as(u16, 2), summary.style_count);
    try std.testing.expectEqual(@as(u16, 2), summary.row_count);
    try std.testing.expectEqual(@as(u16, 1), summary.extension_count);
    try std.testing.expectEqualStrings("Orchbee 🐝", summary.title.?);
    try std.testing.expectEqualStrings("/tmp/项目", summary.current_directory.?);
    try std.testing.expectEqual(protocol.CursorShape.bar, summary.cursor_shape);
    try std.testing.expectEqual(protocol.ActiveScreen.alternate, summary.active_screen);
    try std.testing.expectEqual(@as(u64, 123), summary.scrollback_rows);
    try std.testing.expectEqual(@as(u16, 8), summary.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 16), summary.cell_pixel_height);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x11} ** 16), &summary.host_epoch);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &summary.attachment_id);
}

test "canonical TypeScript envelope carries the exact canonical frame" {
    const summary = try protocol.validateTransportEnvelope(envelope_fixture);
    try std.testing.expect(summary.event);
    try std.testing.expect(!summary.response);
    try std.testing.expect(!summary.optional_message);
    try std.testing.expectEqual(@as(u16, 42), summary.kind);
    try std.testing.expectEqual(@as(u8, 3), summary.payload_encoding);
    try std.testing.expectEqual(@as(u16, 1), summary.protocol_major);
    try std.testing.expectEqual(@as(u16, 1), summary.protocol_minor);
    try std.testing.expectEqualSlices(u8, frame_fixture, summary.payload);
}

test "canonical TypeScript WriteInput fixture conforms in Zig" {
    const summary = try protocol.validateWriteInput(input_fixture);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &summary.attachment_id);
    try std.testing.expectEqualSlices(u8, &.{ 0, 0x1b, 0xf0, 0x9f, 0x90, 0x8d }, summary.bytes);
}

test "canonical TypeScript semantic-input fixtures conform in Zig" {
    const key = try protocol.validateKeyInput(key_input_fixture);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &key.attachment_id);
    try std.testing.expectEqualStrings("KeyA", key.key);
    try std.testing.expectEqualStrings("A", key.text);
    try std.testing.expectEqual(@as(u32, 'a'), key.unshifted_codepoint);

    const text = try protocol.validateCommittedTextInput(text_input_fixture);
    try std.testing.expectEqualStrings("中文", text.text);
    const paste = try protocol.validatePasteInput(paste_input_fixture);
    try std.testing.expectEqualStrings("line 1\nline 2", paste.text);

    const mouse = try protocol.validateMouseInput(mouse_input_fixture);
    try std.testing.expectEqual(@as(?u8, 1), mouse.button);
    try std.testing.expectEqual(@as(f32, 10.5), mouse.x);
    try std.testing.expectEqual(@as(f32, -2.25), mouse.y);

    const focus = try protocol.validateFocusInput(focus_input_fixture);
    try std.testing.expect(focus.focused);

    const invalid_key = try std.testing.allocator.dupe(u8, key_input_fixture);
    defer std.testing.allocator.free(invalid_key);
    const key_offset = std.mem.indexOf(u8, invalid_key, "KeyA") orelse return error.TestUnexpectedResult;
    invalid_key[key_offset + 3] = '0';
    try std.testing.expectError(error.InvalidField, protocol.validateKeyInput(invalid_key));
}

test "native decoder rejects malformed magic and unknown required extension" {
    const allocator = std.testing.allocator;
    const invalid_magic = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(invalid_magic);
    invalid_magic[0] = 0;
    try std.testing.expectError(
        error.InvalidMagic,
        protocol.validateTerminalFrame(allocator, invalid_magic),
    );

    const required_extension = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(required_extension);
    std.mem.writeInt(
        u16,
        required_extension[required_extension.len - 10 ..][0..2],
        0x8000,
        .little,
    );
    try std.testing.expectError(
        error.UnknownRequiredFlag,
        protocol.validateTerminalFrame(allocator, required_extension),
    );
}

test "native envelope decoder matches required-message and payload bounds" {
    const allocator = std.testing.allocator;
    const unknown_required = try allocator.dupe(u8, envelope_fixture);
    defer allocator.free(unknown_required);
    std.mem.writeInt(u16, unknown_required[20..22], 60_000, .little);
    try std.testing.expectError(
        error.UnknownRequiredMessage,
        protocol.validateTransportEnvelope(unknown_required),
    );

    const unknown_optional = try allocator.dupe(u8, unknown_required);
    defer allocator.free(unknown_optional);
    const flags = std.mem.readInt(u16, unknown_optional[6..8], .little);
    std.mem.writeInt(u16, unknown_optional[6..8], flags | 1, .little);
    const optional_summary = try protocol.validateTransportEnvelope(unknown_optional);
    try std.testing.expect(optional_summary.optional_message);

    const oversized_payload_bytes = 64 * 1024 + 1;
    const oversized_control = try allocator.alloc(
        u8,
        protocol.transport_envelope_header_bytes + oversized_payload_bytes,
    );
    defer allocator.free(oversized_control);
    @memcpy(
        oversized_control[0..protocol.transport_envelope_header_bytes],
        envelope_fixture[0..protocol.transport_envelope_header_bytes],
    );
    @memset(oversized_control[protocol.transport_envelope_header_bytes..], 0);
    std.mem.writeInt(u32, oversized_control[8..12], @intCast(oversized_control.len), .little);
    std.mem.writeInt(u32, oversized_control[12..16], oversized_payload_bytes, .little);
    oversized_control[22] = 1;
    try std.testing.expectError(
        error.LimitExceeded,
        protocol.validateTransportEnvelope(oversized_control),
    );
}

test "native WriteInput decoder rejects unscoped and oversized payloads" {
    try std.testing.expectError(error.Truncated, protocol.validateWriteInput(&([_]u8{0x22} ** 16)));

    const oversized = try std.testing.allocator.alloc(u8, 16 + 1024 * 1024 + 1);
    defer std.testing.allocator.free(oversized);
    @memset(oversized, 0x22);
    try std.testing.expectError(error.LimitExceeded, protocol.validateWriteInput(oversized));
}
