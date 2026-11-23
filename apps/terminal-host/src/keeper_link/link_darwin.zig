//! One Keeper attachment and its link thread on macOS. The thread checks and connects to the Keeper,
//! then holds each terminal the dispatch thread queues by sending its PTY master with `hold-pty`,
//! starts the leader of each new terminal a creation hands it with `spawn`, releases each holding
//! that ends or that a detach gives back, reaps each parented leader its terminal asks for, reads
//! the `exited` events the Keeper sends between requests, and watches the Keeper process. Every
//! request waits at most the Keeper's reply deadline; after one miss the link fails every later
//! request at once, so release first never waits on this Keeper again. The thread settles every
//! gate slot it touched, and ends the parent of every leader it parents, before it reports the
//! attachment gone. A master moves with its request, so an unanswered hold leaves nothing to keep.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const connect = @import("connect_darwin.zig");
const notice = @import("notice.zig");
const holds = @import("hold_darwin.zig");
const parenting = @import("parent_darwin.zig");

const message = keeper_protocol.message;
const thread_sync = platform.thread_sync;
const wake_events = platform.keeper.darwin.wake_events;
const parent_module = platform.keeper.parent;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;
const Record = parent_module.Record;
const posix = std.posix;

/// The link thread only connects, encodes, and waits.
const thread_stack_bytes = 256 * 1024;
const hello_id: u32 = 1;
/// How long an idle link waits before it checks the Keeper process again.
const idle_slice_ms = 1000;

pub const Queued = holds.Queued;
/// macOS Hosts hold no background programs.
pub const Program = void;

pub const Held = struct { gate: *Gate, holding: [16]u8 };
const SpawnCall = parenting.SpawnCall;
const recordOf = parenting.recordOf;

pub const Phase = enum { connecting, attached, unresponsive, detaching, ended };

