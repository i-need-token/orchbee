const std = @import("std");
const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const containment_ops = @import("containment.zig");
const process_io_ops = @import("process_io.zig");
const feedback_ops = @import("feedback.zig");

pub fn stopIoForShutdown(self: *state.State) void {
    self.output_abort_requested.store(true, .release);
    // Synchronize with the reader's post-read commit boundary. A batch
    // that already won terminal_mutex finishes before this returns; every
    // later batch observes output_abort_requested and is discarded.
    self.terminal_mutex.lock();
    while (self.reader_commit_in_progress) {
        self.terminal_condition.wait(&self.terminal_mutex);
    }
    self.input_mutex.lock();
    self.writer_stop = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    self.terminal_mutex.unlock();
}

pub fn finishPtyAfterChildExit(self: *state.State) !void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.finish_pty_after_child_exit) |finish| {
            return finish(self.testing_hooks.context.?);
        }
    }
    try self.process.finishPtyAfterChildExit();
}

pub fn releaseOutputForShutdown(self: *state.State) !void {
    var failed = false;
    finishPtyAfterChildExit(self) catch {
        failed = true;
    };
    process_io_ops.abortOutputRead(self) catch {
        failed = true;
    };
    if (failed) return error.OutputReleaseFailed;
}

pub fn shutdown(self: *state.State) contracts.ShutdownError!void {
    if (self.threads_joined) {
        std.debug.assert(self.containment_confirmed.load(.acquire));
        self.process.releaseProcessIdentity() catch
            return error.ProcessIdentityReleaseFailed;
        return;
    }
    // Stop every future writer start and reader commit before any fallible
    // cleanup. Ordinary shutdown keeps TerminalExited semantics; a failure
    // fence, when present, remains the stronger unavailable classification.
    stopIoForShutdown(self);
    const owns_host_first_attempt = containment_ops.awaitFirstContainmentAttempt(self);
    defer {
        if (owns_host_first_attempt) {
            containment_ops.completeFirstContainmentAttempt(self, .host_running);
        }
    }
    if (!self.containment_confirmed.load(.acquire)) {
        containment_ops.containProcessTree(self) catch return error.ProcessContainmentFailed;
    }
    // On Windows, closing the pseudoconsole is the operation that releases
    // blocked input and output pipe calls. Containment makes that close safe
    // even when process observation failed before the wait worker could do it.
    releaseOutputForShutdown(self) catch return error.OutputReleaseFailed;
    self.input_mutex.lock();
    self.release_process_identity = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    joinThreads(self);
    self.process.releaseProcessIdentity() catch
        return error.ProcessIdentityReleaseFailed;
}

pub fn destroy(self: *state.State) void {
    shutdown(self) catch |err| {
        // A failed shutdown must retain the runtime and all worker-owned
        // state. The owner can retry after the platform termination fault
        // is resolved; freeing here would be a use-after-free.
        std.log.err("retaining terminal runtime after shutdown failure: {s}", .{@errorName(err)});
        return;
    };
    destroyAfterShutdown(self);
}

pub fn destroyAfterShutdown(self: *state.State) void {
    std.debug.assert(self.threads_joined);
    std.debug.assert(self.containment_confirmed.load(.acquire));
    self.stream.deinit();
    self.stream_allocator.deinit();
    feedback_ops.deinit(self);
    self.process.deinit();
    self.history.deinit(&self.terminal);
    self.terminal.deinit(self.allocator);
    self.input_mutex.lock();
    self.input_ownership.deinit();
    self.input_queue.deinit();
    self.input_mutex.unlock();
    self.profile.deinit(self.allocator);
    self.projection.deinit();
    const allocator = self.allocator;
    self.* = undefined;
    allocator.destroy(self);
}

pub fn joinThreads(self: *state.State) void {
    if (self.threads_joined) return;
    if (self.wait_thread) |thread| thread.join();
    if (self.reader_thread) |thread| thread.join();
    self.input_mutex.lock();
    self.writer_stop = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    if (self.writer_thread) |thread| thread.join();
    self.threads_joined = true;
}
