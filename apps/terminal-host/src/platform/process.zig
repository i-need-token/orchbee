const builtin = @import("builtin");

const posix_process = @import("process_posix.zig");
const process_common = @import("process_common.zig");
const process_types = @import("process_types.zig");
const windows_process = @import("process_windows.zig");

pub const EnvironmentDelta = process_types.EnvironmentDelta;
pub const Launch = process_types.Launch;
pub const ExitResult = process_types.ExitResult;
pub const ProcessSample = @import("process_sample.zig").Sample;
pub const environmentVariableOwned = process_common.environmentVariableOwned;

pub fn sampleProcesses(process: *const Process, allocator: @import("std").mem.Allocator) @import("process_sample.zig").Error!ProcessSample {
    if (builtin.os.tag != .windows) return error.ProcessSampleUnsupported;
    return @import("process_query_windows.zig").collect(allocator, process.job_handle, process.process_handle);
}

/// Stable platform-process facade. Each operating-system implementation owns
/// its launch, signaling, wait, containment, and resource-release details.
pub const Process = switch (builtin.os.tag) {
    .windows => windows_process.Process,
    .freebsd, .linux, .macos => posix_process.Process,
    else => @compileError("terminal-host has no process implementation for this target"),
};

pub const testing = if (builtin.is_test) struct {
    pub const process_query = @import("process_query_windows.zig").testing;
    pub const foregroundCommandActiveFn = posix_process.foregroundCommandActive;
    pub const posixExecutableCandidatesFn = posix_process.executableCandidatesForTesting;
    pub const PosixProcessType = posix_process.Process;
    pub const SessionIdentityStateType = posix_process.SessionIdentityState;
    pub const windowsCommandLineFn = windows_process.windowsCommandLine;
    pub const windowsSearchCandidatesFn = windows_process.windowsSearchCandidates;
    pub const windowsTerminatedExitResultFn = windows_process.windowsTerminatedExitResult;
} else struct {};
