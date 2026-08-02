const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_protocol = @import("terminal-protocol");

const host_core = @import("host_core.zig");
const host_service = @import("host_service.zig");
const local_ipc = @import("local_ipc.zig");
const pty = @import("pty.zig");
const pty_process = @import("pty_process.zig");
const terminal_runtime = @import("terminal_runtime.zig");
const terminal_projection = @import("terminal_projection.zig");

const ServeConfig = struct {
    endpoint_id: []const u8,
    runtime_directory: []const u8,
};

pub fn main() !void {
    const allocator = std.heap.smp_allocator;
    const arguments = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, arguments);
    const config = try parseArguments(arguments);

    var server = try local_ipc.LocalServer.init(
        allocator,
        config.runtime_directory,
        config.endpoint_id,
    );
    defer server.deinit();
    var service = host_service.HostService.init(allocator);
    service.trace_interactions = std.process.hasEnvVarConstant("ORCHBEE_TERMINAL_HOST_TRACE");
    try server.serveUntilIdle(&service);
}

fn parseArguments(arguments: []const []const u8) !ServeConfig {
    if (arguments.len != 6 or
        !std.mem.eql(u8, arguments[1], "serve") or
        !std.mem.eql(u8, arguments[2], "--runtime-directory") or
        !std.mem.eql(u8, arguments[4], "--endpoint-id"))
    {
        return error.InvalidArguments;
    }
    if (!std.fs.path.isAbsolute(arguments[3])) return error.RuntimeDirectoryNotAbsolute;
    if (!local_ipc.validEndpointId(arguments[5])) return error.InvalidEndpointId;
    return .{
        .endpoint_id = arguments[5],
        .runtime_directory = arguments[3],
    };
}

test "the pinned Ghostty VT dependency parses terminal output" {
    const allocator = std.testing.allocator;
    var terminal: ghostty_vt.Terminal = try .init(allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(allocator);

    try terminal.printString("orchbee");
    const plain = try terminal.plainString(allocator);
    defer allocator.free(plain);

    try std.testing.expectEqualStrings("orchbee", plain);
}

test "launcher accepts only the private serve ABI" {
    const runtime_directory = if (@import("builtin").os.tag == .windows)
        "C:\\orchbee-runtime"
    else
        "/tmp/orchbee-runtime";
    const parsed = try parseArguments(&.{
        "terminal-host",
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
    });
    try std.testing.expectEqualStrings(runtime_directory, parsed.runtime_directory);
    try std.testing.expectEqualStrings(
        "0123456789abcdef0123456789abcdef",
        parsed.endpoint_id,
    );
    try std.testing.expectError(error.InvalidArguments, parseArguments(&.{"terminal-host"}));
    try std.testing.expectError(error.InvalidEndpointId, parseArguments(&.{
        "terminal-host",
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "ABCDEF0123456789abcdef0123456789",
    }));
}

comptime {
    _ = ghostty_vt.Terminal;
    _ = pty.Pty;
    _ = pty_process.Process;
    _ = terminal_runtime.TerminalRuntime;
    _ = terminal_projection.Projection;
    _ = terminal_protocol.terminal_frame_header_bytes;
    _ = host_core.HostCore;
    _ = host_service.HostService;
    _ = local_ipc.LocalServer;
}
