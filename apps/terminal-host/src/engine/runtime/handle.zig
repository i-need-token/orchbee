const std = @import("std");
const identifiers = @import("terminal-host-model").identifiers;
const pty_process = @import("terminal-host-platform").process;
const terminal_configuration = @import("terminal-host-model").configuration;
const terminal_input = @import("terminal-input");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const creation_ops = @import("creation.zig");
const lifecycle_ops = @import("lifecycle.zig");
const metadata_ops = @import("metadata.zig");
const completion_ops = @import("completion.zig");
const failure_ops = @import("failure.zig");
const feedback_handle = @import("feedback_handle.zig");
const projection_ops = @import("projection.zig");
const input_handle = @import("input_handle.zig");
const terminal_control_ops = @import("terminal_control.zig");
const process_io_ops = @import("process_io.zig");
const resize_ops = @import("resize.zig");
const scrollback_compression_ops = @import("scrollback_compression.zig");
const theme_ops = @import("theme.zig");
const attachment_ops = @import("attachment.zig");
const interaction_ops = @import("interaction.zig");
const prepared_input = @import("prepared_input.zig");
const prepared_attachment = @import("prepared_attachment.zig");
const read_commit_guard = @import("read_commit_guard.zig");

pub const PreparedInput = prepared_input.Type(TerminalRuntime);
pub const PreparedAttachment = prepared_attachment.Type(TerminalRuntime);
pub const ReadCommitGuard = read_commit_guard.Type(TerminalRuntime);
const InputHandle = input_handle.Type(TerminalRuntime, PreparedInput);
const FeedbackHandle = feedback_handle.Type(TerminalRuntime);

