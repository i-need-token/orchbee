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

test "launch inspection produces an unambiguous order-independent digest" {
    const environment_a = [_]EnvironmentEntry{
        .{ .name = "B", .value = null },
        .{ .name = "A", .value = "" },
    };
    const environment_b = [_]EnvironmentEntry{
        .{ .name = "A", .value = "" },
        .{ .name = "B", .value = null },
    };
    const environment_aliases = [_]EnvironmentEntry{
        .{ .name = "PATH", .value = "first" },
        .{ .name = "Path", .value = "second" },
    };
    const environment_path_upper = [_]EnvironmentEntry{
        .{ .name = "PATH", .value = "same" },
    };
    const environment_path_mixed = [_]EnvironmentEntry{
        .{ .name = "Path", .value = "same" },
    };
    const empty_argument = [_][]const u8{""};
    const base = validLaunch(&environment_a);
    var reordered = base;
    reordered.environment = &environment_b;
    var with_argument = base;
    with_argument.arguments = &empty_argument;
    var with_configuration = base;
    with_configuration.configuration = .{ .scrollback_bytes = 1_000_000 };

    const base_inspection = try subject.inspect(std.testing.allocator, base);
    const reordered_inspection = try subject.inspect(std.testing.allocator, reordered);
    const argument_inspection = try subject.inspect(std.testing.allocator, with_argument);
    const configuration_inspection = try subject.inspect(std.testing.allocator, with_configuration);
    try std.testing.expectEqualSlices(u8, &base_inspection.launch_digest, &reordered_inspection.launch_digest);
    try std.testing.expect(!std.mem.eql(u8, &base_inspection.launch_digest, &argument_inspection.launch_digest));
    try std.testing.expect(!std.mem.eql(u8, &base_inspection.launch_digest, &configuration_inspection.launch_digest));

    var with_environment_aliases = base;
    with_environment_aliases.environment = &environment_aliases;
    if (target_builtin.os.tag == .windows) {
        try std.testing.expectError(
            error.InvalidLaunch,
            subject.inspect(std.testing.allocator, with_environment_aliases),
        );
    } else {
        _ = try subject.inspect(std.testing.allocator, with_environment_aliases);
    }

    var with_path_upper = base;
    with_path_upper.environment = &environment_path_upper;
    var with_path_mixed = base;
    with_path_mixed.environment = &environment_path_mixed;
    const path_upper = try subject.inspect(std.testing.allocator, with_path_upper);
    const path_mixed = try subject.inspect(std.testing.allocator, with_path_mixed);
    try std.testing.expectEqual(
        target_builtin.os.tag == .windows,
        std.mem.eql(u8, &path_upper.launch_digest, &path_mixed.launch_digest),
    );
}

test "validated launch prepares short-lived runtime input only after inspection" {
    const environment = [_]EnvironmentEntry{
        .{ .name = "ORCHBEE_TEST", .value = "enabled" },
    };
    var launch = validLaunch(&environment);
    launch.arguments = &.{ "--login", "fixture" };
    launch.current_directory = "/tmp";
    const validated = try subject.inspect(std.testing.allocator, launch);
    var prepared = try validated.prepareRuntime(std.testing.allocator, .{ .scrollback_bytes = 12_345 });
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
    var configured_runtime = try configured.prepareRuntime(std.testing.allocator, .{ .scrollback_bytes = 12_345 });
    defer configured_runtime.deinit(std.testing.allocator);
    try std.testing.expectEqual(@as(usize, 54_321), configured_runtime.configuration.scrollback_bytes);
    try std.testing.expectEqual(@as(usize, 54_321), configured_runtime.runtime_options.scrollback_bytes);
}

test "launch inspection rejects invalid geometry profile configuration and environment" {
    const environment = [_]EnvironmentEntry{};
    const base = validLaunch(&environment);

    var invalid_geometry = base;
    invalid_geometry.geometry.cell_pixel_width = 0;
    try std.testing.expectError(
        error.InvalidLaunch,
        subject.inspect(std.testing.allocator, invalid_geometry),
    );

    var invalid_palette = base;
    invalid_palette.profile.palette = &.{};
    try std.testing.expectError(
        error.InvalidLaunch,
        subject.inspect(std.testing.allocator, invalid_palette),
    );

    const duplicate_features = [_]u16{ 22, 22 };
    var invalid_features = base;
    invalid_features.profile.device_primary_features = &duplicate_features;
    try std.testing.expectError(
        error.InvalidLaunch,
        subject.inspect(std.testing.allocator, invalid_features),
    );

    var invalid_configuration = base;
    invalid_configuration.configuration = .{
        .scrollback_bytes = terminal_configuration.testing.scrollback_limit + 1,
    };
    try std.testing.expectError(
        error.InvalidLaunch,
        subject.inspect(std.testing.allocator, invalid_configuration),
    );

    const invalid_environment = [_]EnvironmentEntry{
        .{ .name = "INVALID=NAME", .value = null },
    };
    var invalid_environment_launch = base;
    invalid_environment_launch.environment = &invalid_environment;
    try std.testing.expectError(
        error.InvalidLaunch,
        subject.inspect(std.testing.allocator, invalid_environment_launch),
    );
}
