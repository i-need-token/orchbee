const std = @import("std");
const builtin = @import("builtin");

const process_types = @import("process_types.zig");

pub fn pixelExtent(cells: u16, cell_pixels: u16) !u16 {
    return std.math.mul(u16, cells, cell_pixels);
}

pub fn environmentForLaunch(
    allocator: std.mem.Allocator,
    delta: []const process_types.EnvironmentDelta,
) !std.process.Environ.Map {
    var inherited = try currentEnvironment().createMap(allocator);
    if (builtin.os.tag == .windows) {
        defer inherited.deinit();
        return @import("windows_environment.zig").current(allocator, &inherited, delta);
    }
    var environment = inherited;
    errdefer environment.deinit();
    for (delta) |entry| {
        if (entry.value) |value| {
            try environment.put(entry.name, value);
        } else {
            _ = environment.orderedRemove(entry.name);
        }
    }
    return environment;
}

pub fn environmentVariableOwned(
    allocator: std.mem.Allocator,
    name: []const u8,
) ![]u8 {
    return currentEnvironment().getAlloc(allocator, name);
}

fn currentEnvironment() std.process.Environ {
    return switch (builtin.os.tag) {
        .windows => .{ .block = .global },
        .freebsd, .linux, .macos => .{ .block = .{
            .slice = std.c.environ[0..environmentLength() :null],
        } },
        else => @compileError("terminal-host has no environment adapter for this target"),
    };
}

fn environmentLength() usize {
    var length: usize = 0;
    while (std.c.environ[length]) |_| : (length += 1) {}
    return length;
}
