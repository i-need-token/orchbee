const std = @import("std");
const builtin = @import("builtin");
const thread_sync = @import("thread_sync.zig");
const windows_api = @import("windows_api.zig");

// Private state embedded in the Windows Process owner. It records submission and
// the whole-Job fallback, never child exit, Job emptiness, PTY completion or
// lifecycle containment.
pub const Termination = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    mutex: thread_sync.Mutex = .{},
    root_submitted: bool = false,
    /// Set once force termination targets the whole Job; from then on
    /// kill-on-close stays and every member counts.
    whole_job: std.atomic.Value(bool) = .init(false),

    pub fn terminate(self: *Termination, process: windows.HANDLE) !void {
        const failure = submission: {
            self.mutex.lock();
            defer self.mutex.unlock();
            // A successful force may not yet have signaled the root handle.
            if (self.root_submitted) return;
            if (windows_api.TerminateProcess(process, 1).toBool()) {
                self.root_submitted = true;
                return;
            }
            const last_error = windows.GetLastError();
            if (windows_api.WaitForSingleObject(process, 0) == windows_api.WAIT_OBJECT_0) return;
            break :submission last_error;
        };
        // Known permission refusal is not an unexpected runtime diagnostic.
        return submissionFailure(failure);
    }

    /// Records a force termination that ended the root and every console holder.
    pub fn recordForceSubmission(self: *Termination) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.root_submitted = true;
    }

    /// Terminates every Job member: the fallback of the console-holder close.
    pub fn terminateJob(self: *Termination, job: windows.HANDLE) !void {
        const failure = submission: {
            self.mutex.lock();
            defer self.mutex.unlock();
            self.whole_job.store(true, .release);
            // Root submission never discharges descendant termination.
            if (windows_api.ob_windows_terminal_job_terminate(job, 1).toBool()) {
                self.root_submitted = true;
                return;
            }
            break :submission windows.GetLastError();
        };
        return submissionFailure(failure);
    }

    pub fn wholeJob(self: *const Termination) bool {
        return self.whole_job.load(.acquire);
    }

    fn submissionFailure(failure: windows.Win32Error) anyerror {
        // unexpectedError may write diagnostics; every caller has released its lock.
        return switch (failure) {
            .ACCESS_DENIED => error.AccessDenied,
            else => windows.unexpectedError(failure),
        };
    }
};
