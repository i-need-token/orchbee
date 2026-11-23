//! Move requests on a Keeper link: `mark`, `resume`, `inspect` with the pull of the seven
//! duplicates or of a program's, `claim`, and `decline`. A macOS link's `inspect` reply carries the
//! master or a program's captured streams instead, and a claimed holding whose leader the Keeper
//! parents makes the link its parent. Their maker, the dispatch thread or a restore build, owns
//! each request and keeps it until the link completes it; the link thread makes the request, one
//! at a time with the holds and releases, and completes every request it has not made once the
//! link ends. A completion wakes the service loop, which polls; a build waits.
//!
//! The link's gate slots follow the holding: a `resume` answered `claimed` settles the old Host's
//! slot without a release, since the holding is the claimant's now, and a `claim`, or a `decline`
//! answered `claimed`, gives the claimant's terminal a held slot under the record's holding id.
//! `forget` settles a slot without a release before a drop, so no link keeps a gate the drop frees.

const std = @import("std");
const builtin = @import("builtin");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const link_core = @import("link.zig");

const message = keeper_protocol.message;
const thread_sync = platform.thread_sync;
const handles = platform.keeper.handles;
const is_windows = builtin.os.tag == .windows;
const is_macos = builtin.os.tag == .macos;
const supported = is_windows or is_macos;
const Gate = platform.keeper.Gate;
const Link = switch (builtin.os.tag) {
    .windows => @import("link_windows.zig").Link,
    .macos => @import("link_darwin.zig").Link,
    else => void,
};

pub const Body = union(enum) {
    mark: message.Mark,
    @"resume": struct { gate: *Gate, marked: message.Marked },
    inspect: message.Marked,
    /// `inspect` of a program holding, with the pull of the program's handles.
    inspect_program: message.Marked,
    claim: struct { gate: *Gate, claim: message.Claim },
    /// A program's recovery declines before it claims, so no claim of it can have applied.
    decline: struct { gate: ?*Gate, marked: message.Marked },
    /// Settles the terminal's slot without a release and sends nothing, for a terminal whose
    /// holding the Keeper gave to another Host or whose Keeper missed a deadline.
    forget: *Gate,
};

/// What `inspect` answered and the handles this Host pulled after it; on macOS, the PTY holding's
/// facts and the master its reply carried.
pub const Inspected = if (is_windows) WindowsInspected else PtyInspected;

pub const PtyInspected = struct {
    /// The terminal the holding last ran as.
    terminal: message.Terminal,
    /// The leader, as the Keeper checked it at its hold.
    program: message.Process,
    launch_kind: platform.process.LaunchKind,
    /// The master's duplicate; the request's maker owns it.
    master: std.posix.fd_t,
    slave: [message.pty.maximum_slave_bytes]u8,
    slave_length: usize,
    /// The Keeper parents the leader.
    parented: bool,
    /// The parented leader ended; the Keeper keeps it until the owner's `reap`.
    ended: bool,

    pub fn slaveName(self: *const PtyInspected) []const u8 {
        return self.slave[0..self.slave_length];
    }
};

const WindowsInspected = struct {
    /// The terminal the holding last ran as.
    terminal: message.Terminal,
    program: message.Process,
    console: message.Process,
    console_build: [32]u8,
    signal_format: u16,
    /// How the program was started; a name this Host does not know reads as `other`.
    launch_kind: platform.process.LaunchKind,
    /// This Host's duplicates in role order; the request's maker owns them.
    duplicates: if (is_windows) handles.Duplicates else void,
};

/// What `inspect` answered for a program holding and the handles this Host pulled after it.
pub const InspectedProgram = struct {
    process: message.Process,
    program_id: [16]u8,
    /// Allocated with the link's allocator; the request's maker frees it.
    label: []u8,
    /// This Host's duplicates; the request's maker owns them.
    duplicates: Duplicates,

    pub const Duplicates = if (is_windows) handles.ProgramDuplicates else PipesInspected;
};

pub const Process = message.Process;

