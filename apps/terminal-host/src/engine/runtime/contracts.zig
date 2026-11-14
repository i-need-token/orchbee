const std = @import("std");
const target_builtin = @import("builtin");

const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const pty_process = @import("terminal-host-platform").process;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_history = @import("../history/root.zig");
const terminal_input = @import("terminal-input");
const terminal_input_ownership = @import("../input_ownership.zig");
const terminal_input_queue = @import("../input_queue.zig");
const terminal_input_encoding = @import("../input_encoding.zig");
const terminal_profile = @import("../profile.zig");
const terminal_theme = model.theme;
const terminal_projection = @import("../projection/root.zig");
const terminal_runtime_completion = @import("../runtime_completion.zig");
const terminal_runtime_containment = @import("../runtime_containment.zig");
const terminal_runtime_launch = @import("../runtime_launch.zig");
const terminal_runtime_options = @import("../runtime_options.zig");
const terminal_runtime_startup = @import("../runtime_startup.zig");
const terminal_stream = @import("../terminal_stream.zig");

pub const pending_input_capacity_bytes = terminal_input_queue.client_capacity_bytes;
pub const max_pending_client_input_entries = terminal_input_queue.client_capacity_entries;
pub const max_pty_read_batch_bytes = protocol.Limits.pty_read_batch_bytes;
// Terminal-generated protocol replies must not compete with client input for
// admission. A bounded child can still exhaust its own reply budget, but a
// legal full client queue cannot manufacture a terminal-integrity failure.
pub const pending_terminal_reply_capacity_bytes = terminal_input_queue.terminal_reply_capacity_bytes;
pub const max_pending_terminal_reply_entries = terminal_input_queue.terminal_reply_capacity_entries;
pub const max_pty_write_slice_bytes = terminal_input_queue.writer_slice_bytes;
// Terminal-generated replies keep priority within a bounded reply burst: the
// writer yields to a waiting accepted client entry once this many reply slices
// have been written in the current burst. Sustained reply work therefore
// cannot starve accepted client input indefinitely; each slice is at most
// max_pty_write_slice_bytes, so a burst is at most 4 MiB of reply work.
pub const max_terminal_reply_burst_slices = terminal_input_queue.terminal_reply_burst_slices;
pub const max_process_containment_observations = 8;
pub const process_containment_observation_delay_ns = 10 * std.time.ns_per_ms;
pub const required_consecutive_containment_observations = 2;

pub const max_key_encoded_bytes = terminal_input_encoding.max_key_bytes;
pub const max_mouse_encoded_bytes = terminal_input_encoding.max_mouse_bytes;
pub const max_focus_encoded_bytes = terminal_input_encoding.max_focus_bytes;

pub const ColorScheme = terminal_profile.ColorScheme;
pub const TerminalProfile = terminal_profile.Profile;

pub const Options = terminal_runtime_options.Options;

pub const ProcessLaunch = pty_process.Launch;

pub const OwnedProfile = terminal_profile.Owned;

pub const InputReservation = terminal_input_queue.Reservation;
pub const InputSource = terminal_input_queue.Source;
pub const PreparedInputOwnership = terminal_input_ownership.PreparedMutation;

pub const ReaderBatchOutcome = enum {
    committed,
    failure_fenced,
    output_aborted,
    runtime_failed,
};

pub const ScrollbackCompressionOutcome = enum {
    complete,
    dormant,
    idle_wait,
    lock_busy,
    pending,
    unsupported,
};

pub const ScrollbackCompressionSchedule = struct {
    compression_activity: ?u64 = null,
    eligible_at_ns: i128 = 0,
    interaction_serial: u64 = 0,
    observed_interaction_serial: u64 = 0,
    phase: enum {
        waiting,
        pending,
        dormant,
        unsupported,
    } = .waiting,
};

pub const ReadCommitCheckpoint = enum {
    before_lock,
    locked,
};

