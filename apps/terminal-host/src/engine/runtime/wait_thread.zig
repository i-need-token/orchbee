const platform = @import("terminal-host-platform");
const fatal_record = platform.fatal_record;
const state = @import("state.zig");
const handover_move = @import("handover_move.zig");
const containment_ops = @import("containment.zig");
const lifecycle_ops = @import("lifecycle.zig");
const completion_ops = @import("completion.zig");
const failure_ops = @import("failure.zig");
const writer_ops = @import("writer.zig");

pub fn waitThread(self: *state.State) void {
    fatal_record.markThread(.@"terminal-wait");
    if (!self.startup.await()) return;
    self.exit_result = self.process.wait() catch |err| {
        // The terminal leaves this Host: its exit path belongs to the Host that serves it.
        if (platform.process.waitStopped(err)) return;
        failure_ops.recordProcessObservationFailure(self);
        return;
    };
    if (!handover_move.awaitExitTurn(self)) return;
    self.child_exited.store(true, .release);
    writer_ops.closeClientInput(self);
    lifecycle_ops.finishPtyAfterChildExit(self) catch {
        failure_ops.recordProcessObservationFailure(self);
        return;
    };
    self.input_mutex.lock();
    while ((!self.pty_drained.load(.acquire) or
        !self.writer_stopped.load(.acquire)) and
        !self.failure_fenced and
        !self.release_process_identity)
    {
        self.input_condition.wait(&self.input_mutex);
    }
    const release_requested = self.release_process_identity;
    const normally_drained = self.pty_drained.load(.acquire) and
        self.writer_stopped.load(.acquire) and
        !self.failure_fenced;
    self.input_mutex.unlock();
    if (!release_requested and normally_drained) {
        const contained = containment_ops.processTreeAlreadyContained(self) catch {
            failure_ops.recordProcessObservationFailure(self);
            return;
        };
        if (contained) {
            failure_ops.releaseProcessIdentityFromWorker(self);
            return;
        }
        // A descendant may deliberately remain in the owned session after
        // closing every PTY handle. Root exit plus normal PTY drain is still
        // a normal terminal outcome; disposal will contain that retained
        // process tree before releasing the session identity.
        completion_ops.markNormalCompletionReady(self);
    }
    self.input_mutex.lock();
    while (!self.release_process_identity) {
        self.input_condition.wait(&self.input_mutex);
    }
    self.input_mutex.unlock();
    failure_ops.releaseProcessIdentityFromWorker(self);
}
