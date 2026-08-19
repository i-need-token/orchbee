const std = @import("std");

const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_theme = model.theme;

pub const ApplyTerminalTheme = struct {
    expected_theme_revision: u64,
    operation_id: identifiers.TerminalThemeOperationId,
    terminal: terminal_theme.Theme,
    terminal_id: identifiers.TerminalId,
};

pub fn decodeApplyTerminalTheme(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.ApplyTerminalTheme, ApplyTerminalTheme) {
    const parsed = try control_wire.decode(control_wire.ApplyTerminalTheme, allocator, payload);
    errdefer parsed.deinit();
    const terminal = themeFromWire(parsed.value.terminal) orelse return error.InvalidField;
    return .{
        .parsed = parsed,
        .value = .{
            .expected_theme_revision = try decode_common.parseCanonicalU64(
                parsed.value.expectedThemeRevision,
            ),
            .operation_id = try identifiers.parseLowerHex(
                identifiers.TerminalThemeOperationId,
                parsed.value.operationId,
            ),
            .terminal = terminal,
            .terminal_id = try identifiers.parseLowerHex(
                identifiers.TerminalId,
                parsed.value.terminalId,
            ),
        },
    };
}

fn themeFromWire(wire: control_wire.TerminalTheme) ?terminal_theme.Theme {
    if (wire.palette.len != @typeInfo(@FieldType(terminal_theme.Theme, "palette")).array.len) {
        return null;
    }
    const color_scheme: terminal_theme.ColorScheme = if (std.mem.eql(u8, wire.colorScheme, "dark"))
        .dark
    else if (std.mem.eql(u8, wire.colorScheme, "light"))
        .light
    else
        return null;
    var theme: terminal_theme.Theme = .{
        .color_scheme = color_scheme,
        .default_background_rgb = wire.defaultBackgroundRgb,
        .default_cursor_rgb = wire.defaultCursorRgb,
        .default_foreground_rgb = wire.defaultForegroundRgb,
        .palette = undefined,
    };
    @memcpy(&theme.palette, wire.palette);
    return if (terminal_theme.valid(theme)) theme else null;
}
