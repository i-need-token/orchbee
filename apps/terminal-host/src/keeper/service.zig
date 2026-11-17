//! The Keeper's listener thread: it accepts each connection onto its own pipe instance and thread,
//! closes at once any connection beyond the Keeper's bounds, and ends the process once it is empty
//! for 30 seconds or drained.

const std = @import("std");
const platform = @import("terminal-host-platform");
const local_ipc = @import("terminal-host-transport");
const connection = @import("connection.zig");
const state = @import("state.zig");

const monotonic = platform.monotonic;
const diagnostic_log = platform.diagnostic_log;
const fatal_record = platform.fatal_record;

/// Every instance of the Keeper's pipe declares no system limit, since a declared limit stops at
/// 254 and the Keeper serves 265 connections; the listener bounds them itself.
const pipe_instance_limit = 255;
/// How long one accept waits before the listener checks the Keeper's life again.
const accept_slice_ms = 100;
const accept_failure_delay_ns = 100 * std.time.ns_per_ms;
/// The Keeper ends after it has held nothing, with no Host registered and no Worker connected, for
/// this long, counted from its start.
pub const empty_exit_ns = 30 * std.time.ns_per_s;
/// Each connection thread reads one frame, of at most 1 MiB, into its own heap arena.
const connection_stack_bytes = 256 * 1024;

pub const Options = struct {
    runtime_directory: []const u8,
    endpoint_id: []const u8,
};

pub const StartError = error{ InsecureRuntimeDirectory, AlreadyRunning, Endpoint };

/// Serves the Keeper's endpoint until the Keeper ends; returns only when it could not start.
pub fn run(allocator: std.mem.Allocator, options: Options) (StartError || error{ OutOfMemory, SystemResources })!noreturn {
    var server = local_ipc.LocalServer.initWithInstanceLimit(
        allocator,
        options.runtime_directory,
        options.endpoint_id,
        pipe_instance_limit,
    ) catch |err| {
        const any_error: anyerror = err;
        return switch (any_error) {
            error.InsecureRuntimeDirectory => error.InsecureRuntimeDirectory,
            error.AlreadyRunning => error.AlreadyRunning,
            else => error.Endpoint,
        };
    };
    // The endpoint's lifetime lock is held from here on, so the runtime directory is this Keeper's.
    fatal_record.armAs(.keeper, options.runtime_directory);
    diagnostic_log.armKeeper(options.runtime_directory);
    fatal_record.markThread(.@"keeper-listener");
    const table = try allocator.create(state.Table);
    try table.init();
    state.activate(table);
    var listener: Listener = .{
        .allocator = allocator,
        .server = &server,
        .table = table,
        .clock = monotonic.Timer.start() catch return error.SystemResources,
    };
    listener.run();
}

const Listener = struct {
    allocator: std.mem.Allocator,
    server: *local_ipc.LocalServer,
    table: *state.Table,
    clock: monotonic.Timer,
    /// When the Keeper last became empty; it starts empty.
    empty_since_ns: ?u64 = 0,

    fn run(self: *Listener) noreturn {
        while (true) {
            if (self.server.acceptBefore(accept_slice_ms)) |accepted| {
                if (accepted) self.dispatch();
            } else |err| self.acceptFailed(err);
            diagnostic_log.flushKeeper();
            self.checkLife();
        }
    }

    /// Moves the accepted client to its own instance and thread, or closes it at once when it is
    /// beyond the bounds.
    fn dispatch(self: *Listener) void {
        var transport = self.server.takeConnection() catch {
            self.server.disconnect();
            diagnostic_log.noteConnectionBeyondBounds();
            return;
        };
        const pid = transport.clientProcessId() orelse 0;
        self.table.lock();
        const slot = self.table.reservePending(pid);
        self.table.unlock();
        const reserved = slot orelse {
            transport.deinit();
            diagnostic_log.noteConnectionBeyondBounds();
            return;
        };
        const thread = std.Thread.spawn(.{ .stack_size = connection_stack_bytes }, connection.run, .{connection.Context{
            .allocator = self.allocator,
            .table = self.table,
            .transport = transport,
            .slot = reserved,
        }}) catch {
            self.table.lock();
            self.table.releaseConnection(reserved);
            self.table.unlock();
            transport.deinit();
            diagnostic_log.noteConnectionBeyondBounds();
            return;
        };
        thread.detach();
    }

    fn acceptFailed(self: *Listener, err: anyerror) void {
        const system_code = self.server.lastSystemCode();
        diagnostic_log.acceptFailure(.{
            .error_name = @errorName(err),
            .system_code = system_code,
            .renew_system_code = self.server.renewAfterAcceptFailure(),
            .retry_after_ms = accept_failure_delay_ns / std.time.ns_per_ms,
        });
        monotonic.sleep(accept_failure_delay_ns);
    }

    /// Ends the Keeper once it is drained and holds nothing, or after it has been empty for 30
    /// seconds; a control reply still in flight is let finish first.
    fn checkLife(self: *Listener) void {
        const now = self.clock.read();
        self.table.lock();
        const lifecycle = self.table.lifecycle;
        const empty = self.table.idle();
        const drained = lifecycle == .draining and self.table.holdingCount() == 0;
        const replying = self.table.pending_connections > 0;
        self.table.unlock();
        if (lifecycle == .stopping) return;
        if (!empty) {
            self.empty_since_ns = null;
        } else if (self.empty_since_ns == null) {
            self.empty_since_ns = now;
        }
        const expired = if (self.empty_since_ns) |since| now -| since >= empty_exit_ns else false;
        if ((drained or expired) and !replying) {
            self.table.lock();
            self.table.stop();
            self.table.unlock();
            std.process.exit(0);
        }
    }
};