/// A macOS program holding: the duplicate of each captured stream's read end, which the request's
/// maker owns, and the Keeper's parent state.
pub const PipesInspected = struct {
    streams: [2]?std.posix.fd_t,
    /// The Keeper parents the program.
    parented: bool,
    /// The parented program ended; the Keeper keeps it until its release.
    ended: bool,

    pub fn close(self: PipesInspected) void {
        for (self.streams) |stream| if (stream) |descriptor| platform.posix_compat.close(descriptor);
    }
};

pub const Result = union(enum) {
    /// `mark` or `claim` applied.
    done,
    /// `resume` or `decline` answered.
    outcome: message.Outcome,
    inspected: Inspected,
    inspected_program: InspectedProgram,
    refused: message.Reason,
    /// The Keeper does not know the request, or answered another one.
    unsupported,
    /// The request missed its deadline or ended the connection, so the link failed; the Keeper may
    /// have applied it.
    unanswered,
    /// The link failed or ended before the request was made; nothing was sent.
    failed,
    /// The Keeper ended while the request was made.
    keeper_ended,
    /// `inspect` was answered, but this Host could not pull the handles.
    pull_failed,
};

pub const Request = struct {
    body: Body,
    mutex: thread_sync.Mutex = .{},
    condition: thread_sync.Condition = .{},
    result: ?Result = null,

    pub fn init(body: Body) Request {
        return .{ .body = body };
    }

    /// The answer, once the link has completed the request.
    pub fn poll(self: *Request) ?Result {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.result;
    }

    /// Waits for the answer: within the Keeper's reply deadline once the link takes the request,
    /// and at once when the link ends.
    pub fn wait(self: *Request) Result {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.result == null) self.condition.wait(&self.mutex);
        return self.result.?;
    }

    /// The maker may free the request as soon as this stores the result.
    pub fn complete(self: *Request, result: Result) void {
        self.mutex.lock();
        self.result = result;
        self.condition.broadcast();
        self.mutex.unlock();
        platform.output_wake.signal();
    }
};

/// Queues `request` on `link`; false when the link no longer makes requests, and then the request
/// is already completed as `failed`.
pub fn submit(link: *Link, request: *Request) bool {
    if (comptime !supported) unreachable;
    {
        link.mutex.lock();
        defer link.mutex.unlock();
        const open = !link.finished and !link.detach_requested and !link.stop_requested and
            (link.phase == .attached or link.phase == .unresponsive);
        if (open) if (link.moves.append(link.allocator, request)) {
            link.wakeThread();
            return true;
        } else |_| {};
    }
    request.complete(.failed);
    return false;
}

/// Completes every queued request as `failed`; the link makes no more.
pub fn completeAll(link: *Link) void {
    if (comptime !supported) unreachable;
    var taken = taken: {
        link.mutex.lock();
        defer link.mutex.unlock();
        const queued = link.moves;
        link.moves = .empty;
        break :taken queued;
    };
    defer taken.deinit(link.allocator);
    for (taken.items) |request| request.complete(.failed);
}

/// Makes every queued request in order, on the link thread.
pub fn serve(link: *Link) void {
    if (comptime !supported) unreachable;
    while (next(link)) |request| request.complete(make(link, request.body));
}

fn next(link: *Link) ?*Request {
    link.mutex.lock();
    defer link.mutex.unlock();
    if (link.moves.items.len == 0) return null;
    return link.moves.orderedRemove(0);
}

