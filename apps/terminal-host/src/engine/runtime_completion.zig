const std = @import("std");

const pty_process = @import("terminal-host-platform").process;

pub const FailureKind = enum(u8) {
    process_observation = 1,
    pty_read = 2,
    pty_write = 3,
    resize = 4,
    terminal_reply = 5,
    worker_integrity = 6,
};

pub const State = union(enum) {
    pending,
    normal,
    failed: FailureKind,
};

const normal_code = std.math.maxInt(u8);

/// One atomic terminal classification makes normal completion and fatal
/// failure mutually exclusive. The first terminal outcome is immutable.
pub const Classification = struct {
    code: std.atomic.Value(u8) = .init(0),

    pub fn load(self: *const Classification) State {
        const code = self.code.load(.acquire);
        if (code == 0) return .pending;
        if (code == normal_code) return .normal;
        return .{ .failed = @enumFromInt(code) };
    }

    pub fn markFailure(self: *Classification, failure: FailureKind) void {
        var observed = self.code.load(.acquire);
        while (observed == 0) {
            observed = self.code.cmpxchgWeak(
                observed,
                @intFromEnum(failure),
                .acq_rel,
                .acquire,
            ) orelse return;
        }
    }

    pub fn markNormal(self: *Classification) bool {
        return self.code.cmpxchgStrong(
            0,
            normal_code,
            .acq_rel,
            .acquire,
        ) == null;
    }
};

pub const Snapshot = struct {
    child_exited: bool,
    classification: State = .pending,
    exit_result: ?pty_process.ExitResult,
    final_revision: u64,
    pty_drained: bool,
    writer_stopped: bool = false,
};
