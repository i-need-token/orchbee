const std = @import("std");
const target_builtin = @import("builtin");
const engine_boundary = @import("terminal-host-engine");
const model_boundary = @import("terminal-host-model");

const subject = engine_boundary.launch;
const terminal_configuration = model_boundary.configuration;

const EnvironmentEntry = subject.EnvironmentEntry;
const Launch = subject.Launch;

const features = [_]u16{22};
const palette = [_]u32{0} ** 256;

fn validLaunch(environment: []const EnvironmentEntry) Launch {
    return .{
        .arguments = &.{},
        .configuration = null,
        .current_directory = null,
        .environment = environment,
        .executable = "shell",
        .geometry = .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 80,
            .rows = 24,
        },
        .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &features,
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &palette,
            .version_response = "Orchbee test",
        },
    };
}

test "validated launch prepares short-lived runtime input only after inspection" {
    const environment = [_]EnvironmentEntry{
        .{ .name = "ORCHBEE_TEST", .value = "enabled" },
    };
    var launch = validLaunch(&environment);
    launch.arguments = &.{ "--login", "fixture" };
    launch.current_directory = "/tmp";
    const validated = try subject.inspect(std.testing.allocator, launch);
    var prepared = try subject.prepareRuntime(validated, std.testing.allocator, .{ .scrollback_bytes = 12_345 });
    defer prepared.deinit(std.testing.allocator);

    try std.testing.expectEqual(@as(usize, 12_345), prepared.configuration.scrollback_bytes);
    try std.testing.expectEqualStrings("shell", prepared.process_launch.executable);
    try std.testing.expectEqualStrings("/tmp", prepared.process_launch.current_directory.?);
    try std.testing.expectEqual(@as(usize, 2), prepared.process_launch.arguments.len);
    try std.testing.expectEqual(@as(usize, 1), prepared.process_launch.environment.len);
    try std.testing.expectEqualStrings("ORCHBEE_TEST", prepared.process_launch.environment[0].name);
    try std.testing.expectEqualStrings("enabled", prepared.process_launch.environment[0].value.?);
    try std.testing.expectEqual(@as(u16, 8), prepared.runtime_options.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 16), prepared.runtime_options.cell_pixel_height);
    const runtime_profile = prepared.runtime_options.profile.?;
    try std.testing.expectEqual(.dark, runtime_profile.color_scheme);
    try std.testing.expectEqual(@as(u32, 0xffffff), runtime_profile.default_foreground_rgb);
    try std.testing.expectEqual(@as(usize, 256), runtime_profile.palette.len);

    var configured_launch = launch;
    configured_launch.configuration = .{ .scrollback_bytes = 54_321 };
    const configured = try subject.inspect(std.testing.allocator, configured_launch);
    var configured_runtime = try subject.prepareRuntime(configured, std.testing.allocator, .{ .scrollback_bytes = 12_345 });
    defer configured_runtime.deinit(std.testing.allocator);
    try std.testing.expectEqual(@as(usize, 54_321), configured_runtime.configuration.scrollback_bytes);
    try std.testing.expectEqual(@as(usize, 54_321), configured_runtime.runtime_options.scrollback_bytes);
}
