const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const terminal_input = @import("terminal-input");

pub const max_key_bytes = 8 * 1024;
pub const max_mouse_bytes = 128;
pub const max_focus_bytes = ghostty_vt.input.max_focus_encode_size;

pub fn parseKey(value: []const u8) error{InvalidKey}!ghostty_vt.input.Key {
    return ghostty_vt.input.Key.fromW3C(value) orelse error.InvalidKey;
}

pub fn encodeKey(
    buffer: *[max_key_bytes]u8,
    terminal: *ghostty_vt.Terminal,
    key: ghostty_vt.input.Key,
    input: terminal_input.KeyInput,
) ![]const u8 {
    const options = ghostty_vt.input.KeyEncodeOptions.fromTerminal(terminal);
    var writer: std.Io.Writer = .fixed(buffer);
    try ghostty_vt.input.encodeKey(&writer, .{
        .action = keyAction(input.action),
        .composing = input.composing,
        .consumed_mods = modifiers(input.consumed_modifiers),
        .key = key,
        .mods = modifiers(input.modifiers),
        .unshifted_codepoint = input.unshifted_codepoint,
        .utf8 = input.text,
    }, options);
    return writer.buffered();
}

pub fn encodeMouse(
    buffer: *[max_mouse_bytes]u8,
    terminal: *ghostty_vt.Terminal,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    input: terminal_input.MouseInput,
) ![]const u8 {
    var options = ghostty_vt.input.MouseEncodeOptions.fromTerminal(terminal, .{
        .screen = .{
            .height = @as(u32, terminal.rows) * cell_pixel_height,
            .width = @as(u32, terminal.cols) * cell_pixel_width,
        },
        .cell = .{
            .height = cell_pixel_height,
            .width = cell_pixel_width,
        },
        .padding = .{ .top = 0, .bottom = 0, .left = 0, .right = 0 },
    });
    options.any_button_pressed = mouseHasPressedButton(input);
    var writer: std.Io.Writer = .fixed(buffer);
    try ghostty_vt.input.encodeMouse(&writer, mouseEncodeEvent(input), options);
    return writer.buffered();
}

pub fn reportsFocus(terminal: *const ghostty_vt.Terminal) bool {
    return terminal.modes.get(.focus_event);
}

pub fn encodeFocus(buffer: *[max_focus_bytes]u8, focused: bool) ![]const u8 {
    var writer: std.Io.Writer = .fixed(buffer);
    try ghostty_vt.input.encodeFocus(&writer, if (focused) .gained else .lost);
    return writer.buffered();
}

pub fn encodePaste(
    allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    text: []const u8,
    maximum_bytes: usize,
) ![]u8 {
    const options = ghostty_vt.input.PasteOptions.fromTerminal(terminal);
    const encoded_limit = if (options.bracketed)
        std.math.add(usize, text.len, 12) catch return error.InputLimitExceeded
    else
        text.len;
    if (encoded_limit > maximum_bytes) return error.InputLimitExceeded;

    const mutable = try allocator.dupe(u8, text);
    defer allocator.free(mutable);
    const parts = ghostty_vt.input.encodePaste(mutable, options);
    const total = parts[0].len + parts[1].len + parts[2].len;
    const encoded = try allocator.alloc(u8, total);
    var offset: usize = 0;
    for (parts) |part| {
        @memcpy(encoded[offset..][0..part.len], part);
        offset += part.len;
    }
    if (encoded.len > maximum_bytes) {
        allocator.free(encoded);
        return error.InputLimitExceeded;
    }
    return encoded;
}

fn keyAction(action: terminal_input.KeyAction) ghostty_vt.input.KeyAction {
    return switch (action) {
        .release => .release,
        .press => .press,
        .repeat => .repeat,
    };
}

fn modifiers(value: terminal_input.Modifiers) ghostty_vt.input.KeyMods {
    return .{
        .shift = value.shift,
        .ctrl = value.control,
        .alt = value.alt,
        .super = value.meta,
        .caps_lock = value.caps_lock,
        .num_lock = value.num_lock,
        .sides = .{
            .shift = if (value.shift_right) .right else .left,
            .ctrl = if (value.control_right) .right else .left,
            .alt = if (value.alt_right) .right else .left,
            .super = if (value.meta_right) .right else .left,
        },
    };
}

fn mouseButton(button: terminal_input.MouseButton) ghostty_vt.input.MouseButton {
    return switch (button) {
        .left => .left,
        .right => .right,
        .middle => .middle,
        .four => .four,
        .five => .five,
        .six => .six,
        .seven => .seven,
        .eight => .eight,
        .nine => .nine,
        .ten => .ten,
        .eleven => .eleven,
    };
}

fn mouseEncodeEvent(input: terminal_input.MouseInput) ghostty_vt.input.MouseEncodeEvent {
    const action: ghostty_vt.input.MouseAction = switch (input.action) {
        .press => .press,
        .release => .release,
        .motion => .motion,
    };
    const active_button: ?terminal_input.MouseButton = switch (input.action) {
        .press => |button| button,
        .release => |button| button,
        .motion => |motion| switch (motion) {
            .hover => null,
            .drag => |button| button,
        },
    };
    return .{
        .action = action,
        .button = if (active_button) |button| mouseButton(button) else null,
        .mods = modifiers(input.modifiers),
        .pos = .{ .x = input.x, .y = input.y },
    };
}

fn mouseHasPressedButton(input: terminal_input.MouseInput) bool {
    if (input.pressed_buttons.any()) return true;
    return switch (input.action) {
        .press => true,
        .release => false,
        .motion => |motion| switch (motion) {
            .hover => false,
            .drag => true,
        },
    };
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const modifiersFn = modifiers;
    pub const mouseEncodeEventFn = mouseEncodeEvent;
    pub const mouseHasPressedButtonFn = mouseHasPressedButton;
} else struct {};
