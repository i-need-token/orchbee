const std = @import("std");
const builtin = @import("builtin");
const terminal_input_encoding = @import("../input_encoding.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const handle = @import("handle.zig");
const creation_ops = @import("creation.zig");
const lifecycle_ops = @import("lifecycle.zig");
const completion_ops = @import("completion.zig");
const failure_ops = @import("failure.zig");
const input_admission_ops = @import("input_admission.zig");
const input_commit_ops = @import("input_commit.zig");
const output_ops = @import("output.zig");
const resize_ops = @import("resize.zig");
const terminal_control_ops = @import("terminal_control.zig");
const writer_ops = @import("writer.zig");

pub const testing = if (builtin.is_test) struct {
    pub const InputQueueSnapshot = struct {
        client_charge: contracts.InputCharge,
        entry_count: usize,
        published_client_entries: usize,
        terminal_reply_charge: contracts.InputCharge,
    };

    pub const applyScrollbackBytesFn = terminal_control_ops.applyScrollbackBytes;
    pub const cancelInputFn = cancelInput;
    pub const closeClientInputFn = closeClientInput;
    pub const closeFailureFenceFn = closeFailureFence;
    pub const commitReaderBatchFn = commitReaderBatch;
    pub const publishInputFn = publishInput;
    pub const markNormalCompletionReadyFn = markNormalCompletionReady;
    pub const recordWorkerIntegrityFailureFn = recordWorkerIntegrityFailure;
    pub const recordReaderFailureFn = recordReaderFailure;
    pub const recordWriterFailureFn = recordWriterFailure;
    pub const reserveInputFn = reserveInput;
    pub const completionClassificationLoadFn = contracts.CompletionClassification.load;
    pub const deinitOwnedProfileFn = contracts.OwnedProfile.deinit;
    pub const InputReservationType = contracts.InputReservation;
    pub const InputSourceType = contracts.InputSource;
    pub const FirstContainmentAttemptType = contracts.FirstContainmentAttempt;
    pub const initOwnedProfileFn = contracts.OwnedProfile.init;
    pub const modifiersFn = terminal_input_encoding.testing.modifiersFn;
    pub const mouseEncodeEventFn = terminal_input_encoding.testing.mouseEncodeEventFn;
    pub const mouseHasPressedButtonFn = terminal_input_encoding.testing.mouseHasPressedButtonFn;
    pub const pending_input_entry_limit = contracts.max_pending_client_input_entries;
    pub const pending_input_limit = contracts.pending_input_capacity_bytes;
    pub const pending_terminal_reply_entry_limit = contracts.max_pending_terminal_reply_entries;
    pub const pending_terminal_reply_limit = contracts.pending_terminal_reply_capacity_bytes;
    pub const writer_slice_limit = contracts.max_pty_write_slice_bytes;
    pub const ReaderBatchOutcomeType = contracts.ReaderBatchOutcome;
    pub const ReadCommitCheckpointType = contracts.ReadCommitCheckpoint;
    pub const resizePtyWithRollbackFn = resize_ops.resizePtyWithRollback;
    pub const RuntimeTestingHooksType = contracts.RuntimeTestingHooks;

    pub fn cancelInput(self: *handle.TerminalRuntime, identifier: u64) void {
        return input_commit_ops.cancelInput(handle.stateFrom(self), identifier);
    }

    pub fn closeClientInput(self: *handle.TerminalRuntime) void {
        return writer_ops.closeClientInput(handle.stateFrom(self));
    }

    pub fn closeFailureFence(
        self: *handle.TerminalRuntime,
        failure: ?contracts.RuntimeFailureKind,
    ) bool {
        return failure_ops.closeFailureFence(handle.stateFrom(self), failure);
    }

    pub fn commitReaderBatch(
        self: *handle.TerminalRuntime,
        bytes: []const u8,
    ) contracts.ReaderBatchOutcome {
        return output_ops.commitReaderBatch(handle.stateFrom(self), bytes);
    }

    pub fn publishInput(self: *handle.TerminalRuntime, reservation: contracts.InputReservation, bytes: []u8) !void {
        return input_commit_ops.publishInput(handle.stateFrom(self), reservation, bytes);
    }

    pub fn markNormalCompletionReady(self: *handle.TerminalRuntime) void {
        return completion_ops.markNormalCompletionReady(handle.stateFrom(self));
    }

    pub fn recordWorkerIntegrityFailure(self: *handle.TerminalRuntime) void {
        return failure_ops.recordWorkerIntegrityFailure(handle.stateFrom(self));
    }

    pub fn recordReaderFailure(self: *handle.TerminalRuntime) void {
        return failure_ops.recordReaderFailure(handle.stateFrom(self));
    }

    pub fn recordWriterFailure(self: *handle.TerminalRuntime) void {
        return failure_ops.recordWriterFailure(handle.stateFrom(self));
    }

    pub fn reserveInput(
        self: *handle.TerminalRuntime,
        byte_count: usize,
        wait: bool,
        source: contracts.InputSource,
    ) !contracts.InputReservation {
        return input_admission_ops.reserveInput(handle.stateFrom(self), byte_count, wait, source);
    }

    pub fn createWithOptionsUsingSpawner(
        allocator: std.mem.Allocator,
        launch: contracts.ProcessLaunch,
        options: contracts.Options,
        spawner: anytype,
    ) !*handle.TerminalRuntime {
        return handle.fromState(try creation_ops.createWithOptionsUsingSpawner(
            allocator,
            launch,
            options,
            spawner,
        ));
    }

    pub const createWithOptionsUsingSpawnerFn = createWithOptionsUsingSpawner;

    pub fn stateForTesting(runtime: *handle.TerminalRuntime) *state.State {
        return handle.stateFrom(runtime);
    }

    pub fn clearTestingHooks(runtime: *handle.TerminalRuntime) void {
        handle.stateFrom(runtime).testing_hooks = .{};
    }

    pub fn firstContainmentAttempt(
        runtime: *const handle.TerminalRuntime,
    ) contracts.FirstContainmentAttempt {
        return handle.stateFromConst(runtime).first_containment_attempt.load();
    }

    pub fn hasAttachmentInputOwnership(
        runtime: *handle.TerminalRuntime,
        attachment_id: contracts.AttachmentId,
    ) bool {
        return input_admission_ops.hasAttachmentInputOwnership(
            handle.stateFrom(runtime),
            attachment_id,
        );
    }

    pub fn inputQueueSnapshot(runtime: *handle.TerminalRuntime) InputQueueSnapshot {
        const runtime_state = handle.stateFrom(runtime);
        runtime_state.input_mutex.lock();
        defer runtime_state.input_mutex.unlock();
        return .{
            .client_charge = runtime_state.input_queue.charge(.client),
            .entry_count = runtime_state.input_queue.entryCount(),
            .published_client_entries = runtime_state.input_queue.publishedCount(.client),
            .terminal_reply_charge = runtime_state.input_queue.charge(.terminal),
        };
    }

    pub fn failureFenced(runtime: *handle.TerminalRuntime) bool {
        const runtime_state = handle.stateFrom(runtime);
        runtime_state.terminal_mutex.lock();
        defer runtime_state.terminal_mutex.unlock();
        return runtime_state.failure_fenced;
    }

    pub fn releaseOutputForShutdown(runtime: *handle.TerminalRuntime) !void {
        const runtime_state = handle.stateFrom(runtime);
        lifecycle_ops.stopIoForShutdown(runtime_state);
        try lifecycle_ops.releaseOutputForShutdown(runtime_state);
    }

    pub fn resizeEffectInFlight(runtime: *handle.TerminalRuntime) bool {
        const runtime_state = handle.stateFrom(runtime);
        runtime_state.terminal_mutex.lock();
        defer runtime_state.terminal_mutex.unlock();
        return runtime_state.resize_effect_in_flight;
    }

    pub fn recordTerminalReplyFailure(runtime: *handle.TerminalRuntime) void {
        failure_ops.recordFailure(handle.stateFrom(runtime), .terminal_reply);
    }

    pub fn fenceResizeFailure(runtime: *handle.TerminalRuntime) void {
        resize_ops.fenceResizeFailure(handle.stateFrom(runtime));
    }
} else struct {};
