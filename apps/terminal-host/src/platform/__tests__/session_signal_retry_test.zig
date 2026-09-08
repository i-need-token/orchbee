const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const signaling = platform.testing.session_signaling;

const Scenario = enum {
    owned_replacement,
    foreign_replacement,
    persistent_invalidation,
    invalid_then_denied,
    denied_then_invalid,
    invalid_then_iterator_failure,
    resource_failure,
};

const State = struct {
    scenario: Scenario,
    opens: usize = 0,
    iterator_closes: usize = 0,
    acquired: usize = 0,
    released: usize = 0,
    attempts: usize = 0,
    owned_replacement_effects: usize = 0,
    foreign_membership_checks: usize = 0,
};

const Source = struct {
    state: *State,

    pub fn open(self: @This(), _: std.posix.pid_t) !Iterator {
        // Every old reference and iterator must close before fresh acquisition.
        try std.testing.expectEqual(self.state.acquired, self.state.released);
        try std.testing.expectEqual(self.state.opens, self.state.iterator_closes);
        self.state.opens += 1;
        return .{ .state = self.state };
    }
};

const Iterator = struct {
    state: *State,
    cursor: usize = 0,

    pub fn deinit(self: *@This()) void {
        self.state.iterator_closes += 1;
    }

    pub fn next(self: *@This()) !?Member {
        const state = self.state;
        if (state.opens > 1 and state.scenario == .foreign_replacement) {
            state.foreign_membership_checks += 1;
            return null;
        }
        const count: usize = switch (state.scenario) {
            .invalid_then_denied, .denied_then_invalid, .invalid_then_iterator_failure => 2,
            else => 1,
        };
        if (self.cursor == count) return null;
        if (self.cursor == 1 and state.scenario == .invalid_then_iterator_failure)
            return error.ProcessSessionObservationFailed;
        const member: Member = .{ .state = state, .index = self.cursor, .generation = state.opens };
        self.cursor += 1;
        state.acquired += 1;
        return member;
    }
};

const Member = struct {
    state: *State,
    index: usize,
    generation: usize,

    pub fn deinit(self: *@This()) void {
        self.state.released += 1;
    }

    pub fn signal(self: *@This(), _: std.posix.SIG) !void {
        const state = self.state;
        state.attempts += 1;
        // Captured generation one is invalid. No effect may use its numeric PID
        // to reach a replacement; only a newly acquired generation may act.
        switch (state.scenario) {
            .owned_replacement, .foreign_replacement => {
                if (self.generation == 1) return error.StableMemberIdentityInvalidated;
                try std.testing.expectEqual(@as(usize, 2), self.generation);
                state.owned_replacement_effects += 1;
            },
            .persistent_invalidation => return error.StableMemberIdentityInvalidated,
            .invalid_then_denied => {
                if (self.index == 0) return error.StableMemberIdentityInvalidated;
                return error.AccessDenied;
            },
            .denied_then_invalid => {
                if (self.index == 0) return error.AccessDenied;
                return error.StableMemberIdentityInvalidated;
            },
            .invalid_then_iterator_failure => return error.StableMemberIdentityInvalidated,
            .resource_failure => return error.OutOfMemory,
        }
    }
};

fn signal(state: *State) !void {
    try signaling.signalWithRetry(4242, Source{ .state = state }, std.posix.SIG.HUP);
}

fn expectReleased(state: *const State) !void {
    try std.testing.expectEqual(state.acquired, state.released);
    try std.testing.expectEqual(state.opens, state.iterator_closes);
}

test "POSIX signal retry reacquires an owned replacement before its effect" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var state: State = .{ .scenario = .owned_replacement };
    try signal(&state);
    try std.testing.expectEqual(@as(usize, 2), state.opens);
    try std.testing.expectEqual(@as(usize, 1), state.owned_replacement_effects);
    try expectReleased(&state);
}

test "POSIX signal retry checks fresh membership and never targets a foreign replacement" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var state: State = .{ .scenario = .foreign_replacement };
    try signal(&state);
    try std.testing.expectEqual(@as(usize, 2), state.opens);
    try std.testing.expectEqual(@as(usize, 1), state.foreign_membership_checks);
    try std.testing.expectEqual(@as(usize, 1), state.attempts);
    try std.testing.expectEqual(@as(usize, 0), state.owned_replacement_effects);
    try expectReleased(&state);
}

test "POSIX signal retry exhausts eight fresh scans without claiming success" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var state: State = .{ .scenario = .persistent_invalidation };
    try std.testing.expectError(error.StableMemberIdentityInvalidated, signal(&state));
    try std.testing.expectEqual(@as(usize, 8), state.opens);
    try expectReleased(&state);
}

test "POSIX signal retry preserves permanent member errors in either encounter order" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    for ([_]Scenario{ .invalid_then_denied, .denied_then_invalid }) |scenario| {
        var state: State = .{ .scenario = scenario };
        try std.testing.expectError(error.AccessDenied, signal(&state));
        try std.testing.expectEqual(@as(usize, 1), state.opens);
        try std.testing.expectEqual(@as(usize, 2), state.attempts);
        try expectReleased(&state);
    }
}

test "POSIX signal retry preserves an iterator failure after member invalidation" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var state: State = .{ .scenario = .invalid_then_iterator_failure };
    try std.testing.expectError(error.ProcessSessionObservationFailed, signal(&state));
    try std.testing.expectEqual(@as(usize, 1), state.opens);
    try expectReleased(&state);
}

test "POSIX signal retry returns resource failure without another scan" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var state: State = .{ .scenario = .resource_failure };
    try std.testing.expectError(error.OutOfMemory, signal(&state));
    try std.testing.expectEqual(@as(usize, 1), state.opens);
    try expectReleased(&state);
}

test "POSIX force retry exhaustion still completes CONT compensation" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const Signaler = struct {
        state: *State,
        requested: [3]std.posix.SIG = undefined,
        count: usize = 0,

        pub fn signal(self: *@This(), session_id: std.posix.pid_t, number: std.posix.SIG) !void {
            self.requested[self.count] = number;
            self.count += 1;
            try signaling.signalWithRetry(session_id, Source{ .state = self.state }, number);
        }
    };
    var state: State = .{ .scenario = .persistent_invalidation };
    var signaler: Signaler = .{ .state = &state };
    try std.testing.expectError(
        error.StableMemberIdentityInvalidated,
        platform.process.testing.PosixProcessType.forceTerminateSession(4242, &signaler),
    );
    try std.testing.expectEqualSlices(std.posix.SIG, &.{ .STOP, .KILL, .CONT }, &signaler.requested);
    try std.testing.expectEqual(@as(usize, 24), state.opens);
    try expectReleased(&state);
}
