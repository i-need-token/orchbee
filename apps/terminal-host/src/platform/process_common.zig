const std = @import("std");

const process_types = @import("process_types.zig");

pub fn pixelExtent(cells: u16, cell_pixels: u16) !u16 {
    return std.math.mul(u16, cells, cell_pixels);
}

pub fn environmentForLaunch(
    allocator: std.mem.Allocator,
    delta: []const process_types.EnvironmentDelta,
) !std.process.EnvMap {
    var environment = try std.process.getEnvMap(allocator);
    errdefer environment.deinit();
    for (delta) |entry| {
        if (entry.value) |value| {
            try environment.put(entry.name, value);
        } else {
            environment.remove(entry.name);
        }
    }
    return environment;
}
