const std = @import("std");
const builtin = @import("builtin");
const monotonic = @import("monotonic.zig");
const query = @import("process_query_windows.zig");
const Termination = @import("process_windows_termination.zig").Termination;
const thread_sync = @import("thread_sync.zig");
const windows_api = @import("windows_api.zig");

/// Close rounds before the whole-Job fallback: each ends the current console
/// holders, and the close completes at the first round that finds none.
const close_rounds = 4;
/// The wait worker polls the live root this long for its console connection.
const learning_poll_ms = 10;
const learning_polls = 200;
/// While holders remain, PTY completion re-reads console ownership when Job
/// accounting changes and at least this often, in 10 ms polls.
const recheck_polls = 100;

// Private state embedded in the Windows Process owner: which Job members hold the
// Terminal console. The Job keeps every member and kill-on-close while the Terminal
// runs. Closing ends the console holders and then releases every other member (GUI
// programs, other consoles, no console) from Terminal ownership; any failure falls
// back to terminating the whole Job.
pub const ConsoleTree = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    /// Serializes the one-time publication of `reference` and `host`.
    mutex: thread_sync.Mutex = .{},
    /// Console host PID learned from the live root; 0 until learned.
    reference: std.atomic.Value(u32) = .init(0),
    /// Pins the learned PID so it cannot name another console after the
    /// pseudoconsole closes. Published once; released by deinit.
    host: ?windows.HANDLE = null,
    /// Set once containment observes no member holding the Terminal console.
    contained: std.atomic.Value(bool) = .init(false),
    /// Set once force termination ends every console holder it can see. A console
    /// program started just before its creator ended still reports that creator, so
    /// the round reads it as detached; when it then connects it is a late holder.
    holders_ended: std.atomic.Value(bool) = .init(false),

    pub fn deinit(self: *ConsoleTree) void {
        if (self.host) |handle| _ = windows.CloseHandle(handle);
        self.* = undefined;
    }

    /// Learns the Terminal console while the root is live, bounded, so ownership
    /// stays readable after the root exits.
    pub fn learnWhileLive(self: *ConsoleTree, root: windows.HANDLE) !void {
        for (0..learning_polls) |_| {
            if (self.learn(root)) return;
            const state = windows_api.WaitForSingleObject(root, learning_poll_ms);
            if (state == windows_api.WAIT_OBJECT_0) return;
            if (state != windows_api.WAIT_TIMEOUT) return error.ProcessObservationFailed;
        }
    }

    /// Any reading while the root is live may learn. A root that still reports its
    /// creator, this Host, has not connected to the console yet.
    fn learn(self: *ConsoleTree, root: windows.HANDLE) bool {
        if (self.reference.load(.acquire) != 0) return true;
        const host = query.consoleReference(root) orelse return false;
        if (host == windows_api.GetCurrentProcessId()) return false;
        const handle = windows_api.OpenProcess(windows_api.SYNCHRONIZE, .FALSE, host) orelse return false;
        // The pinned process must still be the live root's console host.
        if (query.consoleReference(root) != host) {
            _ = windows.CloseHandle(handle);
            return false;
        }
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.reference.load(.acquire) != 0) {
            _ = windows.CloseHandle(handle);
            return true;
        }
        self.host = handle;
        self.reference.store(host, .release);
        return true;
    }

    /// Without a learned console, every member with a console reads `unknown`
    /// and counts as a holder.
    fn members(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, action: query.ConsoleAction) !query.ConsoleMembers {
        const reference: ?u32 = if (self.learn(root)) self.reference.load(.acquire) else null;
        return query.consoleMembers(job, root, reference, action);
    }

    /// Ends the root and every console holder and leaves the other members running.
    /// A failed round, or holders that keep appearing, falls back to the whole Job.
    /// Rounds wait without holding the termination mutex.
    pub fn forceTerminate(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, termination: *Termination) !void {
        if (!termination.wholeJob()) {
            for (0..close_rounds) |_| {
                const found = self.members(job, root, .terminate) catch break;
                if (!found.empty()) continue;
                self.holders_ended.store(true, .release);
                termination.recordForceSubmission();
                return;
            }
        }
        return termination.terminateJob(job);
    }

    /// The owned tree is every console holder; after the fallback, every member.
    pub fn treeContained(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, termination: *const Termination) !bool {
        if (termination.wholeJob()) {
            const active = activeProcesses(job) orelse return error.ProcessContainmentQueryFailed;
            return active == 0;
        }
        const found = self.members(job, root, .observe) catch return error.ProcessContainmentQueryFailed;
        if (!found.empty()) return false;
        self.contained.store(true, .release);
        return true;
    }

    /// Whether a member besides the root holds the console. A failed reading keeps
    /// the Job-wide answer, because another member exists.
    pub fn holdsCommand(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, termination: *const Termination) bool {
        if (termination.wholeJob()) return true;
        const found = self.members(job, root, .observe) catch return true;
        return found.others != 0;
    }

    /// Returns once no member is attached to the pseudoconsole: an empty Job, or two
    /// consecutive readings without a holder, because a console program reports its
    /// creator until its connection completes. Natural exit does not signal the Job
    /// handle, so accounting is polled at a quiet cadence, even after force
    /// termination signals that handle while accounting still converges to zero.
    pub fn awaitDetached(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, termination: *Termination) !void {
        var quiet_readings: usize = 0;
        var holders_at: ?windows.DWORD = null;
        var unchanged_polls: usize = 0;
        while (true) {
            const active = activeProcesses(job) orelse return error.ProcessObservationFailed;
            if (active == 0) return;
            if (!termination.wholeJob() and (holders_at != active or unchanged_polls >= recheck_polls)) {
                unchanged_polls = 0;
                if (self.quiet(job, root, termination)) {
                    holders_at = null;
                    quiet_readings += 1;
                    if (quiet_readings == 2) return;
                } else {
                    holders_at = active;
                    quiet_readings = 0;
                }
            } else unchanged_polls += 1;
            monotonic.sleep(10 * std.time.ns_per_ms);
        }
    }

    /// After containment, or force termination, has observed no holder, a holder or a
    /// failed reading is a race, so closing falls back to the whole Job instead of
    /// waiting on a late holder that may never exit.
    fn quiet(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, termination: *Termination) bool {
        const settled = self.contained.load(.acquire) or self.holders_ended.load(.acquire);
        const found = self.members(job, root, .observe) catch null;
        const none = if (found) |value| value.empty() else false;
        if (!none and settled) termination.terminateJob(job) catch {};
        return none;
    }

    /// Before the last Job handle closes: clear kill-on-close, so the members without
    /// the Terminal console outlive the Job, only when the fallback was never taken
    /// and a final reading finds no holder. Otherwise the close ends every member.
    pub fn releaseMembers(self: *ConsoleTree, job: windows.HANDLE, root: windows.HANDLE, termination: *const Termination) void {
        if (termination.wholeJob()) return;
        const found = self.members(job, root, .observe) catch return;
        if (!found.empty()) return;
        _ = windows_api.ob_windows_terminal_job_release_members(job);
    }

    fn activeProcesses(job: windows.HANDLE) ?windows.DWORD {
        var active: windows.DWORD = 0;
        if (!windows_api.ob_windows_terminal_job_active_processes(job, &active).toBool()) return null;
        return active;
    }
};
