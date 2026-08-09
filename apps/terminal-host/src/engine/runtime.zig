const std = @import("std");
const target_builtin = @import("builtin");

const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const pty_process = @import("terminal-host-platform").process;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_history = @import("history.zig");
const terminal_input = @import("terminal-input");
const terminal_input_encoding = @import("input_encoding.zig");
const terminal_profile = @import("profile.zig");
const terminal_projection = @import("projection.zig");

pub const pending_input_capacity_bytes = protocol.Limits.pending_input_bytes;
const max_pending_client_input_entries = 4096;
const max_pty_read_batch_bytes = protocol.Limits.pty_read_batch_bytes;
// Terminal-generated protocol replies must not compete with client input for
// admission. A bounded child can still exhaust its own reply budget, but a
// legal full client queue cannot manufacture a terminal-integrity failure.
const pending_terminal_reply_capacity_bytes = max_pty_read_batch_bytes;
const max_pending_terminal_reply_entries = 4096;
const max_pty_write_slice_bytes = 64 * 1024;
const max_process_containment_observations = 8;
const process_containment_observation_delay_ns = 10 * std.time.ns_per_ms;
const required_consecutive_containment_observations = 2;

pub const max_key_encoded_bytes = terminal_input_encoding.max_key_bytes;
pub const max_mouse_encoded_bytes = terminal_input_encoding.max_mouse_bytes;
pub const max_focus_encoded_bytes = terminal_input_encoding.max_focus_bytes;

pub const ColorScheme = terminal_profile.ColorScheme;
pub const TerminalProfile = terminal_profile.Profile;

pub const Options = struct {
    cell_pixel_height: u16 = 1,
    cell_pixel_width: u16 = 1,
    profile: ?TerminalProfile = null,
    scrollback_bytes: usize = terminal_configuration.default_scrollback_bytes,
};

const OwnedProfile = terminal_profile.Owned;

const InputEntry = struct {
    bytes: ?[]u8 = null,
    identifier: u64,
    offset: usize = 0,
    reserved_bytes: usize,
    source: InputSource,
};

const InputReservation = struct {
    identifier: u64,
    reserved_bytes: usize,
};

const InputSource = enum {
    client,
    terminal,
};

const ReaderBatchOutcome = enum {
    committed,
    failure_fenced,
    output_aborted,
    runtime_failed,
};

const FirstContainmentAttempt = enum(u8) {
    none,
    engine_pending,
    host_pending,
    host_running,
    done,
};

const RuntimeTestingHooks = if (target_builtin.is_test) struct {
    abort_read: ?*const fn (*anyopaque) anyerror!void = null,
    contain_process_tree: ?*const fn (*anyopaque) anyerror!void = null,
    context: ?*anyopaque = null,
    finish_pty_after_child_exit: ?*const fn (*anyopaque) anyerror!void = null,
    host_containment_waiting: ?*const fn (*anyopaque) void = null,
    read_pty: ?*const fn (*anyopaque, []u8) anyerror!usize = null,
    reader_batch_finished: ?*const fn (*anyopaque, ReaderBatchOutcome) void = null,
    reader_stopped: ?*const fn (*anyopaque) void = null,
    resize_effect_returned: ?*const fn (*anyopaque) void = null,
    resize_pty: ?*const fn (
        *anyopaque,
        terminal_geometry.Geometry,
        terminal_geometry.Geometry,
    ) anyerror!void = null,
    write_pty: ?*const fn (*anyopaque, []const u8) anyerror!usize = null,
    writer_slice_copied: ?*const fn (*anyopaque) void = null,
    writer_slice_started: ?*const fn (*anyopaque) void = null,
} else struct {};

const WriterSlice = struct {
    identifier: u64,
    length: usize,
    offset: usize,
    source: InputSource,
};

const WriterSliceStart = enum {
    started,
    stopped,
    ownership_lost,
};

const InputCharge = struct {
    bytes: usize = 0,
    entries: usize = 0,
};

fn inputCapacityBytes(source: InputSource) usize {
    return switch (source) {
        .client => pending_input_capacity_bytes,
        .terminal => pending_terminal_reply_capacity_bytes,
    };
}

fn inputCapacityEntries(source: InputSource) usize {
    return switch (source) {
        .client => max_pending_client_input_entries,
        .terminal => max_pending_terminal_reply_entries,
    };
}

/// Terminal protocol replies are selected before client bytes at each bounded
/// write slice. Relative order remains FIFO within each source. A pending
/// reply reservation blocks another client slice until its allocation is
/// either published or cancelled.
fn nextWritableInputEntry(entries: []const InputEntry) ?usize {
    for (entries, 0..) |entry, index| {
        if (entry.source != .terminal) continue;
        if (entry.bytes == null) return null;
        return index;
    }
    if (entries.len == 0 or entries[0].bytes == null) return null;
    return 0;
}

pub const KeyInput = terminal_input.KeyInput;
pub const MouseInput = terminal_input.MouseInput;

const DeviceAttributes = terminal_profile.DeviceAttributes;

pub const ProjectionCounters = terminal_projection.Counters;
pub const ProjectionEncodeRequest = terminal_projection.EncodeRequest;
pub const AttachmentId = identifiers.AttachmentId;
pub const HostEpoch = identifiers.HostEpoch;
pub const ProjectionFrame = terminal_projection.EncodedFrame;
pub const HistoryDemand = terminal_history.HistoryDemand;
pub const HistoryEvent = terminal_history.HistoryEvent;
pub const HistoryReadRequest = terminal_history.ReadRequest;
pub const HistoryRows = terminal_history.EncodedRows;
pub const SelectionCommandRequest = terminal_history.SelectionCommandRequest;
pub const SelectionExportCancellation = terminal_history.ExportCancellation;
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

