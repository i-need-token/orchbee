const std = @import("std");
const builtin = @import("builtin");
const terminal_input_encoding = @import("../input_encoding.zig");
const terminal_configuration = @import("terminal-host-model").configuration;
const terminal_geometry = @import("terminal-host-model").geometry;
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
const projection_ops = @import("projection.zig");
const resize_ops = @import("resize.zig");
const terminal_control_ops = @import("terminal_control.zig");
const writer_ops = @import("writer.zig");

pub const testing = if (builtin.is_test) struct {
    pub const InputQueueSnapshot = struct {
        client_charge: contracts.InputCharge,
        entry_count: usize,
        input_attachment_count: usize,
        published_client_entries: usize,
        repair_charge: contracts.InputCharge,
        terminal_reply_charge: contracts.InputCharge,
    };

    pub const RuntimeObservation = struct {
        client_input_closed: bool,
        completion: contracts.Completion,
        configuration: terminal_configuration.Configuration,
        containment_confirmed: bool,
        failure_fenced: bool,
        generation: u64,
        geometry: terminal_geometry.Geometry,
        history_attachment_count: usize,
        history_selection_count: usize,
        history_viewing_count: usize,
        input: InputQueueSnapshot,
        projection_captured_generation: u64,
        projection_captured_revision: u64,
        projection_resident: bool,
        resize_fenced: bool,
        threads_joined: bool,
        writer_stop: bool,
    };

    pub const InvariantError = error{
        ClientInputChargeOverflow,
        DuplicateHistoryAttachment,
        FailedCompletionWithoutFence,
        FailureFenceWithoutWriterStop,
        HistoryFlightAttachmentMismatch,
        HistoryFlightSequenceInvalid,
        HistorySelectionCountMismatch,
        HistoryViewingCountMismatch,
        InputQueueEntryCountMismatch,
        InvalidRuntimeConfiguration,
        InvalidRuntimeGeometry,
        NormalCompletionIncomplete,
        RepairChargeEntryOverflow,
        RepairChargeOverflow,
        TerminalReplyChargeOverflow,
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
    pub const resolveCloseActivityFn = projection_ops.resolveCloseActivity;
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
            .input_attachment_count = runtime_state.input_ownership.attachments.count(),
            .published_client_entries = runtime_state.input_queue.publishedCount(.client),
            .repair_charge = runtime_state.input_ownership.repairCharge(),
            .terminal_reply_charge = runtime_state.input_queue.charge(.terminal),
        };
    }

    /// Captures all composition-relevant runtime facts while following the
    /// runtime's canonical projection -> terminal -> input lock order. The
    /// value is an observation only; no semantic owner is duplicated.
    pub fn observe(runtime: *handle.TerminalRuntime) RuntimeObservation {
        const runtime_state = handle.stateFrom(runtime);
        runtime_state.projection_mutex.lock();
        defer runtime_state.projection_mutex.unlock();
        runtime_state.terminal_mutex.lock();
        defer runtime_state.terminal_mutex.unlock();
        failure_ops.waitForReaderCommitLocked(runtime_state);
        runtime_state.input_mutex.lock();
        defer runtime_state.input_mutex.unlock();

        return observeLocked(runtime_state);
    }

    pub fn checkInvariants(runtime: *handle.TerminalRuntime) InvariantError!void {
        const runtime_state = handle.stateFrom(runtime);
        runtime_state.projection_mutex.lock();
        defer runtime_state.projection_mutex.unlock();
        runtime_state.terminal_mutex.lock();
        defer runtime_state.terminal_mutex.unlock();
        failure_ops.waitForReaderCommitLocked(runtime_state);
        runtime_state.input_mutex.lock();
        defer runtime_state.input_mutex.unlock();

        const observation = observeLocked(runtime_state);
        if (!terminal_configuration.valid(observation.configuration)) {
            return error.InvalidRuntimeConfiguration;
        }
        if (!terminal_geometry.isValid(observation.geometry)) {
            return error.InvalidRuntimeGeometry;
        }
        if (observation.input.client_charge.bytes > contracts.pending_input_capacity_bytes) {
            return error.ClientInputChargeOverflow;
        }
        if (observation.input.client_charge.entries > contracts.max_pending_client_input_entries or
            observation.input.terminal_reply_charge.entries >
                contracts.max_pending_terminal_reply_entries)
        {
            return error.InputQueueEntryCountMismatch;
        }
        if (observation.input.terminal_reply_charge.bytes >
            contracts.pending_terminal_reply_capacity_bytes)
        {
            return error.TerminalReplyChargeOverflow;
        }
        if (observation.input.client_charge.entries +
            observation.input.terminal_reply_charge.entries != observation.input.entry_count)
        {
            return error.InputQueueEntryCountMismatch;
        }
        if (observation.input.repair_charge.bytes >
            contracts.pending_input_capacity_bytes -| observation.input.client_charge.bytes)
        {
            return error.RepairChargeOverflow;
        }
        if (observation.input.repair_charge.entries >
            contracts.max_pending_client_input_entries -| observation.input.client_charge.entries)
        {
            return error.RepairChargeEntryOverflow;
        }
        if (observation.failure_fenced and !observation.writer_stop) {
            return error.FailureFenceWithoutWriterStop;
        }
        switch (observation.completion.classification) {
            .pending => {},
            .normal => if (!observation.completion.child_exited or
                !observation.completion.pty_drained or
                !observation.completion.writer_stopped or
                observation.completion.exit_result == null)
            {
                return error.NormalCompletionIncomplete;
            },
            .failed => if (!observation.failure_fenced) {
                return error.FailedCompletionWithoutFence;
            },
        }

        var viewing_count: usize = 0;
        var selection_count: usize = 0;
        for (runtime_state.history.states.items, 0..) |history_state, index| {
            viewing_count += @intFromBool(history_state.demand == .viewing_history);
            selection_count += @intFromBool(history_state.selection_state.active());
            for (runtime_state.history.states.items[index + 1 ..]) |candidate| {
                if (candidate.attachment_id.eql(history_state.attachment_id)) {
                    return error.DuplicateHistoryAttachment;
                }
            }
            if (history_state.history_in_flight) |flight| {
                if (!flight.attachment_id.eql(history_state.attachment_id)) {
                    return error.HistoryFlightAttachmentMismatch;
                }
                if (flight.sequence == 0 or flight.sequence >= history_state.history_next_sequence) {
                    return error.HistoryFlightSequenceInvalid;
                }
            }
        }
        if (viewing_count != observation.history_viewing_count) {
            return error.HistoryViewingCountMismatch;
        }
        if (selection_count != observation.history_selection_count) {
            return error.HistorySelectionCountMismatch;
        }
    }

    pub fn hasHistoryAttachment(
        runtime: *handle.TerminalRuntime,
        attachment_id: contracts.AttachmentId,
    ) bool {
        const runtime_state = handle.stateFrom(runtime);
        runtime_state.terminal_mutex.lock();
        defer runtime_state.terminal_mutex.unlock();
        failure_ops.waitForReaderCommitLocked(runtime_state);
        for (runtime_state.history.states.items) |history_state| {
            if (history_state.attachment_id.eql(attachment_id)) return true;
        }
        return false;
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

    fn observeLocked(runtime_state: *state.State) RuntimeObservation {
        const client_charge = runtime_state.input_queue.charge(.client);
        const terminal_reply_charge = runtime_state.input_queue.charge(.terminal);
        return .{
            .client_input_closed = runtime_state.client_input_closed,
            .completion = completion_ops.completion(runtime_state),
            .configuration = .{ .scrollback_bytes = runtime_state.scrollback_bytes },
            .containment_confirmed = runtime_state.containment_confirmed.load(.acquire),
            .failure_fenced = runtime_state.failure_fenced,
            .generation = runtime_state.generation,
            .geometry = .{
                .cell_pixel_height = runtime_state.cell_pixel_height,
                .cell_pixel_width = runtime_state.cell_pixel_width,
                .columns = runtime_state.terminal.cols,
                .rows = runtime_state.terminal.rows,
            },
            .history_attachment_count = runtime_state.history.states.items.len,
            .history_selection_count = runtime_state.history.selection_count,
            .history_viewing_count = runtime_state.history.viewing_count,
            .input = .{
                .client_charge = client_charge,
                .entry_count = runtime_state.input_queue.entryCount(),
                .input_attachment_count = runtime_state.input_ownership.attachments.count(),
                .published_client_entries = runtime_state.input_queue.publishedCount(.client),
                .repair_charge = runtime_state.input_ownership.repairCharge(),
                .terminal_reply_charge = terminal_reply_charge,
            },
            .projection_captured_generation = runtime_state.projection.captured_generation,
            .projection_captured_revision = runtime_state.projection.captured_revision,
            .projection_resident = runtime_state.projection.isResident(),
            .resize_fenced = runtime_state.resize_fenced.load(.acquire),
            .threads_joined = runtime_state.threads_joined,
            .writer_stop = runtime_state.writer_stop,
        };
    }
} else struct {};
