comptime {
    _ = @import("process_test.zig");
    _ = @import("windows_launch_test.zig");
    _ = @import("windows_environment_test.zig");
    _ = @import("process_query_windows_test.zig");
    _ = @import("pty_test.zig");
    _ = @import("conpty_windows_test.zig");
    _ = @import("windows_termination_test.zig");
    _ = @import("session_signal_retry_test.zig");
    _ = @import("editor_document_test.zig");
}
