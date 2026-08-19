const std = @import("std");

const protocol = @import("terminal-protocol");
const identifiers = @import("identifiers.zig");

pub const ColorScheme = enum {
    dark,
    light,
};

pub const Theme = struct {
    color_scheme: ColorScheme,
    default_background_rgb: u32,
    default_cursor_rgb: ?u32,
    default_foreground_rgb: u32,
    palette: [protocol.Limits.terminal_profile_palette_entries]u32,
};

pub const State = struct {
    operation_id: ?identifiers.TerminalThemeOperationId = null,
    revision: u64 = 0,
    terminal: Theme,
};

pub const default: Theme = .{
    .color_scheme = .dark,
    .default_background_rgb = 0,
    .default_cursor_rgb = null,
    .default_foreground_rgb = 0xff_ff_ff,
    .palette = [_]u32{0} ** protocol.Limits.terminal_profile_palette_entries,
};

pub fn valid(theme: Theme) bool {
    if (theme.default_background_rgb > 0x00ff_ffff or
        theme.default_foreground_rgb > 0x00ff_ffff or
        (theme.default_cursor_rgb != null and theme.default_cursor_rgb.? > 0x00ff_ffff))
    {
        return false;
    }
    for (theme.palette) |color| if (color > 0x00ff_ffff) return false;
    return true;
}

pub fn validState(state: State) bool {
    return valid(state.terminal) and
        ((state.revision == 0) == (state.operation_id == null));
}

pub fn eql(left: Theme, right: Theme) bool {
    return std.meta.eql(left, right);
}
