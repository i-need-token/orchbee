const std = @import("std");
const target_builtin = @import("builtin");
const model_boundary = @import("terminal-host-model");

const subject = model_boundary.launch;
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
    var with_cursor_blinking = base;
    with_cursor_blinking.profile.default_cursor_blinking = true;
    var with_cursor_shape = base;
    with_cursor_shape.profile.default_cursor_shape = .bar;

    const base_inspection = try subject.inspect(std.testing.allocator, base);
    const reordered_inspection = try subject.inspect(std.testing.allocator, reordered);
    const argument_inspection = try subject.inspect(std.testing.allocator, with_argument);
    const configuration_inspection = try subject.inspect(std.testing.allocator, with_configuration);
    const cursor_blinking_inspection = try subject.inspect(std.testing.allocator, with_cursor_blinking);
    const cursor_shape_inspection = try subject.inspect(std.testing.allocator, with_cursor_shape);
    try std.testing.expectEqualSlices(u8, &base_inspection.launch_digest, &reordered_inspection.launch_digest);
    try std.testing.expect(!std.mem.eql(u8, &base_inspection.launch_digest, &argument_inspection.launch_digest));
    try std.testing.expect(!std.mem.eql(u8, &base_inspection.launch_digest, &configuration_inspection.launch_digest));
    try std.testing.expect(!std.mem.eql(u8, &base_inspection.launch_digest, &cursor_blinking_inspection.launch_digest));
    try std.testing.expect(!std.mem.eql(u8, &base_inspection.launch_digest, &cursor_shape_inspection.launch_digest));

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
test "launch environment identity follows the target operating system" {
    // Environment names are target-OS identifiers. The same wire corpus admits
    // `Path` and `PATH` as distinct variables on a POSIX target and rejects
    // them as duplicates on a Windows target with the typed InvalidLaunch
    // failure before any Host dispatch. This mirrors prepareLaunchEnvironment
    // in packages/terminal-protocol/src/control.ts.
    const environment_aliases = [_]EnvironmentEntry{
        .{ .name = "Path", .value = "first" },
        .{ .name = "PATH", .value = "second" },
    };
    const launch = validLaunch(&environment_aliases);
    if (target_builtin.os.tag == .windows) {
        try std.testing.expectError(
            error.InvalidLaunch,
            subject.inspect(std.testing.allocator, launch),
        );
    } else {
        _ = try subject.inspect(std.testing.allocator, launch);
    }

    // The CreationId digest uses the same normalization: equivalent folded
    // inputs produce one fingerprint on Windows and distinct fingerprints on
    // POSIX, so idempotent replays stay stable and platform-consistent.
    const path_upper = [_]EnvironmentEntry{
        .{ .name = "PATH", .value = "same" },
    };
    const path_mixed = [_]EnvironmentEntry{
        .{ .name = "Path", .value = "same" },
    };
    const upper = validLaunch(&path_upper);
    const mixed = validLaunch(&path_mixed);
    const upper_digest = try subject.inspect(std.testing.allocator, upper);
    const mixed_digest = try subject.inspect(std.testing.allocator, mixed);
    try std.testing.expectEqual(
        target_builtin.os.tag == .windows,
        std.mem.eql(u8, &upper_digest.launch_digest, &mixed_digest.launch_digest),
    );
}
test "launch inspection rejects invalid geometry profile configuration and environment" {
    const environment = [_]EnvironmentEntry{};
    const base = validLaunch(&environment);

    var invalid_geometry = base;
    invalid_geometry.geometry.?.cell_pixel_width = 0;
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
