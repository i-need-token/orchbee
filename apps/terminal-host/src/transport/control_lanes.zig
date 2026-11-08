//! TH-003 control lanes: while the Worker session holds the dispatch thread, four lane threads
//! each accept one connection at a time on the broker endpoint, answer its control request, and
//! close it. A lane never touches `HostService`; it replies from the facts the dispatch thread
//! publishes and posts `drain` or `exit` for the dispatch thread to apply. An offer a lane receives
//! is rejected with `capacity` until every lane has stopped, even after the session ended, so the
//! offer's retry reaches the dispatch thread.

const std = @import("std");

const monotonic = @import("terminal-host-platform").monotonic;
const service_boundary = @import("terminal-host-service");
const candidate = @import("candidate.zig");

const candidate_admission = service_boundary.candidate_admission;
const host_lifecycle = service_boundary.host_lifecycle;

pub const lane_count = 4;
/// How long a lane whose endpoint cannot wake it waits for a connection before it rechecks its
/// stop flag.
pub const accept_poll_ms = 50;

/// Starts and stops the lanes around one Worker session. `Endpoint` is the platform's shared lane
/// context: `armStop()` runs before the lanes start and `signalStop()` once `stop` is set, ending a
/// lane's wait in `accept()`. Its `Lane` opens with `open(*Endpoint) !Lane` and offers
/// `accept() !bool`, false when the wait ended without a connection, then `transport()` and
/// `finish()` for the accepted connection, and `close()`. Accepting never reads: the candidate's
/// first frame has its own deadline however long the accept wait is.
pub fn ControlLanes(comptime Endpoint: type) type {
    return struct {
        const Self = @This();

        endpoint: Endpoint,
        allocator: std.mem.Allocator,
        handoff: candidate_admission.LaneHandoff = .{},
        stop: std.atomic.Value(bool) = .init(false),
        finished: [lane_count]std.atomic.Value(bool) = @splat(.init(false)),
        threads: [lane_count]?std.Thread = @splat(null),

        pub fn init(endpoint: Endpoint, allocator: std.mem.Allocator) Self {
            return .{ .endpoint = endpoint, .allocator = allocator };
        }

        /// Starts every lane that can start; a lane that cannot leaves the session running with
        /// fewer control connections.
        pub fn start(self: *Self, facts: candidate_admission.Facts) void {
            self.handoff.publish(facts);
            self.endpoint.armStop();
            for (&self.threads, 0..) |*thread, index| {
                thread.* = std.Thread.spawn(.{}, run, .{ self, index }) catch |err| {
                    std.log.warn("terminal Host control lane did not start: {s}", .{@errorName(err)});
                    continue;
                };
            }
        }

        /// Applies every request a lane posted, then publishes the facts lanes reply from. Only
        /// the dispatch thread calls it, between two Worker requests.
        pub fn serve(self: *Self, service: anytype) void {
            const posted = self.handoff.take();
            for ([_]host_lifecycle.Request{ .drain, .exit }) |request| {
                if (posted.contains(request)) _ = service.applyControlRequest(request);
            }
            self.handoff.publish(service.candidateFacts());
        }

        /// Stops and joins every lane before the dispatch thread accepts again. `service`
        /// keeps maintaining terminals and applying posted requests while a lane finishes its
        /// current reply, and a request a lane stopped waiting for still takes effect.
        pub fn stopAndJoin(self: *Self, service: anytype) void {
            self.stop.store(true, .release);
            self.endpoint.signalStop();
            for (self.threads, &self.finished) |thread, *finished| {
                if (thread == null) continue;
                while (!finished.load(.acquire)) {
                    self.serve(service);
                    service.maintain();
                    monotonic.sleep(std.time.ns_per_ms);
                }
            }
            for (&self.threads) |*thread| {
                if (thread.*) |running| running.join();
                thread.* = null;
            }
            self.serve(service);
        }

        fn run(self: *Self, index: usize) void {
            defer self.finished[index].store(true, .release);
            var lane = Endpoint.Lane.open(&self.endpoint) catch |err| {
                std.log.warn("terminal Host control lane could not listen: {s}", .{@errorName(err)});
                return;
            };
            defer lane.close();
            while (!self.stop.load(.acquire)) {
                const accepted = lane.accept() catch |err| {
                    std.log.warn("terminal Host control lane stopped accepting: {s}", .{@errorName(err)});
                    return;
                };
                if (!accepted) continue;
                defer lane.finish();
                // An accepted candidate always gets its reply, so stopping waits for it.
                _ = candidate.admit(self.allocator, lane.transport(), .{
                    .host = .{ .lane = &self.handoff },
                    .session_slot_free = false,
                }) catch |err| {
                    if (!candidate.isSilentClose(err)) {
                        std.log.warn("closing terminal Host control connection: {s}", .{@errorName(err)});
                    }
                };
            }
        }
    };
}

/// The lanes for a connection served without them, such as a test transport.
pub const None = struct {
    pub fn start(_: *None, _: candidate_admission.Facts) void {}
    pub fn serve(_: *None, _: anytype) void {}
    pub fn stopAndJoin(_: *None, _: anytype) void {}
};
