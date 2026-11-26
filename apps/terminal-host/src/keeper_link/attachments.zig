//! The dispatch thread's side of every Keeper attachment: at most two, each in one slot of every
//! terminal's release-first gate. It starts and detaches links, offers them terminals to hold, and
//! collects what they report. A link whose slots are all settled gives its slot index up at once
//! and is joined once its thread returns, which may wait for its Keeper's end.

const std = @import("std");
const builtin = @import("builtin");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const notice = @import("notice.zig");
const moves = @import("moves.zig");

const message = keeper_protocol.message;
const maximum_attachments = platform.keeper.maximum_attachments;
const is_windows = builtin.os.tag == .windows;
const supported = platform.keeper.supported;
const Link = switch (builtin.os.tag) {
    .windows => @import("link_windows.zig").Link,
    .linux, .macos => @import("link_posix.zig").Link,
    else => void,
};
/// A background program offered for a hold; only Windows, macOS and Linux Hosts hold them.
pub const Program = switch (builtin.os.tag) {
    .windows => @import("link_windows.zig").Program,
    .linux, .macos => @import("link_posix.zig").Program,
    else => void,
};

/// A link pinned for one creation, which may start that terminal's leader through its Keeper; the
/// creation unpins it once its build returned.
pub const Spawner = if (builtin.os.tag == .macos or builtin.os.tag == .linux) struct {
    link: *Link,
    terminal: message.Terminal,

    /// The hook a launch carries; valid while this spawner stays where it is.
    pub fn hook(self: *Spawner) platform.keeper.parent.Spawn {
        return .{ .context = self, .spawn = spawn };
    }

    fn spawn(context: *anyopaque, plan: *const platform.keeper.parent.Plan) platform.keeper.parent.Outcome {
        const self: *Spawner = @ptrCast(@alignCast(context));
        return self.link.spawnLeader(plan, self.terminal);
    }

    pub fn unpin(self: *Spawner) void {
        self.link.unpin();
    }
} else void;

/// A link pinned for one background program's start, which may start it through its Keeper; the
/// start unpins it once it returned.
pub const ProgramSpawner = if (builtin.os.tag == .macos or builtin.os.tag == .linux) struct {
    link: *Link,

    /// The hook a launch carries; valid while this spawner stays where it is.
    pub fn hook(self: *ProgramSpawner) platform.keeper.parent.ProgramSpawn {
        return .{ .context = self, .spawn = spawn };
    }

    fn spawn(context: *anyopaque, plan: *const platform.keeper.parent.ProgramPlan) platform.keeper.parent.Outcome {
        const self: *ProgramSpawner = @ptrCast(@alignCast(context));
        return self.link.spawnProgram(plan);
    }

    pub fn unpin(self: *ProgramSpawner) void {
        self.link.unpin();
    }
} else void;

pub const Attach = enum {
    /// The Keeper is already attached: the answer is due now, without effect.
    attached,
    /// A link is connecting; its `attached` or `unavailable` notice answers.
    pending,
    /// Two attachments are taken.
    limit,
    /// The Keeper cannot be attached here.
    unavailable,
};

pub const Detach = enum {
    /// No attachment names the Keeper: the answer is due now.
    done,
    /// The link releases and closes; its `detached`, `lost`, or `unavailable` notice answers.
    pending,
};

