const std = @import("std");
const builtin = @import("builtin");
const terminal_host = @import("terminal-host");
const platform = @import("terminal-host-platform");

const host_service = terminal_host.host_service;
const launcher_arguments = terminal_host.launcher_arguments;
const local_ipc = terminal_host.local_ipc;

pub fn main(init: std.process.Init) !void {
    // The pinned process.Init has not opened application descriptors or started
    // I/O workers. Discard launcher resources before acquiring any Host state.
    if (builtin.os.tag == .linux) try platform.posix_compat.closeInheritedLinuxDescriptors(init.io);
    const allocator = init.gpa;
    const arguments = try init.minimal.args.toSlice(init.arena.allocator());
    if (arguments.len == 0) return error.InvalidArguments;

    switch (try launcher_arguments.parse(allocator, arguments[1..])) {
        .version => try writeVersion(init.io),
        .serve => |options| serve(allocator, init.io, init.environ_map, options) catch |err| {
            // `serve` has returned, so the endpoint and its lifetime lock are already released.
            const any_error: anyerror = err;
            switch (any_error) {
                error.PeerIdentityMismatch => failed(.peer_identity, err),
                error.WindowsIpcFailure => failed(.local_ipc, err),
                else => return err,
            }
        },
    }
}

fn writeVersion(io: std.Io) !void {
    var output_buffer: [128]u8 = undefined;
    var stdout = std.Io.File.stdout().writerStreaming(io, &output_buffer);
    try stdout.interface.writeAll("orchbee-terminal-host " ++ terminal_host.host_version ++ "\n");
    try stdout.interface.flush();
}

/// The exit code of a failed Host, which the Worker that started it records: a Host that could not
/// begin serving names the step, and one whose serving failed names the failure's class. Every
/// other failure keeps the default error exit code 1.
const FailureExit = enum(u8) {
    control_signal = 10,
    runtime_directory = 11,
    already_running = 12,
    endpoint = 13,
    peer_identity = 14,
    local_ipc = 15,
};

fn failed(exit: FailureExit, err: anyerror) noreturn {
    std.log.err("terminal Host failed: {s}", .{@errorName(err)});
    std.process.exit(@intFromEnum(exit));
}

fn serve(
    allocator: std.mem.Allocator,
    io: std.Io,
    environment: *const std.process.Environ.Map,
    options: launcher_arguments.ServeOptions,
) !void {
    if (builtin.os.tag == .windows) platform.process.prepareWindowsTerminalChildren() catch |err|
        failed(.control_signal, err);
    var server = (if (builtin.os.tag == .windows)
        local_ipc.LocalServer.initWithIo(
            io,
            allocator,
            options.runtime_directory,
            options.endpoint_id,
        )
    else
        local_ipc.LocalServer.initInEndpointDirectory(
            io,
            allocator,
            options.runtime_directory,
            options.endpoint_directory orelse options.runtime_directory,
            options.endpoint_id,
        )) catch |err| {
        const any_error: anyerror = err;
        failed(switch (any_error) {
            error.InsecureRuntimeDirectory => .runtime_directory,
            error.AlreadyRunning => .already_running,
            else => .endpoint,
        }, err);
    };
    defer server.deinit();
    var service = host_service.HostService.initWithOptions(allocator, .{
        .terminal_defaults = options.terminal_defaults,
        .editor_document_directory = options.editor_document_directory,
        .trace_interactions = environment.contains("ORCHBEE_TERMINAL_HOST_TRACE"),
    });
    const serve_result = server.serveUntilIdle(&service);
    const shutdown_failed = service.deinitChecked() catch |shutdown_error| teardown_failed: {
        if (serve_result) |_| {} else |serve_error| {
            std.log.err(
                "terminal Host serving failed before shutdown: {s}",
                .{@errorName(serve_error)},
            );
        }
        // THDD-001: a failed fatal-teardown pass may leave an unconfirmed
        // process tree. The Host retains the cleanup ownership and retries on
        // the bounded cleanup-retry backoff schedule instead of abandoning it
        // after one failed pass. The endpoint and lifetime lock remain held for
        // the duration of the window (server.deinit runs after this function
        // returns), so no second Host can start for the same endpoint while an
        // unconfirmed tree is still owned.
        if (service.retainedCleanupPending()) {
            switch (service.runRetainedCleanupWindow()) {
                .complete => {
                    try service.finishRetainedCleanup();
                    break :teardown_failed null;
                },
                .retained_failed => {
                    // THDD-001 permanent-failure policy: the bounded retry
                    // budget was exhausted while a process tree stayed
                    // unconfirmed. The tree is left in an explicitly retained
                    // failed-cleanup state (never silently released); surface it
                    // loudly before exiting with the shutdown failure.
                    std.log.err(
                        "terminal Host retained an unconfirmed process tree as an explicit failed-cleanup state after exhausting its bounded retry budget; it was never silently released",
                        .{},
                    );
                    return shutdown_error;
                },
            }
        }
        return shutdown_error;
    };
    if (shutdown_failed) |err| return err;
    try serve_result;
}
