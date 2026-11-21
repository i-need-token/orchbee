const std = @import("std");
const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const containment_ops = @import("containment.zig");
const process_io_ops = @import("process_io.zig");
const feedback_ops = @import("feedback.zig");
const graphics_media = @import("graphics_media.zig");

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
    self.terminal_condition.broadcast();
    self.animation_condition.broadcast();
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
    // Release first, started before containment so the Keepers release while the tree is
    // contained. Neither the whole-Job fallback nor the console close waits for it: each step
    // that needs it returns KeeperReleasePending, and the owner retries the shutdown.
    _ = self.process.releaseKeepers();
    if (!self.containment_confirmed.load(.acquire)) {
        containment_ops.containProcessTree(self) catch |err| return if (err == error.KeeperReleasePending)
            error.KeeperReleasePending
        else
            error.ProcessContainmentFailed;
    }
    if (!self.process.releaseKeepers()) return error.KeeperReleasePending;
    // On Windows, closing the pseudoconsole is the operation that releases
    // blocked input and output pipe calls. Containment makes that close safe
    // even when process observation failed before the wait worker could do it.
    releaseOutputForShutdown(self) catch return error.OutputReleaseFailed;
    if (comptime target_builtin.os.tag != .windows) closePtyForShutdown(self);
    self.input_mutex.lock();
    self.release_process_identity = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    joinThreads(self);
    self.process.releaseProcessIdentity() catch
        return error.ProcessIdentityReleaseFailed;
}

/// POSIX: closes the PTY master before the wait for the session leader. A leader
/// that owns a controlling terminal may finish exiting only once the terminal's
/// output queue drains or its master closes, and shutdown no longer reads that
/// output. The reader and writer are the only workers that use the master, so
/// both stop first: the closed descriptor number may be reused by a later open.
fn closePtyForShutdown(self: *state.State) void {
    if (self.reader_thread) |thread| {
        thread.join();
        self.reader_thread = null;
    }
    self.input_mutex.lock();
    self.writer_stop = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    if (self.writer_thread) |thread| {
        thread.join();
        self.writer_thread = null;
    }
    self.process.closePtyForShutdown();
}

pub fn pendingShutdownCleanup(self: *const state.State) ?contracts.ShutdownCleanupPhase {
    if (!self.containment_confirmed.load(.acquire)) return .contain_process;
    if (!self.threads_joined) return .release_output;
    if (!self.process.identityReleased()) return .release_process_identity;
    return null;
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
    self.process.deinit();
    free(self);
}

/// Frees the runtime once every worker is joined and its process record is released.
pub fn free(self: *state.State) void {
    std.debug.assert(self.threads_joined);
    graphics_media.deinit(self);
    self.stream.deinit();
    self.stream_allocator.deinit();
    feedback_ops.deinit(self);
    self.deferred_input.deinit(self.allocator);
    self.history.deinit(&self.terminal);
    self.terminal.deinit(self.allocator);
    if (self.kitty_temp_directory) |path| self.allocator.free(path);
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
    if (self.animation_thread) |thread| thread.join();
    self.input_mutex.lock();
    self.writer_stop = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
    if (self.writer_thread) |thread| thread.join();
    self.threads_joined = true;
}
