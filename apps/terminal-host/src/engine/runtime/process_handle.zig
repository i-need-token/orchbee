const std = @import("std");
const pty_process = @import("terminal-host-platform").process;
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const completion_ops = @import("completion.zig");
const process_io_ops = @import("process_io.zig");

// Process-facing adapters preserve the opaque public runtime boundary.
pub fn Type(comptime Runtime: type) type {
    return struct {
        pub fn sampleProcesses(self: *Runtime, allocator: std.mem.Allocator) !pty_process.ProcessSample {
            return pty_process.sampleProcesses(&stateFrom(self).process, allocator);
        }

        /// How the Host started the root process; fixed for the runtime's life.
        pub fn launchKind(self: *const Runtime) pty_process.LaunchKind {
            return stateFromConst(self).process.launch_kind;
        }

        pub fn completion(self: *const Runtime) contracts.Completion {
            return completion_ops.completion(stateFromConst(self));
        }

        pub fn waitForCompletion(self: *Runtime) !contracts.Completion {
            return completion_ops.waitForCompletion(stateFrom(self));
        }

        pub fn terminate(self: *Runtime) !void {
            return process_io_ops.terminate(stateFrom(self));
        }

        pub fn containmentConfirmed(self: *const Runtime) bool {
            return process_io_ops.containmentConfirmed(stateFromConst(self));
        }

        pub fn forceTerminate(self: *Runtime) !void {
            return process_io_ops.forceTerminate(stateFrom(self));
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }
        fn stateFromConst(runtime: *const Runtime) *const state.State {
            return @ptrCast(@alignCast(runtime));
        }
    };
}
