const std = @import("std");

const model = @import("terminal-host-model");
const pty_process = @import("terminal-host-platform").process;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_launch = @import("launch.zig");
const terminal_profile = @import("profile.zig");
const runtime_options = @import("runtime_options.zig");

// A null runtime profile resolves to the Ghostty default at creation time.
// Launch inspection requires a non-optional profile, so this valid stand-in is
// used only for validation and is never applied to the runtime.
const validation_default_profile: terminal_profile.Profile = .{
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
    .palette = &[_]u32{0} ** 256,
    .version_response = "Orchbee",
};

pub const Checked = struct {
    process_launch: pty_process.Launch,
    runtime_options: runtime_options.Options,
};

/// Validates raw runtime starts before any Ghostty allocation, worker spawn, or
/// process effect. A launch-supplied zero pixel dimension inherits the runtime
/// option; an explicit launch value must agree with that option.
pub fn prepare(
    allocator: std.mem.Allocator,
    launch: pty_process.Launch,
    options: runtime_options.Options,
) (error{ InvalidLaunch, InvalidTerminalProfile } || std.mem.Allocator.Error)!Checked {
    if (options.profile) |profile| if (!terminal_profile.valid(profile)) {
        return error.InvalidTerminalProfile;
    };
    const pixel_width = resolveCellPixelDimension(
        launch.cell_pixel_width,
        options.cell_pixel_width,
    ) orelse return error.InvalidLaunch;
    const pixel_height = resolveCellPixelDimension(
        launch.cell_pixel_height,
        options.cell_pixel_height,
    ) orelse return error.InvalidLaunch;
    const geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = pixel_height,
        .cell_pixel_width = pixel_width,
        .columns = launch.columns,
        .rows = launch.rows,
    };
    const configuration: terminal_configuration.Configuration = .{
        .scrollback_bytes = options.scrollback_bytes,
    };

    const environment = try allocator.alloc(
        terminal_launch.EnvironmentEntry,
        launch.environment.len,
    );
    defer allocator.free(environment);
    for (environment, launch.environment) |*target, source| {
        target.* = .{ .name = source.name, .value = source.value };
    }

    _ = try terminal_launch.inspect(allocator, .{
        .arguments = launch.arguments,
        .configuration = configuration,
        .current_directory = launch.current_directory,
        .environment = environment,
        .executable = launch.executable,
        .geometry = geometry,
        .profile = options.profile orelse validation_default_profile,
    });

    return .{
        .process_launch = .{
            .arguments = launch.arguments,
            .cell_pixel_height = pixel_height,
            .cell_pixel_width = pixel_width,
            .columns = geometry.columns,
            .current_directory = launch.current_directory,
            .environment = launch.environment,
            .executable = launch.executable,
            .rows = geometry.rows,
            .stdin_fd = launch.stdin_fd,
        },
        .runtime_options = .{
            .cell_pixel_height = pixel_height,
            .cell_pixel_width = pixel_width,
            .profile = options.profile,
            .scrollback_bytes = options.scrollback_bytes,
        },
    };
}

fn resolveCellPixelDimension(launch_pixel: u16, options_pixel: u16) ?u16 {
    if (launch_pixel == 0) return options_pixel;
    if (launch_pixel != options_pixel) return null;
    return launch_pixel;
}
