//! The requests a macOS Keeper link makes as a terminal leader's parent: the `spawn` a creation
//! hands it, the `reap` a terminal asks for once its leader ended, and the `exited` the Keeper
//! sends. Each runs on the link thread; a creation thread only waits for its call.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const link_module = @import("link_darwin.zig");

const message = keeper_protocol.message;
const thread_sync = platform.thread_sync;
const parent_module = platform.keeper.parent;
const Gate = platform.keeper.Gate;
const Record = parent_module.Record;
const Link = link_module.Link;
const posix = std.posix;

/// A creation's `spawn`, which the creation thread waits for; the link completes every one.
pub const SpawnCall = struct {
    plan: *const parent_module.Plan,
    terminal: message.Terminal,
    mutex: thread_sync.Mutex = .{},
    condition: thread_sync.Condition = .{},
    outcome: ?parent_module.Outcome = null,

    pub fn complete(self: *SpawnCall, outcome: parent_module.Outcome) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.outcome = outcome;
        self.condition.broadcast();
    }

    pub fn wait(self: *SpawnCall) parent_module.Outcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.outcome == null) self.condition.wait(&self.mutex);
        return self.outcome.?;
    }
};

/// The record of the terminal whose gate this is.
pub fn recordOf(gate: *Gate) *Record {
    return @fieldParentPtr("gate", gate);
}

/// The Keeper's `exited`: the parented leader of a holding this link holds ended.
pub fn onExited(context: *anyopaque, exited: message.pty.Exited) void {
    const self: *Link = @ptrCast(@alignCast(context));
    for (self.held.items) |entry| if (std.mem.eql(u8, &entry.holding, &exited.holding)) {
        recordOf(entry.gate).exited(switch (exited.status) {
            .code => |code| .{ .exit_code = code },
            .signal => |number| .{ .signal = number },
        });
        return;
    };
}

/// Asks the Keeper to start `call`'s leader. A started leader's terminal is held here at once,
/// under a holding id this link mints.
pub fn spawn(self: *Link, call: *SpawnCall) parent_module.Outcome {
    const plan = call.plan;
    const record = plan.record;
    const slave = record.slaveName() orelse return .refused;
    self.held.ensureUnusedCapacity(self.allocator, 1) catch return .refused;
    var arena_state = std.heap.ArenaAllocator.init(self.allocator);
    defer arena_state.deinit();
    const launch = launchOf(arena_state.allocator(), plan) catch return .refused;
    if (!message.pty.validLaunch(launch)) return .refused;
    var holding: [16]u8 = undefined;
    thread_sync.io().random(&holding);
    holding[0] |= 1;
    const descriptors = [_]posix.fd_t{ plan.master, plan.slave, plan.input orelse -1 };
    const count: usize = if (plan.input != null) 3 else 2;
    const outcome = self.callSending(.{ .spawn = .{
        .holding = holding,
        .input = plan.input != null,
        .launch = launch,
        .facts = .{ .terminal = call.terminal, .slave = slave, .launch = plan.launch_kind.wireName() },
    } }, descriptors[0..count]);
    const reply = switch (outcome) {
        .reply => |body| body,
        .failed => return .refused,
        .unanswered, .keeper_ended => return .unanswered,
    };
    switch (reply) {
        .spawn => |leader| {
            record.parentedBy(self.serial, leader.created);
            record.gate.adopt(self.index, self.serial, self.wake, holding);
            self.held.appendAssumeCapacity(.{ .gate = &record.gate, .holding = holding });
            return .{ .started = @intCast(leader.pid) };
        },
        .refused => |reason| return if (reason == .spawn_failed) .spawn_failed else .refused,
        else => return .refused,
    }
}

/// Reaps each leader this link parents whose terminal asks for it.
pub fn serveReaps(self: *Link) void {
    for (self.held.items) |entry| {
        const record = recordOf(entry.gate);
        if (!record.reapRequestedOf(self.serial)) continue;
        switch (self.call(.{ .reap = entry.holding })) {
            .reply => record.reapedBy(self.serial),
            else => record.lostBy(self.serial),
        }
    }
}

/// The launch a `spawn` names, read from the Host's prepared plan: the arguments after the first.
fn launchOf(arena: std.mem.Allocator, plan: *const parent_module.Plan) !message.pty.Launch {
    const argv = std.mem.span(plan.argv);
    const arguments = try arena.alloc([]const u8, argv.len -| 1);
    for (arguments, argv[@min(1, argv.len)..]) |*argument, entry| argument.* = std.mem.span(entry.?);
    const entries = std.mem.span(plan.environment);
    const environment = try arena.alloc([]const u8, entries.len);
    for (environment, entries) |*entry, value| entry.* = std.mem.span(value.?);
    const candidates = try arena.alloc([]const u8, plan.candidates.len);
    for (candidates, plan.candidates) |*candidate, value| candidate.* = std.mem.span(value);
    return .{
        .executable = plan.executable,
        .arguments = arguments,
        .environment = environment,
        .candidates = candidates,
        .directory = plan.directory,
    };
}