fn make(link: *Link, body: Body) Result {
    switch (body) {
        .mark => |mark| return applied(callLink(link, .{ .mark = mark })),
        .@"resume" => |item| {
            const result = answered(callLink(link, .{ .@"resume" = item.marked }));
            if (claimedOutcome(result)) forget(link, item.gate);
            return result;
        },
        .inspect => |marked| {
            if (comptime is_macos) return inspectPty(link, marked);
            const reply = switch (callLink(link, .{ .inspect = marked })) {
                .reply => |reply| reply,
                else => |other| return unanswered(other),
            };
            const inspection = switch (reply) {
                .inspect => |inspection| inspection,
                .refused => |reason| return .{ .refused = reason },
                // A program holding answers a terminal's maker as a request it cannot serve.
                else => return .unsupported,
            };
            // Only scalar facts are read: the reply's text borrows the call's arena.
            const values: handles.Values = .{
                inspection.handles.job,    inspection.handles.program,   inspection.handles.console,
                inspection.handles.signal, inspection.handles.reference, inspection.handles.input,
                inspection.handles.output,
            };
            const duplicates = handles.pull(link.connection.?.keeper_process, values) catch return .pull_failed;
            return .{ .inspected = .{
                .terminal = inspection.facts.terminal,
                .launch_kind = launchKind(inspection.facts.launch),
                .program = inspection.facts.program,
                .console = inspection.facts.console,
                .console_build = inspection.facts.console_build,
                .signal_format = inspection.facts.signal_format,
                .duplicates = duplicates,
            } };
        },
        .inspect_program => |marked| {
            if (comptime is_macos) return inspectPipes(link, marked);
            if (comptime !is_windows) unreachable;
            const reply = switch (callLink(link, .{ .inspect = marked })) {
                .reply => |reply| reply,
                else => |other| return unanswered(other),
            };
            const inspection = switch (reply) {
                .inspect_program => |inspection| inspection,
                .refused => |reason| return .{ .refused = reason },
                else => return .unsupported,
            };
            // The label borrows the call's arena, so it is copied before the pull.
            const label = link.allocator.dupe(u8, inspection.facts.label) catch return .pull_failed;
            const duplicates = handles.pullProgram(link.connection.?.keeper_process, .{
                .job = inspection.handles.job,
                .process = inspection.handles.process,
                .streams = .{ inspection.handles.stdout, inspection.handles.stderr },
            }) catch {
                link.allocator.free(label);
                return .pull_failed;
            };
            return .{ .inspected_program = .{
                .process = inspection.facts.process,
                .program_id = inspection.facts.program_id,
                .label = label,
                .duplicates = duplicates,
            } };
        },
        .claim => |item| {
            // The adopted slot must be tracked, so the room is taken before the claim is sent.
            link.held.ensureUnusedCapacity(link.allocator, 1) catch return .failed;
            const result = applied(callLink(link, .{ .claim = item.claim }));
            if (result == .done) adopt(link, item.gate, item.claim.holding);
            return result;
        },
        .forget => |gate| {
            forget(link, gate);
            return .done;
        },
        .decline => |item| {
            link.held.ensureUnusedCapacity(link.allocator, 1) catch return .failed;
            const result = answered(callLink(link, .{ .decline = item.marked }));
            if (claimedOutcome(result)) if (item.gate) |gate| adopt(link, gate, item.marked.holding);
            return result;
        },
    }
}

/// One request on the link thread; a macOS link's requests here carry no descriptor.
fn callLink(link: *Link, body: message.Request.Body) link_core.Outcome {
    return link.call(body);
}

/// `inspect` of a PTY holding on macOS: the reply's facts, and the master it carried.
fn inspectPty(link: *Link, marked: message.Marked) Result {
    const reply = switch (callLink(link, .{ .inspect = marked })) {
        .reply => |reply| reply,
        else => |other| return unanswered(other),
    };
    const received = link.connection.?.takeReceived();
    const master = received[0];
    const inspection = switch (reply) {
        .inspect_pty => |inspection| inspection,
        .refused => |reason| return .{ .refused = reason },
        // A program holding answers a terminal's maker as a request it cannot serve.
        else => {
            closeReceived(received);
            return .unsupported;
        },
    };
    const facts = inspection.facts;
    const descriptor = master orelse return .pull_failed;
    if (facts.slave.len > message.pty.maximum_slave_bytes) {
        platform.posix_compat.close(descriptor);
        return .pull_failed;
    }
    // The slave's name borrows the call's arena, so it is copied.
    var inspected: PtyInspected = .{
        .terminal = facts.terminal,
        .program = facts.leader,
        .launch_kind = launchKind(facts.launch),
        .master = descriptor,
        .slave = undefined,
        .slave_length = facts.slave.len,
        .parented = inspection.parented,
        .ended = inspection.ended != null,
    };
    @memcpy(inspected.slave[0..facts.slave.len], facts.slave);
    return .{ .inspected = inspected };
}