pub const FirstContainmentAttempt = terminal_runtime_containment.FirstAttempt;

pub const RuntimeTestingHooks = if (target_builtin.is_test) struct {
    animation_wake_requested: ?*const fn (*anyopaque) void = null,
    abort_read: ?*const fn (*anyopaque) anyerror!void = null,
    contain_process_tree: ?*const fn (*anyopaque) anyerror!void = null,
    context: ?*anyopaque = null,
    failure_fence_starting: ?*const fn (*anyopaque) void = null,
    finish_pty_after_child_exit: ?*const fn (*anyopaque) anyerror!void = null,
    has_active_command: ?*const fn (*anyopaque) anyerror!bool = null,
    host_containment_waiting: ?*const fn (*anyopaque) void = null,
    projection_capture_starting: ?*const fn (*anyopaque) void = null,
    pty_drain_publication_starting: ?*const fn (*anyopaque) void = null,
    read_pty: ?*const fn (*anyopaque, []u8) anyerror!usize = null,
    read_commit_checkpoint: ?*const fn (*anyopaque, ReadCommitCheckpoint) void = null,
    reader_commit_waiting: ?*const fn (*anyopaque) void = null,
    reader_resize_waiting: ?*const fn (*anyopaque) void = null,
    reader_batch_finished: ?*const fn (*anyopaque, ReaderBatchOutcome) void = null,
    reader_stopped: ?*const fn (*anyopaque) void = null,
    monotonic_time_nanoseconds: ?*const fn (*anyopaque) u64 = null,
    resize_effect_returned: ?*const fn (*anyopaque) void = null,
    resize_pty: ?*const fn (
        *anyopaque,
        terminal_geometry.Geometry,
        terminal_geometry.Geometry,
    ) anyerror!void = null,
    write_pty: ?*const fn (*anyopaque, []const u8) anyerror!usize = null,
    writer_waiting_for_input: ?*const fn (*anyopaque) void = null,
    writer_slice_accounted: ?*const fn (*anyopaque) void = null,
    writer_slice_copied: ?*const fn (*anyopaque) void = null,
    writer_slice_retired: ?*const fn (*anyopaque) void = null,
    writer_slice_started: ?*const fn (*anyopaque) void = null,
    wall_time_milliseconds: ?*const fn (*anyopaque) u64 = null,
} else struct {};

pub const WriterSlice = terminal_input_queue.WriterSlice;

pub const WriterSliceStart = enum {
    integrity_failure,
    retired,
    started,
    stopped,
    ownership_lost,
};

pub const InputCharge = terminal_input_queue.Charge;

pub const KeyInput = terminal_input.KeyInput;
pub const MouseInput = terminal_input.MouseInput;

pub const DeviceAttributes = terminal_profile.DeviceAttributes;
pub const TerminalProgress = protocol.TerminalProgress;
pub const TerminalCommandCompletion = protocol.TerminalCommandCompletion;
pub const TerminalProgramStatus = protocol.TerminalProgramStatus;

pub const MetadataEvent = struct {
    allocator: std.mem.Allocator,
    connection_id: u64,
    current_directory: ?[]u8,
    revision: u64,
    title: ?[]u8,

    pub fn deinit(self: *@This()) void {
        if (self.current_directory) |value| self.allocator.free(value);
        if (self.title) |value| self.allocator.free(value);
        self.* = undefined;
    }
};

pub const ClipboardWriteRequest = struct {
    sequence: u64,
    text: []u8,
};

pub const PreparedClipboardWriteRequest = struct {
    sequence: u64,
    text: []const u8,
};

pub const ApplicationSignal = struct {
    payload: []u8,
    sequence: u64,
};

pub const max_pending_application_signals: usize = 64;

pub const ApplicationSignalEvent = struct {
    allocator: std.mem.Allocator,
    payload: []u8,
    sequence: u64,

    pub fn deinit(self: *@This()) void {
        self.allocator.free(self.payload);
        self.* = undefined;
    }
};

