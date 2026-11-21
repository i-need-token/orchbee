//! Move requests on a Keeper link: `mark`, `resume`, `inspect` with the pull of the seven
//! duplicates, `claim`, and `decline`. Their maker, the dispatch thread or a restore build, owns
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

const message = keeper_protocol.message;
const thread_sync = platform.thread_sync;
const handles = platform.keeper.handles;
const is_windows = builtin.os.tag == .windows;
const Gate = platform.keeper.Gate;
const Link = if (is_windows) @import("link_windows.zig").Link else void;

pub const Body = union(enum) {
    mark: message.Mark,
    @"resume": struct { gate: *Gate, marked: message.Marked },
    inspect: message.Marked,
    claim: struct { gate: *Gate, claim: message.Claim },
    decline: struct { gate: *Gate, marked: message.Marked },
    /// Settles the terminal's slot without a release and sends nothing, for a terminal whose
    /// holding the Keeper gave to another Host or whose Keeper missed a deadline.
    forget: *Gate,
};

/// What `inspect` answered and the handles this Host pulled after it.
pub const Inspected = struct {
    program: message.Process,
    console: message.Process,
    console_build: [32]u8,
    signal_format: u16,
    /// This Host's duplicates in role order; the request's maker owns them.
    duplicates: if (is_windows) handles.Duplicates else void,
};

pub const Result = union(enum) {
    /// `mark` or `claim` applied.
    done,
    /// `resume` or `decline` answered.
    outcome: message.Outcome,
    inspected: Inspected,
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
    if (comptime !is_windows) unreachable;
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
    if (comptime !is_windows) unreachable;
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
    if (comptime !is_windows) unreachable;
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
        .mark => |mark| return applied(link.call(.{ .mark = mark })),
        .@"resume" => |item| {
            const result = answered(link.call(.{ .@"resume" = item.marked }));
            if (claimedOutcome(result)) forget(link, item.gate);
            return result;
        },
        .inspect => |marked| {
            const reply = switch (link.call(.{ .inspect = marked })) {
                .reply => |reply| reply,
                else => |other| return unanswered(other),
            };
            const inspection = switch (reply) {
                .inspect => |inspection| inspection,
                .refused => |reason| return .{ .refused = reason },
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
                .program = inspection.facts.program,
                .console = inspection.facts.console,
                .console_build = inspection.facts.console_build,
                .signal_format = inspection.facts.signal_format,
                .duplicates = duplicates,
            } };
        },
        .claim => |item| {
            // The adopted slot must be tracked, so the room is taken before the claim is sent.
            link.held.ensureUnusedCapacity(link.allocator, 1) catch return .failed;
            const result = applied(link.call(.{ .claim = item.claim }));
            if (result == .done) adopt(link, item.gate, item.claim.holding);
            return result;
        },
        .forget => |gate| {
            forget(link, gate);
            return .done;
        },
        .decline => |item| {
            link.held.ensureUnusedCapacity(link.allocator, 1) catch return .failed;
            const result = answered(link.call(.{ .decline = item.marked }));
            if (claimedOutcome(result)) adopt(link, item.gate, item.marked.holding);
            return result;
        },
    }
}

fn claimedOutcome(result: Result) bool {
    return switch (result) {
        .outcome => |outcome| outcome == .claimed,
        else => false,
    };
}

/// A `mark` or `claim` answer.
fn applied(outcome: Link.Outcome) Result {
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
fn answered(outcome: Link.Outcome) Result {
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

fn unanswered(outcome: Link.Outcome) Result {
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
}
