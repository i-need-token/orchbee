const std = @import("std");

const model = @import("terminal-host-model");
const pty_process = @import("terminal-host-platform").process;
const runtime_options = @import("runtime_options.zig");
const terminal_configuration = model.configuration;
const terminal_launch = model.launch;

pub const Digest = terminal_launch.Digest;
pub const EnvironmentEntry = terminal_launch.EnvironmentEntry;
pub const Launch = terminal_launch.Launch;
pub const Validated = terminal_launch.Validated;
pub const inspect = terminal_launch.inspect;

// This short-lived value owns only the EnvironmentDelta array. Its entries,
// process launch, and runtime profile borrow bytes from Validated.launch.
pub const RuntimePreparation = struct {
    configuration: terminal_configuration.Configuration,
    environment: []pty_process.EnvironmentDelta,
    process_launch: pty_process.Launch,
    runtime_options: runtime_options.Options,

    pub fn deinit(self: *RuntimePreparation, allocator: std.mem.Allocator) void {
        allocator.free(self.environment);
        self.* = undefined;
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
    const environment = try allocator.alloc(pty_process.EnvironmentDelta, launch.environment.len);
    for (launch.environment, 0..) |entry, index| {
        environment[index] = .{ .name = entry.name, .value = entry.value };
    }

    const geometry = launch.geometry;
    const effective_configuration = launch.configuration orelse default_configuration;

    return .{
        .configuration = effective_configuration,
        .environment = environment,
        .process_launch = .{
            .arguments = launch.arguments,
            .cell_pixel_height = geometry.cell_pixel_height,
            .cell_pixel_width = geometry.cell_pixel_width,
            .columns = geometry.columns,
            .current_directory = launch.current_directory,
            .environment = environment,
            .executable = launch.executable,
            .rows = geometry.rows,
        },
        .runtime_options = .{
            .cell_pixel_height = geometry.cell_pixel_height,
            .cell_pixel_width = geometry.cell_pixel_width,
            .profile = launch.profile,
            .scrollback_bytes = effective_configuration.scrollback_bytes,
        },
    };
}