pub const Attachments = struct {
    allocator: std.mem.Allocator,
    /// The link serving each gate slot index.
    slots: [maximum_attachments]?*Link = @splat(null),
    /// Links that gave their index up and whose thread has not returned.
    retiring: std.ArrayList(*Link) = .empty,
    /// Names each attachment in the gates, so one attachment tries a terminal once.
    next_serial: u64 = 1,
    /// The folder of the Host's own socket, where a macOS or Linux Keeper binds its socket too.
    endpoint_directory: []const u8 = "",

    pub fn init(allocator: std.mem.Allocator) Attachments {
        return .{ .allocator = allocator };
    }

    /// Stops every link and joins it. Every terminal has ended, so no gate slot is active.
    pub fn deinit(self: *Attachments) void {
        if (comptime supported) {
            for (&self.slots) |*slot| if (slot.*) |link| {
                link.requestStop();
                if (!pinned(link)) link.destroy();
                slot.* = null;
            };
            for (self.retiring.items) |link| {
                link.requestStop();
                // A creation abandoned at teardown may still hold its link, which then stays.
                if (!pinned(link)) link.destroy();
            }
        }
        self.retiring.deinit(self.allocator);
        self.* = undefined;
    }

    /// Whether any link may take a hold.
    pub fn any(self: *const Attachments) bool {
        for (self.slots) |slot| if (slot != null) return true;
        return false;
    }

    pub fn attach(self: *Attachments, keeper: message.Process, endpoint_id: [32]u8) Attach {
        if (comptime !supported) return .unavailable;
        self.retireSettled();
        var free: ?usize = null;
        for (self.slots, 0..) |slot, index| {
            const link = slot orelse {
                free = free orelse index;
                continue;
            };
            if (!link.keeper.eql(keeper)) continue;
            const status = link.status();
            // Notices name only the Keeper, so a second link to a Keeper still detaching would
            // share them with the first.
            if (status.detaching) return .unavailable;
            return switch (status.phase) {
                .connecting => .pending,
                .attached, .unresponsive => .attached,
                // An ended link's `lost` or `unavailable` notice is on its way.
                .detaching, .ended => .pending,
            };
        }
        const index = free orelse return .limit;
        self.retiring.ensureUnusedCapacity(self.allocator, 1) catch return .unavailable;
        const serial = self.next_serial;
        self.slots[index] = self.startLink(index, serial, keeper, endpoint_id) catch return .unavailable;
        self.next_serial += 1;
        return .pending;
    }

    pub fn detach(self: *Attachments, keeper: message.Process) Detach {
        if (comptime !supported) return .done;
        for (self.slots) |slot| if (slot) |link| if (link.keeper.eql(keeper)) {
            if (link.status().index_free) continue;
            link.requestDetach();
            return .pending;
        };
        return .done;
    }

    /// Offers one terminal to every link; a link that takes no holds, or tried this terminal,
    /// ignores it.
    pub fn offer(self: *Attachments, process: *platform.process.Process, terminal: message.Terminal) void {
        if (comptime !supported) return;
        const gate = platform.keeper.gate(process) orelse return;
        for (self.slots) |slot| if (slot) |link| link.offer(gate, process, terminal);
    }

    /// Offers a background program, whose gate is `gate`, to every attached Keeper that takes
    /// programs.
    pub fn offerProgram(self: *Attachments, gate: *platform.keeper.Gate, program: Program) void {
        if (comptime !supported) return;
        // No hold names facts the wire cannot carry, since a request that does not encode fails
        // the link.
        const facts = program.facts;
        if (!facts.valid() or facts.process.pid == 0 or facts.process.created == 0) return;
        for (self.slots) |slot| if (slot) |link| link.offerProgram(gate, program);
    }

    /// The dispatch thread: pins the newest attachment whose Keeper starts leaders for one creation
    /// of `terminal`; null when none does, and the creation forks its leader itself.
    pub fn spawner(self: *Attachments, terminal: message.Terminal) ?Spawner {
        if (comptime !(builtin.os.tag == .macos or builtin.os.tag == .linux)) return null;
        const link = self.pinNewest(false) orelse return null;
        return .{ .link = link, .terminal = terminal };
    }

    /// The dispatch thread: pins the newest attachment whose Keeper starts background programs for
    /// one start; null when none does, and the Host starts the program itself.
    pub fn programSpawner(self: *Attachments) ?ProgramSpawner {
        if (comptime !(builtin.os.tag == .macos or builtin.os.tag == .linux)) return null;
        const link = self.pinNewest(true) orelse return null;
        return .{ .link = link };
    }

    fn pinNewest(self: *Attachments, programs: bool) ?*Link {
        var tried: [maximum_attachments]bool = @splat(false);
        while (true) {
            var newest: ?usize = null;
            for (self.slots, 0..) |slot, index| if (slot) |link| {
                if (tried[index]) continue;
                if (newest == null or link.serial > self.slots[newest.?].?.serial) newest = index;
            };
            const index = newest orelse return null;
            tried[index] = true;
            const link = self.slots[index].?;
            if (link.pin(programs)) return link;
        }
    }

    /// How many Keepers are attached, for the moves that need the Host's one Keeper.
    pub fn count(self: *const Attachments) usize {
        var attached: usize = 0;
        for (self.slots) |slot| attached += @intFromBool(slot != null);
        return attached;
    }

    /// The Keeper of the one attachment, when exactly one is attached.
    pub fn sole(self: *const Attachments) ?message.Process {
        if (comptime !supported) return null;
        if (self.count() != 1) return null;
        for (self.slots) |slot| if (slot) |link| return link.keeper;
        unreachable;
    }

    /// The Keeper whose attachment holds the terminal behind `gate`, when exactly one does.
    pub fn holder(self: *const Attachments, gate: *platform.keeper.Gate) ?message.Process {
        if (comptime !supported) return null;
        const index = gate.heldIndex() orelse return null;
        const link = self.slots[index] orelse return null;
        return link.keeper;
    }

    /// Whether `keeper` is attached and its link has missed no deadline.
    pub fn answers(self: *const Attachments, keeper: message.Process) bool {
        if (comptime !supported) return false;
        const link = self.find(keeper) orelse return false;
        const status = link.status();
        return status.phase == .attached and !status.detaching;
    }

    /// Queues a move request on `keeper`'s link; without one, completes it as `failed` at once.
    pub fn submit(self: *const Attachments, keeper: message.Process, request: *moves.Request) void {
        if (comptime !supported) return request.complete(.failed);
        const link = self.find(keeper) orelse return request.complete(.failed);
        _ = moves.submit(link, request);
    }

    fn find(self: *const Attachments, keeper: message.Process) ?*Link {
        for (self.slots) |slot| if (slot) |link| if (link.keeper.eql(keeper)) return link;
        return null;
    }

    /// Moves every posted notice into `out` and joins links whose thread returned. On a failed
    /// append the notices stay posted for the next call.
    pub fn poll(self: *Attachments, out: *std.ArrayList(notice.Notice)) void {
        if (comptime !supported) return;
        for (self.slots) |slot| if (slot) |link| link.takeNotices(self.allocator, out);
        self.retireSettled();
        var index: usize = 0;
        while (index < self.retiring.items.len) {
            const link = self.retiring.items[index];
            // Every notice precedes `finished`, so one read after it takes the last of them.
            const finished = link.status().finished;
            link.takeNotices(self.allocator, out);
            if (finished and link.notices.items.len == 0 and !pinned(link)) {
                link.destroy();
                _ = self.retiring.swapRemove(index);
            } else index += 1;
        }
    }

    /// Starts a link to `keeper`: on Windows through its pipe, on macOS and Linux through its
    /// socket beside the Host's own.
    fn startLink(self: *Attachments, index: usize, serial: u64, keeper: message.Process, endpoint_id: [32]u8) !*Link {
        if (comptime is_windows) return Link.start(self.allocator, index, serial, keeper, endpoint_id);
        const path = try std.fmt.allocPrint(self.allocator, "{s}/{s}-host.sock", .{ self.endpoint_directory, &endpoint_id });
        defer self.allocator.free(path);
        return Link.start(self.allocator, index, serial, keeper, path);
    }

    /// Moves each link whose slots are all settled out of its index.
    fn retireSettled(self: *Attachments) void {
        for (&self.slots) |*slot| if (slot.*) |link| {
            if (!link.status().index_free) continue;
            self.retiring.append(self.allocator, link) catch continue;
            slot.* = null;
        };
    }
};

/// Whether a creation still holds `link`, which is then not freed.
fn pinned(link: *Link) bool {
    if (comptime !(builtin.os.tag == .macos or builtin.os.tag == .linux)) return false;
    return link.status().users > 0;
}