const SystemThreadSpawner = struct {
    fn spawn(
        _: *SystemThreadSpawner,
        comptime thread_function: fn (*TerminalRuntime) void,
        runtime: *TerminalRuntime,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

const RuntimeStartupState = enum {
    pending,
    ready,
    aborted,
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

pub const RuntimeFailureKind = enum(u8) {
    process_observation = 1,
    pty_read = 2,
    pty_write = 3,
    resize = 4,
    terminal_reply = 5,
    worker_integrity = 6,
};

pub const HostFailureFenceResult = union(enum) {
    host_won,
    engine_won: RuntimeFailureKind,
};

pub const CompletionState = union(enum) {
    pending,
    normal,
    failed: RuntimeFailureKind,
};

const normal_completion_classification = std.math.maxInt(u8);

const CompletionClassification = struct {
    code: std.atomic.Value(u8) = .init(0),

    fn load(self: *const CompletionClassification) CompletionState {
        const code = self.code.load(.acquire);
        if (code == 0) return .pending;
        if (code == normal_completion_classification) return .normal;
        return .{ .failed = @enumFromInt(code) };
    }

    fn markFailure(
        self: *CompletionClassification,
        failure: RuntimeFailureKind,
    ) void {
        var observed = self.code.load(.acquire);
        while (observed == 0) {
            observed = self.code.cmpxchgWeak(
                observed,
                @intFromEnum(failure),
                .acq_rel,
                .acquire,
            ) orelse return;
        }
    }

    fn markNormal(self: *CompletionClassification) bool {
        return self.code.cmpxchgStrong(
            0,
            normal_completion_classification,
            .acq_rel,
            .acquire,
        ) == null;
    }
};

pub const Completion = struct {
    child_exited: bool,
    classification: CompletionState = .pending,
    exit_result: ?pty_process.ExitResult,
    final_revision: u64,
    pty_drained: bool,
    writer_stopped: bool = false,
};

/// Semantic owner-cleanup failures. Platform errors are classified at this
/// boundary so HostCore never has to infer which shutdown phase failed.
pub const ShutdownError = error{
    OutputReleaseFailed,
    ProcessContainmentFailed,
    ProcessIdentityReleaseFailed,
};

const PreparedInputState = enum {
    pending,
    consumed,
};

/// Owns an encoded client-input copy and its ordered queue reservation until
/// HostService has prepared the matching success reply. Deinitializing a
/// pending value removes the reservation, so a pre-commit failure consumes no
/// queue position and admits no terminal bytes.
pub const PreparedInput = struct {
    bytes: ?[]u8,
    reservation: ?InputReservation,
    runtime: *TerminalRuntime,
    state: PreparedInputState = .pending,

    pub fn byteCount(self: *const PreparedInput) usize {
        std.debug.assert(self.state == .pending);
        return if (self.bytes) |bytes| bytes.len else 0;
    }

    pub fn commit(self: *PreparedInput) !usize {
        std.debug.assert(self.state == .pending);
        const byte_count = self.byteCount();
        const bytes = self.bytes;
        const reservation = self.reservation;
        self.bytes = null;
        self.reservation = null;
        self.state = .consumed;
        if (bytes) |owned| {
            try self.runtime.publishInput(reservation.?, owned);
        } else {
            std.debug.assert(reservation == null);
            try self.runtime.validatePreparedInputCommit();
        }
        return byte_count;
    }

    pub fn deinit(self: *PreparedInput) void {
        if (self.state == .pending) {
            if (self.reservation) |reservation| {
                self.runtime.cancelInput(reservation.identifier);
            }
            if (self.bytes) |bytes| self.runtime.allocator.free(bytes);
        }
        self.* = undefined;
    }
};

const PreparedAttachmentState = enum {
    pending,
    consumed,
};

/// Holds the terminal-state lock and any history capacity needed for a new
/// attachment until Core has completed all of its fallible admission work.
/// Committing this value is allocation-free, so Core and runtime-derived
/// attachment state become observable as one Host transition.
pub const PreparedAttachment = struct {
    attachment_id: AttachmentId,
    history_reserved: bool,
    runtime: *TerminalRuntime,
    state: PreparedAttachmentState = .pending,

    pub fn commit(self: *PreparedAttachment) void {
        std.debug.assert(self.state == .pending);
        if (self.history_reserved) {
            self.runtime.history.commitReservedAttachment(self.attachment_id);
        }
        self.runtime.terminal_mutex.unlock();
        self.state = .consumed;
    }

    pub fn deinit(self: *PreparedAttachment) void {
        if (self.state == .pending) self.runtime.terminal_mutex.unlock();
        self.* = undefined;
    }
};

pub const TerminalRuntime = struct {
    allocator: std.mem.Allocator,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    child_exited: std.atomic.Value(bool) = .init(false),
    client_input_closed: bool = false,
    containment_confirmed: std.atomic.Value(bool) = .init(false),
    containment_mutex: std.Thread.Mutex = .{},
    exit_result: ?pty_process.ExitResult = null,
    // terminal_mutex and input_mutex jointly protect this irreversible gate.
    // Parser/state readers observe it while holding terminal_mutex; queue and
    // writer-start readers observe it while holding input_mutex.
    failure_fenced: bool = false,
    // A successful engine fence winner owns the first bounded containment
    // attempt. A Host-originated fence reserves that attempt for Host cleanup;
    // later Host retries wait for an engine_pending owner to publish done.
    first_containment_attempt: std.atomic.Value(FirstContainmentAttempt) =
        .init(.none),
    first_containment_condition: std.Thread.Condition = .{},
    first_containment_mutex: std.Thread.Mutex = .{},
    generation: u64 = 1,
    history: terminal_history.Manager,
    // input_mutex and input_condition protect queue order, charged bytes,
    // reservation identifiers, the writer stop transition, and completion
    // notifications that must not be missed between a predicate check and wait.
    client_input_charge: InputCharge = .{},
    input_condition: std.Thread.Condition = .{},
    input_entries: std.ArrayList(InputEntry) = .empty,
    input_mutex: std.Thread.Mutex = .{},
    input_next_identifier: u64 = 1,
    terminal_reply_charge: InputCharge = .{},
    // Distinguishes an owner-requested read interruption from a normal PTY EOF.
    // Only the reader may publish pty_drained, and only after a non-aborted EOF.
    output_abort_requested: std.atomic.Value(bool) = .init(false),
    // One terminal atomic classification makes normal completion and fatal
    // failure mutually exclusive. Zero is pending, RuntimeFailureKind values
    // are failures, and normal_completion_classification is normal completion.
    // Once normal is published, later disposal diagnostics cannot rewrite the
    // retained terminal outcome.
    completion_classification: CompletionClassification = .{},
    reader_thread: ?std.Thread = null,
    release_process_identity: bool = false,
    revision: std.atomic.Value(u64) = .init(0),
    output_batches_since_sync: std.atomic.Value(u64) = .init(0),
    process: pty_process.Process,
    profile: OwnedProfile,
    // projection_mutex gives one encoder ownership of projection capture state.
    // The Host owner alone updates the projection cadence timestamp; engine
    // workers never read it.
    next_projection_at_ns: i128 = 0,
    projection: terminal_projection.Projection,
    projection_mutex: std.Thread.Mutex = .{},
    pty_drained: std.atomic.Value(bool) = .init(false),
    // Ghostty may discover a reply-admission failure synchronously while a
    // reader batch is being parsed. terminal_mutex protects this deferred
    // marker until the complete batch has committed coherently.
    reply_failure_pending: bool = false,
    // A failed Ghostty resize may leave its internal screen state undefined.
    // Once set, no new terminal work is admitted and the owner must tear down
    // the runtime instead of attempting to continue with mixed geometry.
    resize_fenced: std.atomic.Value(bool) = .init(false),
    // terminal_mutex owns the interval between a successful resize preflight
    // and its canonical commit or platform compensation. Normal completion
    // cannot publish while this effect is in flight.
    resize_effect_in_flight: bool = false,
    stream: ghostty_vt.TerminalStream,
    scrollback_bytes: usize,
    startup_condition: std.Thread.Condition = .{},
    startup_mutex: std.Thread.Mutex = .{},
    startup_state: RuntimeStartupState = .pending,
    terminal: ghostty_vt.Terminal,
    // terminal_mutex protects terminal parsing, history, generation, scrollback,
    // and cell geometry as one state transition.
    terminal_mutex: std.Thread.Mutex = .{},
    testing_hooks: RuntimeTestingHooks = .{},
    threads_joined: bool = false,
    wait_thread: ?std.Thread = null,
    writer_stop: bool = false,
    writer_stopped: std.atomic.Value(bool) = .init(false),
    writer_thread: ?std.Thread = null,
    written_since_sync: std.atomic.Value(usize) = .init(0),

    pub fn create(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
    ) !*TerminalRuntime {
        return createWithOptions(allocator, launch, .{});
    }

    pub fn createWithOptions(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
        options: Options,
    ) !*TerminalRuntime {
        var spawner: SystemThreadSpawner = .{};
        return createWithOptionsUsingSpawner(allocator, launch, options, &spawner);
    }

    fn createWithOptionsUsingSpawner(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
        options: Options,
        spawner: anytype,
    ) !*TerminalRuntime {
        const colors = try terminal_profile.terminalColors(options.profile);
        const self = try allocator.create(TerminalRuntime);
        errdefer allocator.destroy(self);

        const resources = resources: {
            var profile = try OwnedProfile.init(allocator, options.profile);
            errdefer profile.deinit(allocator);

            var terminal: ghostty_vt.Terminal = try .init(allocator, .{
                .cols = launch.columns,
                .colors = colors,
                .max_scrollback = options.scrollback_bytes,
                .rows = launch.rows,
            });
            errdefer terminal.deinit(allocator);
            break :resources .{
                .profile = profile,
                .terminal = terminal,
            };
        };

        self.* = .{
            .allocator = allocator,
            .cell_pixel_height = options.cell_pixel_height,
            .cell_pixel_width = options.cell_pixel_width,
            .process = undefined,
            .profile = resources.profile,
            .projection = .init(allocator),
            .history = .init(allocator),
            .stream = undefined,
            .scrollback_bytes = options.scrollback_bytes,
            .terminal = resources.terminal,
        };
        if (comptime target_builtin.is_test and
            @hasDecl(@TypeOf(spawner.*), "terminalRuntimeTestingHooks"))
        {
            self.testing_hooks = spawner.terminalRuntimeTestingHooks();
        }
        errdefer self.cleanupFailedThreadStartup();
        self.stream = .init(self.terminal.vtHandler());
        self.stream.handler.effects.write_pty = writePtyEffect;
        self.stream.handler.effects.color_scheme = colorSchemeEffect;
        self.stream.handler.effects.device_attributes = deviceAttributesEffect;
        self.stream.handler.effects.enquiry = enquiryEffect;
        self.stream.handler.effects.size = sizeEffect;
        self.stream.handler.effects.xtversion = versionEffect;

        self.wait_thread = try spawner.spawn(waitThread, self);
        self.reader_thread = try spawner.spawn(readerThread, self);
        self.writer_thread = try spawner.spawn(writerThread, self);
        self.process = try pty_process.Process.spawn(allocator, launch);
        self.process.registerReaderThread(self.reader_thread.?.getHandle());
        self.finishStartup(.ready);
        return self;
    }

    fn cleanupFailedThreadStartup(self: *TerminalRuntime) void {
        // Workers are deliberately gated before process creation. A failed
        // thread start or process spawn therefore has no child owned by this
        // partially-constructed runtime; wake and join only the workers that
        // were successfully created.
        self.finishStartup(.aborted);
        if (self.wait_thread) |thread| thread.join();
        if (self.reader_thread) |thread| thread.join();
        if (self.writer_thread) |thread| thread.join();
        self.stream.deinit();
        self.history.deinit(&self.terminal);
        self.terminal.deinit(self.allocator);
        for (self.input_entries.items) |entry| {
            if (entry.bytes) |bytes| self.allocator.free(bytes);
        }
        self.input_entries.deinit(self.allocator);
        self.profile.deinit(self.allocator);
        self.projection.deinit();
    }

    fn finishStartup(self: *TerminalRuntime, state: RuntimeStartupState) void {
        std.debug.assert(state != .pending);
        self.startup_mutex.lock();
        std.debug.assert(self.startup_state == .pending);
        self.startup_state = state;
        self.startup_condition.broadcast();
        self.startup_mutex.unlock();
    }

    fn awaitStartup(self: *TerminalRuntime) bool {
        self.startup_mutex.lock();
        defer self.startup_mutex.unlock();
        while (self.startup_state == .pending) {
            self.startup_condition.wait(&self.startup_mutex);
        }
        return self.startup_state == .ready;
    }

    fn processTreeAlreadyContained(self: *TerminalRuntime) !bool {
        self.containment_mutex.lock();
        defer self.containment_mutex.unlock();
        if (self.containment_confirmed.load(.acquire)) return true;
        for (0..required_consecutive_containment_observations) |observation| {
            if (!try self.process.processTreeContained()) return false;
            if (observation + 1 < required_consecutive_containment_observations) {
                std.Thread.sleep(process_containment_observation_delay_ns);
            }
        }
        self.containment_confirmed.store(true, .release);
        return true;
    }

    fn containProcessTree(self: *TerminalRuntime) !void {
        if (self.containment_confirmed.load(.acquire)) return;
        self.containment_mutex.lock();
        defer self.containment_mutex.unlock();
        if (self.containment_confirmed.load(.acquire)) return;
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.contain_process_tree) |contain| {
                try contain(self.testing_hooks.context.?);
                self.containment_confirmed.store(true, .release);
                return;
            }
        }
        var consecutive_contained: usize = 0;
        var last_failure: anyerror = error.ProcessContainmentIncomplete;
        for (0..max_process_containment_observations) |observation| {
            if (self.containment_confirmed.load(.acquire)) return;
            const contained = self.process.processTreeContained() catch |err| query_failed: {
                consecutive_contained = 0;
                last_failure = err;
                break :query_failed false;
            };
            if (contained) {
                consecutive_contained += 1;
                if (consecutive_contained == required_consecutive_containment_observations) {
                    self.containment_confirmed.store(true, .release);
                    return;
                }
            } else {
                consecutive_contained = 0;
                self.process.forceTerminate() catch |err| {
                    last_failure = err;
                    if (observation + 1 < max_process_containment_observations) {
                        std.Thread.sleep(process_containment_observation_delay_ns);
                    }
                    continue;
                };
                last_failure = error.ProcessContainmentIncomplete;
            }
            if (observation + 1 < max_process_containment_observations) {
                std.Thread.sleep(process_containment_observation_delay_ns);
            }
        }
        return last_failure;
    }

    fn completeFirstContainmentAttempt(
        self: *TerminalRuntime,
        expected: FirstContainmentAttempt,
    ) void {
        self.first_containment_mutex.lock();
        std.debug.assert(self.first_containment_attempt.load(.acquire) == expected);
        self.first_containment_attempt.store(.done, .release);
        self.first_containment_condition.broadcast();
        self.first_containment_mutex.unlock();
    }

    fn notifyHostContainmentWaiting(self: *TerminalRuntime) void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.host_containment_waiting) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
    }

    /// Returns true only to the Host caller that owns a Host-originated first
    /// attempt. Engine-owned pending work is awaited without terminal_mutex or
    /// input_mutex; a completed engine attempt turns every Host call into a
    /// retry rather than allowing Host to overtake it.
    fn awaitFirstContainmentAttempt(self: *TerminalRuntime) bool {
        while (true) {
            self.first_containment_mutex.lock();
            switch (self.first_containment_attempt.load(.acquire)) {
                .none, .done => {
                    self.first_containment_mutex.unlock();
                    return false;
                },
                .host_pending => {
                    self.first_containment_attempt.store(.host_running, .release);
                    self.first_containment_mutex.unlock();
                    return true;
                },
                .engine_pending, .host_running => {
                    self.first_containment_mutex.unlock();
                    self.notifyHostContainmentWaiting();
                    self.first_containment_mutex.lock();
                    while (switch (self.first_containment_attempt.load(.acquire)) {
                        .engine_pending, .host_running => true,
                        .none, .host_pending, .done => false,
                    }) {
                        self.first_containment_condition.wait(&self.first_containment_mutex);
                    }
                    self.first_containment_mutex.unlock();
                },
            }
        }
    }

    fn stopIoForShutdown(self: *TerminalRuntime) void {
        self.output_abort_requested.store(true, .release);
        // Synchronize with the reader's post-read commit boundary. A batch
        // that already won terminal_mutex finishes before this returns; every
        // later batch observes output_abort_requested and is discarded.
        self.terminal_mutex.lock();
        self.input_mutex.lock();
        self.writer_stop = true;
        self.input_condition.broadcast();
        self.input_mutex.unlock();
        self.terminal_mutex.unlock();
    }

    fn finishPtyAfterChildExit(self: *TerminalRuntime) !void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.finish_pty_after_child_exit) |finish| {
                return finish(self.testing_hooks.context.?);
            }
        }
        try self.process.finishPtyAfterChildExit();
    }

    fn releaseOutputForShutdown(self: *TerminalRuntime) !void {
        var failed = false;
        self.finishPtyAfterChildExit() catch {
            failed = true;
        };
        self.abortOutputRead() catch {
            failed = true;
        };
        if (failed) return error.OutputReleaseFailed;
    }

    pub fn shutdown(self: *TerminalRuntime) ShutdownError!void {
        if (self.threads_joined) {
            std.debug.assert(self.containment_confirmed.load(.acquire));
            self.process.releaseProcessIdentity() catch
                return error.ProcessIdentityReleaseFailed;
            return;
        }
        // Stop every future writer start and reader commit before any fallible
        // cleanup. Ordinary shutdown keeps TerminalExited semantics; a failure
        // fence, when present, remains the stronger unavailable classification.
        self.stopIoForShutdown();
        const owns_host_first_attempt = self.awaitFirstContainmentAttempt();
        defer {
            if (owns_host_first_attempt) {
                self.completeFirstContainmentAttempt(.host_running);
            }
        }
        if (!self.containment_confirmed.load(.acquire)) {
            self.containProcessTree() catch return error.ProcessContainmentFailed;
        }
        // On Windows, closing the pseudoconsole is the operation that releases
        // blocked input and output pipe calls. Containment makes that close safe
        // even when process observation failed before the wait worker could do it.
        self.releaseOutputForShutdown() catch return error.OutputReleaseFailed;
        self.input_mutex.lock();
        self.release_process_identity = true;
        self.input_condition.broadcast();
        self.input_mutex.unlock();
        self.joinThreads();
        self.process.releaseProcessIdentity() catch
            return error.ProcessIdentityReleaseFailed;
    }

    pub fn destroy(self: *TerminalRuntime) void {
        self.shutdown() catch |err| {
            // A failed shutdown must retain the runtime and all worker-owned
            // state. The owner can retry after the platform termination fault
            // is resolved; freeing here would be a use-after-free.
            std.log.err("retaining terminal runtime after shutdown failure: {s}", .{@errorName(err)});
            return;
        };
        self.destroyAfterShutdown();
    }

    pub fn destroyAfterShutdown(self: *TerminalRuntime) void {
        std.debug.assert(self.threads_joined);
        std.debug.assert(self.containment_confirmed.load(.acquire));
        self.stream.deinit();
        self.process.deinit();
        self.history.deinit(&self.terminal);
        self.terminal.deinit(self.allocator);
        self.input_mutex.lock();
        for (self.input_entries.items) |entry| if (entry.bytes) |bytes| self.allocator.free(bytes);
        self.input_entries.deinit(self.allocator);
        self.input_mutex.unlock();
        self.profile.deinit(self.allocator);
        self.projection.deinit();
        const allocator = self.allocator;
        self.* = undefined;
        allocator.destroy(self);
    }

    pub fn completion(self: *const TerminalRuntime) Completion {
        // Normal classification is linearized under terminal_mutex after every
        // terminal mutation. Once it is visible, final_revision cannot advance.
        const classification = self.completion_classification.load();
        const pty_drained = self.pty_drained.load(.acquire);
        const child_exited = self.child_exited.load(.acquire);
        const writer_stopped = self.writer_stopped.load(.acquire);
        switch (classification) {
            .normal => {
                std.debug.assert(child_exited);
                std.debug.assert(self.exit_result != null);
                std.debug.assert(pty_drained);
                std.debug.assert(writer_stopped);
            },
            .pending, .failed => {},
        }
        return .{
            .child_exited = child_exited,
            .classification = classification,
            .exit_result = if (child_exited) self.exit_result else null,
            .final_revision = self.revision.load(.acquire),
            .pty_drained = pty_drained,
            .writer_stopped = writer_stopped,
        };
    }

    // Wait without polling until the process and PTY settle, or until a runtime
    // health failure makes successful completion impossible.
    pub fn waitForCompletion(self: *TerminalRuntime) !Completion {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        while (true) {
            const state = self.completion();
            switch (state.classification) {
                .pending => {},
                .normal => return state,
                .failed => |failure| return switch (failure) {
                    .resize => error.TerminalResizeFailed,
                    .process_observation => error.ProcessObservationFailed,
                    .pty_read => error.PtyReadFailed,
                    .pty_write => error.PtyWriteFailed,
                    .terminal_reply => error.TerminalReplyDropped,
                    .worker_integrity => error.WorkerIntegrityFailed,
                },
            }
            if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
            self.input_condition.wait(&self.input_mutex);
        }
    }

    pub fn isResizeFenced(self: *const TerminalRuntime) bool {
        return self.resize_fenced.load(.acquire);
    }

    /// Closes the shared failure gate while terminal_mutex is already held.
    /// Every failure-fence write follows terminal_mutex -> input_mutex, and the
    /// winning engine classification is published in that same transition.
    fn closeFailureFenceLocked(
        self: *TerminalRuntime,
        failure: ?RuntimeFailureKind,
    ) bool {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        if (self.failure_fenced) return false;
        self.failure_fenced = true;
        self.writer_stop = true;
        if (failure) |classification| {
            self.completion_classification.markFailure(classification);
        }
        std.debug.assert(self.first_containment_attempt.load(.acquire) == .none);
        self.first_containment_attempt.store(
            if (failure == null) .host_pending else .engine_pending,
            .release,
        );
        self.input_condition.broadcast();
        return true;
    }

    fn closeFailureFence(
        self: *TerminalRuntime,
        failure: ?RuntimeFailureKind,
    ) bool {
        self.terminal_mutex.lock();
        const won = self.closeFailureFenceLocked(failure);
        self.terminal_mutex.unlock();
        return won;
    }

    /// Host-side effect failures have their own Core classification. This
    /// bounded engine transition closes I/O before Core publishes that cause;
    /// fallible containment deliberately remains a later owner phase.
    pub fn fenceHostFailure(self: *TerminalRuntime) HostFailureFenceResult {
        if (self.closeFailureFence(null)) return .host_won;
        return if (self.primaryFailure()) |failure|
            .{ .engine_won = failure }
        else
            .host_won;
    }

    pub fn primaryFailure(self: *const TerminalRuntime) ?RuntimeFailureKind {
        return switch (self.completion_classification.load()) {
            .failed => |failure| failure,
            .pending, .normal => null,
        };
    }

    fn ensureRuntimeWorkAvailableLocked(self: *const TerminalRuntime) !void {
        if (self.isResizeFenced()) return error.TerminalResizeFailed;
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        switch (self.completion_classification.load()) {
            .pending => {},
            .normal, .failed => return error.TerminalRuntimeUnavailable,
        }
    }

    fn ensureReadableStateAvailableLocked(self: *const TerminalRuntime) !void {
        if (self.isResizeFenced()) return error.TerminalResizeFailed;
        if (self.failure_fenced and !self.containment_confirmed.load(.acquire)) {
            return error.TerminalRuntimeUnavailable;
        }
    }

    pub fn readableStateAvailable(self: *TerminalRuntime) bool {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        self.ensureReadableStateAvailableLocked() catch return false;
        return true;
    }

    fn joinThreads(self: *TerminalRuntime) void {
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

    pub fn enqueueInput(self: *TerminalRuntime, bytes: []const u8) !void {
        _ = try self.enqueueRawInput(bytes);
    }

    pub fn enqueueRawInput(self: *TerminalRuntime, bytes: []const u8) !usize {
        var prepared = try self.prepareRawInput(bytes);
        defer prepared.deinit();
        return prepared.commit();
    }

    pub fn prepareRawInput(self: *TerminalRuntime, bytes: []const u8) !PreparedInput {
        if (bytes.len == 0) return error.EmptyInput;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        const reservation = try self.reserveInput(bytes.len, false, .client);
        errdefer self.cancelInput(reservation.identifier);
        const copy = self.allocator.dupe(u8, bytes) catch |err| {
            return err;
        };
        return .{
            .bytes = copy,
            .reservation = reservation,
            .runtime = self,
        };
    }

    pub fn enqueueKeyInput(self: *TerminalRuntime, input: KeyInput) !usize {
        return self.enqueueKeyInputWithLimit(input, std.math.maxInt(usize));
    }

    pub fn keyInputWouldEmit(self: *TerminalRuntime, input: KeyInput) !bool {
        const key = try terminal_input_encoding.parseKey(input.key);
        var buffer: [max_key_encoded_bytes]u8 = undefined;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        const encoded = try terminal_input_encoding.encodeKey(
            &buffer,
            &self.terminal,
            key,
            input,
        );
        return encoded.len != 0;
    }

    pub fn enqueueKeyInputWithLimit(
        self: *TerminalRuntime,
        input: KeyInput,
        max_output_bytes: usize,
    ) !usize {
        var prepared = try self.prepareKeyInput(input, max_output_bytes);
        defer prepared.deinit();
        return prepared.commit();
    }

    pub fn prepareKeyInput(
        self: *TerminalRuntime,
        input: KeyInput,
        max_output_bytes: usize,
    ) !PreparedInput {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        const key = try terminal_input_encoding.parseKey(input.key);
        var buffer: [max_key_encoded_bytes]u8 = undefined;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        const encoded = try terminal_input_encoding.encodeKey(
            &buffer,
            &self.terminal,
            key,
            input,
        );
        if (encoded.len > max_output_bytes) return error.InputLimitExceeded;
        return self.prepareEncodedInputLocked(encoded);
    }

    pub fn enqueueTextInput(self: *TerminalRuntime, text: []const u8) !usize {
        return self.enqueueTextInputWithLimit(text, std.math.maxInt(usize));
    }

    pub fn enqueueTextInputWithLimit(
        self: *TerminalRuntime,
        text: []const u8,
        max_output_bytes: usize,
    ) !usize {
        var prepared = try self.prepareTextInput(text, max_output_bytes);
        defer prepared.deinit();
        return prepared.commit();
    }

    pub fn prepareTextInput(
        self: *TerminalRuntime,
        text: []const u8,
        max_output_bytes: usize,
    ) !PreparedInput {
        if (text.len == 0) return error.EmptyInput;
        if (text.len > max_output_bytes) return error.InputLimitExceeded;
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        return self.prepareEncodedInputLocked(text);
    }

    pub fn enqueueMouseInput(self: *TerminalRuntime, input: MouseInput) !usize {
        return self.enqueueMouseInputWithLimit(input, std.math.maxInt(usize));
    }

    pub fn mouseInputWouldEmit(self: *TerminalRuntime, input: MouseInput) !bool {
        var buffer: [max_mouse_encoded_bytes]u8 = undefined;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        const encoded = try terminal_input_encoding.encodeMouse(
            &buffer,
            &self.terminal,
            self.cell_pixel_width,
            self.cell_pixel_height,
            input,
        );
        return encoded.len != 0;
    }

    pub fn enqueueMouseInputWithLimit(
        self: *TerminalRuntime,
        input: MouseInput,
        max_output_bytes: usize,
    ) !usize {
        var prepared = try self.prepareMouseInput(input, max_output_bytes);
        defer prepared.deinit();
        return prepared.commit();
    }

    pub fn prepareMouseInput(
        self: *TerminalRuntime,
        input: MouseInput,
        max_output_bytes: usize,
    ) !PreparedInput {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        var buffer: [max_mouse_encoded_bytes]u8 = undefined;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        const encoded = try terminal_input_encoding.encodeMouse(
            &buffer,
            &self.terminal,
            self.cell_pixel_width,
            self.cell_pixel_height,
            input,
        );
        if (encoded.len > max_output_bytes) return error.InputLimitExceeded;
        return self.prepareEncodedInputLocked(encoded);
    }

    pub fn enqueueFocusInput(self: *TerminalRuntime, focused: bool) !usize {
        return self.enqueueFocusInputWithLimit(focused, std.math.maxInt(usize));
    }

    pub fn focusInputWouldEmit(self: *TerminalRuntime) bool {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        return terminal_input_encoding.reportsFocus(&self.terminal);
    }

    pub fn enqueueFocusInputWithLimit(
        self: *TerminalRuntime,
        focused: bool,
        max_output_bytes: usize,
    ) !usize {
        var prepared = try self.prepareFocusInput(focused, max_output_bytes);
        defer prepared.deinit();
        return prepared.commit();
    }

    pub fn prepareFocusInput(
        self: *TerminalRuntime,
        focused: bool,
        max_output_bytes: usize,
    ) !PreparedInput {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        var buffer: [max_focus_encoded_bytes]u8 = undefined;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        if (!terminal_input_encoding.reportsFocus(&self.terminal)) {
            return self.prepareEncodedInputLocked("");
        }
        const encoded = try terminal_input_encoding.encodeFocus(&buffer, focused);
        if (encoded.len > max_output_bytes) return error.InputLimitExceeded;
        return self.prepareEncodedInputLocked(encoded);
    }

    pub fn enqueuePasteInput(self: *TerminalRuntime, text: []const u8) !usize {
        return self.enqueuePasteInputWithLimit(text, std.math.maxInt(usize));
    }

    pub fn enqueuePasteInputWithLimit(
        self: *TerminalRuntime,
        text: []const u8,
        max_output_bytes: usize,
    ) !usize {
        var prepared = try self.preparePasteInput(text, max_output_bytes);
        defer prepared.deinit();
        return prepared.commit();
    }

    pub fn preparePasteInput(
        self: *TerminalRuntime,
        text: []const u8,
        max_output_bytes: usize,
    ) !PreparedInput {
        if (text.len == 0) return error.EmptyInput;
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        const encoded = try terminal_input_encoding.encodePaste(
            self.allocator,
            &self.terminal,
            text,
            max_output_bytes,
        );
        return self.prepareOwnedInputLocked(encoded);
    }

    pub fn prepareSemanticInput(
        self: *TerminalRuntime,
        input: terminal_input.SemanticInput,
        maximum_bytes: usize,
    ) !PreparedInput {
        return switch (input) {
            .focus => |value| self.prepareFocusInput(value.focused, maximum_bytes),
            .key => |value| self.prepareKeyInput(value, maximum_bytes),
            .mouse => |value| self.prepareMouseInput(value, maximum_bytes),
            .paste => |value| self.preparePasteInput(value.text, maximum_bytes),
            .text => |value| self.prepareTextInput(value.text, maximum_bytes),
        };
    }

    pub fn pendingInputBytes(self: *TerminalRuntime) usize {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        return self.client_input_charge.bytes + self.terminal_reply_charge.bytes;
    }

    pub fn takeWrittenBytes(self: *TerminalRuntime) usize {
        return self.written_since_sync.swap(0, .acq_rel);
    }

    pub fn takeOutputBatches(self: *TerminalRuntime) u64 {
        return self.output_batches_since_sync.swap(0, .acq_rel);
    }

    pub fn resize(self: *TerminalRuntime, columns: u16, rows: u16) !void {
        self.terminal_mutex.lock();
        const cell_pixel_width = self.cell_pixel_width;
        const cell_pixel_height = self.cell_pixel_height;
        self.terminal_mutex.unlock();
        _ = try self.resizeGeometry(columns, rows, cell_pixel_width, cell_pixel_height);
    }

    pub fn resizeGeometry(
        self: *TerminalRuntime,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !u64 {
        self.terminal_mutex.lock();
        self.ensureRuntimeWorkAvailableLocked() catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        if (self.resize_effect_in_flight) {
            self.terminal_mutex.unlock();
            return error.TerminalRuntimeUnavailable;
        }
        const old_columns = self.terminal.cols;
        const old_rows = self.terminal.rows;
        const cells_changed = old_columns != columns or old_rows != rows;
        const pixels_changed = self.cell_pixel_width != cell_pixel_width or
            self.cell_pixel_height != cell_pixel_height;
        if (!cells_changed and !pixels_changed) {
            const revision = self.revision.load(.acquire);
            self.terminal_mutex.unlock();
            return revision;
        }
        if (self.generation == std.math.maxInt(u64) or
            self.revision.load(.monotonic) == std.math.maxInt(u64))
        {
            self.terminal_mutex.unlock();
            return error.RevisionOverflow;
        }
        const previous_geometry: terminal_geometry.Geometry = .{
            .cell_pixel_height = self.cell_pixel_height,
            .cell_pixel_width = self.cell_pixel_width,
            .columns = old_columns,
            .rows = old_rows,
        };
        const next_geometry: terminal_geometry.Geometry = .{
            .cell_pixel_height = cell_pixel_height,
            .cell_pixel_width = cell_pixel_width,
            .columns = columns,
            .rows = rows,
        };
        self.resize_effect_in_flight = true;
        self.terminal_mutex.unlock();

        const resize_effect_result = self.resizePtyEffect(previous_geometry, next_geometry);
        // This notification exposes the post-platform-effect interval to
        // deterministic tests. The effect has returned, its result has not
        // been consumed, and no engine state mutex is held. Restore effects
        // deliberately bypass this one-shot initial-effect seam.
        self.notifyResizeEffectReturned();
        resize_effect_result catch |err| {
            if (err == error.ResizeRollbackFailed) {
                self.fenceResizeFailure();
                return err;
            }
            self.terminal_mutex.lock();
            const failure_fenced = self.failure_fenced;
            std.debug.assert(self.resize_effect_in_flight);
            self.resize_effect_in_flight = false;
            self.terminal_mutex.unlock();
            if (failure_fenced) return error.TerminalRuntimeUnavailable;
            self.markNormalCompletionReady();
            return err;
        };

        self.terminal_mutex.lock();
        if (self.failure_fenced) {
            self.terminal_mutex.unlock();
            const compensation = self.compensateResizeEffect(
                previous_geometry,
                next_geometry,
            );
            if (compensation.rollback_failed) return error.ResizeRollbackFailed;
            return error.TerminalRuntimeUnavailable;
        }
        // HostRuntimeAggregate is the sole geometry writer. PTY output and
        // configuration may advance while the platform effect is in flight,
        // but no second geometry effect may overtake this commit.
        std.debug.assert(self.terminal.cols == old_columns);
        std.debug.assert(self.terminal.rows == old_rows);
        std.debug.assert(self.cell_pixel_width == previous_geometry.cell_pixel_width);
        std.debug.assert(self.cell_pixel_height == previous_geometry.cell_pixel_height);
        const next_generation = std.math.add(u64, self.generation, 1) catch {
            self.terminal_mutex.unlock();
            const compensation = self.compensateResizeEffect(
                previous_geometry,
                next_geometry,
            );
            if (compensation.rollback_failed) return error.ResizeRollbackFailed;
            if (compensation.failure_fenced) return error.TerminalRuntimeUnavailable;
            return error.RevisionOverflow;
        };
        const revision = std.math.add(u64, self.revision.load(.monotonic), 1) catch {
            self.terminal_mutex.unlock();
            const compensation = self.compensateResizeEffect(
                previous_geometry,
                next_geometry,
            );
            if (compensation.rollback_failed) return error.ResizeRollbackFailed;
            if (compensation.failure_fenced) return error.TerminalRuntimeUnavailable;
            return error.RevisionOverflow;
        };
        if (cells_changed) {
            self.terminal.resize(self.allocator, columns, rows) catch |err| {
                self.resize_fenced.store(true, .release);
                const won = self.closeFailureFenceLocked(.resize);
                self.terminal_mutex.unlock();
                const rollback_failed = rollback: {
                    self.restorePtyGeometry(
                        next_geometry,
                        previous_geometry,
                    ) catch break :rollback true;
                    break :rollback false;
                };
                self.terminal_mutex.lock();
                std.debug.assert(self.resize_effect_in_flight);
                self.resize_effect_in_flight = false;
                self.terminal_mutex.unlock();
                if (won) self.finishFailureFence();
                if (rollback_failed) return error.ResizeRollbackFailed;
                return err;
            };
        }
        self.cell_pixel_width = cell_pixel_width;
        self.cell_pixel_height = cell_pixel_height;
        self.generation = next_generation;
        self.revision.store(revision, .release);
        self.history.generationChanged(&self.terminal, revision, self.generation);
        std.debug.assert(self.resize_effect_in_flight);
        self.resize_effect_in_flight = false;
        self.terminal_mutex.unlock();
        self.markNormalCompletionReady();
        return revision;
    }

    fn fenceResizeFailure(self: *TerminalRuntime) void {
        self.terminal_mutex.lock();
        self.resize_fenced.store(true, .release);
        const won = self.closeFailureFenceLocked(.resize);
        self.resize_effect_in_flight = false;
        self.terminal_mutex.unlock();
        if (won) self.finishFailureFence();
    }

    const ResizeCompensation = struct {
        failure_fenced: bool,
        rollback_failed: bool,
    };

    fn compensateResizeEffect(
        self: *TerminalRuntime,
        previous_geometry: terminal_geometry.Geometry,
        next_geometry: terminal_geometry.Geometry,
    ) ResizeCompensation {
        const rollback_failed = rollback: {
            self.restorePtyGeometry(next_geometry, previous_geometry) catch
                break :rollback true;
            break :rollback false;
        };
        self.terminal_mutex.lock();
        var won = false;
        if (rollback_failed) {
            self.resize_fenced.store(true, .release);
            won = self.closeFailureFenceLocked(.resize);
        }
        std.debug.assert(self.resize_effect_in_flight);
        self.resize_effect_in_flight = false;
        const failure_fenced = self.failure_fenced;
        self.terminal_mutex.unlock();
        if (won) self.finishFailureFence();
        if (!failure_fenced) self.markNormalCompletionReady();
        return .{
            .failure_fenced = failure_fenced,
            .rollback_failed = rollback_failed,
        };
    }

    fn notifyResizeEffectReturned(self: *TerminalRuntime) void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.resize_effect_returned) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
    }

    fn resizePtyEffect(
        self: *TerminalRuntime,
        previous_geometry: terminal_geometry.Geometry,
        next_geometry: terminal_geometry.Geometry,
    ) !void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.resize_pty) |resize_pty| {
                return resize_pty(
                    self.testing_hooks.context.?,
                    previous_geometry,
                    next_geometry,
                );
            }
        }
        try resizePtyWithRollback(&self.process, previous_geometry, next_geometry);
    }

    fn restorePtyGeometry(
        self: *TerminalRuntime,
        current_geometry: terminal_geometry.Geometry,
        previous_geometry: terminal_geometry.Geometry,
    ) !void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.resize_pty) |resize_pty| {
                return resize_pty(
                    self.testing_hooks.context.?,
                    current_geometry,
                    previous_geometry,
                );
            }
        }
        try self.process.resizePixels(
            previous_geometry.columns,
            previous_geometry.rows,
            previous_geometry.cell_pixel_width,
            previous_geometry.cell_pixel_height,
        );
    }

    fn abortOutputRead(self: *TerminalRuntime) !void {
        self.output_abort_requested.store(true, .release);
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.abort_read) |abort_read| {
                return abort_read(self.testing_hooks.context.?);
            }
        }
        try self.process.abortRead();
    }

    pub fn configuration(self: *TerminalRuntime) terminal_configuration.Configuration {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        return .{ .scrollback_bytes = self.scrollback_bytes };
    }

    pub fn updateConfiguration(
        self: *TerminalRuntime,
        next_configuration: terminal_configuration.Configuration,
    ) !ConfigurationUpdate {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureRuntimeWorkAvailableLocked();
        if (next_configuration.scrollback_bytes == self.scrollback_bytes) return .{
            .generation = self.generation,
            .history_rows_evicted = 0,
            .revision = self.revision.load(.monotonic),
        };
        if (!terminal_configuration.valid(next_configuration)) return error.InvalidConfiguration;

        const primary = self.terminal.screens.get(.primary).?;
        const may_evict = next_configuration.scrollback_bytes < self.scrollback_bytes and
            primary.pages.total_rows > self.terminal.rows;
        if (may_evict and (self.generation == std.math.maxInt(u64) or
            self.revision.load(.monotonic) == std.math.maxInt(u64)))
        {
            return error.RevisionOverflow;
        }

        const history_rows_evicted = applyScrollbackBytes(
            &self.terminal,
            next_configuration.scrollback_bytes,
        );
        self.scrollback_bytes = next_configuration.scrollback_bytes;
        if (history_rows_evicted == 0) return .{
            .generation = self.generation,
            .history_rows_evicted = 0,
            .revision = self.revision.load(.monotonic),
        };

        const next_generation = std.math.add(u64, self.generation, 1) catch unreachable;
        const next_revision = std.math.add(u64, self.revision.load(.monotonic), 1) catch unreachable;
        self.generation = next_generation;
        self.revision.store(next_revision, .release);
        self.history.screenChanged(&self.terminal);
        return .{
            .generation = next_generation,
            .history_rows_evicted = history_rows_evicted,
            .revision = next_revision,
        };
    }

    pub fn terminate(self: *const TerminalRuntime) !void {
        try self.process.terminate();
    }

    pub fn forceTerminate(self: *const TerminalRuntime) !void {
        try self.process.forceTerminate();
    }

    pub fn plainString(self: *TerminalRuntime, allocator: std.mem.Allocator) ![]const u8 {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.terminal.plainString(allocator);
    }

    pub fn geometry(self: *TerminalRuntime) Geometry {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        return .{
            .cell_pixel_height = self.cell_pixel_height,
            .cell_pixel_width = self.cell_pixel_width,
            .columns = self.terminal.cols,
            .generation = self.generation,
            .revision = self.revision.load(.monotonic),
            .rows = self.terminal.rows,
        };
    }

    pub fn projectionCounters(self: *TerminalRuntime) ProjectionCounters {
        self.projection_mutex.lock();
        defer self.projection_mutex.unlock();
        return self.projection.counters;
    }

    pub fn nextProjectionAt(self: *const TerminalRuntime) i128 {
        return self.next_projection_at_ns;
    }

    pub fn setNextProjectionAt(self: *TerminalRuntime, value: i128) void {
        self.next_projection_at_ns = value;
    }

    pub fn projectionResident(self: *TerminalRuntime) bool {
        self.projection_mutex.lock();
        defer self.projection_mutex.unlock();
        return self.projection.isResident();
    }

    pub fn releaseProjectionResources(self: *TerminalRuntime) void {
        self.projection_mutex.lock();
        defer self.projection_mutex.unlock();
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        self.projection.releaseResources();
    }

    pub fn encodeProjectionFrame(
        self: *TerminalRuntime,
        request: ProjectionEncodeRequest,
    ) !ProjectionFrame {
        return self.encodeProjectionFrameOwned(self.allocator, 0, request);
    }

    pub fn encodeProjectionFrameOwned(
        self: *TerminalRuntime,
        output_allocator: std.mem.Allocator,
        output_prefix_bytes: usize,
        request: ProjectionEncodeRequest,
    ) !ProjectionFrame {
        self.projection_mutex.lock();
        defer self.projection_mutex.unlock();

        self.terminal_mutex.lock();
        self.ensureReadableStateAvailableLocked() catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        const revision = self.revision.load(.monotonic);
        const generation = self.generation;
        if (self.projection.needsCapture(revision, generation)) {
            self.projection.beginCapture(
                &self.terminal,
                revision,
                generation,
                self.cell_pixel_width,
                self.cell_pixel_height,
            ) catch |err| {
                self.terminal_mutex.unlock();
                self.projection.resetAfterCaptureFailure();
                return err;
            };
            self.terminal_mutex.unlock();
            self.projection.endCapture();
        } else {
            self.terminal_mutex.unlock();
        }
        return self.projection.encodeOwned(
            output_allocator,
            output_prefix_bytes,
            request,
        ) catch |err| {
            // Encoding is the final fallible step of a capture. If the frame
            // cannot be materialized (including aggregate-size or OOM
            // failures), discard the capture-owned link/title/row cache before
            // the scheduler marks the attachment blocked. Retaining it here
            // would make a failed capture permanently resident until an
            // unrelated future capture happened to clean it up.
            self.projection.resetAfterCaptureFailure();
            return err;
        };
    }

    pub fn prepareAttachment(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        requires_runtime_work: bool,
        requires_readable_state: bool,
    ) !PreparedAttachment {
        self.terminal_mutex.lock();
        errdefer self.terminal_mutex.unlock();
        if (requires_runtime_work) {
            try self.ensureRuntimeWorkAvailableLocked();
        } else if (requires_readable_state) {
            try self.ensureReadableStateAvailableLocked();
        }
        if (requires_readable_state) try self.history.reserveAttachment(attachment_id);
        return .{
            .attachment_id = attachment_id,
            .history_reserved = requires_readable_state,
            .runtime = self,
        };
    }

    pub fn detachHistory(self: *TerminalRuntime, attachment_id: identifiers.AttachmentId) void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        self.history.detach(&self.terminal, attachment_id);
    }

    pub fn setHistoryDemand(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        demand: HistoryDemand,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        try self.history.setDemand(&self.terminal, attachment_id, demand);
    }

    pub fn readHistoryRows(
        self: *TerminalRuntime,
        output_allocator: std.mem.Allocator,
        request: HistoryReadRequest,
    ) !HistoryRows {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.readRows(
            output_allocator,
            &self.terminal,
            self.revision.load(.monotonic),
            self.generation,
            self.cell_pixel_width,
            self.cell_pixel_height,
            request,
        );
    }

    pub fn readHistoryRowsInto(
        self: *TerminalRuntime,
        scratch_allocator: std.mem.Allocator,
        destination: []u8,
        request: HistoryReadRequest,
    ) !HistoryRows {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.readRowsInto(
            scratch_allocator,
            destination,
            &self.terminal,
            self.revision.load(.monotonic),
            self.generation,
            self.cell_pixel_width,
            self.cell_pixel_height,
            request,
        );
    }

    pub fn historyRowsApplied(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        request_sequence: u64,
        revision: u64,
        generation: u64,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        try self.history.rowsApplied(
            &self.terminal,
            attachment_id,
            request_sequence,
            revision,
            generation,
        );
    }

    pub fn takeHistoryEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?HistoryEvent {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.takeHistoryEvent(
            &self.terminal,
            attachment_id,
            self.revision.load(.monotonic),
            self.generation,
        );
    }

    pub fn rollbackHistoryEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        event: HistoryEvent,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.history.rollbackHistoryEvent(attachment_id, event);
    }

    pub fn historyObserved(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        revision: u64,
        generation: u64,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        try self.history.historyObserved(attachment_id, revision, generation);
    }

    pub fn takeSelectionEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?SelectionSnapshot {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.takeSelectionEvent(attachment_id);
    }

    pub fn rollbackSelectionEvent(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        event: SelectionSnapshot,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.history.rollbackSelectionEvent(attachment_id, event);
    }

    pub fn takeSelectionExportCancellation(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?SelectionExportCancellation {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.takeSelectionExportCancellation(attachment_id);
    }

    pub fn peekSelectionExportCancellation(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
    ) !?SelectionExportCancellation {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.peekSelectionExportCancellation(attachment_id);
    }

    pub fn acknowledgeSelectionExportCancellation(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        cancellation: SelectionExportCancellation,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        try self.history.acknowledgeSelectionExportCancellation(attachment_id, cancellation);
    }

    pub fn applySelectionGesture(
        self: *TerminalRuntime,
        request: SelectionGesture,
    ) !SelectionSnapshot {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.applyGesture(
            &self.terminal,
            self.revision.load(.monotonic),
            self.generation,
            request,
        );
    }

    pub fn applySelectionCommand(
        self: *TerminalRuntime,
        request: SelectionCommandRequest,
    ) !SelectionSnapshot {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.applySelectionCommand(
            &self.terminal,
            self.revision.load(.monotonic),
            self.generation,
            request,
        );
    }

    pub fn startSelectionExport(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        export_sequence: u64,
        format: SelectionExportFormat,
    ) !SelectionExportStartResult {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        const lock_started_at = std.time.nanoTimestamp();
        // The reader can invalidate an export between IPC dispatches. Keep
        // cancellation extraction and the replacement start in one state transition;
        // HostService serializes the returned cancellation records before the start response.
        const invalidation_history = try self.history.snapshotInvalidatedExportHistory(attachment_id);
        var cancellations = std.mem.zeroes(
            [terminal_history.max_pending_export_cancellations]SelectionExportCancellation,
        );
        var cancellation_count: usize = 0;
        while (true) {
            const cancellation = (try self.history.takeSelectionExportCancellation(attachment_id)) orelse break;
            std.debug.assert(
                cancellation_count < terminal_history.max_pending_export_cancellations,
            );
            cancellations[cancellation_count] = cancellation;
            cancellation_count += 1;
        }
        const start_transaction = self.history.startExportTransaction(
            &self.terminal,
            attachment_id,
            export_sequence,
            format,
        ) catch |err| {
            self.history.restoreInvalidatedExportHistory(attachment_id, invalidation_history);
            self.history.restoreSelectionExportCancellations(
                attachment_id,
                cancellations[0..cancellation_count],
            ) catch |restore_err| return restore_err;
            return err;
        };
        const elapsed = @max(std.time.nanoTimestamp() - lock_started_at, 0);
        return .{
            .cancellation_count = cancellation_count,
            .cancellations = cancellations,
            .invalidation_history = invalidation_history,
            .lock_hold_nanoseconds = @intCast(@min(elapsed, std.math.maxInt(u64))),
            .previous_completed_export_sequence = start_transaction.previous_completed_export_sequence,
            .previous_last_export_sequence = start_transaction.previous_last_export_sequence,
            .started = start_transaction.started,
        };
    }

    pub fn rollbackSelectionExportStart(
        self: *TerminalRuntime,
        result: SelectionExportStartResult,
    ) void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        self.history.rollbackExportStart(
            result.started.attachment_id,
            result.started.export_sequence,
            result.previous_completed_export_sequence,
            result.previous_last_export_sequence,
            result.cancellations[0..result.cancellation_count],
            result.invalidation_history,
        );
    }

    pub fn nextSelectionExportChunk(
        self: *TerminalRuntime,
        output_allocator: std.mem.Allocator,
        attachment_id: identifiers.AttachmentId,
    ) !?SelectionExportChunk {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.encodeNextExportChunk(output_allocator, attachment_id);
    }

    pub fn rollbackSelectionExportChunk(
        self: *TerminalRuntime,
        token: SelectionExportChunkToken,
    ) !void {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.history.rollbackExportChunk(token);
    }

    pub fn selectionExportChunkApplied(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        export_sequence: u64,
        chunk_sequence: u32,
    ) !?SelectionExportFinished {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.exportChunkApplied(
            attachment_id,
            export_sequence,
            chunk_sequence,
        );
    }

    pub fn cancelSelectionExport(
        self: *TerminalRuntime,
        attachment_id: identifiers.AttachmentId,
        export_sequence: u64,
    ) !SelectionExportFinished {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        try self.ensureReadableStateAvailableLocked();
        return self.history.cancelExport(attachment_id, export_sequence);
    }

    fn prepareEncodedInputLocked(
        self: *TerminalRuntime,
        bytes: []const u8,
    ) !PreparedInput {
        try self.ensureRuntimeWorkAvailableLocked();
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        if (bytes.len == 0) return .{
            .bytes = null,
            .reservation = null,
            .runtime = self,
        };
        const reservation = try self.reserveInput(bytes.len, false, .client);
        errdefer self.cancelInput(reservation.identifier);
        const copy = try self.allocator.dupe(u8, bytes);
        return .{
            .bytes = copy,
            .reservation = reservation,
            .runtime = self,
        };
    }

    fn prepareOwnedInputLocked(
        self: *TerminalRuntime,
        bytes: []u8,
    ) !PreparedInput {
        self.ensureRuntimeWorkAvailableLocked() catch |err| {
            self.allocator.free(bytes);
            return err;
        };
        if (self.child_exited.load(.acquire)) {
            self.allocator.free(bytes);
            return error.TerminalExited;
        }
        if (bytes.len == 0) {
            self.allocator.free(bytes);
            return .{
                .bytes = null,
                .reservation = null,
                .runtime = self,
            };
        }
        const reservation = self.reserveInput(bytes.len, false, .client) catch |err| {
            self.allocator.free(bytes);
            return err;
        };
        return .{
            .bytes = bytes,
            .reservation = reservation,
            .runtime = self,
        };
    }

    fn reserveInput(
        self: *TerminalRuntime,
        byte_count: usize,
        wait: bool,
        source: InputSource,
    ) !InputReservation {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        if (self.writer_stop) return error.TerminalExited;
        if (source == .client and self.client_input_closed) return error.TerminalExited;
        const capacity_bytes = inputCapacityBytes(source);
        const capacity_entries = inputCapacityEntries(source);
        if (byte_count > capacity_bytes) return error.InputBackpressure;
        const charge = self.inputCharge(source);
        while (byte_count > capacity_bytes -| charge.bytes or
            charge.entries >= capacity_entries)
        {
            if (!wait) return error.InputBackpressure;
            self.input_condition.wait(&self.input_mutex);
            if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
            if (self.writer_stop) return error.TerminalExited;
            if (source == .client and self.client_input_closed) return error.TerminalExited;
        }
        const identifier = self.input_next_identifier;
        if (identifier == std.math.maxInt(u64)) return error.InputSequenceExhausted;
        try self.input_entries.ensureUnusedCapacity(self.allocator, 1);
        self.input_entries.appendAssumeCapacity(.{
            .identifier = identifier,
            .reserved_bytes = byte_count,
            .source = source,
        });
        self.input_next_identifier = identifier + 1;
        charge.bytes += byte_count;
        charge.entries += 1;
        return .{ .identifier = identifier, .reserved_bytes = byte_count };
    }

    fn validatePreparedInputCommit(self: *TerminalRuntime) !void {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        if (self.writer_stop) return error.TerminalExited;
    }

    fn publishInput(self: *TerminalRuntime, reservation: InputReservation, bytes: []u8) !void {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        for (self.input_entries.items, 0..) |*entry, index| {
            if (entry.identifier != reservation.identifier) continue;
            std.debug.assert(entry.bytes == null);
            std.debug.assert(bytes.len <= reservation.reserved_bytes);
            if (self.failure_fenced or self.writer_stop) {
                self.releaseInputCharge(entry.*);
                _ = self.input_entries.orderedRemove(index);
                self.allocator.free(bytes);
                self.input_condition.broadcast();
                if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
                return error.TerminalExited;
            }
            self.inputCharge(entry.source).bytes -= reservation.reserved_bytes - bytes.len;
            if (bytes.len == 0) {
                self.inputCharge(entry.source).entries -= 1;
                _ = self.input_entries.orderedRemove(index);
                self.allocator.free(bytes);
                self.input_condition.broadcast();
                return;
            }
            entry.reserved_bytes = bytes.len;
            entry.bytes = bytes;
            self.input_condition.broadcast();
            return;
        }
        self.allocator.free(bytes);
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        return error.TerminalExited;
    }

    fn cancelInput(self: *TerminalRuntime, identifier: u64) void {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        for (self.input_entries.items, 0..) |entry, index| {
            if (entry.identifier != identifier) continue;
            self.releaseInputCharge(entry);
            if (entry.bytes) |bytes| self.allocator.free(bytes);
            _ = self.input_entries.orderedRemove(index);
            self.input_condition.broadcast();
            return;
        }
    }

    fn clearInputQueueLocked(self: *TerminalRuntime) void {
        for (self.input_entries.items) |entry| {
            if (entry.bytes) |bytes| {
                self.allocator.free(bytes);
            }
        }
        self.input_entries.clearRetainingCapacity();
        self.client_input_charge = .{};
        self.terminal_reply_charge = .{};
        self.input_condition.broadcast();
    }

    fn inputCharge(self: *TerminalRuntime, source: InputSource) *InputCharge {
        return switch (source) {
            .client => &self.client_input_charge,
            .terminal => &self.terminal_reply_charge,
        };
    }

    fn releaseInputCharge(self: *TerminalRuntime, entry: InputEntry) void {
        const charge = self.inputCharge(entry.source);
        std.debug.assert(charge.bytes >= entry.reserved_bytes);
        std.debug.assert(charge.entries > 0);
        charge.bytes -= entry.reserved_bytes;
        charge.entries -= 1;
    }

    fn inputEntryIndex(self: *const TerminalRuntime, identifier: u64) ?usize {
        for (self.input_entries.items, 0..) |entry, index| {
            if (entry.identifier == identifier) return index;
        }
        return null;
    }

    fn readPty(self: *TerminalRuntime, buffer: []u8) !usize {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.read_pty) |read| {
                return read(self.testing_hooks.context.?, buffer);
            }
        }
        return self.process.read(buffer);
    }

    fn writePtySlice(self: *TerminalRuntime, bytes: []const u8) !usize {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.write_pty) |write| {
                return write(self.testing_hooks.context.?, bytes);
            }
        }
        return self.process.write(bytes);
    }

    fn finishFailureFence(self: *TerminalRuntime) void {
        // The bounded fence and classification are already visible. External
        // PTY/process operations and fallible containment happen only after
        // both engine state locks have been released.
        defer self.completeFirstContainmentAttempt(.engine_pending);
        self.abortOutputRead() catch {};
        self.containAfterRuntimeFailure();
    }

    fn containAfterRuntimeFailure(self: *TerminalRuntime) void {
        self.containProcessTree() catch {};
    }

    fn recordFailure(self: *TerminalRuntime, failure: RuntimeFailureKind) void {
        if (self.closeFailureFence(failure)) self.finishFailureFence();
    }

    fn recordReaderFailure(self: *TerminalRuntime) void {
        self.recordFailure(.pty_read);
    }

    fn recordWorkerIntegrityFailure(self: *TerminalRuntime) void {
        self.recordFailure(.worker_integrity);
    }

    fn recordWriterFailure(self: *TerminalRuntime) void {
        const won = self.closeFailureFence(.pty_write);
        self.input_mutex.lock();
        self.clearInputQueueLocked();
        self.input_mutex.unlock();
        if (won) self.finishFailureFence();
    }

    fn releaseProcessIdentityFromWorker(self: *TerminalRuntime) void {
        self.process.releaseProcessIdentity() catch {
            self.recordFailure(.process_observation);
            return;
        };
        self.markNormalCompletionReady();
    }

    fn markNormalCompletionReady(self: *TerminalRuntime) void {
        // This lock is the engine's settled boundary. A mutation already in
        // progress completes before classification; every later mutation sees
        // .normal and is rejected by ensureRuntimeWorkAvailableLocked.
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        if (self.failure_fenced or self.resize_effect_in_flight or
            !self.child_exited.load(.acquire) or
            !self.pty_drained.load(.acquire) or
            !self.writer_stopped.load(.acquire)) return;
        if (!self.completion_classification.markNormal()) return;
        self.input_mutex.lock();
        self.input_condition.broadcast();
        self.input_mutex.unlock();
    }

    fn writePtyEffect(handler: *ghostty_vt.TerminalStream.Handler, bytes: [:0]const u8) void {
        const self = runtimeFromHandler(handler);
        const reservation = self.reserveInput(bytes.len, false, .terminal) catch {
            self.reply_failure_pending = true;
            return;
        };
        const copy = self.allocator.dupe(u8, bytes) catch {
            self.cancelInput(reservation.identifier);
            self.reply_failure_pending = true;
            return;
        };
        self.publishInput(reservation, copy) catch {
            self.reply_failure_pending = true;
        };
    }

    fn colorSchemeEffect(
        handler: *ghostty_vt.TerminalStream.Handler,
    ) ?ghostty_vt.device_status.ColorScheme {
        return runtimeFromHandler(handler).profile.reportedColorScheme();
    }

    fn deviceAttributesEffect(handler: *ghostty_vt.TerminalStream.Handler) DeviceAttributes {
        return runtimeFromHandler(handler).profile.deviceAttributes();
    }

    fn enquiryEffect(handler: *ghostty_vt.TerminalStream.Handler) []const u8 {
        return runtimeFromHandler(handler).profile.enquiry_response;
    }

    fn sizeEffect(
        handler: *ghostty_vt.TerminalStream.Handler,
    ) ?ghostty_vt.size_report.Size {
        const self = runtimeFromHandler(handler);
        return .{
            .cell_height = self.cell_pixel_height,
            .cell_width = self.cell_pixel_width,
            .columns = self.terminal.cols,
            .rows = self.terminal.rows,
        };
    }

    fn versionEffect(handler: *ghostty_vt.TerminalStream.Handler) []const u8 {
        return runtimeFromHandler(handler).profile.version_response;
    }

    fn readerMayRead(self: *TerminalRuntime) bool {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        return !self.failure_fenced and
            !self.output_abort_requested.load(.acquire);
    }

    fn notifyReaderBatchFinished(
        self: *TerminalRuntime,
        outcome: ReaderBatchOutcome,
    ) void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.reader_batch_finished) |notify| {
                notify(self.testing_hooks.context.?, outcome);
            }
        }
    }

    fn notifyReaderStopped(self: *TerminalRuntime) void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.reader_stopped) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
    }

    fn commitReaderBatch(
        self: *TerminalRuntime,
        bytes: []const u8,
    ) ReaderBatchOutcome {
        self.terminal_mutex.lock();
        if (self.failure_fenced) {
            self.terminal_mutex.unlock();
            self.notifyReaderBatchFinished(.failure_fenced);
            return .failure_fenced;
        }
        if (self.output_abort_requested.load(.acquire)) {
            self.terminal_mutex.unlock();
            self.notifyReaderBatchFinished(.output_aborted);
            return .output_aborted;
        }

        const current_revision = self.revision.load(.monotonic);
        if (current_revision == std.math.maxInt(u64) or
            self.generation == std.math.maxInt(u64))
        {
            const won = self.closeFailureFenceLocked(.worker_integrity);
            self.terminal_mutex.unlock();
            self.notifyReaderBatchFinished(.runtime_failed);
            if (won) self.finishFailureFence();
            return .runtime_failed;
        }

        std.debug.assert(!self.reply_failure_pending);
        const previous_screen = self.terminal.screens.active_key;
        // Ghostty's 1049 path can reset the active screen without changing
        // its key (for example, enabling it twice). Isolate the terminal clear
        // marker for this batch so same-key replacement still invalidates
        // history while preserving a clear already pending for the renderer.
        const previous_clear = self.terminal.flags.dirty.clear;
        self.terminal.flags.dirty.clear = false;
        self.stream.nextSlice(bytes);
        const screen_reset = self.terminal.flags.dirty.clear;
        self.terminal.flags.dirty.clear = previous_clear or screen_reset;
        const screen_changed = previous_screen != self.terminal.screens.active_key or
            screen_reset;
        if (screen_changed) self.generation += 1;
        const revision = current_revision + 1;
        self.revision.store(revision, .release);
        if (screen_changed) {
            self.history.screenChanged(&self.terminal);
        } else {
            self.history.noteMutation(&self.terminal, revision, self.generation);
        }
        _ = self.output_batches_since_sync.fetchAdd(1, .release);

        const reply_failed = self.reply_failure_pending;
        self.reply_failure_pending = false;
        const reply_failure_won = reply_failed and
            self.closeFailureFenceLocked(.terminal_reply);
        self.terminal_mutex.unlock();

        const outcome: ReaderBatchOutcome = if (reply_failed)
            .runtime_failed
        else
            .committed;
        self.notifyReaderBatchFinished(outcome);
        if (reply_failure_won) self.finishFailureFence();
        return outcome;
    }

    fn publishPtyDrained(self: *TerminalRuntime) bool {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        if (self.failure_fenced) return false;
        self.pty_drained.store(true, .release);
        self.writer_stop = true;
        self.input_condition.broadcast();
        return true;
    }

    fn readerThread(self: *TerminalRuntime) void {
        defer self.notifyReaderStopped();
        if (!self.awaitStartup()) return;
        var buffer: [max_pty_read_batch_bytes]u8 = undefined;
        while (true) {
            if (!self.readerMayRead()) return;
            const length = self.readPty(&buffer) catch {
                if (self.output_abort_requested.load(.acquire)) return;
                self.recordReaderFailure();
                return;
            };
            if (length == 0) {
                if (self.output_abort_requested.load(.acquire)) return;
                break;
            }
            if (self.commitReaderBatch(buffer[0..length]) != .committed) return;
        }
        if (self.output_abort_requested.load(.acquire)) return;
        _ = self.publishPtyDrained();
    }

    fn closeClientInput(self: *TerminalRuntime) void {
        self.input_mutex.lock();
        self.client_input_closed = true;
        self.input_condition.broadcast();
        self.input_mutex.unlock();
    }

    fn waitThread(self: *TerminalRuntime) void {
        if (!self.awaitStartup()) return;
        self.exit_result = self.process.wait() catch {
            self.recordProcessObservationFailure();
            return;
        };
        self.child_exited.store(true, .release);
        self.closeClientInput();
        self.finishPtyAfterChildExit() catch {
            self.recordProcessObservationFailure();
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
            const contained = self.processTreeAlreadyContained() catch {
                self.recordProcessObservationFailure();
                return;
            };
            if (contained) {
                self.releaseProcessIdentityFromWorker();
                return;
            }
            // A descendant may deliberately remain in the owned session after
            // closing every PTY handle. Root exit plus normal PTY drain is still
            // a normal terminal outcome; disposal will contain that retained
            // process tree before releasing the session identity.
            self.markNormalCompletionReady();
        }
        self.input_mutex.lock();
        while (!self.release_process_identity) {
            self.input_condition.wait(&self.input_mutex);
        }
        self.input_mutex.unlock();
        self.releaseProcessIdentityFromWorker();
    }

    fn recordProcessObservationFailure(self: *TerminalRuntime) void {
        self.recordFailure(.process_observation);
        self.input_mutex.lock();
        while (!self.release_process_identity) {
            self.input_condition.wait(&self.input_mutex);
        }
        self.input_mutex.unlock();
        self.releaseProcessIdentityFromWorker();
    }

    fn notifyWriterSliceCopied(self: *TerminalRuntime) void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.writer_slice_copied) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
    }

    fn notifyWriterSliceStarted(self: *TerminalRuntime) void {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.writer_slice_started) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
    }

    fn startWriterSlice(
        self: *TerminalRuntime,
        slice: WriterSlice,
    ) WriterSliceStart {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        if (self.failure_fenced or self.writer_stop) {
            self.clearInputQueueLocked();
            return .stopped;
        }
        const current_index = self.inputEntryIndex(slice.identifier) orelse
            return .ownership_lost;
        const current = self.input_entries.items[current_index];
        const current_bytes = current.bytes orelse return .ownership_lost;
        if (current.identifier != slice.identifier or
            current.offset != slice.offset or
            current.source != slice.source or
            current.offset > current_bytes.len or
            slice.length == 0 or
            slice.length > current_bytes.len - current.offset)
        {
            return .ownership_lost;
        }
        // This successful recheck is the write-start linearization point. A
        // later fence may stop the next slice, but this exact bounded platform
        // call is allowed to complete and is accounted once.
        return .started;
    }

    fn accountWriterSlice(
        self: *TerminalRuntime,
        slice: WriterSlice,
        written: usize,
    ) bool {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        const current_index = self.inputEntryIndex(slice.identifier) orelse return false;
        const current = &self.input_entries.items[current_index];
        if (current.offset != slice.offset or current.source != slice.source or
            current.bytes == null or written == 0 or written > slice.length)
        {
            return false;
        }
        current.offset += written;
        if (current.offset == current.bytes.?.len) {
            self.releaseInputCharge(current.*);
            self.allocator.free(current.bytes.?);
            _ = self.input_entries.orderedRemove(current_index);
            self.input_condition.broadcast();
        }
        return true;
    }

    fn stopWriterAfterOwnershipLoss(self: *TerminalRuntime) void {
        const won = self.closeFailureFence(.worker_integrity);
        self.input_mutex.lock();
        self.clearInputQueueLocked();
        self.input_mutex.unlock();
        if (won) self.finishFailureFence();
    }

    fn writerThread(self: *TerminalRuntime) void {
        defer {
            self.writer_stopped.store(true, .release);
            self.input_mutex.lock();
            self.input_condition.broadcast();
            self.input_mutex.unlock();
        }
        if (!self.awaitStartup()) return;
        var batch: [max_pty_write_slice_bytes]u8 = undefined;
        while (true) {
            self.input_mutex.lock();
            while (!self.writer_stop and !self.failure_fenced and
                nextWritableInputEntry(self.input_entries.items) == null)
            {
                self.input_condition.wait(&self.input_mutex);
            }
            if (self.writer_stop or self.failure_fenced) {
                self.clearInputQueueLocked();
                self.input_mutex.unlock();
                return;
            }
            const entry_index = nextWritableInputEntry(self.input_entries.items) orelse unreachable;
            const entry = &self.input_entries.items[entry_index];
            const bytes = entry.bytes.?;
            if (bytes.len == 0) {
                self.releaseInputCharge(entry.*);
                self.allocator.free(bytes);
                _ = self.input_entries.orderedRemove(entry_index);
                self.input_condition.broadcast();
                self.input_mutex.unlock();
                continue;
            }
            const available = bytes[entry.offset..];
            const length = @min(batch.len, available.len);
            const slice: WriterSlice = .{
                .identifier = entry.identifier,
                .length = length,
                .offset = entry.offset,
                .source = entry.source,
            };
            @memcpy(batch[0..length], available[0..length]);
            self.input_mutex.unlock();

            self.notifyWriterSliceCopied();
            switch (self.startWriterSlice(slice)) {
                .started => {},
                .stopped => return,
                .ownership_lost => {
                    self.stopWriterAfterOwnershipLoss();
                    return;
                },
            }
            self.notifyWriterSliceStarted();
            const written = self.writePtySlice(batch[0..length]) catch {
                self.recordWriterFailure();
                return;
            };
            if (written == 0 or written > length) {
                self.recordWriterFailure();
                return;
            }
            if (!self.accountWriterSlice(slice, written)) {
                self.stopWriterAfterOwnershipLoss();
                return;
            }
            if (slice.source == .client) {
                _ = self.written_since_sync.fetchAdd(written, .release);
            }
        }
    }
};

