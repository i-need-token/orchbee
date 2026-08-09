const std = @import("std");
const terminal_host = @import("terminal-host");

const host_service = terminal_host.host_service;
const launcher_arguments = terminal_host.launcher_arguments;
const local_ipc = terminal_host.local_ipc;

pub fn main() !void {
    const allocator = std.heap.smp_allocator;
    const arguments = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, arguments);
    if (arguments.len == 0) return error.InvalidArguments;

    switch (try launcher_arguments.parse(arguments[1..])) {
        .version => try writeVersion(),
        .serve => |options| try serve(allocator, options),
    }
}

fn writeVersion() !void {
    var output_buffer: [128]u8 = undefined;
    var stdout = std.fs.File.stdout().writerStreaming(&output_buffer);
    try stdout.interface.writeAll("orchbee-terminal-host " ++ terminal_host.host_version ++ "\n");
    try stdout.interface.flush();
}

fn serve(allocator: std.mem.Allocator, options: launcher_arguments.ServeOptions) !void {
    var server = try local_ipc.LocalServer.init(
        allocator,
        options.runtime_directory,
        options.endpoint_id,
    );
    defer server.deinit();
    var service = host_service.HostService.initWithOptions(allocator, .{
        .terminal_defaults = options.terminal_defaults,
        .trace_interactions = std.process.hasEnvVarConstant("ORCHBEE_TERMINAL_HOST_TRACE"),
    });
    const serve_result = server.serveUntilIdle(&service);
    service.deinitChecked() catch |shutdown_error| {
        if (serve_result) |_| {} else |serve_error| {
            std.log.err(
                "terminal Host serving failed before shutdown: {s}",
                .{@errorName(serve_error)},
            );
        }
        return shutdown_error;
    };
    try serve_result;
}