/// `inspect` of a program holding on macOS: the reply's facts, and the read end of each captured
/// stream it carried, stdout first.
fn inspectPipes(link: *Link, marked: message.Marked) Result {
    const reply = switch (callLink(link, .{ .inspect = marked })) {
        .reply => |reply| reply,
        else => |other| return unanswered(other),
    };
    const received = link.connection.?.takeReceived();
    const inspection = switch (reply) {
        .inspect_pipes => |inspection| inspection,
        .refused => |reason| return .{ .refused = reason },
        else => {
            closeReceived(received);
            return .unsupported;
        },
    };
    // Each captured stream's read end must have arrived; a reply whose descriptors were lost
    // carries none.
    const captures = [_]bool{ inspection.capture.stdout, inspection.capture.stderr };
    const count = @as(usize, @intFromBool(captures[0])) + @intFromBool(captures[1]);
    if (count > 0 and received[count - 1] == null) return .pull_failed;
    var pipes: PipesInspected = .{ .streams = .{ null, null }, .parented = inspection.parented, .ended = inspection.ended != null };
    var at: usize = 0;
    for (captures, &pipes.streams) |captured, *stream| if (captured) {
        stream.* = received[at];
        at += 1;
    };
    // The label borrows the call's arena, so it is copied.
    const label = link.allocator.dupe(u8, inspection.facts.label) catch {
        pipes.close();
        return .pull_failed;
    };
    return .{ .inspected_program = .{
        .process = inspection.facts.process,
        .program_id = inspection.facts.program_id,
        .label = label,
        .duplicates = pipes,
    } };
}

fn closeReceived(received: [2]?std.posix.fd_t) void {
    for (received) |named| if (named) |descriptor| platform.posix_compat.close(descriptor);
}

/// The launch name `hold` reported, such as `managed-powershell`, as this Host's kind.
fn launchKind(name: []const u8) platform.process.LaunchKind {
    inline for (@typeInfo(platform.process.LaunchKind).@"enum".fields) |field| {
        const spelled = comptime spelled: {
            var text: [field.name.len]u8 = undefined;
            for (field.name, &text) |byte, *out| out.* = if (byte == '_') '-' else byte;
            const final = text;
            break :spelled &final;
        };
        if (std.mem.eql(u8, name, spelled)) return @field(platform.process.LaunchKind, field.name);
    }
    return .other;
}

fn claimedOutcome(result: Result) bool {
    return switch (result) {
        .outcome => |outcome| outcome == .claimed,
        else => false,
    };
}

/// A `mark` or `claim` answer.
fn applied(outcome: link_core.Outcome) Result {
    const reply = switch (outcome) {
        .reply => |reply| reply,
        else => |other| return unanswered(other),
    };
    return switch (reply) {
        .mark, .claim => .done,
        .refused => |reason| .{ .refused = reason },
        else => .unsupported,
    };
}

/// A `resume` or `decline` answer.
fn answered(outcome: link_core.Outcome) Result {
    const reply = switch (outcome) {
        .reply => |reply| reply,
        else => |other| return unanswered(other),
    };
    return switch (reply) {
        .@"resume", .decline => |value| .{ .outcome = value },
        .refused => |reason| .{ .refused = reason },
        else => .unsupported,
    };
}

fn unanswered(outcome: link_core.Outcome) Result {
    return switch (outcome) {
        .reply => unreachable,
        .unanswered => .unanswered,
        .failed => .failed,
        .keeper_ended => .keeper_ended,
    };
}

/// The holding moved to its claimant: the slot settles without a release.
fn forget(link: *Link, gate: *Gate) void {
    for (link.held.items, 0..) |entry, index| if (entry.gate == gate) {
        _ = link.held.swapRemove(index);
        gate.settle(link.index);
        return;
    };
}

fn adopt(link: *Link, gate: *Gate, holding: [16]u8) void {
    gate.adopt(link.index, link.serial, link.wake, holding);
    link.held.appendAssumeCapacity(.{ .gate = gate, .holding = holding });
    // A claimed leader the Keeper parents is reaped through this link from now on.
    if (comptime is_macos) {
        const record: *platform.keeper.parent.Record = @fieldParentPtr("gate", gate);
        if (record.claims_parent) record.parentedBy(link.serial, record.leader_created);
    }
}
