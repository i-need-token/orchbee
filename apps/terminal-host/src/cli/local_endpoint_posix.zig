const std = @import("std");

const launcher_arguments = @import("launcher_arguments.zig");
const local_ipc = @import("terminal-host-transport");

pub fn prepareRuntime(options: launcher_arguments.RuntimeDirectoryOptions) !void {
    try local_ipc.preparePosixRuntimeDirectory(options.runtime_directory);
}

pub fn connect(
    allocator: std.mem.Allocator,
    options: launcher_arguments.EndpointOptions,
) !void {
    try local_ipc.runPosixClientProxy(
        allocator,
        options.runtime_directory,
        options.endpoint_id,
        .{ .handle = 3 },
        std.fs.File.stdin(),
        std.fs.File.stdout(),
    );
}
