const std = @import("std");
const builtin = @import("builtin");
const monotonic = @import("monotonic.zig");
const thread_sync = @import("thread_sync.zig");

extern "c" fn orchbee_prepare_process_tracking() c_int;
extern "c" fn orchbee_session_member_iterator_open(
    session_id: c_int,
    iterator: *?*anyopaque,
) c_int;
extern "c" fn orchbee_session_member_iterator_close(iterator: ?*anyopaque) void;
extern "c" fn orchbee_session_member_iterator_next(
    iterator: *anyopaque,
    member_ref: *?*anyopaque,
) c_int;
extern "c" fn orchbee_session_member_ref_close(member_ref: ?*anyopaque) void;
extern "c" fn orchbee_session_member_ref_signal(
    member_ref: *anyopaque,
    signal_number: c_int,
) c_int;
extern "c" fn orchbee_session_member_ref_is_live(
    member_ref: *anyopaque,
    live: *c_int,
) c_int;
extern "c" fn orchbee_signal_process_session(
    session_id: c_int,
    signal_number: c_int,
) c_int;
extern "c" fn orchbee_process_session_member_pass(
    session_id: c_int,
    live_members: *u32,
    targeted_zombies_remain: *c_int,
) c_int;
extern "c" fn orchbee_reap_process_exit(process_id: c_int) c_int;

/// Serializes stable-member signaling, containment confirmation, and leader
/// identity release at the POSIX process-identity owner (THCR-006). The numeric
/// session identifier is consulted only while the launch leader is retained.
/// Each macOS/Linux member is bound to a stable kernel reference before its
/// authoritative numeric membership observation; macOS may first use the
/// reusable PID only as a negative prefilter. FreeBSD effects use the
/// reaper-owned direct-child subtree. Neither path can retarget a later process
/// that inherited a PID.
pub const SessionIdentityState = if (builtin.os.tag == .windows)
    struct {}
else
    struct {
        mutex: thread_sync.Mutex = .{},
        released: std.atomic.Value(bool) = .init(false),
        signals_issued: std.atomic.Value(u64) = .init(0),

        pub fn signal(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            signaler: anytype,
            signal_number: std.posix.SIG,
        ) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return;
            _ = self.signals_issued.fetchAdd(1, .monotonic);
            try signaler.signal(session_id, signal_number);
        }

        pub fn forceTerminate(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            signaler: anytype,
        ) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return;
            _ = self.signals_issued.fetchAdd(1, .monotonic);
            try forceTerminateSession(session_id, signaler);
        }

        pub fn contained(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            query: anytype,
        ) !bool {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return true;
            var count: u32 = 0;
            try query.liveMembers(session_id, &count);
            return count == 0;
        }

        pub fn release(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            reaper: anytype,
        ) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return;
            try reaper.reap(session_id);
            self.released.store(true, .release);
        }

        pub fn isReleased(self: *const SessionIdentityState) bool {
            return self.released.load(.acquire);
        }
    };

pub fn signalStableSessionMembers(
    session_id: std.posix.pid_t,
    source: anytype,
    signal_number: std.posix.SIG,
) !void {
    var iterator = try source.open(session_id);
    defer iterator.deinit();

    var first_error: ?anyerror = null;
    while (true) {
        const captured_member = iterator.next() catch |err| {
            if (first_error == null) first_error = err;
            break;
        } orelse break;
        {
            var member = captured_member;
            defer member.deinit();
            member.signal(signal_number) catch |err| {
                if (first_error == null) first_error = err;
            };
        }
    }
    if (first_error) |err| return err;
}

pub fn countStableSessionMembers(
    session_id: std.posix.pid_t,
    source: anytype,
) !u32 {
    var iterator = try source.open(session_id);
    defer iterator.deinit();
    var count: u32 = 0;
    while (try iterator.next()) |captured_member| {
        var member = captured_member;
        defer member.deinit();
        if (!try member.isLive()) continue;
        if (count == std.math.maxInt(u32)) return error.ProcessContainmentQueryFailed;
        count += 1;
    }
    return count;
}

const NativeSessionMemberSource = struct {
    fn open(_: @This(), session_id: std.posix.pid_t) !NativeSessionMemberIterator {
        var raw: ?*anyopaque = null;
        if (orchbee_session_member_iterator_open(session_id, &raw) != 0) {
            return error.ProcessSessionObservationFailed;
        }
        return .{ .raw = raw orelse return error.ProcessSessionObservationFailed };
    }
};

const NativeSessionMemberIterator = struct {
    raw: *anyopaque,

    fn deinit(self: *@This()) void {
        orchbee_session_member_iterator_close(self.raw);
        self.* = undefined;
    }

    fn next(self: *@This()) !?NativeSessionMemberRef {
        var raw: ?*anyopaque = null;
        const result = orchbee_session_member_iterator_next(self.raw, &raw);
        if (result < 0) return error.ProcessSessionObservationFailed;
        if (result == 0) return null;
        return .{ .raw = raw orelse return error.ProcessSessionObservationFailed };
    }
};

const NativeSessionMemberRef = struct {
    raw: *anyopaque,

    fn deinit(self: *@This()) void {
        orchbee_session_member_ref_close(self.raw);
        self.* = undefined;
    }

    fn signal(self: *@This(), signal_number: std.posix.SIG) !void {
        if (orchbee_session_member_ref_signal(
            self.raw,
            @intCast(@intFromEnum(signal_number)),
        ) != 0) {
            return error.SessionTerminationFailed;
        }
    }

    fn isLive(self: *@This()) !bool {
        var live: c_int = 0;
        if (orchbee_session_member_ref_is_live(self.raw, &live) != 0) {
            return error.ProcessContainmentQueryFailed;
        }
        return live != 0;
    }
};

