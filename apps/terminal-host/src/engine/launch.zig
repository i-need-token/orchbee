const std = @import("std");

const model = @import("terminal-host-model");
const pty_process = @import("terminal-host-platform").process;
const runtime_options = @import("runtime_options.zig");
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_launch = model.launch;
const terminal_profile = model.profile;

pub const Digest = terminal_launch.Digest;
pub const EnvironmentEntry = terminal_launch.EnvironmentEntry;
pub const Launch = terminal_launch.Launch;
pub const Validated = terminal_launch.Validated;
pub const inspect = terminal_launch.inspect;

// This value owns every launch byte required after CreateTerminal decoding.
// A deferred visual launch can therefore survive until mounted geometry is
// supplied without retaining a transport arena or inventing fallback cells.
pub const RuntimePreparation = struct {
    arena: std.heap.ArenaAllocator,
    arguments: []const []const u8,
    configuration: terminal_configuration.Configuration,
    current_directory: ?[]const u8,
    environment: []pty_process.EnvironmentDelta,
    executable: []const u8,
    profile: terminal_profile.Profile,

    pub fn deinit(self: *RuntimePreparation, _: std.mem.Allocator) void {
        self.arena.deinit();
        self.* = undefined;
    }

    pub fn processLaunch(
        self: *const RuntimePreparation,
        geometry: terminal_geometry.Geometry,
    ) pty_process.Launch {
        std.debug.assert(terminal_geometry.isValid(geometry));
        return .{
            .arguments = self.arguments,
            .cell_pixel_height = geometry.cell_pixel_height,
            .cell_pixel_width = geometry.cell_pixel_width,
            .columns = geometry.columns,
            .current_directory = self.current_directory,
            .environment = self.environment,
            .executable = self.executable,
            .rows = geometry.rows,
        };
    }

    pub fn runtimeOptions(
        self: *const RuntimePreparation,
        geometry: terminal_geometry.Geometry,
    ) runtime_options.Options {
        std.debug.assert(terminal_geometry.isValid(geometry));
        return .{
            .cell_pixel_height = geometry.cell_pixel_height,
            .cell_pixel_width = geometry.cell_pixel_width,
            .profile = self.profile,
            .scrollback_bytes = self.configuration.scrollback_bytes,
        };
    }
};

/// Adapts one already inspected domain launch to the platform and engine
/// runtime values. Validation and digest identity remain owned by model.launch.
pub fn prepareRuntime(
    validated: Validated,
    allocator: std.mem.Allocator,
    default_configuration: terminal_configuration.Configuration,
) std.mem.Allocator.Error!RuntimePreparation {
    std.debug.assert(terminal_configuration.valid(default_configuration));

    const launch = validated.launch;
    var arena = std.heap.ArenaAllocator.init(allocator);
    errdefer arena.deinit();
    const owned = arena.allocator();
    const arguments = try owned.alloc([]const u8, launch.arguments.len);
    for (arguments, launch.arguments) |*target, argument| {
        target.* = try owned.dupe(u8, argument);
    }
    const environment = try owned.alloc(pty_process.EnvironmentDelta, launch.environment.len);
    for (launch.environment, 0..) |entry, index| {
        environment[index] = .{
            .name = try owned.dupe(u8, entry.name),
            .value = if (entry.value) |value| try owned.dupe(u8, value) else null,
        };
    }
    const effective_configuration = launch.configuration orelse default_configuration;
    const current_directory = if (launch.current_directory) |directory|
        try owned.dupe(u8, directory)
    else
        null;
    const executable = try owned.dupe(u8, launch.executable);
    const profile = try cloneProfile(owned, launch.profile);

    // Do not copy the arena into the returned owner until every allocation is
    // complete. Its allocator points at the local arena state; allocating after
    // this copy could prepend a node that the returned state would not own.
    return .{
        .arena = arena,
        .arguments = arguments,
        .configuration = effective_configuration,
        .current_directory = current_directory,
        .environment = environment,
        .executable = executable,
        .profile = profile,
    };
}

fn cloneProfile(
    allocator: std.mem.Allocator,
    profile: terminal_profile.Profile,
) std.mem.Allocator.Error!terminal_profile.Profile {
    return .{
        .color_scheme = profile.color_scheme,
        .default_background_rgb = profile.default_background_rgb,
        .default_cursor_blinking = profile.default_cursor_blinking,
        .default_cursor_rgb = profile.default_cursor_rgb,
        .default_cursor_shape = profile.default_cursor_shape,
        .default_foreground_rgb = profile.default_foreground_rgb,
        .device_primary_conformance_level = profile.device_primary_conformance_level,
        .device_primary_features = try allocator.dupe(u16, profile.device_primary_features),
        .device_secondary_type = profile.device_secondary_type,
        .device_secondary_firmware_version = profile.device_secondary_firmware_version,
        .device_secondary_rom_cartridge = profile.device_secondary_rom_cartridge,
        .device_tertiary_unit_id = profile.device_tertiary_unit_id,
        .enquiry_response = try allocator.dupe(u8, profile.enquiry_response),
        .palette = try allocator.dupe(u32, profile.palette),
        .version_response = try allocator.dupe(u8, profile.version_response),
    };
}
