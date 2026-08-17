const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const terminal_input = @import("terminal-input");

// In the pinned Kitty encoder, an ASCII byte needs at most three decimal
// digits plus its delimiter; a multi-byte scalar needs at most seven digits
// plus its delimiter, which is still below four bytes of output per source
// byte. The remaining key, alternate, modifier, event, and terminator fields
// require fewer than 64 bytes.
pub const max_key_bytes = protocol.Limits.key_text_bytes * 4 + 64;
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
    var options = ghostty_vt.input.KeyEncodeOptions.fromTerminal(terminal);
    options.macos_option_as_alt = if (input.option_as_alt) .true else .false;
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
    var event = mouseEncodeEvent(input);
    const original_position = event.pos;
    event.pos = gridSafeMousePosition(event.pos, options);
    const extended_button = remapExtendedMouseButton(&event);
    var writer: std.Io.Writer = .fixed(buffer);
    try ghostty_vt.input.encodeMouse(&writer, event, options);
    const encoded = writer.buffered();
    if (extended_button) try restoreExtendedMouseButton(encoded, event.action, options);
    if (options.format == .sgr_pixels and encoded.len != 0) {
        return restoreSgrPixelPosition(buffer, encoded, original_position, event.action);
    }
    return encoded;
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

fn gridSafeMousePosition(
    position: ghostty_vt.input.MouseEncodeEvent.Pos,
    options: ghostty_vt.input.MouseEncodeOptions,
) ghostty_vt.input.MouseEncodeEvent.Pos {
    return .{
        .x = normalizeGridAxis(position.x, options.size.screen.width),
        .y = normalizeGridAxis(position.y, options.size.screen.height),
    };
}

fn restoreSgrPixelPosition(
    buffer: *[max_mouse_bytes]u8,
    encoded: []const u8,
    original_position: ghostty_vt.input.MouseEncodeEvent.Pos,
    action: ghostty_vt.input.MouseAction,
) ![]const u8 {
    if (encoded.len <= 3 or !std.mem.eql(u8, encoded[0..3], "\x1b[<")) {
        return error.MouseEncodingFailed;
    }
    const button_end = std.mem.indexOfScalarPos(u8, encoded, 3, ';') orelse
        return error.MouseEncodingFailed;
    const button_code = std.fmt.parseInt(u8, encoded[3..button_end], 10) catch
        return error.MouseEncodingFailed;
    var writer: std.Io.Writer = .fixed(buffer);
    try writer.print("\x1b[<{d};{d};{d}{c}", .{
        button_code,
        boundedPixelCoordinate(original_position.x),
        boundedPixelCoordinate(original_position.y),
        @as(u8, if (action == .release) 'm' else 'M'),
    });
    return writer.buffered();
}

fn boundedPixelCoordinate(value: f32) i32 {
    const minimum: f32 = @floatFromInt(std.math.minInt(i32));
    const rounded_maximum: f32 = @floatFromInt(std.math.maxInt(i32));
    const maximum = std.math.nextAfter(f32, rounded_maximum, 0);
    const bounded = @min(@max(value, minimum), maximum);
    return @intFromFloat(@round(bounded));
}

fn normalizeGridAxis(value: f32, screen_pixels: u32) f32 {
    const viewport_maximum: f32 = @floatFromInt(screen_pixels);
    if (value < 0) return -1;
    if (value > viewport_maximum) {
        // Preserve Ghostty's strict outside-viewport decision while keeping
        // its unconditional grid conversion inside the u16 cell domain. SGR
        // pixel coordinates are restored from the original position afterward.
        return std.math.nextAfter(f32, viewport_maximum, std.math.inf(f32));
    }
    return value;
}

fn remapExtendedMouseButton(event: *ghostty_vt.input.MouseEncodeEvent) bool {
    const button = event.button orelse return false;
    event.button = switch (button) {
        .ten => .eight,
        .eleven => .nine,
        else => return false,
    };
    return true;
}

fn restoreExtendedMouseButton(
    encoded: []u8,
    action: ghostty_vt.input.MouseAction,
    options: ghostty_vt.input.MouseEncodeOptions,
) !void {
    if (encoded.len == 0) return;
    if (action == .release and options.format != .sgr and options.format != .sgr_pixels) {
        return;
    }
    switch (options.format) {
        .x10, .utf8 => {
            if (encoded.len < 4 or !std.mem.eql(u8, encoded[0..3], "\x1b[M")) {
                return error.MouseEncodingFailed;
            }
            encoded[3] = std.math.add(u8, encoded[3], 2) catch
                return error.MouseEncodingFailed;
        },
        .sgr, .sgr_pixels => try incrementDecimalField(encoded, 3),
        .urxvt => try incrementDecimalField(encoded, 2),
    }
}

fn incrementDecimalField(encoded: []u8, start: usize) !void {
    if (encoded.len <= start) return error.MouseEncodingFailed;
    const end = std.mem.indexOfScalarPos(u8, encoded, start, ';') orelse
        return error.MouseEncodingFailed;
    const field = encoded[start..end];
    const value = std.fmt.parseInt(u8, field, 10) catch
        return error.MouseEncodingFailed;
    const incremented = std.math.add(u8, value, 2) catch
        return error.MouseEncodingFailed;
    const written = std.fmt.bufPrint(field, "{d}", .{incremented}) catch
        return error.MouseEncodingFailed;
    if (written.len != field.len) return error.MouseEncodingFailed;
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
