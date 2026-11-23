//! A terminal's move between Hosts, as its runtime sees it. The old Host holds the terminal from the
//! start of its quiesce: it waits for the writer and the signal sender to go idle and parks the
//! reader at ground, then resumes the terminal or, once another Host claimed it, drops it. The new
//! Host holds a restored terminal behind its startup gate until a claim opens the gate or a discard
//! drops it. While a move is held nothing ends or contains the terminal, and an observed program
//! exit waits for the move's result. A drop joins every worker without I/O on the terminal's pipes
//! and closes only this Host's own handles, so the program and its console are untouched.

const std = @import("std");
const platform = @import("terminal-host-platform");
const pty_process = platform.process;
const state = @import("state.zig");
const reader_pause = @import("reader_pause.zig");
const lifecycle = @import("lifecycle.zig");
const attachment_ops = @import("attachment.zig");

/// Each of the quiesce's waits: for idle input and signals, for the park, and for idle again.
pub const step_ns: u64 = std.time.ns_per_s;
/// How often an idle wait looks at the signal sender, which reports to no condition.
const idle_poll_ns: u64 = 2 * std.time.ns_per_ms;

pub const BeginError = error{NotRunning};

/// Holds the terminal for a quiesce. Refused once the wait thread observed the program's end, or
/// when the terminal failed or a resize fenced it.
pub fn begin(self: *state.State) BeginError!void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    std.debug.assert(self.move == .none);
    if (self.exit_observed or self.failure_fenced or self.resize_fenced.load(.acquire)) return error.NotRunning;
    switch (self.completion_classification.load()) {
        .pending => {},
        .normal, .failed => return error.NotRunning,
    }
    self.move = .held;
}

/// Whether a move holds the terminal or it is leaving: nothing may end or contain it.
pub fn moving(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    return self.move != .none;
}

/// Runs on the wait thread once the program ended. Waits while a move holds the terminal and
/// returns whether the exit path runs: a terminal that leaves this Host takes none.
pub fn awaitExitTurn(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    self.exit_observed = true;
    while (self.move == .held) self.terminal_condition.wait(&self.terminal_mutex);
    return self.move == .none;
}

pub const Park = enum {
    parked,
    /// Accepted input was still unwritten at a deadline.
    input_pending,
    /// A resize or clear was still unsent at a deadline.
    signal_pending,
    /// The reader read on inside a sequence, or did not reach ground, until its deadline.
    mid_sequence,
    /// The reader ended: abort, failure, or end of output.
    stopped,
};

/// Quiesce steps 3 to 5, off the dispatch thread: the writer and the signal sender go idle, the
/// reader parks at ground with its last batch applied, and both go idle again, since that batch
/// may have queued replies and resizes. Every outcome but `parked` leaves the reader reading on.
pub fn park(self: *state.State) Park {
    if (awaitIdle(self, step_ns)) |pending| return pending;
    reader_pause.pause(self) catch return .stopped;
    switch (reader_pause.awaitParked(self, step_ns)) {
        .parked => {},
        .mid_sequence, .timed_out => return readOn(self, .mid_sequence),
        .stopped => return readOn(self, .stopped),
    }
    if (awaitIdle(self, step_ns)) |pending| return readOn(self, pending);
    return .parked;
}

/// Withdraws the pause or resumes the parked reader, and reports `outcome`.
fn readOn(self: *state.State, outcome: Park) Park {
    reader_pause.resumeReader(self) catch {};
    return outcome;
}

/// Waits up to `timeout_ns` for every accepted input to be written and the signal sender to go
/// idle; null once both are, otherwise what is still pending.
fn awaitIdle(self: *state.State, timeout_ns: u64) ?Park {
    var timer = platform.monotonic.Timer.start() catch null;
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    while (true) {
        const input_idle = self.input_queue.entries.items.len == 0;
        if (input_idle and pty_process.signalsIdle(&self.process)) return null;
        const elapsed = if (timer) |*value| value.read() else timeout_ns;
        if (elapsed >= timeout_ns) return if (input_idle) .signal_pending else .input_pending;
        self.input_condition.timedWait(&self.input_mutex, @min(idle_poll_ns, timeout_ns - elapsed)) catch {};
    }
}

/// Ends a held move without leaving: the old Host resumed, or the new Host claimed. A parked
/// reader reads on, an observed program exit takes its exit path, and the attachments' input
/// cleanups that waited for the move run in order.
pub fn end(self: *state.State) void {
    if (readerParked(self)) reader_pause.resumeReader(self) catch {};
    var deferred = deferred: {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        std.debug.assert(self.move == .held);
        self.move = .none;
        self.terminal_condition.broadcast();
        const taken = self.deferred_input;
        self.deferred_input = .empty;
        break :deferred taken;
    };
    defer deferred.deinit(self.allocator);
    for (deferred.items) |item| {
        if (item.detach) attachment_ops.detachInput(self, item.attachment_id) else attachment_ops.resetInput(self, item.attachment_id);
    }
}

fn readerParked(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    return self.reader_pause != .none;
}

/// Lets a held terminal go: the old Host's moved terminal, or the new Host's discarded import. The
/// parked or gated reader, the idle or gated writer, the animation thread and the wait thread,
/// woken by its stop event, end without I/O on the pipes and without the exit path; then only this
/// Host's own handles close, and the runtime is freed. Nothing ends, is signalled, or is written,
/// and the Job keeps its limits.
pub fn drop(self: *state.State) void {
    if (comptime !pty_process.recovery_supported) unreachable;
    {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        std.debug.assert(self.move == .held);
        self.move = .dropping;
        self.terminal_condition.broadcast();
    }
    self.startup.abortIfPending();
    pty_process.stopWait(&self.process);
    lifecycle.stopIoForShutdown(self);
    // A wait thread that failed to observe the program waits for this before it returns.
    self.input_mutex.lock();
    self.release_process_identity = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    lifecycle.joinThreads(self);
    self.process.drop();
    lifecycle.free(self);
}

/// Opens a restored terminal's startup gate after its claim: its workers start, and the reader
/// reads the console from where the old Host's reader parked.
pub fn claim(self: *state.State) void {
    self.startup.ready();
    end(self);
}
