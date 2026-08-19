const std = @import("std");

const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_profile = model.profile;
const terminal_theme = model.theme;

test "terminal theme copies every profile color into one fixed valid value" {
    var palette: [256]u32 = undefined;
    for (&palette, 0..) |*color, index| color.* = @intCast(index * 0x01_01_01);
    const theme = terminal_profile.theme(.{
        .color_scheme = .light,
        .default_background_rgb = 0xf0_f0_f0,
        .default_cursor_rgb = 0x12_34_56,
        .default_foreground_rgb = 0x10_10_10,
        .device_primary_conformance_level = 62,
        .device_primary_features = &.{22},
        .device_secondary_type = 1,
        .device_secondary_firmware_version = 0,
        .device_secondary_rom_cartridge = 0,
        .device_tertiary_unit_id = 0,
        .enquiry_response = "",
        .palette = &palette,
        .version_response = "Orchbee",
    });

    try std.testing.expect(terminal_theme.valid(theme));
    try std.testing.expectEqual(terminal_theme.ColorScheme.light, theme.color_scheme);
    try std.testing.expectEqual(@as(?u32, 0x12_34_56), theme.default_cursor_rgb);
    try std.testing.expectEqual(@as(u32, 0xff_ff_ff), theme.palette[255]);
    palette[255] = 0;
    try std.testing.expectEqual(@as(u32, 0xff_ff_ff), theme.palette[255]);
}

test "theme state reserves revision zero for the launch baseline" {
    const operation_id = identifiers.TerminalThemeOperationId.fromBytes([_]u8{0x44} ** 16);
    try std.testing.expect(terminal_theme.validState(.{
        .operation_id = null,
        .revision = 0,
        .terminal = terminal_theme.default,
    }));
    try std.testing.expect(terminal_theme.validState(.{
        .operation_id = operation_id,
        .revision = 1,
        .terminal = terminal_theme.default,
    }));
    try std.testing.expect(!terminal_theme.validState(.{
        .operation_id = operation_id,
        .revision = 0,
        .terminal = terminal_theme.default,
    }));
    try std.testing.expect(!terminal_theme.validState(.{
        .operation_id = null,
        .revision = 1,
        .terminal = terminal_theme.default,
    }));
}
