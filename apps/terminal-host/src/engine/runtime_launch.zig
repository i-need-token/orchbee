const std = @import("std");

const model = @import("terminal-host-model");
const pty_process = @import("terminal-host-platform").process;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_launch = model.launch;
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
/// process effect. The launch and option cell-pixel dimensions resolve as one
/// atomic pair: a launch-supplied {0, 0} inherits the complete option pair
/// verbatim — which may itself be {0, 0} unknown — while an explicit launch
/// pair must agree with a measured option pair. A public unknown launch can
/// therefore never gain a fabricated measurement from defaults, and a mixed
/// pair is rejected rather than silently filled.
pub fn prepare(
    allocator: std.mem.Allocator,
    launch: pty_process.Launch,
    options: runtime_options.Options,
) (error{ InvalidLaunch, InvalidTerminalProfile } || std.mem.Allocator.Error)!Checked {
    if (options.profile) |profile| if (!terminal_profile.valid(profile)) {
        return error.InvalidTerminalProfile;
    };
    const pixel_pair = resolveCellPixelPair(
        launch.cell_pixel_width,
        launch.cell_pixel_height,
        options.cell_pixel_width,
        options.cell_pixel_height,
    ) orelse return error.InvalidLaunch;
    const geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = pixel_pair.height,
        .cell_pixel_width = pixel_pair.width,
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
            .cell_pixel_height = pixel_pair.height,
            .cell_pixel_width = pixel_pair.width,
            .columns = geometry.columns,
            .current_directory = launch.current_directory,
            .environment = launch.environment,
            .executable = launch.executable,
            .rows = geometry.rows,
            .stdin_fd = launch.stdin_fd,
        },
        .runtime_options = .{
            .cell_pixel_height = pixel_pair.height,
            .cell_pixel_width = pixel_pair.width,
            .profile = options.profile,
            .scrollback_bytes = options.scrollback_bytes,
            .output_seed = options.output_seed,
        },
    };
}

const CellPixelPair = struct {
    width: u16,
    height: u16,
};

fn resolveCellPixelPair(
    launch_width: u16,
    launch_height: u16,
    options_width: u16,
    options_height: u16,
) ?CellPixelPair {
    if (launch_width == 0 and launch_height == 0) {
        // Unknown launch inherits the complete option pair verbatim. That pair
        // may itself be unknown, or a measured private-start value; it is never
        // synthesized here. A mixed option pair is still never admitted.
        if ((options_width == 0) != (options_height == 0)) return null;
        return .{ .width = options_width, .height = options_height };
    }
    if (launch_width == 0 or launch_height == 0) return null;
    if (options_width == 0 and options_height == 0) {
        return .{ .width = launch_width, .height = launch_height };
    }
    if (launch_width != options_width or launch_height != options_height) {
        return null;
    }
    return .{ .width = launch_width, .height = launch_height };
}
