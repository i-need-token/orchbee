//! TH-003 control lanes: while the Worker session holds the dispatch thread, four lane threads
//! each accept one connection at a time on the broker endpoint, answer its control request, and
//! close it. A lane never touches `HostService`; it replies from the facts the dispatch thread
//! publishes, and an offer it receives is rejected with `capacity` until every lane has stopped,
//! even after the session ended, so the offer's retry reaches the dispatch thread.

const std = @import("std");

const monotonic = @import("terminal-host-platform").monotonic;
const service_boundary = @import("terminal-host-service");
const candidate = @import("candidate.zig");

const candidate_admission = service_boundary.candidate_admission;

pub const lane_count = 4;
/// How long a lane waits for a connection before it rechecks its stop flag.
pub const accept_poll_ms = 50;

/// Starts and stops the lanes around one Worker session. `Endpoint` is the platform's shared lane
/// context; its `Lane` opens with `open(*Endpoint) !Lane` and offers `acceptBefore(u32) !bool`,
/// `transport()`, `finish()` for the accepted connection, and `close()`.
pub fn ControlLanes(comptime Endpoint: type) type {
    return struct {
        const Self = @This();

        endpoint: Endpoint,
        allocator: std.mem.Allocator,
        facts: candidate_admission.SharedFacts = .{},
        stop: std.atomic.Value(bool) = .init(false),
        finished: [lane_count]std.atomic.Value(bool) = @splat(.init(false)),
        threads: [lane_count]?std.Thread = @splat(null),

        pub fn init(endpoint: Endpoint, allocator: std.mem.Allocator) Self {
            return .{ .endpoint = endpoint, .allocator = allocator };
        }

        /// Starts every lane that can start; a lane that cannot leaves the session running with
        /// fewer control connections.
        pub fn start(self: *Self, facts: candidate_admission.Facts) void {
            self.facts.publish(facts);
            for (&self.threads, 0..) |*thread, index| {
                thread.* = std.Thread.spawn(.{}, run, .{ self, index }) catch |err| {
                    std.log.warn("terminal Host control lane did not start: {s}", .{@errorName(err)});
                    continue;
                };
            }
        }

        pub fn publish(self: *Self, facts: candidate_admission.Facts) void {
            self.facts.publish(facts);
        }

        /// Stops and joins every lane before the dispatch thread accepts again. `service`
        /// keeps maintaining terminals while a lane finishes its current reply.
        pub fn stopAndJoin(self: *Self, service: anytype) void {
            self.stop.store(true, .release);
            for (self.threads, &self.finished) |thread, *finished| {
                if (thread == null) continue;
                while (!finished.load(.acquire)) {
                    service.maintain();
                    monotonic.sleep(std.time.ns_per_ms);
                }
            }
            for (&self.threads) |*thread| {
                if (thread.*) |running| running.join();
                thread.* = null;
            }
        }

        fn run(self: *Self, index: usize) void {
            defer self.finished[index].store(true, .release);
            var lane = Endpoint.Lane.open(&self.endpoint) catch |err| {
                std.log.warn("terminal Host control lane could not listen: {s}", .{@errorName(err)});
                return;
            };
            defer lane.close();
            while (!self.stop.load(.acquire)) {
                const accepted = lane.acceptBefore(accept_poll_ms) catch |err| {
                    std.log.warn("terminal Host control lane stopped accepting: {s}", .{@errorName(err)});
                    return;
                };
                if (!accepted) continue;
                defer lane.finish();
                // An accepted candidate always gets its reply, so stopping waits for it.
                _ = candidate.admit(self.allocator, lane.transport(), .{
                    .facts = .{ .shared = &self.facts },
                    .session_slot_free = false,
                }) catch |err| switch (err) {
                    error.ConnectionClosed => {},
                    else => std.log.warn("closing terminal Host control connection: {s}", .{@errorName(err)}),
                };
            }
        }
    };
}

/// The lanes for a connection served without them, such as a test transport.
pub const None = struct {
    pub fn start(_: *None, _: candidate_admission.Facts) void {}
    pub fn publish(_: *None, _: candidate_admission.Facts) void {}
    pub fn stopAndJoin(_: *None, _: anytype) void {}
};
