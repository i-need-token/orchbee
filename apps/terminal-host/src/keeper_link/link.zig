//! One Keeper attachment and its link thread, shared by the platforms. The thread checks and
//! connects to the Keeper, then holds each terminal the dispatch thread queues, releases each one
//! that ends or that a detach gives back, makes the move requests the dispatch thread posts
//! (`moves.zig`), and watches the Keeper process; its platform adds what only it does
//! (`link_windows.zig`, `link_darwin.zig`). Every request waits at most the Keeper's reply deadline;
//! after one miss the link fails every later request at once, so release first never waits on this
//! Keeper again. The thread settles every gate slot it touched before it reports the attachment
//! gone.
//!
//! `Link(P)` takes its platform `P` as a namespace: `Wake`, `createWake`, `signal`, `closeWake`,
//! `Connection`, `Watch`, `Queued`, `Endpoint`, `Own`, `Sent`, `nothing_sent`, `queuedTerminal`,
//! `unheldTerminal`, `open`, `serve`, `idle`, `hold`, `exchange`, and `breakOff`, and optionally
//! `released`, `lostHeld`, `settleLocked`, `afterDetach`, and `dropKept`, each documented there.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const notice = @import("notice.zig");
const move_requests = @import("moves.zig");

const message = keeper_protocol.message;
const thread_sync = platform.thread_sync;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;

/// The link thread only connects, encodes, and waits.
const thread_stack_bytes = 256 * 1024;
pub const hello_id: u32 = 1;

pub const Phase = enum { connecting, attached, unresponsive, detaching, ended };

pub const Held = struct { gate: *Gate, holding: [16]u8 };

/// What one request's exchange came to, as a platform's connection reports it.
pub const Exchanged = union(enum) {
    reply: message.Reply,
    keeper_ended,
    /// The request could not be encoded, so nothing was sent.
    unsent,
    /// The exchange missed its deadline or broke the connection.
    broken,
};

pub const Outcome = union(enum) {
    reply: message.Reply.Body,
    /// This request missed its deadline or ended the connection.
    unanswered,
    /// An earlier miss failed the link, or the request could not be sent; nothing was sent.
    failed,
    keeper_ended,
};

