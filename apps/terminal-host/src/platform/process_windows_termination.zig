const std = @import("std");
const builtin = @import("builtin");
const thread_sync = @import("thread_sync.zig");
const windows_api = @import("windows_api.zig");

// Private state embedded in the Windows Process owner. It records submission,
// never child exit, Job emptiness, PTY completion or lifecycle containment.
pub const Termination = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    mutex: thread_sync.Mutex = .{},
    root_submitted: bool = false,

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
        // unexpectedError prints diagnostics; keep its I/O outside the lock.
        return windows.unexpectedError(failure);
    }

    pub fn forceTerminate(self: *Termination, job: windows.HANDLE) !void {
        const failure = submission: {
            self.mutex.lock();
            defer self.mutex.unlock();
            // Root submission never discharges descendant termination.
            if (windows_api.ob_windows_terminal_job_terminate(job, 1).toBool()) {
                self.root_submitted = true;
                return;
            }
            break :submission windows.GetLastError();
        };
        return windows.unexpectedError(failure);
    }
};
