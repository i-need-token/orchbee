//! The dispatch thread's accept loop over the Host's lifetime. Before the Worker's first session
//! the Host holds no terminal, and an accept failure that is not one connection's own still ends
//! it. Once a session has run, the loop continues only while terminals are retained, and then no
//! accept failure ends the Host: it records the failure, renews the endpoint where the platform
//! can, and retries after a backoff while it keeps maintaining every terminal.
const std = @import("std");

const diagnostic_log = @import("terminal-host-platform").diagnostic_log;
const monotonic = @import("terminal-host-platform").monotonic;
const connection = @import("connection.zig");

pub const first_retry_ms: u32 = 100;
pub const last_retry_ms: u32 = 5000;

/// The wait before the next accept after consecutive failures: 100 ms, doubling up to 5 s.
pub const AcceptBackoff = struct {
    next_ms: u32 = first_retry_ms,

    pub fn take(self: *AcceptBackoff) u32 {
        const delay_ms = self.next_ms;
        self.next_ms = @min(last_retry_ms, self.next_ms *| 2);
        return delay_ms;
    }

    pub fn reset(self: *AcceptBackoff) void {
        self.next_ms = first_retry_ms;
    }
};

/// `recoverable_failure` names a failure that belongs to one connection, such as a client that
/// left before its accept; it retries at once without a record.
pub fn serveUntilIdleLoop(
    server: anytype,
    service: anytype,
    comptime recoverable_failure: fn (anyerror) bool,
) !void {
    var completed_accept = false;
    var backoff: AcceptBackoff = .{};
    while (true) {
        // `drain` or `exit` can end the Host before its Worker arrives as well as after.
        if (service.controlExitDue()) return;
        if (!completed_accept) {
            // A control request answered before the Worker arrives keeps the Host waiting.
            const served: connection.Served = server.serveOne(service) catch |err| served: {
                if (!recoverable_failure(err)) return err;
                break :served .worker;
            };
            completed_accept = served != .control_request;
            continue;
        }
        service.maintain();
        if (service.canStopWhenIdle()) return;
        // Terminals are retained from here on, so no accept failure may end the Host.
        _ = server.serveOneBefore(service, connection.poll_interval_ms) catch |err| {
            if (recoverable_failure(err)) continue;
            const delay_ms = backoff.take();
            diagnostic_log.acceptFailure(@errorName(err), systemCode(server), delay_ms);
            if (@hasDecl(@TypeOf(server.*), "renewAfterAcceptFailure")) server.renewAfterAcceptFailure();
            waitMaintaining(service, delay_ms);
            continue;
        };
        backoff.reset();
    }
}

fn systemCode(server: anytype) ?u32 {
    if (!@hasDecl(@TypeOf(server.*), "lastSystemCode")) return null;
    return server.lastSystemCode();
}

/// Waits `delay_ms` in maintenance-sized steps, ending early once the Host may stop.
fn waitMaintaining(service: anytype, delay_ms: u32) void {
    var waited_ms: u32 = 0;
    while (waited_ms < delay_ms) : (waited_ms += connection.poll_interval_ms) {
        if (service.controlExitDue()) return;
        service.maintain();
        if (service.canStopWhenIdle()) return;
        monotonic.sleep(@as(u64, connection.poll_interval_ms) * std.time.ns_per_ms);
    }
}
