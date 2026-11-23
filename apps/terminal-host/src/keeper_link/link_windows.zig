//! One Keeper attachment and its link thread. The thread checks and connects to the Keeper, then
//! holds each terminal and background program the dispatch thread queues, releases each one that
//! ends or that a detach gives back, and watches the Keeper process. Every request waits at most
//! the Keeper's reply deadline; after one miss the link fails every later request at once, so
//! release first never waits on this Keeper again. The thread settles every gate slot it touched
//! before it reports the attachment gone, and keeps the duplicates of an unanswered hold until the
//! Keeper ends.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const connect = @import("connect_windows.zig");
const notice = @import("notice.zig");
const move_requests = @import("moves.zig");
const holds = @import("hold_windows.zig");

const message = keeper_protocol.message;
const thread_sync = platform.thread_sync;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;
const Retained = holds.Retained;

const windows = std.os.windows;
const HANDLE = windows.HANDLE;
const WAIT_OBJECT_0: windows.DWORD = 0;
const INFINITE: windows.DWORD = 0xFFFF_FFFF;

extern "kernel32" fn CreateEventW(attributes: ?*anyopaque, manual_reset: windows.BOOL, initial: windows.BOOL, name: ?[*:0]const u16) callconv(.winapi) ?HANDLE;
extern "kernel32" fn SetEvent(event: HANDLE) callconv(.winapi) windows.BOOL;
extern "kernel32" fn WaitForMultipleObjects(count: windows.DWORD, handles: [*]const HANDLE, wait_all: windows.BOOL, milliseconds: windows.DWORD) callconv(.winapi) windows.DWORD;

/// The link thread only connects, encodes, and waits.
const thread_stack_bytes = 256 * 1024;
const hello_id: u32 = 1;

pub const Queued = holds.Queued;
pub const Program = holds.Program;

const Held = struct { gate: *Gate, holding: [16]u8 };

pub const Phase = enum { connecting, attached, unresponsive, detaching, ended };

