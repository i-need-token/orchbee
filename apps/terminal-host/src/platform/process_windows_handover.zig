//! A terminal that moves between Hosts, as its Windows process sees it. The new Host adopts a
//! held console from the seven duplicates a claim checked; the old Host drops a moved terminal
//! and the new Host discards an import, each closing only its own handles; and a settled terminal
//! restored from a record keeps no process at all.

const std = @import("std");
const keeper_handles = @import("keeper_handles_windows.zig");
const process_types = @import("process_types.zig");
const windows_api = @import("windows_api.zig");
const Process = @import("process_windows.zig").Process;

const windows = std.os.windows;
const Role = keeper_handles.Role;
const DUPLICATE_SAME_ACCESS: windows.DWORD = 0x2;

extern "kernel32" fn DuplicateHandle(
    source_process: windows.HANDLE,
    source: windows.HANDLE,
    target_process: windows.HANDLE,
    target: *windows.HANDLE,
    access: windows.DWORD,
    inherit: windows.BOOL,
    options: windows.DWORD,
) callconv(.winapi) windows.BOOL;

pub const Held = struct {
    /// This Host's checked duplicates, in role order. `adopt` owns all of them once it returns;
    /// on failure the caller still owns every one.
    duplicates: keeper_handles.Duplicates,
    /// The checked OpenConsole's process id: the console reference, not learned again.
    console_pid: u32,
    launch_kind: process_types.LaunchKind,
    interactive_shell: bool,
};

/// The process record of a held console: the program and its Job as pulled, and a pseudoconsole
/// the bundled conpty packs from the console's process, reference, and signal handles.
pub fn adopt(allocator: std.mem.Allocator, held: Held) !Process {
    const stop_event = windows_api.CreateEventW(null, .TRUE, .FALSE, null) orelse
        return windows.unexpectedError(windows.GetLastError());
    errdefer windows.CloseHandle(stop_event);
    // The tree pins the console with its own handle, since the pseudoconsole owns the one pulled.
    var tree_host: windows.HANDLE = undefined;
    const current = windows.GetCurrentProcess();
    if (!DuplicateHandle(current, held.duplicates[@intFromEnum(Role.console)], current, &tree_host, 0, .FALSE, DUPLICATE_SAME_ACCESS).toBool()) {
        return windows.unexpectedError(windows.GetLastError());
    }
    errdefer windows.CloseHandle(tree_host);
    const pty = try @import("pty.zig").Pty.adopt(allocator, .{
        .input = held.duplicates[@intFromEnum(Role.input)],
        .output = held.duplicates[@intFromEnum(Role.output)],
        .console = held.duplicates[@intFromEnum(Role.console)],
        .reference = held.duplicates[@intFromEnum(Role.reference)],
        .signal = held.duplicates[@intFromEnum(Role.signal)],
    });
    var process: Process = .{
        .job_handle = held.duplicates[@intFromEnum(Role.job)],
        .process_handle = held.duplicates[@intFromEnum(Role.program)],
        .interactive_shell = held.interactive_shell,
        .launch_kind = held.launch_kind,
        .pty = pty,
        .stop_event = stop_event,
    };
    process.console.adoptReference(held.console_pid, tree_host);
    return process;
}

/// Lets the terminal go without ending anything: every thread of the terminal has been joined,
/// and no Keeper slot is active. Only this Host's own handles close: the Job and program handles,
/// the pipes, the console reference this Host pinned, the stop event, and the three inside the
/// pseudoconsole. The Job keeps its limits, nothing is terminated, and no signal or input is sent.
pub fn drop(self: *Process) void {
    if (self.inert) {
        self.* = undefined;
        return;
    }
    self.pty.drop();
    _ = windows.CloseHandle(self.process_handle);
    _ = windows.CloseHandle(self.job_handle);
    self.console.deinit();
    _ = windows.CloseHandle(self.stop_event);
    self.* = undefined;
}

/// The process record of a settled terminal restored from a record: its program and console
/// ended on the old Host, so nothing here is open, watched, or ended.
pub fn settled(launch_kind: process_types.LaunchKind, interactive_shell: bool) Process {
    return .{
        .job_handle = undefined,
        .process_handle = undefined,
        .interactive_shell = interactive_shell,
        .launch_kind = launch_kind,
        .pty = undefined,
        .stop_event = undefined,
        .inert = true,
    };
}
