//! What the program was last told of focus, keys, and buttons, per attachment of the exporting
//! Host. A held key is the terminal protocol's KeyInput payload, and the mouse sample its
//! MouseInput payload, each without the attachment id and capability revision that open them, so
//! the protocol's own validators judge them.
const std = @import("std");
const protocol = @import("terminal-protocol");
const terminal_input = @import("terminal-input");
const wire = @import("wire.zig");

const Error = wire.Error;
const Allocation = std.mem.Allocator.Error;

pub const Held = struct {
    focused: bool,
    /// Presses, in the order the attachment pressed them.
    keys: []const terminal_input.KeyInput,
    /// Buttons, in the order the attachment pressed them.
    buttons: []const terminal_input.MouseButton,
    /// Present exactly when a button is held.
    sample: ?terminal_input.MouseInput,
};

const max_attachments = 4096;
const max_keys = 1024;
const max_buttons = @typeInfo(terminal_input.MouseButton).@"enum".fields.len;
/// The longest W3C code the protocol admits is 20 bytes; 64 leaves room without trusting it.
const max_key_body_bytes = 16 + 64 + protocol.Limits.key_text_bytes;
const mouse_body_bytes = 16;
/// The attachment id and capability revision the protocol payloads begin with.
const prefix_bytes = 24;
const prefix: [prefix_bytes]u8 = [_]u8{1} ** 16 ++ [_]u8{ 1, 0, 0, 0, 0, 0, 0, 0 };

pub fn encode(out: wire.Writer, attachments: []const Held) (Error || Allocation)!void {
    try out.int(u16, std.math.cast(u16, attachments.len) orelse return error.InvalidRecord);
    for (attachments) |held| {
        try out.flag(held.focused);
        try out.int(u16, std.math.cast(u16, held.keys.len) orelse return error.InvalidRecord);
        for (held.keys) |key| {
            const key_length = std.math.cast(u16, key.key.len) orelse return error.InvalidRecord;
            const text_length = std.math.cast(u32, key.text.len) orelse return error.InvalidRecord;
            try out.int(u16, std.math.cast(u16, 16 + key.key.len + key.text.len) orelse return error.InvalidRecord);
            try out.int(u8, 1);
            try out.int(u8, @as(u8, @intFromBool(key.composing)) | @as(u8, @intFromBool(key.option_as_alt)) << 1);
            try out.int(u16, key.modifiers.bits());
            try out.int(u16, key.consumed_modifiers.bits());
            try out.int(u16, key_length);
            try out.int(u32, key.unshifted_codepoint);
            try out.int(u32, text_length);
            try out.bytes(key.key);
            try out.bytes(key.text);
        }
        try out.int(u8, std.math.cast(u8, held.buttons.len) orelse return error.InvalidRecord);
        for (held.buttons) |button| try out.int(u8, @intFromEnum(button));
        try out.flag(held.sample != null);
        if (held.sample) |sample| {
            const action: u8, const button: u8 = switch (sample.action) {
                .press => |value| .{ 0, @intFromEnum(value) },
                .release => |value| .{ 1, @intFromEnum(value) },
                .motion => |motion| .{ 2, switch (motion) {
                    .hover => 0,
                    .drag => |value| @intFromEnum(value),
                } },
            };
            try out.int(u8, action);
            try out.int(u8, button);
            try out.int(u16, sample.modifiers.bits());
            try out.int(u32, @bitCast(sample.x));
            try out.int(u32, @bitCast(sample.y));
            try out.int(u16, sample.pressed_buttons.bits());
            try out.int(u16, 0);
        }
    }
}

/// Every slice of the result lives in `arena`.
pub fn decode(arena: std.mem.Allocator, in: *wire.Reader) (Error || Allocation)![]Held {
    const count = try in.count(u16, max_attachments, 5);
    const result = try arena.alloc(Held, count);
    for (result) |*held| {
        const focused = try in.flag();
        const keys = try arena.alloc(terminal_input.KeyInput, try in.count(u16, max_keys, 2 + 16));
        for (keys) |*key| key.* = try decodeKey(arena, try in.counted(u16, max_key_body_bytes));
        const buttons = try arena.alloc(terminal_input.MouseButton, try in.count(u8, max_buttons, 1));
        for (buttons, 0..) |*button, index| {
            button.* = std.enums.fromInt(terminal_input.MouseButton, try in.int(u8)) orelse
                return error.InvalidRecord;
            for (buttons[0..index]) |earlier| if (earlier == button.*) return error.InvalidRecord;
        }
        const sample: ?terminal_input.MouseInput = if (try in.flag())
            try decodeMouse(try in.take(mouse_body_bytes))
        else
            null;
        if ((sample != null) != (buttons.len != 0)) return error.InvalidRecord;
        held.* = .{ .focused = focused, .keys = keys, .buttons = buttons, .sample = sample };
    }
    return result;
}

fn decodeKey(arena: std.mem.Allocator, body: []const u8) (Error || Allocation)!terminal_input.KeyInput {
    var buffer: [prefix_bytes + max_key_body_bytes]u8 = undefined;
    @memcpy(buffer[0..prefix_bytes], &prefix);
    @memcpy(buffer[prefix_bytes..][0..body.len], body);
    const decoded = protocol.validateKeyInput(buffer[0 .. prefix_bytes + body.len]) catch
        return error.InvalidRecord;
    const key = decoded.input.key;
    if (key.action != .press) return error.InvalidRecord;
    var copy = key;
    copy.key = try arena.dupe(u8, key.key);
    copy.text = try arena.dupe(u8, key.text);
    return copy;
}

fn decodeMouse(body: []const u8) Error!terminal_input.MouseInput {
    var buffer: [prefix_bytes + mouse_body_bytes]u8 = undefined;
    @memcpy(buffer[0..prefix_bytes], &prefix);
    @memcpy(buffer[prefix_bytes..], body);
    const decoded = protocol.validateMouseInput(&buffer) catch return error.InvalidRecord;
    return decoded.input.mouse;
}