pub const ProjectionCounters = terminal_projection.Counters;
pub const ProjectionEncodeRequest = terminal_projection.EncodeRequest;
pub const Capabilities = model.attachment.Capabilities;
pub const AttachmentId = identifiers.AttachmentId;
pub const HostEpoch = identifiers.HostEpoch;
pub const ProjectionFrame = terminal_projection.EncodedFrame;
pub const ProjectionBlockReason = enum {
    capture_allocation_failed,
    projection_too_large,
};
pub const ProjectionBlockedCapture = struct {
    attempted_revision: u64,
    reason: ProjectionBlockReason,
};
pub const ProjectionEncodeOutcome = union(enum) {
    blocked: ProjectionBlockedCapture,
    frame: ProjectionFrame,
};
pub const ProjectionEncodeError = error{
    SynchronizedOutput,
    TerminalResizeFailed,
    TerminalRuntimeUnavailable,
};

pub const HistoryDemand = terminal_history.HistoryDemand;
pub const HistoryEvent = terminal_history.HistoryEvent;
pub const HistoryReadRequest = terminal_history.ReadRequest;
pub const HistoryRows = terminal_history.EncodedRows;
pub const OwnedHistoryRows = terminal_history.OwnedEncodedRows;
pub const SelectionCommandRequest = terminal_history.SelectionCommandRequest;
pub const SelectionExportCancellation = terminal_history.ExportCancellation;
pub const PreparedSelectionExportCancellation = terminal_history.PreparedExportCancellation;
pub const SelectionExportChunk = terminal_history.EncodedExportChunk;
pub const SelectionExportChunkToken = terminal_history.ExportChunkToken;
pub const SelectionExportFinished = terminal_history.ExportFinished;
pub const SelectionExportFormat = terminal_history.ExportFormat;
pub const SelectionExportInvalidationSnapshot = terminal_history.ExportInvalidationSnapshot;
pub const SelectionExportStarted = terminal_history.ExportStarted;
pub const SelectionGesture = terminal_history.SelectionGesture;
pub const SelectionSnapshot = terminal_history.SelectionSnapshot;

pub const SelectionExportStartResult = struct {
    cancellation_count: usize,
    cancellations: [terminal_history.max_pending_export_cancellations]SelectionExportCancellation,
    invalidation_history: SelectionExportInvalidationSnapshot,
    lock_hold_nanoseconds: u64,
    previous_completed_export_sequence: ?u64,
    previous_last_export_sequence: u64,
    started: SelectionExportStarted,
};

pub const Geometry = struct {
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    columns: u16,
    generation: u64,
    revision: u64,
    rows: u16,
};

pub const ConfigurationUpdate = struct {
    generation: u64,
    history_rows_evicted: u64,
    revision: u64,
};

pub const TerminalTheme = terminal_theme.Theme;

pub const ThemeUpdate = struct {
    generation: u64,
    revision: u64,
};

pub const TerminalControlOperation = enum {
    clear,
    clear_prompt,
    reset,
};

pub const TerminalControlUpdate = struct {
    generation: u64,
    revision: u64,
};

pub const RuntimeFailureKind = terminal_runtime_completion.FailureKind;

pub const HostFailureFenceResult = union(enum) {
    host_won,
    engine_won: RuntimeFailureKind,
};

pub const CompletionState = terminal_runtime_completion.State;
pub const CompletionClassification = terminal_runtime_completion.Classification;
pub const Completion = terminal_runtime_completion.Snapshot;

/// Semantic owner-cleanup failures. Platform errors are classified at this
/// boundary so HostCore never has to infer which shutdown phase failed.
pub const ShutdownError = error{
    OutputReleaseFailed,
    ProcessContainmentFailed,
    ProcessIdentityReleaseFailed,
};

pub const ShutdownCleanupPhase = enum {
    contain_process,
    release_output,
    release_process_identity,
};
