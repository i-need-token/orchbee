const builtin = @import("builtin");

pub const process = @import("process.zig");

pub const testing = if (builtin.is_test) struct {
    pub const pty = @import("pty.zig");
    pub const windows_api = struct {
        const adapter = @import("windows_api.zig");

        pub const CreateProcessW = adapter.CreateProcessW;
        pub const CREATE_SUSPENDED = adapter.CREATE_SUSPENDED;
        pub const ob_windows_terminal_job_assign_and_resume = adapter.ob_windows_terminal_job_assign_and_resume;
        pub const ob_windows_terminal_job_active_processes = adapter.ob_windows_terminal_job_active_processes;
    };
} else struct {};