pub const Link = struct {
    allocator: std.mem.Allocator,
    /// The attachment's slot in every terminal's gate.
    index: usize,
    serial: u64,
    keeper: message.Process,
    endpoint_id: [32]u8,
    /// Auto-reset: the dispatch thread, a gate's `pass`, or teardown wakes the thread.
    wake: HANDLE,
    thread: std.Thread = undefined,

    mutex: thread_sync.Mutex = .{},
    // Guarded by `mutex`. Lock order: a link's mutex, then a gate's.
    queue: std.ArrayList(Queued) = .empty,
    /// Whether the dispatch thread may queue holds: only while attached and answering.
    accepting: bool = false,
    /// The Keeper negotiated `programs`, so it takes background programs.
    programs: bool = false,
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

    // Owned by the link thread.
    connection: ?connect.Connection = null,
    held: std.ArrayList(Held) = .empty,
    /// Duplicates an unanswered `hold` named, kept until the Keeper ends.
    retained: std.ArrayList(Retained) = .empty,
    failed: bool = false,
    woke: bool = false,
    next_request: u32 = hello_id + 1,

    pub fn start(allocator: std.mem.Allocator, index: usize, serial: u64, keeper: message.Process, endpoint_id: [32]u8) !*Link {
        const link = try allocator.create(Link);
        errdefer allocator.destroy(link);
        const wake = CreateEventW(null, .FALSE, .FALSE, null) orelse return error.SystemResources;
        errdefer windows.CloseHandle(wake);
        link.* = .{ .allocator = allocator, .index = index, .serial = serial, .keeper = keeper, .endpoint_id = endpoint_id, .wake = wake };
        link.thread = try std.Thread.spawn(.{ .stack_size = thread_stack_bytes }, run, .{link});
        return link;
    }

    /// Joins a finished link and frees it.
    pub fn destroy(self: *Link) void {
        self.thread.join();
        self.queue.deinit(self.allocator);
        self.notices.deinit(self.allocator);
        self.moves.deinit(self.allocator);
        self.held.deinit(self.allocator);
        self.retained.deinit(self.allocator);
        windows.CloseHandle(self.wake);
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
        self.queue.appendAssumeCapacity(.{ .gate = gate, .holding = holding, .subject = .{ .terminal = .{
            .process = process,
            .terminal = terminal,
        } } });
        _ = SetEvent(self.wake);
    }

    /// Queues a background program's hold unless this attachment takes no holds, its Keeper takes
    /// no programs, or it already tried this one.
    pub fn offerProgram(self: *Link, gate: *Gate, program: Program) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (!self.accepting or !self.programs) return;
        self.queue.ensureUnusedCapacity(self.allocator, 1) catch return;
        const holding = gate.queue(self.index, self.serial, self.wake) orelse return;
        self.queue.appendAssumeCapacity(.{ .gate = gate, .holding = holding, .subject = .{ .program = program } });
        _ = SetEvent(self.wake);
    }

    pub fn requestDetach(self: *Link) void {
        self.request(&self.detach_requested);
    }

    pub fn requestStop(self: *Link) void {
        self.request(&self.stop_requested);
    }

    /// Wakes the link thread; the caller holds `mutex`.
    pub fn wakeThread(self: *Link) void {
        _ = SetEvent(self.wake);
    }

    fn request(self: *Link, flag: *bool) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        flag.* = true;
        self.accepting = false;
        _ = SetEvent(self.wake);
    }

    pub const Status = struct { phase: Phase, detaching: bool, index_free: bool, finished: bool };

    pub fn status(self: *Link) Status {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{ .phase = self.phase, .detaching = self.detach_requested, .index_free = self.index_free, .finished = self.finished };
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

    /// Reports a terminal left unheld; the Worker hears of no program left unheld.
    fn postUnheld(self: *Link, item: Queued, reason: []const u8) void {
        switch (item.subject) {
            .terminal => |terminal| self.post(.{ .unheld = .{
                .keeper = self.keeper,
                .terminal_id = terminal.terminal.terminal_id,
                .reason = reason,
            } }),
            .program => {},
        }
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
        self.connection = connect.open(self.keeper, &self.endpoint_id, hello_id, self.watch(), self.allocator) catch null;
        if (self.connection == null) {
            self.post(.{ .unavailable = self.keeper });
            return self.finish();
        }
        self.mutex.lock();
        self.programs = self.connection.?.programs;
        self.mutex.unlock();
        self.setPhase(.attached, true);
        self.post(.{ .attached = self.keeper });
        while (true) {
            // A Keeper call's wait takes the wake event and only sets `woke`, so a terminal that
            // began ending during a call is served by another pass: the link waits only after a
            // pass in which no call took the wake.
            self.woke = false;
            self.cancelEnding();
            const standing = self.requests();
            if (standing.stop) return self.stop();
            self.serveReleases(standing.detach);
            if (!self.keeperEnded()) self.serveHolds(standing.detach);
            if (!self.keeperEnded()) move_requests.serve(self);
            if (self.keeperEnded()) return self.lose(standing.detach);
            if (standing.detach) return self.detach();
            if (self.woke) continue;
            if (!self.idle()) return self.lose(self.requests().detach);
        }
    }

    /// Waits for work; false when the Keeper process ended.
    fn idle(self: *Link) bool {
        const handles = [_]HANDLE{ self.connection.?.keeper_process, self.wake };
        return WaitForMultipleObjects(handles.len, &handles, .FALSE, INFINITE) != WAIT_OBJECT_0;
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

    /// Holds what `item` names under duplicates this link owns, so the values the request names stay
    /// valid until the Keeper answered.
    fn hold(self: *Link, item: Queued, detaching: bool) void {
        var prepared = holds.prepare(item) catch |err| {
            item.gate.settle(self.index);
            return self.postUnheld(item, switch (err) {
                error.SystemConsole => "system-console",
                error.HoldFailed => "hold-failed",
            });
        };
        self.held.ensureUnusedCapacity(self.allocator, 1) catch {
            prepared.retained.close();
            item.gate.settle(self.index);
            return self.postUnheld(item, "hold-failed");
        };
        const outcome = self.call(prepared.body);
        if (outcome == .unanswered) {
            // The Keeper may still pull these values, so they stay open until it ends.
            self.retained.append(self.allocator, prepared.retained) catch prepared.retained.close();
            item.gate.settle(self.index);
            return self.postUnheld(item, "unanswered");
        }
        prepared.retained.close();
        self.settleHold(item, outcome, detaching);
    }

    /// Records what the Keeper answered a hold; the room for a held entry is already taken.
    fn settleHold(self: *Link, item: Queued, outcome: Outcome, detaching: bool) void {
        const reply = switch (outcome) {
            .reply => |body| body,
            else => return item.gate.settle(self.index),
        };
        switch (reply) {
            .hold, .hold_program => if (item.gate.held(self.index, detaching)) {
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

    /// Releases a holding, or gives up at once on a failed link, and settles its slot. A refused
    /// or unanswered release leaves the duplicates with the Keeper, which then keeps the console
    /// open until it ends.
    fn release(self: *Link, gate: *Gate, holding: [16]u8) void {
        _ = self.call(.{ .release = holding });
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
    pub fn call(self: *Link, body: message.Request.Body) Outcome {
        if (self.failed) return .failed;
        const id = self.next_request;
        self.next_request +%= 1;
        var arena_state = std.heap.ArenaAllocator.init(self.allocator);
        defer arena_state.deinit();
        const reply = self.connection.?.request(arena_state.allocator(), .{ .id = id, .body = body }, self.watch()) catch |err| switch (err) {
            error.KeeperEnded => return .keeper_ended,
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
        self.connection.?.closePipe();
        self.setPhase(.unresponsive, false);
        self.post(.{ .unresponsive = self.keeper });
    }

    fn settleAll(self: *Link) void {
        self.mutex.lock();
        self.accepting = false;
        for (self.queue.items) |item| if (item.gate.take(self.index)) item.gate.settle(self.index);
        self.queue.clearRetainingCapacity();
        self.mutex.unlock();
        for (self.held.items) |entry| entry.gate.settle(self.index);
        self.held.clearRetainingCapacity();
        move_requests.completeAll(self);
    }

    fn closeRetained(self: *Link) void {
        for (self.retained.items) |*retained| retained.close();
        self.retained.clearRetainingCapacity();
    }

    /// The Keeper process ended: its duplicates closed with it, so every slot settles.
    fn lose(self: *Link, detaching: bool) void {
        self.setPhase(.ended, false);
        self.settleAll();
        self.closeRetained();
        self.markIndexFree();
        self.post(if (detaching) .{ .detached = self.keeper } else .{ .lost = self.keeper });
        self.finish();
    }

    fn detach(self: *Link) void {
        self.setPhase(.detaching, false);
        self.settleAll();
        self.connection.?.closePipe();
        self.markIndexFree();
        self.post(.{ .detached = self.keeper });
        // An unanswered `hold` named values the Keeper may still pull; keep them until it ends.
        while (self.retained.items.len > 0 and !self.keeperEnded()) {
            if (!self.idle() or self.requests().stop) break;
        }
        self.closeRetained();
        self.finish();
    }

    /// Host teardown: every terminal has ended, so no slot is active, and the Host's exit closes
    /// whatever the Keeper could still pull.
    fn stop(self: *Link) void {
        self.settleAll();
        self.closeRetained();
        self.finish();
    }

    fn markIndexFree(self: *Link) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.index_free = true;
    }

    fn finish(self: *Link) void {
        move_requests.completeAll(self);
        if (self.connection) |*connection| connection.close();
        self.connection = null;
        self.mutex.lock();
        defer self.mutex.unlock();
        self.phase = .ended;
        self.accepting = false;
        self.index_free = true;
        self.finished = true;
        platform.output_wake.signal();
    }
};
