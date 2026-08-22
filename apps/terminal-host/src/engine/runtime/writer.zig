const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const failure_ops = @import("failure.zig");
const input_commit_ops = @import("input_commit.zig");
const process_io_ops = @import("process_io.zig");

pub fn closeClientInput(self: *state.State) void {
    self.input_mutex.lock();
    self.client_input_closed = true;
    self.input_condition.broadcast();
    self.input_mutex.unlock();
}

pub fn notifyWriterSliceCopied(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.writer_slice_copied) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn notifyWriterWaitingForInput(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.writer_waiting_for_input) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn notifyWriterSliceStarted(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.writer_slice_started) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn notifyWriterSliceAccounted(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.writer_slice_accounted) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn startWriterSlice(
    self: *state.State,
    slice: contracts.WriterSlice,
) contracts.WriterSliceStart {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.failure_fenced or self.writer_stop) {
        input_commit_ops.clearInputQueueLocked(self);
        return .stopped;
    }
    if (!self.input_queue.ownsWriterSlice(slice)) return .ownership_lost;
    // This successful recheck is the write-start linearization point. A
    // later fence may stop the next slice, but this exact bounded platform
    // call is allowed to complete and is accounted once.
    return .started;
}

pub fn accountWriterSlice(
    self: *state.State,
    slice: contracts.WriterSlice,
    written: usize,
) bool {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    const outcome = self.input_queue.accountWriterSlice(slice, written);
    return switch (outcome) {
        .advanced => true,
        .completed => completed: {
            self.input_condition.broadcast();
            break :completed true;
        },
        .ownership_lost => false,
    };
}

pub fn stopWriterAfterOwnershipLoss(self: *state.State) void {
    const won = failure_ops.closeFailureFence(self, .worker_integrity);
    self.input_mutex.lock();
    input_commit_ops.clearInputQueueLocked(self);
    self.input_mutex.unlock();
    if (won) failure_ops.finishFailureFence(self);
}

pub fn writerThread(self: *state.State) void {
    defer {
        self.writer_stopped.store(true, .release);
        self.input_mutex.lock();
        self.input_condition.broadcast();
        self.input_mutex.unlock();
    }
    if (!self.startup.await()) return;
    var batch: [contracts.max_pty_write_slice_bytes]u8 = undefined;
    while (true) {
        self.input_mutex.lock();
        while (!self.writer_stop and !self.failure_fenced and
            !self.input_queue.hasWritableEntry())
        {
            notifyWriterWaitingForInput(self);
            self.input_condition.wait(&self.input_mutex);
        }
        if (self.writer_stop or self.failure_fenced) {
            input_commit_ops.clearInputQueueLocked(self);
            self.input_mutex.unlock();
            return;
        }
        const slice = self.input_queue.copyNextWriterSlice(&batch);
        self.input_mutex.unlock();

        notifyWriterSliceCopied(self);
        switch (startWriterSlice(self, slice)) {
            .started => {},
            .stopped => return,
            .ownership_lost => {
                stopWriterAfterOwnershipLoss(self);
                return;
            },
        }
        notifyWriterSliceStarted(self);
        const written = process_io_ops.writePtySlice(self, batch[0..slice.length]) catch {
            failure_ops.recordWriterFailure(self);
            return;
        };
        if (written == 0 or written > slice.length) {
            failure_ops.recordWriterFailure(self);
            return;
        }
        if (!accountWriterSlice(self, slice, written)) {
            stopWriterAfterOwnershipLoss(self);
            return;
        }
        notifyWriterSliceAccounted(self);
        if (slice.source == .client) {
            _ = self.written_since_sync.fetchAdd(written, .release);
        }
    }
}
