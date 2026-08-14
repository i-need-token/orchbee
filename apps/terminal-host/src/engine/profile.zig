const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const protocol = @import("terminal-protocol");
const terminal_profile = model.profile;

pub const ColorScheme = terminal_profile.ColorScheme;
pub const Profile = terminal_profile.Profile;
pub const valid = terminal_profile.valid;

comptime {
    // Model validation establishes the protocol count. This adapter assertion
    // proves that every validated palette also has Ghostty's physical shape.
    if (protocol.Limits.terminal_profile_palette_entries !=
        @typeInfo(ghostty_vt.color.Palette).array.len)
    {
        @compileError("terminal profile palette limit must match Ghostty's palette shape");
    }
}

const DeviceAttributesCallback = @typeInfo(@FieldType(
    ghostty_vt.TerminalStream.Handler.Effects,
    "device_attributes",
)).optional.child;
pub const DeviceAttributes = @typeInfo(@typeInfo(DeviceAttributesCallback).pointer.child).@"fn".return_type.?;
const DeviceFeature = std.meta.Elem(@FieldType(@FieldType(DeviceAttributes, "primary"), "features"));

pub const Owned = struct {
    color_scheme: ColorScheme,
    device_primary_conformance_level: u16,
    device_primary_features: []DeviceFeature,
    device_secondary_type: u16,
    device_secondary_firmware_version: u16,
    device_secondary_rom_cartridge: u16,
    device_tertiary_unit_id: u32,
    enquiry_response: []u8,
    version_response: []u8,

    pub fn init(allocator: std.mem.Allocator, source: ?Profile) !Owned {
        if (source) |profile| if (!valid(profile)) return error.InvalidTerminalProfile;
        const profile = source orelse default_profile;
        const features = try allocator.alloc(DeviceFeature, profile.device_primary_features.len);
        errdefer allocator.free(features);
        for (profile.device_primary_features, 0..) |feature, index| {
            features[index] = @enumFromInt(feature);
        }
        const enquiry = try allocator.dupe(u8, profile.enquiry_response);
        errdefer allocator.free(enquiry);
        const version = try allocator.dupe(u8, profile.version_response);
        return .{
            .color_scheme = profile.color_scheme,
            .device_primary_conformance_level = profile.device_primary_conformance_level,
            .device_primary_features = features,
            .device_secondary_type = profile.device_secondary_type,
            .device_secondary_firmware_version = profile.device_secondary_firmware_version,
            .device_secondary_rom_cartridge = profile.device_secondary_rom_cartridge,
            .device_tertiary_unit_id = profile.device_tertiary_unit_id,
            .enquiry_response = enquiry,
            .version_response = version,
        };
    }

    pub fn deinit(self: *Owned, allocator: std.mem.Allocator) void {
        allocator.free(self.device_primary_features);
        allocator.free(self.enquiry_response);
        allocator.free(self.version_response);
        self.* = undefined;
    }

    pub fn reportedColorScheme(self: *const Owned) ghostty_vt.device_status.ColorScheme {
        return switch (self.color_scheme) {
            .dark => .dark,
            .light => .light,
        };
    }

    pub fn deviceAttributes(self: *const Owned) DeviceAttributes {
        return .{
            .primary = .{
                .conformance_level = @enumFromInt(self.device_primary_conformance_level),
                .features = self.device_primary_features,
            },
            .secondary = .{
                .device_type = @enumFromInt(self.device_secondary_type),
                .firmware_version = self.device_secondary_firmware_version,
                .rom_cartridge = self.device_secondary_rom_cartridge,
            },
            .tertiary = .{ .unit_id = self.device_tertiary_unit_id },
        };
    }
};

pub fn terminalColors(source: ?Profile) error{InvalidTerminalProfile}!ghostty_vt.Terminal.Colors {
    const profile = source orelse return .default;
    if (!valid(profile)) return error.InvalidTerminalProfile;
    var colors: ghostty_vt.Terminal.Colors = .default;
    colors.background = .init(rgb(profile.default_background_rgb));
    colors.foreground = .init(rgb(profile.default_foreground_rgb));
    colors.cursor = if (profile.default_cursor_rgb) |value| .init(rgb(value)) else .unset;
    var palette: ghostty_vt.color.Palette = undefined;
    for (&palette, profile.palette) |*target, value| target.* = rgb(value);
    colors.palette = .init(palette);
    return colors;
}

const default_profile: Profile = .{
    .color_scheme = .dark,
    .default_background_rgb = 0,
    .default_cursor_rgb = null,
    .default_foreground_rgb = 0xffffff,
    .device_primary_conformance_level = 62,
    .device_primary_features = &.{22},
    .device_secondary_type = 1,
    .device_secondary_firmware_version = 0,
    .device_secondary_rom_cartridge = 0,
    .device_tertiary_unit_id = 0,
    .enquiry_response = "",
    .palette = &.{},
    .version_response = "Orchbee",
};

fn rgb(value: u32) ghostty_vt.color.RGB {
    return .{
        .r = @truncate(value >> 16),
        .g = @truncate(value >> 8),
        .b = @truncate(value),
    };
}
