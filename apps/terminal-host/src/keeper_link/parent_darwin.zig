//! The requests a macOS Keeper link makes as a leader's parent: the `spawn` a terminal's creation
//! hands it and the `spawn-program` of a background program, the `reap` a terminal asks for once
//! its leader ended, and the `exited` the Keeper sends. Each runs on the link thread; the thread
//! that hands it a start only waits for its call.

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

/// A terminal's or a program's start, which the thread that handed it waits for; the link
/// completes every one.
pub const SpawnCall = struct {
    subject: union(enum) {
        terminal: struct { plan: *const parent_module.Plan, terminal: message.Terminal },
        program: *const parent_module.ProgramPlan,
    },
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

/// Asks the Keeper to start `call`'s leader. A started leader's terminal or program is held here
/// at once, under a holding id this link mints.
pub fn spawn(self: *Link, call: *SpawnCall) parent_module.Outcome {
    self.held.ensureUnusedCapacity(self.allocator, 1) catch return .refused;
    var arena_state = std.heap.ArenaAllocator.init(self.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    var holding: [16]u8 = undefined;
    thread_sync.io().random(&holding);
    holding[0] |= 1;
    var descriptors: [4]posix.fd_t = undefined;
    var count: usize = 0;
    const record, const body: message.Request.Body = switch (call.subject) {
        .terminal => |terminal| request: {
            const plan = terminal.plan;
            const slave = plan.record.slaveName() orelse return .refused;
            const launch = launchOf(arena, plan.executable, plan.argv, plan.environment, plan.candidates, plan.directory) catch return .refused;
            if (!message.pty.validLaunch(launch)) return .refused;
            for ([_]?posix.fd_t{ plan.master, plan.slave, plan.input }) |sent| if (sent) |descriptor| {
                descriptors[count] = descriptor;
                count += 1;
            };
            break :request .{ plan.record, .{ .spawn = .{
                .holding = holding,
                .input = plan.input != null,
                .launch = launch,
                .facts = .{ .terminal = terminal.terminal, .slave = slave, .launch = plan.launch_kind.wireName() },
            } } };
        },
        .program => |plan| request: {
            const launch = launchOf(arena, plan.executable, plan.argv, plan.environment, &.{}, plan.directory) catch return .refused;
            if (!message.pty.validLaunch(launch) or !message.validLabel(plan.label)) return .refused;
            for (plan.pipes) |pair| if (pair) |ends| {
                descriptors[count] = ends[0];
                descriptors[count + 1] = ends[1];
                count += 2;
            };
            break :request .{ plan.record, .{ .spawn_program = .{
                .holding = holding,
                .launch = launch,
                .facts = .{ .program_id = plan.program_id, .label = plan.label },
                .capture = .{ .stdout = plan.pipes[0] != null, .stderr = plan.pipes[1] != null },
            } } };
        },
    };
    const reply = switch (self.callSending(body, descriptors[0..count])) {
        .reply => |reply| reply,
        .failed => return .refused,
        .unanswered, .keeper_ended => return .unanswered,
    };
    const leader = switch (reply) {
        .spawn => |leader| if (body == .spawn) leader else return .refused,
        .spawn_program => |leader| if (body == .spawn_program) leader else return .refused,
        .refused => |reason| return if (reason == .spawn_failed) .spawn_failed else .refused,
        else => return .refused,
    };
    record.parentedBy(self.serial, leader.created);
    record.gate.adopt(self.index, self.serial, self.wake, holding);
    self.held.appendAssumeCapacity(.{ .gate = &record.gate, .holding = holding });
    return .{ .started = @intCast(leader.pid) };
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

/// The launch a start names, read from the Host's prepared plan: the arguments after the first.
fn launchOf(
    arena: std.mem.Allocator,
    executable: []const u8,
    argv_pointer: [*:null]const ?[*:0]const u8,
    environment_pointer: [*:null]const ?[*:0]const u8,
    candidate_pointers: []const [*:0]const u8,
    directory: ?[]const u8,
) !message.pty.Launch {
    const argv = std.mem.span(argv_pointer);
    const arguments = try arena.alloc([]const u8, argv.len -| 1);
    for (arguments, argv[@min(1, argv.len)..]) |*argument, entry| argument.* = std.mem.span(entry.?);
    const entries = std.mem.span(environment_pointer);
    const environment = try arena.alloc([]const u8, entries.len);
    for (environment, entries) |*entry, value| entry.* = std.mem.span(value.?);
    const candidates = try arena.alloc([]const u8, candidate_pointers.len);
    for (candidates, candidate_pointers) |*candidate, value| candidate.* = std.mem.span(value);
    return .{
        .executable = executable,
        .arguments = arguments,
        .environment = environment,
        .candidates = candidates,
        .directory = directory,
    };
}
