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

    switch (try launcher_arguments.parse(allocator, arguments[1..])) {
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