pub fn Link(comptime P: type) type {
    return struct {
        const Self = @This();

        allocator: std.mem.Allocator,
        /// The attachment's slot in every terminal's gate.
        index: usize,
        serial: u64,
        keeper: message.Process,
        /// The dispatch thread, a gate's `pass`, or teardown wakes the thread.
        wake: P.Wake,
        thread: std.Thread = undefined,

        mutex: thread_sync.Mutex = .{},
        // Guarded by `mutex`. Lock order: a link's mutex, then a gate's.
        queue: std.ArrayList(P.Queued) = .empty,
        /// Whether the dispatch thread may queue holds: only while attached and answering.
        accepting: bool = false,
        phase: Phase = .connecting,
        detach_requested: bool = false,
        stop_requested: bool = false,
        notices: std.ArrayList(notice.Notice) = .empty,
        /// Move requests the link has not made yet, in order; see `moves.zig`.
        moves: std.ArrayList(*move_requests.Request) = .empty,
        /// Every gate slot of this attachment is settled, so its index may serve another Keeper.
        index_free: bool = false,
        /// The thread has returned or is about to; the dispatch thread joins it.
        finished: bool = false,
        /// What only this platform's link keeps; its own comments say which lock guards each.
        own: P.Own,

        // Owned by the link thread.
        connection: ?P.Connection = null,
        held: std.ArrayList(Held) = .empty,
        failed: bool = false,
        woke: bool = false,
        next_request: u32 = hello_id + 1,

        pub fn start(allocator: std.mem.Allocator, index: usize, serial: u64, keeper: message.Process, endpoint: P.Endpoint) !*Self {
            const link = try allocator.create(Self);
            errdefer allocator.destroy(link);
            var own = try P.Own.init(allocator, endpoint);
            errdefer own.deinit(allocator);
            const wake = P.createWake() orelse return error.SystemResources;
            errdefer P.closeWake(wake);
            link.* = .{ .allocator = allocator, .index = index, .serial = serial, .keeper = keeper, .wake = wake, .own = own };
            link.thread = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, run, .{link});
            return link;
        }

        /// Joins a finished link and frees it.
        pub fn destroy(self: *Self) void {
            self.thread.join();
            self.queue.deinit(self.allocator);
            self.notices.deinit(self.allocator);
            self.moves.deinit(self.allocator);
            self.held.deinit(self.allocator);
            self.own.deinit(self.allocator);
            P.closeWake(self.wake);
            self.allocator.destroy(self);
        }

        /// Queues `process`'s hold unless this attachment takes no holds or already tried this one.
        pub fn offer(self: *Self, gate: *Gate, process: *Process, terminal: message.Terminal) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (!self.accepting) return;
            self.queueLocked(gate, P.queuedTerminal(process, terminal));
        }

        /// Queues a hold of `item`, whose gate and holding this call fills in; the caller holds
        /// `mutex` and checked that the link accepts it.
        pub fn queueLocked(self: *Self, gate: *Gate, item: P.Queued) void {
            // `pass` waits for a queued slot, so its entry must fit before the slot is taken.
            self.queue.ensureUnusedCapacity(self.allocator, 1) catch return;
            var entry = item;
            entry.gate = gate;
            entry.holding = gate.queue(self.index, self.serial, self.wake) orelse return;
            self.queue.appendAssumeCapacity(entry);
            P.signal(self.wake);
        }

        /// The platform's own requests; see its module.
        pub const offerProgram = P.offerProgram;
        pub const pin = P.pin;
        pub const unpin = P.unpin;
        pub const spawnLeader = P.spawnLeader;
        pub const spawnProgram = P.spawnProgram;

        pub fn requestDetach(self: *Self) void {
            self.request(&self.detach_requested);
        }

        pub fn requestStop(self: *Self) void {
            self.request(&self.stop_requested);
        }

        /// Wakes the link thread.
        pub fn wakeThread(self: *Self) void {
            P.signal(self.wake);
        }

        fn request(self: *Self, flag: *bool) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            flag.* = true;
            self.accepting = false;
            P.signal(self.wake);
        }

        pub const Status = struct { phase: Phase, detaching: bool, index_free: bool, finished: bool, users: u32 };

        pub fn status(self: *Self) Status {
            self.mutex.lock();
            defer self.mutex.unlock();
            const users: u32 = if (@hasField(P.Own, "users")) self.own.users else 0;
            return .{ .phase = self.phase, .detaching = self.detach_requested, .index_free = self.index_free, .finished = self.finished, .users = users };
        }

        /// Moves every posted notice into `out`; on a failed append they stay posted.
        pub fn takeNotices(self: *Self, allocator: std.mem.Allocator, out: *std.ArrayList(notice.Notice)) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            out.appendSlice(allocator, self.notices.items) catch return;
            self.notices.clearRetainingCapacity();
        }

        pub fn post(self: *Self, value: notice.Notice) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            // A notice that cannot be stored is dropped; the Worker reads the Keeper's status instead.
            self.notices.append(self.allocator, value) catch {};
            platform.output_wake.signal();
        }

        /// Reports a terminal left unheld; the Worker hears of nothing else left unheld.
        pub fn postUnheld(self: *Self, item: P.Queued, reason: []const u8) void {
            const terminal_id = P.unheldTerminal(item) orelse return;
            self.post(.{ .unheld = .{ .keeper = self.keeper, .terminal_id = terminal_id, .reason = reason } });
        }

        pub fn setPhase(self: *Self, phase: Phase, accepting: bool) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            self.phase = phase;
            self.accepting = accepting and !self.detach_requested and !self.stop_requested;
        }

        const Requests = struct { detach: bool, stop: bool };

        pub fn requests(self: *Self) Requests {
            self.mutex.lock();
            defer self.mutex.unlock();
            return .{ .detach = self.detach_requested, .stop = self.stop_requested };
        }

        fn nextQueued(self: *Self) ?P.Queued {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.queue.items.len == 0) return null;
            return self.queue.orderedRemove(0);
        }

        /// Cancels every queued hold whose terminal began ending, so its `pass` goes on at once.
        fn cancelEnding(self: *Self) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            var index: usize = 0;
            while (index < self.queue.items.len) {
                if (self.queue.items[index].gate.cancelIfEnding(self.index)) {
                    _ = self.queue.orderedRemove(index);
                } else index += 1;
            }
        }

        fn woken(context: *anyopaque) void {
            const self: *Self = @ptrCast(@alignCast(context));
            self.woke = true;
            self.cancelEnding();
        }

        pub fn watch(self: *Self) P.Watch {
            return .{ .wake = self.wake, .context = self, .woken = woken };
        }

        fn run(self: *Self) void {
            if (!P.open(self)) {
                self.post(.{ .unavailable = self.keeper });
                return self.finish();
            }
            self.setPhase(.attached, true);
            self.post(.{ .attached = self.keeper });
            while (true) {
                // A Keeper call's wait takes the wake and only sets `woke`, so a terminal that began
                // ending during a call is served by another pass: the link waits only after a pass
                // in which no call took the wake.
                self.woke = false;
                self.cancelEnding();
                const standing = self.requests();
                if (standing.stop) return self.stop();
                self.serveReleases(standing.detach);
                if (!self.keeperEnded()) P.serve(self, standing.detach);
                if (self.keeperEnded()) return self.lose(standing.detach);
                if (standing.detach) return self.detach();
                if (self.woke) continue;
                if (!P.idle(self)) return self.lose(self.requests().detach);
            }
        }

        pub fn keeperEnded(self: *const Self) bool {
            return self.connection.?.keeperEnded();
        }

        fn serveReleases(self: *Self, detaching: bool) void {
            var index: usize = 0;
            while (index < self.held.items.len) {
                const entry = self.held.items[index];
                if (!entry.gate.startRelease(self.index, detaching)) {
                    index += 1;
                    continue;
                }
                _ = self.held.swapRemove(index);
                self.release(entry.gate, entry.holding);
            }
        }

        pub fn serveHolds(self: *Self, detaching: bool) void {
            while (self.nextQueued()) |item| {
                if (!item.gate.take(self.index)) continue;
                if (detaching or self.failed) {
                    item.gate.settle(self.index);
                    continue;
                }
                if (self.held.ensureUnusedCapacity(self.allocator, 1)) {
                    P.hold(self, item, detaching);
                } else |_| {
                    item.gate.settle(self.index);
                    self.postUnheld(item, "hold-failed");
                }
                if (self.keeperEnded()) return;
                // A terminal that began ending meanwhile releases before the next hold.
                if (self.woke) {
                    self.woke = false;
                    self.serveReleases(detaching);
                }
            }
        }

        /// Records what the Keeper answered a hold; the room for a held entry is already taken.
        pub fn settleHold(self: *Self, item: P.Queued, outcome: Outcome, detaching: bool) void {
            if (outcome == .unanswered) {
                item.gate.settle(self.index);
                return self.postUnheld(item, "unanswered");
            }
            const reply = switch (outcome) {
                .reply => |body| body,
                else => return item.gate.settle(self.index),
            };
            switch (reply) {
                .hold, .hold_program, .hold_pty, .hold_pipes => if (item.gate.held(self.index, detaching)) {
                    self.release(item.gate, item.holding);
                } else {
                    self.held.appendAssumeCapacity(.{ .gate = item.gate, .holding = item.holding });
                },
                .refused => |reason| {
                    item.gate.settle(self.index);
                    self.postUnheld(item, reason.name());
                },
                else => {
                    item.gate.settle(self.index);
                    self.postUnheld(item, "unsupported");
                },
            }
        }

        /// Releases a holding, or gives up at once on a failed link, and settles its slot. A
        /// refused or unanswered release leaves what the holding names with the Keeper.
        fn release(self: *Self, gate: *Gate, holding: [16]u8) void {
            const outcome = self.call(.{ .release = holding });
            if (@hasDecl(P, "released")) P.released(self, gate, outcome);
            gate.settle(self.index);
        }

        /// Makes one request on the link thread, bounded by the Keeper's reply deadline.
        pub fn call(self: *Self, body: message.Request.Body) Outcome {
            return self.callSending(body, P.nothing_sent);
        }

        /// `call`, sending what `sent` names with the request where the platform can.
        pub fn callSending(self: *Self, body: message.Request.Body, sent: P.Sent) Outcome {
            if (self.failed) return .failed;
            const id = self.next_request;
            self.next_request +%= 1;
            var arena_state = std.heap.ArenaAllocator.init(self.allocator);
            defer arena_state.deinit();
            const reply = switch (P.exchange(&self.connection.?, arena_state.allocator(), .{ .id = id, .body = body }, sent, self.watch())) {
                .reply => |reply| reply,
                .keeper_ended => return .keeper_ended,
                .unsent => return .failed,
                .broken => {
                    self.fail();
                    return .unanswered;
                },
            };
            if (reply.id != id or !reply.body.answers(std.meta.activeTag(body))) {
                self.fail();
                return .unanswered;
            }
            return .{ .reply = reply.body };
        }

        /// One miss: report it once, take no more holds, and fail every later request at once.
        pub fn fail(self: *Self) void {
            self.failed = true;
            P.breakOff(&self.connection.?);
            self.setPhase(.unresponsive, false);
            self.post(.{ .unresponsive = self.keeper });
        }

        fn settleAll(self: *Self) void {
            self.mutex.lock();
            self.accepting = false;
            for (self.queue.items) |item| if (item.gate.take(self.index)) item.gate.settle(self.index);
            self.queue.clearRetainingCapacity();
            if (@hasDecl(P, "settleLocked")) P.settleLocked(self);
            self.mutex.unlock();
            for (self.held.items) |entry| {
                if (@hasDecl(P, "lostHeld")) P.lostHeld(self, entry);
                entry.gate.settle(self.index);
            }
            self.held.clearRetainingCapacity();
            move_requests.completeAll(self);
        }

        fn dropKept(self: *Self) void {
            if (@hasDecl(P, "dropKept")) P.dropKept(self);
        }

        /// The Keeper process ended: what it held closed with it, so every slot settles.
        fn lose(self: *Self, detaching: bool) void {
            self.setPhase(.ended, false);
            self.settleAll();
            self.dropKept();
            self.markIndexFree();
            self.post(if (detaching) .{ .detached = self.keeper } else .{ .lost = self.keeper });
            self.finish();
        }

        fn detach(self: *Self) void {
            self.setPhase(.detaching, false);
            self.settleAll();
            P.breakOff(&self.connection.?);
            self.markIndexFree();
            self.post(.{ .detached = self.keeper });
            if (@hasDecl(P, "afterDetach")) P.afterDetach(self);
            self.finish();
        }

        /// Host teardown: every terminal has ended, so no slot is active, and the Host's exit closes
        /// whatever the Keeper could still use.
        fn stop(self: *Self) void {
            self.settleAll();
            self.dropKept();
            self.finish();
        }

        fn markIndexFree(self: *Self) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            self.index_free = true;
        }

        fn finish(self: *Self) void {
            move_requests.completeAll(self);
            if (self.connection) |*connection| connection.close();
            self.connection = null;
            self.mutex.lock();
            defer self.mutex.unlock();
            if (@hasDecl(P, "settleLocked")) P.settleLocked(self);
            self.phase = .ended;
            self.accepting = false;
            self.index_free = true;
            self.finished = true;
            platform.output_wake.signal();
        }
    };
}
