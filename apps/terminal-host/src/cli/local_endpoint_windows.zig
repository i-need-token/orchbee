const std = @import("std");

const launcher_arguments = @import("launcher_arguments.zig");

pub fn prepareRuntime(options: launcher_arguments.RuntimeDirectoryOptions) !void {
    _ = options;
    return error.UnsupportedPlatform;
}

pub fn connect(
    allocator: std.mem.Allocator,
    options: launcher_arguments.EndpointOptions,
) !void {
    _ = allocator;
    _ = options;
    return error.UnsupportedPlatform;
}