fn applyScrollbackBytes(terminal: *ghostty_vt.Terminal, scrollback_bytes: usize) u64 {
    const primary = terminal.screens.get(.primary).?;
    const previous_total_rows = primary.pages.total_rows;
    primary.no_scrollback = scrollback_bytes == 0;
    primary.pages.explicit_max_size = scrollback_bytes;

    while (primary.pages.total_rows > terminal.rows and
        (scrollback_bytes == 0 or primary.pages.page_size > primary.pages.maxSize()))
    {
        const history_rows = primary.pages.total_rows - terminal.rows;
        const first_page_rows = primary.pages.pages.first.?.data.size.rows;
        const rows_to_erase = @min(history_rows, first_page_rows);
        primary.pages.eraseHistory(.{ .history = .{ .y = rows_to_erase - 1 } });
    }
    return @intCast(previous_total_rows - primary.pages.total_rows);
}

fn runtimeFromHandler(handler: *ghostty_vt.TerminalStream.Handler) *TerminalRuntime {
    const stream: *ghostty_vt.TerminalStream = @fieldParentPtr("handler", handler);
    return @alignCast(@fieldParentPtr("stream", stream));
}

fn resizePtyWithRollback(
    process: anytype,
    previous: terminal_geometry.Geometry,
    next: terminal_geometry.Geometry,
) !void {
    process.resizePixels(
        next.columns,
        next.rows,
        next.cell_pixel_width,
        next.cell_pixel_height,
    ) catch |resize_error| {
        // Platform adapters do not promise that an error is side-effect free.
        // A successful compensation re-establishes the last committed
        // geometry; otherwise the caller must fence the terminal engine.
        process.resizePixels(
            previous.columns,
            previous.rows,
            previous.cell_pixel_width,
            previous.cell_pixel_height,
        ) catch return error.ResizeRollbackFailed;
        return resize_error;
    };
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const applyScrollbackBytesFn = applyScrollbackBytes;
    pub const cancelInputFn = TerminalRuntime.cancelInput;
    pub const closeClientInputFn = TerminalRuntime.closeClientInput;
    pub const closeFailureFenceFn = TerminalRuntime.closeFailureFence;
    pub const CompletionClassificationType = CompletionClassification;
    pub const completionClassificationLoadFn = CompletionClassification.load;
    pub const createWithOptionsUsingSpawnerFn = TerminalRuntime.createWithOptionsUsingSpawner;
    pub const deinitOwnedProfileFn = OwnedProfile.deinit;
    pub const InputReservationType = InputReservation;
    pub const InputEntryType = InputEntry;
    pub const InputSourceType = InputSource;
    pub const FirstContainmentAttemptType = FirstContainmentAttempt;
    pub const initOwnedProfileFn = OwnedProfile.init;
    pub const modifiersFn = terminal_input_encoding.testing.modifiersFn;
    pub const mouseEncodeEventFn = terminal_input_encoding.testing.mouseEncodeEventFn;
    pub const mouseHasPressedButtonFn = terminal_input_encoding.testing.mouseHasPressedButtonFn;
    pub const nextWritableInputEntryFn = nextWritableInputEntry;
    pub const pending_input_entry_limit = max_pending_client_input_entries;
    pub const pending_input_limit = pending_input_capacity_bytes;
    pub const pending_terminal_reply_entry_limit = max_pending_terminal_reply_entries;
    pub const pending_terminal_reply_limit = pending_terminal_reply_capacity_bytes;
    pub const writer_slice_limit = max_pty_write_slice_bytes;
    pub const publishInputFn = TerminalRuntime.publishInput;
    pub const markCompletionFailureFn = CompletionClassification.markFailure;
    pub const markCompletionNormalFn = CompletionClassification.markNormal;
    pub const markNormalCompletionReadyFn = TerminalRuntime.markNormalCompletionReady;
    pub const recordWorkerIntegrityFailureFn = TerminalRuntime.recordWorkerIntegrityFailure;
    pub const recordReaderFailureFn = TerminalRuntime.recordReaderFailure;
    pub const recordWriterFailureFn = TerminalRuntime.recordWriterFailure;
    pub const ReaderBatchOutcomeType = ReaderBatchOutcome;
    pub const reserveInputFn = TerminalRuntime.reserveInput;
    pub const resizePtyWithRollbackFn = resizePtyWithRollback;
    pub const RuntimeTestingHooksType = RuntimeTestingHooks;

    pub fn clearTestingHooks(runtime: *TerminalRuntime) void {
        runtime.testing_hooks = .{};
    }

    pub fn firstContainmentAttempt(
        runtime: *const TerminalRuntime,
    ) FirstContainmentAttempt {
        return runtime.first_containment_attempt.load(.acquire);
    }

    pub fn failureFenced(runtime: *TerminalRuntime) bool {
        runtime.terminal_mutex.lock();
        defer runtime.terminal_mutex.unlock();
        return runtime.failure_fenced;
    }

    pub fn releaseOutputForShutdown(runtime: *TerminalRuntime) !void {
        runtime.stopIoForShutdown();
        try runtime.releaseOutputForShutdown();
    }

    pub fn resizeEffectInFlight(runtime: *TerminalRuntime) bool {
        runtime.terminal_mutex.lock();
        defer runtime.terminal_mutex.unlock();
        return runtime.resize_effect_in_flight;
    }

    pub fn recordTerminalReplyFailure(runtime: *TerminalRuntime) void {
        runtime.recordFailure(.terminal_reply);
    }

    pub fn fenceResizeFailure(runtime: *TerminalRuntime) void {
        runtime.fenceResizeFailure();
    }
} else struct {};
