//! A `hold`'s view of one Windows terminal, taken while its release-first gate keeps the handles
//! open: a dedicated duplicate of each of its seven handles, in the Keeper's role order, and the
//! facts the Keeper checks against them. The pseudoconsole's three handles are read through the
//! Host's own conpty layout from the table of known builds. Each duplicate keeps its source's access
//! and is not inheritable, so the Keeper pulls the very objects the terminal uses and no child
//! becomes a holder. The link closes the duplicates once the hold is answered, and keeps them until
//! the Keeper ends when it is not, so no value a `hold` named can come to name another object.

const std = @import("std");
const builds = @import("conpty_builds.zig");
const LaunchKind = @import("process_types.zig").LaunchKind;
const Process = @import("process_windows.zig").Process;
const handles = @import("keeper_handles_windows.zig");

const windows = std.os.windows;
const HANDLE = windows.HANDLE;
const DUPLICATE_SAME_ACCESS: windows.DWORD = 0x2;

/// job, program, console, signal, reference, input, output.
pub const role_count = 7;

pub const Identity = handles.Identity;

pub const Snapshot = struct {
    handles: [role_count]HANDLE,
    program: Identity,
    console: Identity,
    launch_kind: LaunchKind,

    pub fn close(self: *Snapshot) void {
        for (self.handles) |handle| windows.CloseHandle(handle);
        self.* = undefined;
    }
};

pub const Error = error{
    /// The terminal runs on the system ConHost, whose terminals are never held.
    SystemConsole,
    /// A duplicate or an identity could not be taken.
    HoldFailed,
};

extern "kernel32" fn GetCurrentProcess() callconv(.winapi) HANDLE;
extern "kernel32" fn DuplicateHandle(
    source_process: HANDLE,
    source: HANDLE,
    target_process: HANDLE,
    target: *HANDLE,
    access: windows.DWORD,
    inherit: windows.BOOL,
    options: windows.DWORD,
) callconv(.winapi) windows.BOOL;
/// Takes the snapshot. The caller's gate slot must be active, so no handle closes meanwhile.
pub fn take(process: *Process) Error!Snapshot {
    if (process.pty.conpty.module == null) return error.SystemConsole;
    const pseudo_console: [*]const u8 = @ptrCast(process.pty.signals.pseudo_console);
    const layout = builds.own_layout;
    const sources = [role_count]HANDLE{
        process.job_handle,
        process.process_handle,
        member(pseudo_console, layout.console),
        member(pseudo_console, layout.signal),
        member(pseudo_console, layout.reference),
        process.pty.input,
        process.pty.output,
    };
    var snapshot: Snapshot = undefined;
    var taken: usize = 0;
    errdefer for (snapshot.handles[0..taken]) |handle| windows.CloseHandle(handle);
    const self_process = GetCurrentProcess();
    for (sources, &snapshot.handles) |source, *duplicate| {
        if (!DuplicateHandle(self_process, source, self_process, duplicate, 0, .FALSE, DUPLICATE_SAME_ACCESS).toBool())
            return error.HoldFailed;
        taken += 1;
    }
    snapshot.program = try identity(snapshot.handles[1]);
    snapshot.console = try identity(snapshot.handles[2]);
    snapshot.launch_kind = process.launch_kind;
    return snapshot;
}

/// Whether the terminal runs on the system ConHost, whose terminals are never held or moved.
pub fn onSystemConsole(process: *const Process) bool {
    return process.pty.conpty.module == null;
}

/// The program and console identities a record carries, read through the terminal's own handles.
pub const Facts = struct { program: Identity, console: Identity };

/// Reads the facts of a terminal on the bundled OpenConsole. The caller keeps its handles open.
pub fn facts(process: *Process) Error!Facts {
    if (process.pty.conpty.module == null) return error.SystemConsole;
    const pseudo_console: [*]const u8 = @ptrCast(process.pty.signals.pseudo_console);
    return .{
        .program = try identity(process.process_handle),
        .console = try identity(member(pseudo_console, builds.own_layout.console)),
    };
}

fn member(pseudo_console: [*]const u8, offset: usize) HANDLE {
    return @as(*align(1) const HANDLE, @ptrCast(pseudo_console + offset)).*;
}

fn identity(process: HANDLE) Error!Identity {
    return handles.processIdentity(process) orelse error.HoldFailed;
}
