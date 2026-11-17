//! The reader's undoable stop, which only a quiesce requests. A pause stops the platform read;
//! the reader applies what the read returned and parks only after a batch that leaves Ghostty's
//! parser, its UTF-8 decoder and the Host's scanners all at ground, so a resume reads on with
//! nothing lost or repeated.

const monotonic = @import("terminal-host-platform").monotonic;
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const failure_ops = @import("failure.zig");
const process_io_ops = @import("process_io.zig");

pub const Settled = enum {
    /// Read on, stopping at a standing pause.
    read_on,
    /// Read through a standing pause: a parser or scanner is inside a sequence.
    read_through,
    /// The reader ends: abort or failure.
    stopped,
};

/// Runs on the reader after a batch while a pause is pending, and after a read a pause stopped.
/// At ground it publishes that it is parked and waits, under the terminal's lock, for resume or
/// abort.
pub fn settle(self: *state.State) Settled {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    if (self.reader_pause == .none) return .read_on;
    if (!atGroundLocked(self)) {
        self.reader_pause = .reading_on;
        return .read_through;
    }
    self.reader_pause = .parked;
    self.terminal_condition.broadcast();
    while (self.reader_pause == .parked and !stoppedLocked(self)) {
        self.terminal_condition.wait(&self.terminal_mutex);
    }
    return if (stoppedLocked(self)) .stopped else .read_on;
}

/// Requests the stop and stops the platform read. When the platform cannot, the pause is
/// withdrawn and the reader reads on.
pub fn pause(self: *state.State) !void {
    {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        if (self.reader_pause != .none) return;
        self.reader_pause = .requested;
        self.reader_pause_pending.store(true, .release);
    }
    process_io_ops.pauseOutputRead(self) catch |err| {
        withdraw(self);
        return err;
    };
}

/// Waits up to `timeout_ns` for the reader to park.
pub fn awaitParked(self: *state.State, timeout_ns: u64) contracts.ReaderParkWait {
    var timer = monotonic.Timer.start() catch null;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    while (true) {
        if (stoppedLocked(self) or self.pty_drained.load(.acquire)) return .stopped;
        if (self.reader_pause == .parked) return .parked;
        const elapsed = if (timer) |*value| value.read() else timeout_ns;
        const remaining = if (elapsed < timeout_ns) timeout_ns - elapsed else 0;
        if (remaining == 0) {
            return if (self.reader_pause == .reading_on) .mid_sequence else .timed_out;
        }
        self.terminal_condition.timedWait(&self.terminal_mutex, remaining) catch {};
    }
}

/// Resumes a parked reader, or withdraws a pause it has not reached. A pause the platform cannot
/// withdraw would stop the reader for good, so the terminal fails instead.
pub fn resumeReader(self: *state.State) !void {
    process_io_ops.resumeOutputRead(self) catch |err| {
        withdraw(self);
        failure_ops.recordReaderFailure(self);
        return err;
    };
    withdraw(self);
}

fn withdraw(self: *state.State) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    self.reader_pause = .none;
    self.reader_pause_pending.store(false, .release);
    self.terminal_condition.broadcast();
}

fn atGroundLocked(self: *const state.State) bool {
    return self.stream.ground() and
        !self.stream.handler.in_apc and
        self.program_status_scanner.atGround();
}

fn stoppedLocked(self: *const state.State) bool {
    return self.failure_fenced or self.output_abort_requested.load(.acquire);
}
