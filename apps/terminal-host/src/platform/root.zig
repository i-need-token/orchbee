const builtin = @import("builtin");

pub const process = @import("process.zig");
pub const monotonic = @import("monotonic.zig");
pub const output_wake = @import("output_wake.zig");
pub const posix_compat = @import("posix_compat.zig");
pub const thread_sync = @import("thread_sync.zig");
pub const temp_directory = @import("temp_directory.zig");
pub const editor_document = @import("editor_document.zig");

pub const testing = if (builtin.is_test) struct {
    pub const conpty = @import("conpty_windows.zig").testing;
    pub const powershell_startup = @import("powershell_integration.zig").script;
    pub const session_signaling = @import("process_session.zig").testing;
    pub const pty = @import("pty.zig");
    pub const windows_api = struct {
        const adapter = @import("windows_api.zig");

        pub const CreateProcessW = adapter.CreateProcessW;
        pub const CREATE_SUSPENDED = adapter.CREATE_SUSPENDED;
        pub const TerminateProcess = adapter.TerminateProcess;
        pub const WaitForSingleObject = adapter.WaitForSingleObject;
        pub const WAIT_OBJECT_0 = adapter.WAIT_OBJECT_0;
        pub const ob_windows_terminal_job_assign_and_resume = adapter.ob_windows_terminal_job_assign_and_resume;
        pub const ob_windows_terminal_job_active_processes = adapter.ob_windows_terminal_job_active_processes;
    };
} else struct {};