pub const TerminalRuntime = opaque {
    pub fn create(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
    ) !*TerminalRuntime {
        return fromState(try creation_ops.create(allocator, launch));
    }

    pub fn createWithOptions(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
        options: contracts.Options,
    ) !*TerminalRuntime {
        return fromState(try creation_ops.createWithOptions(allocator, launch, options));
    }

    pub fn shutdown(self: *TerminalRuntime) contracts.ShutdownError!void {
        return lifecycle_ops.shutdown(stateFrom(self));
    }

    pub fn pendingShutdownCleanup(self: *const TerminalRuntime) ?contracts.ShutdownCleanupPhase {
        return lifecycle_ops.pendingShutdownCleanup(stateFromConst(self));
    }

    pub fn destroy(self: *TerminalRuntime) void {
        return lifecycle_ops.destroy(stateFrom(self));
    }

    pub fn destroyAfterShutdown(self: *TerminalRuntime) void {
        return lifecycle_ops.destroyAfterShutdown(stateFrom(self));
    }

    pub fn completion(self: *const TerminalRuntime) contracts.Completion {
        return completion_ops.completion(stateFromConst(self));
    }

    pub fn waitForCompletion(self: *TerminalRuntime) !contracts.Completion {
        return completion_ops.waitForCompletion(stateFrom(self));
    }

    pub fn isResizeFenced(self: *const TerminalRuntime) bool {
        return failure_ops.isResizeFenced(stateFromConst(self));
    }

    pub fn fenceHostFailure(self: *TerminalRuntime) contracts.HostFailureFenceResult {
        return failure_ops.fenceHostFailure(stateFrom(self));
    }

    pub fn primaryFailure(self: *const TerminalRuntime) ?contracts.RuntimeFailureKind {
        return failure_ops.primaryFailure(stateFromConst(self));
    }

    pub fn readableStateAvailable(self: *TerminalRuntime) bool {
        return failure_ops.readableStateAvailable(stateFrom(self));
    }

    pub fn bellSequence(self: *TerminalRuntime) u64 {
        return projection_ops.bellSequence(stateFrom(self));
    }

    pub fn prepareMetadataEvent(
        self: *TerminalRuntime,
        allocator: std.mem.Allocator,
        connection_id: u64,
    ) !?contracts.MetadataEvent {
        return metadata_ops.prepareEvent(stateFrom(self), allocator, connection_id);
    }

    pub fn commitMetadataEvent(
        self: *TerminalRuntime,
        event: contracts.MetadataEvent,
    ) void {
        return metadata_ops.commitEvent(stateFrom(self), event);
    }

    pub const prepareClipboardWriteRequest = FeedbackHandle.prepareClipboardWriteRequest;
    pub const rollbackClipboardWriteRequest = FeedbackHandle.rollbackClipboardWriteRequest;
    pub const commitClipboardWriteRequest = FeedbackHandle.commitClipboardWriteRequest;
    pub const prepareApplicationSignal = FeedbackHandle.prepareApplicationSignal;
    pub const commitApplicationSignal = FeedbackHandle.commitApplicationSignal;

    pub fn acquireReadCommitGuard(
        self: *TerminalRuntime,
        expected_close_activity_epoch: u64,
        expected_close_activity_epoch_exhausted: bool,
        expected_generation: u64,
        expected_read_commit_epoch: u64,
    ) contracts.ProjectionEncodeError!ReadCommitGuard {
        return exposeReadCommitGuard(try projection_ops.acquireReadCommitGuard(
            stateFrom(self),
            expected_close_activity_epoch,
            expected_close_activity_epoch_exhausted,
            expected_generation,
            expected_read_commit_epoch,
        ));
    }

    pub const enqueueInput = InputHandle.enqueueInput;
    pub const enqueueRawInput = InputHandle.enqueueRawInput;
    pub const prepareRawInput = InputHandle.prepareRawInput;
    pub const enqueueKeyInput = InputHandle.enqueueKeyInput;
    pub const keyInputWouldEmit = InputHandle.keyInputWouldEmit;
    pub const enqueueKeyInputWithLimit = InputHandle.enqueueKeyInputWithLimit;
    pub const prepareKeyInput = InputHandle.prepareKeyInput;
    pub const enqueueTextInput = InputHandle.enqueueTextInput;
    pub const enqueueTextInputWithLimit = InputHandle.enqueueTextInputWithLimit;
    pub const prepareTextInput = InputHandle.prepareTextInput;
    pub const enqueueMouseInput = InputHandle.enqueueMouseInput;
    pub const mouseInputWouldEmit = InputHandle.mouseInputWouldEmit;
    pub const enqueueMouseInputWithLimit = InputHandle.enqueueMouseInputWithLimit;
    pub const prepareMouseInput = InputHandle.prepareMouseInput;
    pub const enqueueFocusInput = InputHandle.enqueueFocusInput;
    pub const focusInputWouldEmit = InputHandle.focusInputWouldEmit;
    pub const enqueueFocusInputWithLimit = InputHandle.enqueueFocusInputWithLimit;
    pub const prepareFocusInput = InputHandle.prepareFocusInput;
    pub const enqueuePasteInput = InputHandle.enqueuePasteInput;
    pub const enqueuePasteInputWithLimit = InputHandle.enqueuePasteInputWithLimit;
    pub const preparePasteInput = InputHandle.preparePasteInput;
    pub const prepareSemanticInput = InputHandle.prepareSemanticInput;
    pub const prepareAttachmentSemanticInput = InputHandle.prepareAttachmentSemanticInput;
    pub const prepareAttachmentInputRepair = InputHandle.prepareAttachmentInputRepair;
    pub const prepareAttachmentInputReset = InputHandle.prepareAttachmentInputReset;
    pub const pendingInputBytes = InputHandle.pendingInputBytes;
    pub const takeWrittenBytes = InputHandle.takeWrittenBytes;
    pub const takeOutputBatches = InputHandle.takeOutputBatches;

    pub fn applyTerminalControl(
        self: *TerminalRuntime,
        operation: contracts.TerminalControlOperation,
    ) !contracts.TerminalControlUpdate {
        return terminal_control_ops.applyTerminalControl(stateFrom(self), operation);
    }

    pub fn resize(self: *TerminalRuntime, columns: u16, rows: u16) !void {
        return resize_ops.resize(stateFrom(self), columns, rows);
    }

    pub fn resizeGeometry(
        self: *TerminalRuntime,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !u64 {
        return resize_ops.resizeGeometry(stateFrom(self), columns, rows, cell_pixel_width, cell_pixel_height);
    }

    pub fn configuration(self: *TerminalRuntime) terminal_configuration.Configuration {
        return terminal_control_ops.configuration(stateFrom(self));
    }

    pub fn updateConfiguration(
        self: *TerminalRuntime,
        next_configuration: terminal_configuration.Configuration,
    ) !contracts.ConfigurationUpdate {
        return terminal_control_ops.updateConfiguration(stateFrom(self), next_configuration);
    }

    pub fn themeDefaults(self: *TerminalRuntime) contracts.TerminalTheme {
        return theme_ops.themeDefaults(stateFrom(self));
    }

    pub fn applyTheme(
        self: *TerminalRuntime,
        next_theme: contracts.TerminalTheme,
    ) !contracts.ThemeUpdate {
        return theme_ops.applyTheme(stateFrom(self), next_theme);
    }

    pub fn terminate(self: *TerminalRuntime) !void {
        return process_io_ops.terminate(stateFrom(self));
    }

    pub fn containmentConfirmed(self: *const TerminalRuntime) bool {
        return process_io_ops.containmentConfirmed(stateFromConst(self));
    }

    pub fn forceTerminate(self: *TerminalRuntime) !void {
        return process_io_ops.forceTerminate(stateFrom(self));
    }

    pub fn plainString(self: *TerminalRuntime, allocator: std.mem.Allocator) ![]const u8 {
        return projection_ops.plainString(stateFrom(self), allocator);
    }

    pub fn geometry(self: *TerminalRuntime) contracts.Geometry {
        return projection_ops.geometry(stateFrom(self));
    }

    pub fn projectionCounters(self: *TerminalRuntime) contracts.ProjectionCounters {
        return projection_ops.projectionCounters(stateFrom(self));
    }

    pub fn nextProjectionAt(self: *const TerminalRuntime) i128 {
        return projection_ops.nextProjectionAt(stateFromConst(self));
    }

    pub fn setNextProjectionAt(self: *TerminalRuntime, value: i128) void {
        return projection_ops.setNextProjectionAt(stateFrom(self), value);
    }

    pub fn nextCloseActivitySampleAt(self: *const TerminalRuntime) i128 {
        return projection_ops.nextCloseActivitySampleAt(stateFromConst(self));
    }

    pub fn setNextCloseActivitySampleAt(self: *TerminalRuntime, value: i128) void {
        return projection_ops.setNextCloseActivitySampleAt(stateFrom(self), value);
    }

    pub fn sampleCloseActivity(
        self: *TerminalRuntime,
    ) contracts.ProjectionEncodeError!contracts.CloseActivityRefresh {
        return projection_ops.sampleCloseActivity(stateFrom(self));
    }

    pub fn projectionResident(self: *TerminalRuntime) bool {
        return projection_ops.projectionResident(stateFrom(self));
    }

    pub fn releaseProjectionResources(self: *TerminalRuntime) void {
        return projection_ops.releaseProjectionResources(stateFrom(self));
    }

    pub fn maintainScrollbackCompression(
        self: *TerminalRuntime,
        now_ns: i128,
    ) contracts.ScrollbackCompressionOutcome {
        return scrollback_compression_ops.maintain(stateFrom(self), now_ns);
    }

    pub fn encodeProjectionFrame(
        self: *TerminalRuntime,
        request: contracts.ProjectionEncodeRequest,
    ) !contracts.ProjectionFrame {
        return projection_ops.encodeProjectionFrame(stateFrom(self), request);
    }

    pub fn encodeProjectionFrameOwned(
        self: *TerminalRuntime,
        output_allocator: std.mem.Allocator,
        output_prefix_bytes: usize,
        request: contracts.ProjectionEncodeRequest,
    ) contracts.ProjectionEncodeError!contracts.ProjectionEncodeOutcome {
        return projection_ops.encodeProjectionFrameOwned(stateFrom(self), output_allocator, output_prefix_bytes, request);
    }

    pub fn prepareAttachment(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        capabilities: contracts.Capabilities,
    ) !PreparedAttachment {
        return exposePreparedAttachment(try attachment_ops.prepareAttachment(stateFrom(self), attachment_id, capabilities));
    }

    pub fn detachInput(self: *TerminalRuntime, attachment_id: identifiers.AttachmentId) void {
        return attachment_ops.detachInput(stateFrom(self), attachment_id);
    }

    pub fn resetInput(self: *TerminalRuntime, attachment_id: identifiers.AttachmentId) void {
        return attachment_ops.resetInput(stateFrom(self), attachment_id);
    }

    pub fn detachHistory(self: *TerminalRuntime, attachment_id: identifiers.AttachmentId) void {
        return attachment_ops.detachHistory(stateFrom(self), attachment_id);
    }

    pub fn setHistoryDemand(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        demand: contracts.HistoryDemand,
    ) !void {
        return interaction_ops.setHistoryDemand(stateFrom(self), attachment_id, demand);
    }

    pub fn readHistoryRows(
        self: *TerminalRuntime,
        output_allocator: std.mem.Allocator,
        request: contracts.HistoryReadRequest,
    ) !contracts.HistoryRows {
        return interaction_ops.readHistoryRows(stateFrom(self), output_allocator, request);
    }

    pub fn readHistoryRowsInto(
        self: *TerminalRuntime,
        scratch_allocator: std.mem.Allocator,
        destination: []u8,
        request: contracts.HistoryReadRequest,
    ) !contracts.HistoryRows {
        return interaction_ops.readHistoryRowsInto(stateFrom(self), scratch_allocator, destination, request);
    }

    pub fn historyRowsApplied(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        request_sequence: u64,
        revision: u64,
        generation: u64,
    ) !void {
        return interaction_ops.historyRowsApplied(stateFrom(self), attachment_id, request_sequence, revision, generation);
    }

    pub fn takeHistoryEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?contracts.HistoryEvent {
        return interaction_ops.takeHistoryEvent(stateFrom(self), attachment_id);
    }

    pub fn rollbackHistoryEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        event: contracts.HistoryEvent,
    ) !void {
        return interaction_ops.rollbackHistoryEvent(stateFrom(self), attachment_id, event);
    }

    pub fn historyObserved(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) !void {
        return interaction_ops.historyObserved(stateFrom(self), attachment_id, sequence, revision, generation);
    }

    pub fn takeSelectionEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?contracts.SelectionSnapshot {
        return interaction_ops.takeSelectionEvent(stateFrom(self), attachment_id);
    }

    pub fn rollbackSelectionEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        event: contracts.SelectionSnapshot,
    ) !void {
        return interaction_ops.rollbackSelectionEvent(stateFrom(self), attachment_id, event);
    }

    pub fn takeSelectionExportCancellation(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?contracts.SelectionExportCancellation {
        return interaction_ops.takeSelectionExportCancellation(stateFrom(self), attachment_id);
    }

    pub fn prepareSelectionExportCancellation(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?contracts.PreparedSelectionExportCancellation {
        return interaction_ops.prepareSelectionExportCancellation(stateFrom(self), attachment_id);
    }

    pub fn commitPreparedSelectionExportCancellation(
        self: *TerminalRuntime,
        prepared: contracts.PreparedSelectionExportCancellation,
    ) void {
        return interaction_ops.commitPreparedSelectionExportCancellation(stateFrom(self), prepared);
    }

    pub fn applySelectionGesture(
        self: *TerminalRuntime,
        request: contracts.SelectionGesture,
    ) !contracts.SelectionSnapshot {
        return interaction_ops.applySelectionGesture(stateFrom(self), request);
    }

    pub fn applySelectionCommand(
        self: *TerminalRuntime,
        request: contracts.SelectionCommandRequest,
    ) !contracts.SelectionSnapshot {
        return interaction_ops.applySelectionCommand(stateFrom(self), request);
    }

    pub fn startSelectionExport(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        export_sequence: u64,
        format: contracts.SelectionExportFormat,
    ) !contracts.SelectionExportStartResult {
        return interaction_ops.startSelectionExport(stateFrom(self), attachment_id, export_sequence, format);
    }

    pub fn rollbackSelectionExportStart(
        self: *TerminalRuntime,
        result: contracts.SelectionExportStartResult,
    ) void {
        return interaction_ops.rollbackSelectionExportStart(stateFrom(self), result);
    }

    pub fn nextSelectionExportChunk(
        self: *TerminalRuntime,
        output_allocator: std.mem.Allocator,
        attachment_id: identifiers.AttachmentId,
    ) !?contracts.SelectionExportChunk {
        return interaction_ops.nextSelectionExportChunk(stateFrom(self), output_allocator, attachment_id);
    }

    pub fn rollbackSelectionExportChunk(
        self: *TerminalRuntime,
        token: contracts.SelectionExportChunkToken,
    ) !void {
        return interaction_ops.rollbackSelectionExportChunk(stateFrom(self), token);
    }

    pub fn selectionExportChunkApplied(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        export_sequence: u64,
        chunk_sequence: u32,
    ) !?contracts.SelectionExportFinished {
        return interaction_ops.selectionExportChunkApplied(stateFrom(self), attachment_id, export_sequence, chunk_sequence);
    }

    pub fn cancelSelectionExport(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        export_sequence: u64,
    ) !contracts.SelectionExportFinished {
        return interaction_ops.cancelSelectionExport(stateFrom(self), attachment_id, export_sequence);
    }
};

pub fn fromState(runtime: *state.State) *TerminalRuntime {
    return @ptrCast(runtime);
}

pub fn stateFrom(runtime: *TerminalRuntime) *state.State {
    return @ptrCast(@alignCast(runtime));
}

pub fn stateFromConst(runtime: *const TerminalRuntime) *const state.State {
    return @ptrCast(@alignCast(runtime));
}

fn exposePreparedAttachment(internal: prepared_attachment.Internal) PreparedAttachment {
    return .{
        .attachment_id = internal.attachment_id,
        .bell_sequence = internal.bell_sequence,
        .history_reserved = internal.history_reserved,
        .input_state_reserved = internal.input_state_reserved,
        .runtime = fromState(internal.runtime),
    };
}

fn exposeReadCommitGuard(internal: read_commit_guard.Internal) ReadCommitGuard {
    return .{ .runtime = fromState(internal.runtime) };
}