pub const Link = struct {
    allocator: std.mem.Allocator,
    /// The attachment's slot in every terminal's gate.
    index: usize,
    serial: u64,
    keeper: message.Process,
    /// The Keeper's socket, owned.
    socket_path: [:0]u8,
    /// The dispatch thread, a gate's `pass`, or teardown wakes the thread.
    wake: posix.fd_t,
    thread: std.Thread = undefined,

    mutex: thread_sync.Mutex = .{},
    // Guarded by `mutex`. Lock order: a link's mutex, then a gate's.
    queue: std.ArrayList(Queued) = .empty,
    /// Whether the dispatch thread may queue holds: only while attached and answering.
    accepting: bool = false,
    /// The Keeper negotiated `parent`, so it starts leaders.
    parent: bool = false,
    /// Creations waiting for their `spawn`, in order.
    spawns: std.ArrayList(*SpawnCall) = .empty,
    /// Creations that may still hand this link a `spawn`; the link is freed only once none does.
    users: u32 = 0,
    phase: Phase = .connecting,
    detach_requested: bool = false,
    stop_requested: bool = false,
    notices: std.ArrayList(notice.Notice) = .empty,
    /// Every gate slot of this attachment is settled, so its index may serve another Keeper.
    index_free: bool = false,
    /// The thread has returned or is about to; the dispatch thread joins it.
    finished: bool = false,

    // Owned by the link thread.
    connection: ?connect.Connection = null,
    held: std.ArrayList(Held) = .empty,
    failed: bool = false,
    woke: bool = false,
    next_request: u32 = hello_id + 1,

    pub fn start(allocator: std.mem.Allocator, index: usize, serial: u64, keeper: message.Process, socket_path: []const u8) !*Link {
        const link = try allocator.create(Link);
        errdefer allocator.destroy(link);
        const path = try allocator.dupeZ(u8, socket_path);
        errdefer allocator.free(path);
        const wake = wake_events.create() orelse return error.SystemResources;
        errdefer platform.posix_compat.close(wake);
        link.* = .{ .allocator = allocator, .index = index, .serial = serial, .keeper = keeper, .socket_path = path, .wake = wake };
        link.thread = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, run, .{link});
        return link;
    }

    /// Joins a finished link and frees it.
    pub fn destroy(self: *Link) void {
        self.thread.join();
        self.queue.deinit(self.allocator);
        self.notices.deinit(self.allocator);
        self.spawns.deinit(self.allocator);
        self.held.deinit(self.allocator);
        platform.posix_compat.close(self.wake);
        self.allocator.free(self.socket_path);
        self.allocator.destroy(self);
    }

    /// Queues `process`'s hold unless this attachment takes no holds or already tried this one.
    pub fn offer(self: *Link, gate: *Gate, process: *Process, terminal: message.Terminal) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (!self.accepting) return;
        // `pass` waits for a queued slot, so its entry must fit before the slot is taken.
        self.queue.ensureUnusedCapacity(self.allocator, 1) catch return;
        const holding = gate.queue(self.index, self.serial, self.wake) orelse return;
        self.queue.appendAssumeCapacity(.{ .gate = gate, .holding = holding, .process = process, .terminal = terminal });
        wake_events.signal(self.wake);
    }

    /// Takes a creation's claim on this link, which keeps it from being freed; false when the link
    /// starts no leaders.
    pub fn pin(self: *Link) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (!self.accepting or !self.parent) return false;
        self.users += 1;
        return true;
    }

    pub fn unpin(self: *Link) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.users -= 1;
        platform.output_wake.signal();
    }

    /// The creation thread: starts a new terminal's leader through this Keeper, within its reply
    /// deadline once the link takes it.
    pub fn spawnLeader(self: *Link, plan: *const parent_module.Plan, terminal: message.Terminal) parent_module.Outcome {
        var waiting: SpawnCall = .{ .plan = plan, .terminal = terminal };
        {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (!self.accepting) return .refused;
            self.spawns.append(self.allocator, &waiting) catch return .refused;
            wake_events.signal(self.wake);
        }
        return waiting.wait();
    }

    pub fn requestDetach(self: *Link) void {
        self.request(&self.detach_requested);
    }

    pub fn requestStop(self: *Link) void {
        self.request(&self.stop_requested);
    }

    fn request(self: *Link, flag: *bool) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        flag.* = true;
        self.accepting = false;
        wake_events.signal(self.wake);
    }

    pub const Status = struct { phase: Phase, detaching: bool, index_free: bool, finished: bool, users: u32 };

    pub fn status(self: *Link) Status {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{ .phase = self.phase, .detaching = self.detach_requested, .index_free = self.index_free, .finished = self.finished, .users = self.users };
    }

    /// Moves every posted notice into `out`; on a failed append they stay posted.
    pub fn takeNotices(self: *Link, allocator: std.mem.Allocator, out: *std.ArrayList(notice.Notice)) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        out.appendSlice(allocator, self.notices.items) catch return;
        self.notices.clearRetainingCapacity();
    }

    fn post(self: *Link, value: notice.Notice) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        // A notice that cannot be stored is dropped; the Worker reads the Keeper's status instead.
        self.notices.append(self.allocator, value) catch {};
        platform.output_wake.signal();
    }

    fn postUnheld(self: *Link, item: Queued, reason: []const u8) void {
        self.post(.{ .unheld = .{ .keeper = self.keeper, .terminal_id = item.terminal.terminal_id, .reason = reason } });
    }

    fn setPhase(self: *Link, phase: Phase, accepting: bool) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.phase = phase;
        self.accepting = accepting and !self.detach_requested and !self.stop_requested;
    }

    const Requests = struct { detach: bool, stop: bool };

    fn requests(self: *Link) Requests {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{ .detach = self.detach_requested, .stop = self.stop_requested };
    }

    fn nextSpawn(self: *Link) ?*SpawnCall {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.spawns.items.len == 0) return null;
        return self.spawns.orderedRemove(0);
    }

    fn nextQueued(self: *Link) ?Queued {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.queue.items.len == 0) return null;
        return self.queue.orderedRemove(0);
    }

    /// Cancels every queued hold whose terminal began ending, so its `pass` goes on at once.
    fn cancelEnding(self: *Link) void {
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
        const self: *Link = @ptrCast(@alignCast(context));
        self.woke = true;
        self.cancelEnding();
    }

    fn watch(self: *Link) connect.Watch {
        return .{ .wake = self.wake, .context = self, .woken = woken };
    }

    fn run(self: *Link) void {
        self.connection = connect.open(self.keeper, self.socket_path.ptr, .host, hello_id, self.watch(), self.allocator) catch null;
        if (self.connection == null or !self.connection.?.pty) {
            self.post(.{ .unavailable = self.keeper });
            return self.finish();
        }
        self.connection.?.events = .{ .context = self, .exited = parenting.onExited };
        self.mutex.lock();
        self.parent = self.connection.?.parent;
        self.mutex.unlock();
        self.setPhase(.attached, true);
        self.post(.{ .attached = self.keeper });
        while (true) {
            // A Keeper call's wait takes the wake and only sets `woke`, so a terminal that began
            // ending during a call is served by another pass: the link waits only after a pass in
            // which no call took the wake.
            self.woke = false;
            self.cancelEnding();
            const standing = self.requests();
            if (standing.stop) return self.stop();
            self.serveReleases(standing.detach);
            if (!self.keeperEnded()) self.serveSpawns(standing.detach);
            if (!self.keeperEnded()) parenting.serveReaps(self);
            if (!self.keeperEnded()) self.serveHolds(standing.detach);
            if (self.keeperEnded()) return self.lose(standing.detach);
            if (standing.detach) return self.detach();
            if (self.woke) continue;
            if (!self.idle()) return self.lose(self.requests().detach);
        }
    }

    /// Waits for work, reading what the Keeper sends meanwhile; false when the Keeper ended.
    fn idle(self: *Link) bool {
        while (true) {
            if (self.keeperEnded()) return false;
            if (self.connection.?.waitIdle(self.watch(), idle_slice_ms)) self.readEvents();
            if (self.woke) return true;
        }
    }

    /// Reads the Keeper's events; a broken exchange fails the link, and a closed socket after the
    /// Keeper ended is its loss, which the next check sees.
    fn readEvents(self: *Link) void {
        var arena_state = std.heap.ArenaAllocator.init(self.allocator);
        defer arena_state.deinit();
        self.connection.?.readEvents(arena_state.allocator(), self.watch()) catch |err| switch (err) {
            error.KeeperEnded => {},
            else => if (!self.failed) self.fail(),
        };
    }

    fn keeperEnded(self: *const Link) bool {
        return self.connection.?.keeperEnded();
    }

    fn serveReleases(self: *Link, detaching: bool) void {
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

    fn serveSpawns(self: *Link, detaching: bool) void {
        while (self.nextSpawn()) |pending| {
            pending.complete(if (detaching or self.failed) .refused else parenting.spawn(self, pending));
            if (self.keeperEnded()) return;
        }
    }

    fn serveHolds(self: *Link, detaching: bool) void {
        while (self.nextQueued()) |item| {
            if (!item.gate.take(self.index)) continue;
            if (detaching or self.failed) {
                item.gate.settle(self.index);
                continue;
            }
            self.hold(item, detaching);
            if (self.keeperEnded()) return;
            // A terminal that began ending meanwhile releases before the next hold.
            if (self.woke) {
                self.woke = false;
                self.serveReleases(detaching);
            }
        }
    }

    fn hold(self: *Link, item: Queued, detaching: bool) void {
        const prepared = holds.prepare(item) catch {
            item.gate.settle(self.index);
            return self.postUnheld(item, "hold-failed");
        };
        self.held.ensureUnusedCapacity(self.allocator, 1) catch {
            item.gate.settle(self.index);
            return self.postUnheld(item, "hold-failed");
        };
        const outcome = self.call(prepared.body, &.{prepared.master});
        if (outcome == .unanswered) {
            item.gate.settle(self.index);
            return self.postUnheld(item, "unanswered");
        }
        const reply = switch (outcome) {
            .reply => |body| body,
            else => return item.gate.settle(self.index),
        };
        switch (reply) {
            .hold_pty => if (item.gate.held(self.index, detaching)) {
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

    /// Releases a holding, or gives up at once on a failed link, and settles its slot. A released
    /// parented leader is the Keeper's to reap once it ends.
    fn release(self: *Link, gate: *Gate, holding: [16]u8) void {
        const record = recordOf(gate);
        switch (self.call(.{ .release = holding }, &.{})) {
            .reply => |body| if (body == .release) record.releasedBy(self.serial) else record.lostBy(self.serial),
            else => record.lostBy(self.serial),
        }
        gate.settle(self.index);
    }

    pub const Outcome = union(enum) {
        reply: message.Reply.Body,
        /// This request missed its deadline or ended the connection.
        unanswered,
        /// An earlier miss failed the link; nothing was sent.
        failed,
        keeper_ended,
    };

    /// Makes one request on the link thread, bounded by the Keeper's reply deadline.
    pub fn call(self: *Link, body: message.Request.Body, descriptors: []const posix.fd_t) Outcome {
        if (self.failed) return .failed;
        const id = self.next_request;
        self.next_request +%= 1;
        var arena_state = std.heap.ArenaAllocator.init(self.allocator);
        defer arena_state.deinit();
        const reply = self.connection.?.request(arena_state.allocator(), .{ .id = id, .body = body }, descriptors, self.watch()) catch |err| switch (err) {
            error.KeeperEnded => return .keeper_ended,
            error.Unencodable => return .failed,
            else => {
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
    fn fail(self: *Link) void {
        self.failed = true;
        self.connection.?.closeStream();
        self.setPhase(.unresponsive, false);
        self.post(.{ .unresponsive = self.keeper });
    }

    fn settleAll(self: *Link) void {
        self.mutex.lock();
        self.accepting = false;
        for (self.queue.items) |item| if (item.gate.take(self.index)) item.gate.settle(self.index);
        self.queue.clearRetainingCapacity();
        // Nothing was sent for these, so each creation forks its leader itself.
        for (self.spawns.items) |pending| pending.complete(.refused);
        self.spawns.clearRetainingCapacity();
        self.mutex.unlock();
        for (self.held.items) |entry| {
            recordOf(entry.gate).lostBy(self.serial);
            entry.gate.settle(self.index);
        }
        self.held.clearRetainingCapacity();
    }

    /// The Keeper process ended: the masters it held closed with it, so every slot settles.
    fn lose(self: *Link, detaching: bool) void {
        self.setPhase(.ended, false);
        self.settleAll();
        self.markIndexFree();
        self.post(if (detaching) .{ .detached = self.keeper } else .{ .lost = self.keeper });
        self.finish();
    }

    fn detach(self: *Link) void {
        self.setPhase(.detaching, false);
        self.settleAll();
        self.connection.?.closeStream();
        self.markIndexFree();
        self.post(.{ .detached = self.keeper });
        self.finish();
    }

    /// Host teardown: every terminal has ended, so no slot is active.
    fn stop(self: *Link) void {
        self.settleAll();
        self.finish();
    }

    fn markIndexFree(self: *Link) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.index_free = true;
    }

    fn finish(self: *Link) void {
        if (self.connection) |*connection| connection.close();
        self.connection = null;
        self.mutex.lock();
        defer self.mutex.unlock();
        for (self.spawns.items) |pending| pending.complete(.refused);
        self.spawns.clearRetainingCapacity();
        self.phase = .ended;
        self.accepting = false;
        self.index_free = true;
        self.finished = true;
        platform.output_wake.signal();
    }
};
