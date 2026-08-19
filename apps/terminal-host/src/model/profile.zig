const std = @import("std");

const protocol = @import("terminal-protocol");
const terminal_theme = @import("theme.zig");

pub const ColorScheme = terminal_theme.ColorScheme;

pub const CursorShape = enum {
    block,
    underline,
    bar,
};

pub const Profile = struct {
    color_scheme: ColorScheme,
    default_background_rgb: u32,
    default_cursor_blinking: bool = false,
    default_cursor_rgb: ?u32,
    default_cursor_shape: CursorShape = .block,
    default_foreground_rgb: u32,
    device_primary_conformance_level: u16,
    device_primary_features: []const u16,
    device_secondary_type: u16,
    device_secondary_firmware_version: u16,
    device_secondary_rom_cartridge: u16,
    device_tertiary_unit_id: u32,
    enquiry_response: []const u8,
    palette: []const u32,
    version_response: []const u8,
};

pub fn valid(profile: Profile) bool {
    if (profile.default_background_rgb > 0x00ffffff or
        profile.default_foreground_rgb > 0x00ffffff or
        (profile.default_cursor_rgb != null and profile.default_cursor_rgb.? > 0x00ffffff) or
        profile.palette.len != protocol.Limits.terminal_profile_palette_entries or
        profile.device_primary_features.len > protocol.Limits.terminal_profile_features or
        !validResponse(profile.enquiry_response) or
        !validResponse(profile.version_response))
    {
        return false;
    }
    for (profile.palette) |color| if (color > 0x00ffffff) return false;
    for (profile.device_primary_features, 0..) |feature, index| {
        for (profile.device_primary_features[0..index]) |previous| {
            if (previous == feature) return false;
        }
    }
    return true;
}

pub fn theme(profile: Profile) terminal_theme.Theme {
    std.debug.assert(valid(profile));
    var result: terminal_theme.Theme = .{
        .color_scheme = profile.color_scheme,
        .default_background_rgb = profile.default_background_rgb,
        .default_cursor_rgb = profile.default_cursor_rgb,
        .default_foreground_rgb = profile.default_foreground_rgb,
        .palette = undefined,
    };
    @memcpy(&result.palette, profile.palette);
    return result;
}

fn validResponse(value: []const u8) bool {
    return value.len <= protocol.Limits.terminal_response_bytes and
        std.mem.indexOfScalar(u8, value, 0) == null;
}
