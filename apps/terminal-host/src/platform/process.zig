const builtin = @import("builtin");

const posix_process = @import("process_posix.zig");
const process_common = @import("process_common.zig");
const process_types = @import("process_types.zig");
const windows_process = @import("process_windows.zig");

pub const EnvironmentDelta = process_types.EnvironmentDelta;
pub const Launch = process_types.Launch;
pub const ExitResult = process_types.ExitResult;
pub const LaunchKind = process_types.LaunchKind;
pub const ProcessSample = @import("process_sample.zig").Sample;
pub const ProcessConsole = @import("process_sample.zig").Console;
pub const environmentVariableOwned = process_common.environmentVariableOwned;
/// Windows Host startup clears inherited signal suppression before any child launch.
pub const prepareWindowsTerminalChildren = windows_process.prepareTerminalChildren;

pub fn sampleProcesses(process: *Process, allocator: @import("std").mem.Allocator) @import("process_sample.zig").Error!ProcessSample {
    return switch (builtin.os.tag) {
        .windows => @import("process_query_windows.zig").collect(allocator, process.job_handle, process.process_handle),
        .linux, .macos => process.sampleProcesses(allocator),
        else => error.ProcessSampleUnsupported,
    };
}

/// Stable platform-process facade. Each operating-system implementation owns
/// its launch, signaling, wait, containment, and resource-release details.
pub const Process = switch (builtin.os.tag) {
    .windows => windows_process.Process,
    .freebsd, .linux, .macos => posix_process.Process,
    else => @compileError("terminal-host has no process implementation for this target"),
};

pub const testing = if (builtin.is_test) struct {
    pub const interactiveShellLaunchFn = @import("interactive_shell.zig").isInteractive;
    pub const cmdPromptFn = @import("cmd_integration.zig").prompt;
    pub const launchKindFn = @import("interactive_shell.zig").launchKind;
    pub const powershellArgumentsFn = @import("powershell_integration.zig").arguments;
    pub const process_query = @import("process_query_windows.zig").testing;
    pub const posix_process_query = @import("process_query_posix.zig").testing;
    pub const foregroundCommandActiveFn = posix_process.foregroundCommandActive;
    pub const posixExecutableCandidatesFn = posix_process.executableCandidatesForTesting;
    pub const PosixProcessType = posix_process.Process;
    pub const SessionIdentityStateType = posix_process.SessionIdentityState;
    pub const windowsCommandLineFn = windows_process.windowsCommandLine;
    pub const windowsBatchCommandLineFn = @import("windows_command_line.zig").batch;
    pub const windowsChildCurrentDirectoryFn = windows_process.childCurrentDirectory;
    pub const windows_environment = @import("windows_environment.zig").testing;
    pub const windowsSearchCandidatesFn = windows_process.windowsSearchCandidates;
    pub const windowsTerminatedExitResultFn = windows_process.windowsTerminatedExitResult;
} else struct {};