// FreeBSD process-reaper snapshots identify descendant zombies that this Host
// now owns as direct children. Serialize those targeted waits with retained
// leader waits across terminals so a snapshot PID cannot be released and reused
// by another terminal between ownership classification and waitpid.
var freebsd_reap_mutex: thread_sync.Mutex = .{};

const freebsd_reaper_max_passes = 8;

const FreebsdReaperPass = struct {
    live_members: u32,
    targeted_zombies_remain: bool,
};

const NativeFreebsdReaperQuery = struct {
    fn observe(_: @This(), session_id: std.posix.pid_t) !FreebsdReaperPass {
        var live_members: u32 = 0;
        var targeted_zombies_remain: c_int = 0;
        if (orchbee_process_session_member_pass(
            session_id,
            &live_members,
            &targeted_zombies_remain,
        ) != 0) return error.ProcessContainmentQueryFailed;
        return .{
            .live_members = live_members,
            .targeted_zombies_remain = targeted_zombies_remain != 0,
        };
    }
};

/// A FreeBSD pass snapshots the reaper subtree, reaps only zombies classified
/// as Host direct children in that snapshot, and then returns a fresh snapshot
/// observation. A fresh live member is conservatively non-contained. Fresh
/// zombies require another pass, so one that changed from live to zombie after
/// the prior reap cannot be mistaken for an empty subtree. Persistent churn is
/// bounded and fails without releasing the retained leader identity.
pub fn countFreebsdReaperMembers(
    session_id: std.posix.pid_t,
    query: anytype,
) !u32 {
    for (0..freebsd_reaper_max_passes) |_| {
        const observation = try query.observe(session_id);
        if (observation.live_members != 0) return observation.live_members;
        if (!observation.targeted_zombies_remain) return 0;
    }
    return error.ProcessContainmentQueryFailed;
}

/// STOP narrows the race in which a descendant can create another session
/// member while the platform enumerates it. A partial STOP failure must not
/// leave already-stopped members frozen forever.
pub fn forceTerminateSession(session_id: std.posix.pid_t, signaler: anytype) !void {
    signaler.signal(session_id, std.posix.SIG.STOP) catch {};
    signaler.signal(session_id, std.posix.SIG.KILL) catch |err| {
        signaler.signal(session_id, std.posix.SIG.CONT) catch {};
        return err;
    };
}

fn signalSession(session_id: std.posix.pid_t, signal_number: std.posix.SIG) !void {
    switch (builtin.os.tag) {
        .freebsd => if (orchbee_signal_process_session(
            session_id,
            @intCast(@intFromEnum(signal_number)),
        ) != 0) {
            return error.SessionTerminationFailed;
        },
        .linux, .macos => try signalStableSessionMembers(
            session_id,
            NativeSessionMemberSource{},
            signal_number,
        ),
        else => unreachable,
    }
}

pub fn sessionLiveMembers(session_id: std.posix.pid_t) !u32 {
    switch (builtin.os.tag) {
        .freebsd => {
            freebsd_reap_mutex.lock();
            defer freebsd_reap_mutex.unlock();
            return try countFreebsdReaperMembers(
                session_id,
                NativeFreebsdReaperQuery{},
            );
        },
        .linux, .macos => return try countStableSessionMembers(
            session_id,
            NativeSessionMemberSource{},
        ),
        else => unreachable,
    }
}

pub const NativeSessionSignaler = struct {
    pub fn signal(_: @This(), session_id: std.posix.pid_t, signal_number: std.posix.SIG) !void {
        try signalSession(session_id, signal_number);
    }
};

pub const NativeSessionQuery = struct {
    pub fn liveMembers(_: @This(), session_id: std.posix.pid_t, count: *u32) !void {
        count.* = try sessionLiveMembers(session_id);
    }
};

pub const NativeSessionReaper = struct {
    pub fn reap(_: @This(), session_id: std.posix.pid_t) !void {
        if (comptime builtin.os.tag == .freebsd) {
            freebsd_reap_mutex.lock();
            defer freebsd_reap_mutex.unlock();
        }
        const result = orchbee_reap_process_exit(session_id);
        if (result == @intFromEnum(std.posix.E.AGAIN)) return error.ProcessReapPending;
        if (result != 0) return error.ProcessReapFailed;
    }
};

const observed_exit_reap_retry_limit = 1000;
const observed_exit_reap_retry_delay_ns = std.time.ns_per_ms;

const NativeReapDelay = struct {
    fn pause(_: @This()) void {
        monotonic.sleep(observed_exit_reap_retry_delay_ns);
    }
};

/// A prior non-reaping exit observation proves that this is not a wait for a
/// running child. macOS can still expose a narrow interval in which waitid has
/// reported that exit but waitpid(WNOHANG) cannot reap it yet, so retry only
/// that pending result within a fixed bound while the identity fence is held.
pub fn reapObservedProcessExit(
    session_id: std.posix.pid_t,
    reaper: anytype,
    delay: anytype,
) !void {
    for (0..observed_exit_reap_retry_limit) |attempt| {
        reaper.reap(session_id) catch |err| switch (err) {
            error.ProcessReapPending => {
                if (attempt + 1 == observed_exit_reap_retry_limit) return err;
                delay.pause();
                continue;
            },
            else => return err,
        };
        return;
    }
    unreachable;
}

pub const NativeObservedExitReaper = struct {
    pub fn reap(_: @This(), session_id: std.posix.pid_t) !void {
        try reapObservedProcessExit(
            session_id,
            NativeSessionReaper{},
            NativeReapDelay{},
        );
    }
};

pub fn prepareProcessTracking() !void {
    if (orchbee_prepare_process_tracking() != 0) {
        return error.ProcessTrackingUnavailable;
    }
}
