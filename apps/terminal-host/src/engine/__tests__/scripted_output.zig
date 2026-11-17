//! A scripted platform read for engine tests: the reader reads only the chunks a test hands it,
//! and a pause acts as the platform's does.
const harness = @import("runtime_harness.zig");
const std = harness.std;
const pty_process = harness.pty_process;
const thread_sync = harness.thread_sync;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const TerminalRuntime = harness.TerminalRuntime;
const ReaderParkWait = harness.subject.ReaderParkWait;

/// Stands in for the platform read: it serves the chunks the test pushes and reports a pause as
/// the platform does. A pause stops a read that stops at pauses, and cancels only the read
/// active when it was requested.
pub const ScriptedOutput = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    aborted: bool = false,
    chunk: ?[]const u8 = null,
    committed: usize = 0,
    pause_cancel: bool = false,
    pause_standing: bool = false,
    reading: ?pty_process.ReadMode = null,
    stopped: bool = false,
    /// Runs inside the platform pause instead of standing it, then refuses it.
    refuse_pause: ?*const fn (*ScriptedOutput) void = null,
    refuse_resume: bool = false,
    runtime: *TerminalRuntime = undefined,
    refusal_park: ?ReaderParkWait = null,

    fn fromContext(context: *anyopaque) *ScriptedOutput {
        return @ptrCast(@alignCast(context));
    }

    fn read(context: *anyopaque, buffer: []u8, mode: pty_process.ReadMode) anyerror!pty_process.Read {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.pause_standing and mode == .stop_at_pause) return .paused;
        self.reading = mode;
        self.pause_cancel = false;
        self.condition.broadcast();
        defer {
            self.reading = null;
            self.condition.broadcast();
        }
        while (true) {
            if (self.aborted) return .end;
            if (self.chunk) |chunk| {
                std.debug.assert(chunk.len <= buffer.len);
                @memcpy(buffer[0..chunk.len], chunk);
                self.chunk = null;
                return .{ .bytes = chunk.len };
            }
            if (self.pause_cancel) return .paused;
            self.condition.wait(&self.mutex);
        }
    }

    fn pauseRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        if (self.refuse_pause) |refuse| {
            refuse(self);
            return error.InjectedPauseFailure;
        }
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.pause_standing) return;
        self.pause_standing = true;
        if (self.reading != null) self.pause_cancel = true;
        self.condition.broadcast();
    }

    fn resumeRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        if (self.refuse_resume) return error.InjectedResumeFailure;
        self.mutex.lock();
        defer self.mutex.unlock();
        self.pause_standing = false;
    }

    fn abortRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        self.aborted = true;
        self.condition.broadcast();
    }

    fn batchFinished(context: *anyopaque, outcome: harness.ReaderBatchOutcome) void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(outcome == .committed);
        self.committed += 1;
        self.condition.broadcast();
    }

    fn readerStopped(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        self.stopped = true;
        self.condition.broadcast();
    }

    pub fn hooks(self: *ScriptedOutput) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .context = self,
            .pause_read = pauseRead,
            .read_pty_pausable = read,
            .reader_batch_finished = batchFinished,
            .reader_stopped = readerStopped,
            .resume_read = resumeRead,
        };
    }

    /// Hands the reader one chunk and waits until its batch committed.
    pub fn commit(self: *ScriptedOutput, chunk: []const u8) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(self.chunk == null);
        const target = self.committed + 1;
        self.chunk = chunk;
        self.condition.broadcast();
        while (self.committed < target) self.condition.wait(&self.mutex);
    }

    /// Waits until the reader reads on through a standing pause.
    pub fn awaitReadThrough(self: *ScriptedOutput) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.reading != .through_pause) self.condition.wait(&self.mutex);
    }

    pub fn awaitStopped(self: *ScriptedOutput) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (!self.stopped) self.condition.wait(&self.mutex);
    }
};
