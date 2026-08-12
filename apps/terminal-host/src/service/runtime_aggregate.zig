const std = @import("std");
const target_builtin = @import("builtin");

const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const host_input_admission = @import("input_admission.zig");
const host_runtime_maintenance = @import("runtime_maintenance.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_input = @import("terminal-input");
const terminal_launch = terminal_engine.launch;
const terminal_runtime = terminal_engine.runtime;

pub const TerminalIdentity = struct {
    creation_id: core_module.CreationId,
    terminal_id: core_module.TerminalId,
};

pub const TerminalExitNotice = host_runtime_maintenance.TerminalExitNotice;
pub const TerminalFailureNotice = host_runtime_maintenance.TerminalFailureNotice;
pub const TerminalOutcomeNotice = host_runtime_maintenance.TerminalOutcomeNotice;
pub const PendingOutcomeBatch = host_runtime_maintenance.PendingOutcomeBatch;

pub const TerminationMode = enum {
    graceful,
    force,
};

pub const TerminalConfigurationUpdate = struct {
    configuration: terminal_configuration.Configuration,
    configuration_revision: u64,
    generation: u64,
    history_rows_evicted: u64,
    revision: u64,
    terminal_id: core_module.TerminalId,
};

pub const TerminalResize = struct {
    attachment_id: core_module.AttachmentId,
    geometry: terminal_geometry.Geometry,
    revision: u64,
    terminal_id: core_module.TerminalId,
};

pub const TerminalControl = struct {
    attachment_id: core_module.AttachmentId,
    generation: u64,
    operation: terminal_runtime.TerminalControlOperation,
    revision: u64,
    terminal_id: core_module.TerminalId,
};

pub const CreateOutcome = union(enum) {
    created: TerminalIdentity,
    replayed: TerminalIdentity,

    pub fn identity(self: CreateOutcome) TerminalIdentity {
        return switch (self) {
            .created => |value| value,
            .replayed => |value| value,
        };
    }

    pub fn isCreated(self: CreateOutcome) bool {
        return switch (self) {
            .created => true,
            .replayed => false,
        };
    }
};

const PendingCreate = struct {
    allocator: std.mem.Allocator,
    connection_id: core_module.ConnectionId,
    creation_id: core_module.CreationId,
    launch_digest: terminal_launch.Digest,
    runtime_preparation: terminal_launch.RuntimePreparation,
    terminal_id: core_module.TerminalId,
};

pub const CreatePreparation = union(enum) {
    replayed: TerminalIdentity,
    pending: PendingCreate,
    consumed,

    pub fn deinit(self: *CreatePreparation) void {
        switch (self.*) {
            .pending => |*pending| pending.runtime_preparation.deinit(pending.allocator),
            .replayed, .consumed => {},
        }
        self.* = .consumed;
    }

    pub fn identity(self: *const CreatePreparation) TerminalIdentity {
        return switch (self.*) {
            .replayed => |value| value,
            .pending => |pending| .{
                .creation_id = pending.creation_id,
                .terminal_id = pending.terminal_id,
            },
            .consumed => unreachable,
        };
    }

    pub fn requiresEffect(self: *const CreatePreparation) bool {
        return switch (self.*) {
            .pending => true,
            .replayed => false,
            .consumed => unreachable,
        };
    }
};

pub const AttachmentBinding = struct {
    attachment_id: core_module.AttachmentId,
    capabilities: core_module.Capabilities,
    resize_lease: bool,
    terminal_id: core_module.TerminalId,
};

pub const AttachOutcome = struct {
    binding: AttachmentBinding,
    transition: core_module.TransitionOutcome,
};

pub const AcceptedInput = host_input_admission.AcceptedInput;
pub const InputRejection = host_input_admission.Rejection;
pub const InputAdmission = host_input_admission.Admission;
pub const InputPreparation = host_input_admission.Preparation;

pub const CreatePrepareError = core_module.CoreError || std.mem.Allocator.Error || error{
    LaunchPreparationOutOfMemory,
};

pub const CreateCommitError = error{
    RuntimeAllocationOutOfMemory,
    RuntimeSpawnFailed,
};

pub const AttachError = core_module.CoreError || std.mem.Allocator.Error || error{
    HistoryAttachmentFailed,
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
};

pub const DisposeError = core_module.CoreError || error{TerminalRuntimeMissing};

pub const DisposeResult = union(enum) {
    transition: core_module.TransitionOutcome,
    cleanup_blocked: core_module.RuntimeCleanupFailure,
};

pub const TerminationError = core_module.CoreError || error{
    TerminalRuntimeMissing,
    TerminationDispatchFailed,
};

pub const ConfigurationUpdateError = error{
    ConfigurationRevisionOverflow,
    InvalidConfiguration,
    InvalidPhase,
    MissingConnection,
    MissingResource,
    RuntimeStateDiverged,
    TerminalRevisionOverflow,
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
};

pub const ResizeError = core_module.CoreError || error{
    InvalidGeometry,
    RuntimeResizeFailed,
    RuntimeStateDiverged,
    TerminalRevisionOverflow,
    TerminalRuntimeMissing,
    TerminalRuntimeUnavailable,
    UnsafeRuntimeAfterResizeFailure,
};

pub const TerminalControlError = core_module.CoreError || error{
    RuntimeStateDiverged,
    TerminalRuntimeMissing,
};

/// THDD-001 retained-cleanup observability status. While a fatal-teardown pass
/// leaves an unconfirmed-contained process tree, the Host keeps the cleanup
/// ownership and retries containment on the bounded backoff schedule; this
/// status surfaces the retained-cleanup state (active), the bounded retry
/// progress (attempts), and the permanent-failure policy outcome (exhausted).
pub const RetainedCleanupStatus = struct {
    active: bool = false,
    attempts: usize = 0,
    exhausted: bool = false,
};

/// Outcome of the bounded retained-cleanup window (THDD-001). complete means
/// every retained process tree became containment-confirmed and teardown can
/// finish; retained_failed means the bounded retry budget was exhausted while
/// a tree was still unconfirmed, leaving an explicitly retained failed-cleanup
/// state instead of a silent orphan.
pub const RetainedCleanupResult = enum {
    complete,
    retained_failed,
};

/// Bounded number of retained-cleanup passes before the permanent-failure
/// policy leaves an explicit retained failed state. Production paces each pass
/// by the bounded cleanup-retry backoff cadence, so the window retries on that
/// bounded schedule rather than spinning.
const retained_cleanup_max_attempts = 32;

const retained_cleanup_backoff_ns = std.time.ns_per_s;

// HostRuntimeAggregate is the single owner of the deterministic Core record
// and the native runtime bound to each retained TerminalId.
pub const HostRuntimeAggregate = struct {
    allocator: std.mem.Allocator,
    core_state: core_module.HostCore,
    maintenance: host_runtime_maintenance.Maintenance = .{},
    /// THDD-001 observability: retained-cleanup ownership state after a failed
    /// fatal-teardown pass (see RetainedCleanupStatus).
    retained_cleanup: RetainedCleanupStatus = .{},
    /// Test-only teardown-cost sensor: total projection-release decisions
    /// (demand idle) recorded since the last reset. A bounded disconnect
    /// releases projection resources at most once per affected terminal.
    projection_release_decisions: usize = 0,

    pub fn init(allocator: std.mem.Allocator) HostRuntimeAggregate {
        return .{
            .allocator = allocator,
            .core_state = .init(allocator, identifiers.HostEpoch.random()),
        };
    }

    pub fn deinitChecked(
        self: *HostRuntimeAggregate,
        request_incomplete_termination: bool,
    ) !void {
        if (request_incomplete_termination) {
            // Runtime destruction escalates to force termination when this
            // graceful request fails or the PTY has not drained.
            for (self.core_state.terminalValues()) |session| {
                const engine = session.runtime() orelse continue;
                const completion = engine.completion();
                if (!completion.child_exited or !completion.pty_drained) {
                    engine.terminate() catch {};
                }
            }
        }
        shutdownAndReleaseTerminalEngines(self) catch |err| {
            // THDD-001: a failed fatal-teardown pass must never abandon an
            // unconfirmed process tree after one failed pass. Retain the
            // cleanup ownership by recording the exact blocked containment with
            // a bounded retry deadline, so the bounded cleanup-retry scheduler
            // retries it and the release owner stays alive until containment is
            // confirmed. The complete aggregate is retained for that window.
            self.retainUnconfirmedCleanup();
            return err;
        };
        self.core_state.deinit();
        self.* = undefined;
    }

    /// Whether the Host still owns any retained process tree that is not
    /// confirmed contained (THDD-001). While true, teardown must not release
    /// the cleanup ownership and the release owner must keep retrying.
    pub fn retainedCleanupPending(self: *const HostRuntimeAggregate) bool {
        for (self.core_state.terminalValues()) |session| {
            const runtime = session.runtime() orelse continue;
            if (!runtime.containmentConfirmed()) return true;
        }
        return false;
    }

    /// THDD-001 observability status for the retained-cleanup state.
    pub fn retainedCleanupStatus(self: *const HostRuntimeAggregate) RetainedCleanupStatus {
        return self.retained_cleanup;
    }

    /// One retained-cleanup pass: drives the existing bounded cleanup-retry
    /// scheduler (so only retries whose bounded backoff deadline is due fire)
    /// and returns whether every retained tree is now containment-confirmed.
    pub fn runRetainedCleanupPass(self: *HostRuntimeAggregate) bool {
        self.synchronizeRuntimes();
        self.retained_cleanup.attempts += 1;
        return !self.retainedCleanupPending();
    }

    /// THDD-001 bounded retained-cleanup window. Reuses the existing bounded
    /// cleanup-retry scheduler and its monotonic backoff cadence; each pass
    /// retries only due cleanups. On the bounded budget being exhausted with a
    /// tree still unconfirmed, the permanent-failure policy leaves an explicit
    /// retained failed state (observable via retainedCleanupStatus) and never
    /// silently releases the tree. The caller surfaces the retained-failed
    /// outcome as the loud shutdown signal.
    pub fn runRetainedCleanupWindow(self: *HostRuntimeAggregate) RetainedCleanupResult {
        var attempts: usize = 0;
        while (attempts < retained_cleanup_max_attempts) : (attempts += 1) {
            if (self.runRetainedCleanupPass()) {
                std.log.info("terminal Host completed retained cleanup after {d} pass(es)", .{attempts + 1});
                return .complete;
            }
            if (comptime !target_builtin.is_test) {
                std.Thread.sleep(retained_cleanup_backoff_ns);
            }
        }
        self.markRetainedCleanupExhausted();
        return .retained_failed;
    }

    /// Completes teardown once every retained runtime is containment-confirmed.
    /// Destroys the runtimes and releases the Core record, mirroring the
    /// successful fatal-teardown path.
    pub fn finishRetainedCleanup(self: *HostRuntimeAggregate) !void {
        try shutdownAndReleaseTerminalEngines(self);
        self.core_state.deinit();
        self.* = undefined;
    }

    /// THDD-001: records retained cleanup ownership for every retained runtime
    /// whose process tree is not confirmed contained after a failed
    /// fatal-teardown pass. Each such terminal becomes `containing_failure` with
    /// a blocked containment and a bounded retry deadline, so the existing
    /// cleanup-retry scheduler can retry it without a broker.
    fn retainUnconfirmedCleanup(self: *HostRuntimeAggregate) void {
        var retained: bool = false;
        for (self.core_state.terminalValues()) |session| {
            const terminal_id = session.terminal_id;
            const engine = session.runtime() orelse continue;
            if (engine.containmentConfirmed()) continue;
            retained = true;
            const primary_failure: core_module.RuntimeFailure = switch (engine.completion().classification) {
                .failed => |kind| runtimeFailureFromEngineKind(kind),
                .pending, .normal => .observe_process,
            };
            _ = self.core_state.runtimeFailed(terminal_id, primary_failure) catch continue;
            _ = self.core_state.failureContainmentBlocked(
                terminal_id,
                .contain_process,
                self.maintenance.nextCleanupRetryAt(),
            ) catch continue;
        }
        if (retained) {
            self.retained_cleanup = .{ .active = true };
        }
    }

    /// THDD-001 permanent-failure policy: after the bounded retained-cleanup
    /// budget is exhausted, every still-unconfirmed blocked cleanup becomes a
    /// stable explicit retained failed state (never-due retry) that is never
    /// silently released. The observable status records the exhausted outcome.
    fn markRetainedCleanupExhausted(self: *HostRuntimeAggregate) void {
        for (self.core_state.terminalValues()) |session| {
            const terminal_id = session.terminal_id;
            const runtime = session.runtime() orelse continue;
            if (runtime.containmentConfirmed()) continue;
            const containment = session.failureContainment() orelse continue;
            const cleanup_failure: core_module.RuntimeCleanupFailure = switch (containment) {
                .blocked => |blocked| blocked.cleanup_failure,
                .stopping => .contain_process,
            };
            _ = self.core_state.failureContainmentBlocked(
                terminal_id,
                cleanup_failure,
                std.math.maxInt(i128),
            ) catch continue;
        }
        self.retained_cleanup.exhausted = true;
    }

    fn ownedTerminalCount(self: *const HostRuntimeAggregate) usize {
        return self.core_state.terminalCount();
    }

    fn runtimeAtOwnedTerminal(
        self: *HostRuntimeAggregate,
        index: usize,
    ) ?*terminal_runtime.TerminalRuntime {
        return self.core_state.terminalValues()[index].runtime();
    }

    fn takeLastOwnedRuntime(
        self: *HostRuntimeAggregate,
    ) ?*terminal_runtime.TerminalRuntime {
        const index = self.core_state.terminalCount() - 1;
        const terminal_id = self.core_state.terminalKeys()[index];
        return self.core_state.takeTerminalForHostShutdown(terminal_id).runtimePointer();
    }

    pub fn core(self: *HostRuntimeAggregate) *core_module.HostCore {
        return &self.core_state;
    }

    pub fn isEmpty(self: *const HostRuntimeAggregate) bool {
        return self.core_state.connectionCount() == 0 and
            self.core_state.terminalCount() == 0;
    }

    pub fn runtimeCount(self: *const HostRuntimeAggregate) usize {
        var count: usize = 0;
        for (self.core_state.terminalValues()) |session| {
            if (session.runtime() != null) count += 1;
        }
        return count;
    }

    pub fn runtimeFor(
        self: *HostRuntimeAggregate,
        terminal_id: core_module.TerminalId,
    ) ?*terminal_runtime.TerminalRuntime {
        const session = self.core_state.findTerminal(terminal_id) orelse return null;
        return session.runtime();
    }

    pub fn synchronizeRuntimeObservation(
        self: *HostRuntimeAggregate,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) core_module.CoreError!terminal_runtime.Geometry {
        return host_runtime_maintenance.synchronizeRuntimeObservation(
            &self.core_state,
            terminal_id,
            runtime,
        );
    }

    pub fn nextProjectionAt(
        self: *HostRuntimeAggregate,
        terminal_id: core_module.TerminalId,
    ) ?i128 {
        const runtime = self.runtimeFor(terminal_id) orelse return null;
        return runtime.nextProjectionAt();
    }

    pub fn setNextProjectionAt(
        self: *HostRuntimeAggregate,
        terminal_id: core_module.TerminalId,
        next_projection_at_ns: i128,
    ) bool {
        const runtime = self.runtimeFor(terminal_id) orelse return false;
        runtime.setNextProjectionAt(next_projection_at_ns);
        return true;
    }

    pub fn prepareCreateTerminal(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        creation_id: core_module.CreationId,
        validated_launch: terminal_launch.Validated,
        default_configuration: terminal_configuration.Configuration,
    ) CreatePrepareError!CreatePreparation {
        if (try self.core_state.resolveCreation(
            connection_id,
            creation_id,
            validated_launch.launch_digest,
        )) |terminal_id| {
            const terminal = self.core_state.findTerminal(terminal_id) orelse unreachable;
            return .{ .replayed = .{
                .creation_id = terminal.creation_id,
                .terminal_id = terminal_id,
            } };
        }

        var prepared_launch = validated_launch.prepareRuntime(
            self.allocator,
            default_configuration,
        ) catch return error.LaunchPreparationOutOfMemory;
        errdefer prepared_launch.deinit(self.allocator);

        try self.core_state.reserveTerminal();

        var terminal_id = identifiers.TerminalId.random();
        while (self.core_state.findTerminal(terminal_id) != null) {
            terminal_id = identifiers.TerminalId.random();
        }

        return .{ .pending = .{
            .allocator = self.allocator,
            .connection_id = connection_id,
            .creation_id = creation_id,
            .launch_digest = validated_launch.launch_digest,
            .runtime_preparation = prepared_launch,
            .terminal_id = terminal_id,
        } };
    }

    pub fn commitPreparedTerminal(
        self: *HostRuntimeAggregate,
        preparation: *CreatePreparation,
    ) CreateCommitError!CreateOutcome {
        switch (preparation.*) {
            .replayed => |identity| return .{ .replayed = identity },
            .consumed => unreachable,
            .pending => |*pending| {
                const runtime = terminal_runtime.TerminalRuntime.createWithOptions(
                    self.allocator,
                    pending.runtime_preparation.process_launch,
                    pending.runtime_preparation.runtime_options,
                ) catch |err| {
                    return if (err == error.OutOfMemory)
                        error.RuntimeAllocationOutOfMemory
                    else
                        error.RuntimeSpawnFailed;
                };
                // All domain capacity and the complete engine exist before
                // this infallible commit. The caller has already encoded and
                // reserved the records that make the commit observable.
                self.core_state.commitRunningTerminal(
                    pending.connection_id,
                    pending.creation_id,
                    pending.terminal_id,
                    pending.launch_digest,
                    pending.runtime_preparation.process_launch.columns,
                    pending.runtime_preparation.process_launch.rows,
                    pending.runtime_preparation.process_launch.cell_pixel_width,
                    pending.runtime_preparation.process_launch.cell_pixel_height,
                    pending.runtime_preparation.configuration,
                    runtime,
                );
                const identity: TerminalIdentity = .{
                    .creation_id = pending.creation_id,
                    .terminal_id = pending.terminal_id,
                };
                pending.runtime_preparation.deinit(self.allocator);
                preparation.* = .consumed;
                return .{ .created = identity };
            },
        }
    }

    pub fn requestTermination(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        terminal_id: core_module.TerminalId,
        mode: TerminationMode,
    ) TerminationError!core_module.TransitionOutcome {
        var dispatcher: SystemTerminationDispatcher = .{};
        var failure_fencer: RuntimeFailureFencer = .{};
        var containment: RuntimeFailureContainment = .{ .maintenance = &self.maintenance };
        return self.requestTerminationUsing(
            connection_id,
            terminal_id,
            mode,
            &dispatcher,
            &failure_fencer,
            &containment,
        );
    }

    fn requestTerminationUsing(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        terminal_id: core_module.TerminalId,
        mode: TerminationMode,
        dispatcher: anytype,
        failure_fencer: anytype,
        containment: anytype,
    ) TerminationError!core_module.TransitionOutcome {
        // THCR-036: the bounded background scan may not have visited this
        // exact terminal. Copy an already-published engine classification
        // (normal completion or failure) and complete or record containment
        // before any Core admission or dispatch decision, so a skipped target
        // is never dispatched against a stale running/finishing lifecycle.
        self.synchronizeTerminal(terminal_id);
        const request: core_module.TerminationRequest = switch (mode) {
            .graceful => .graceful,
            .force => .force,
        };
        const admission = try self.core_state.prepareTermination(
            connection_id,
            terminal_id,
            request,
        );
        if (admission == .replay) return .idempotent;

        const runtime = self.runtimeFor(terminal_id) orelse {
            const failure: core_module.RuntimeFailure = .synchronize_worker;
            _ = self.core_state.runtimeFailed(terminal_id, failure) catch {};
            _ = self.core_state.failureContainmentBlocked(
                terminal_id,
                .contain_process,
                self.maintenance.nextCleanupRetryAt(),
            ) catch {};
            return error.TerminalRuntimeMissing;
        };
        if (admission == .containment_retry) {
            containment.contain(&self.core_state, terminal_id, runtime) catch
                return error.TerminationDispatchFailed;
            return .applied;
        }
        dispatcher.dispatch(runtime, mode) catch {
            // The engine gate is the first visible failure transition. Core
            // publishes the Host-side primary cause only after admission,
            // writer start, and reader mutation are irreversibly closed.
            switch (failure_fencer.fence(runtime)) {
                .host_won => self.core_state.commitTerminationDispatchFailure(terminal_id),
                .engine_won => |failure| {
                    const transition = self.core_state.runtimeFailed(
                        terminal_id,
                        runtimeFailureFromEngineKind(failure),
                    ) catch unreachable;
                    std.debug.assert(transition == .applied);
                },
            }
            containment.contain(&self.core_state, terminal_id, runtime) catch {};
            return error.TerminationDispatchFailed;
        };
        self.core_state.commitPreparedTermination(terminal_id, request);
        return .applied;
    }

    pub fn attach(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        terminal_id: core_module.TerminalId,
        capabilities: core_module.Capabilities,
        request_resize_lease: bool,
    ) AttachError!AttachOutcome {
        const admission = try self.core_state.validateAttachmentAdmission(
            connection_id,
            attachment_id,
            terminal_id,
            capabilities,
            request_resize_lease,
        );
        if (admission == .replay) {
            return .{
                .binding = attachmentBinding(self.core_state.findAttachment(attachment_id).?),
                .transition = .idempotent,
            };
        }

        const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
        var runtime_preparation = runtime.prepareAttachment(
            attachment_id,
            capabilities.input or capabilities.resize,
            capabilities.read,
        ) catch |err| return switch (err) {
            error.OutOfMemory => error.HistoryAttachmentFailed,
            error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => error.TerminalRuntimeUnavailable,
        };
        defer runtime_preparation.deinit();
        const transition = try self.core_state.attachAtBellSequence(
            connection_id,
            attachment_id,
            terminal_id,
            capabilities,
            request_resize_lease,
            runtime_preparation.bellSequence(),
        );
        std.debug.assert(transition == .applied);
        runtime_preparation.commit();
        const attachment = self.core_state.findAttachment(attachment_id) orelse unreachable;
        return .{
            .binding = attachmentBinding(attachment),
            .transition = transition,
        };
    }

    pub fn acceptRawInput(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        bytes: []const u8,
    ) InputAdmission {
        var preparation = self.prepareRawInput(connection_id, attachment_id, bytes);
        defer preparation.deinit();
        return preparation.commit();
    }

    pub fn prepareRawInput(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        bytes: []const u8,
    ) InputPreparation {
        const attachment = self.core_state.authorizeInput(
            connection_id,
            attachment_id,
        ) catch |err| return InputPreparation.initCoreRejected(err);
        const terminal_id = attachment.terminal_id;
        const runtime = self.runtimeFor(terminal_id) orelse
            return InputPreparation.initRuntimeMissing();

        const runtime_input = runtime.prepareRawInput(bytes) catch |err|
            return InputPreparation.initRuntimeRejected(err);
        return InputPreparation.initPrepared(attachment_id, runtime_input);
    }

    pub fn acceptSemanticInput(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        input: terminal_input.SemanticInput,
        maximum_bytes: usize,
    ) InputAdmission {
        var preparation = self.prepareSemanticInput(
            connection_id,
            attachment_id,
            input,
            maximum_bytes,
        );
        defer preparation.deinit();
        return preparation.commit();
    }

    pub fn prepareSemanticInput(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        input: terminal_input.SemanticInput,
        maximum_bytes: usize,
    ) InputPreparation {
        const attachment = self.core_state.authorizeInput(
            connection_id,
            attachment_id,
        ) catch |err| return InputPreparation.initCoreRejected(err);
        const terminal_id = attachment.terminal_id;
        const runtime = self.runtimeFor(terminal_id) orelse
            return InputPreparation.initRuntimeMissing();

        const runtime_input = runtime.prepareSemanticInput(input, maximum_bytes) catch |err|
            return InputPreparation.initRuntimeRejected(err);
        return InputPreparation.initPrepared(attachment_id, runtime_input);
    }

    pub fn detach(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
    ) core_module.CoreError!core_module.TransitionOutcome {
        if (self.core_state.findAttachment(attachment_id) == null) {
            return self.core_state.detach(connection_id, attachment_id);
        }
        const terminal_id = (try self.core_state.resolveAttachment(
            connection_id,
            attachment_id,
            .none,
        )).terminal_id;
        // resolveAttachment proved the only fallible Core precondition. Commit
        // broker authority first, then release infallible runtime-derived state.
        const transition = self.core_state.detach(connection_id, attachment_id) catch unreachable;
        if (self.runtimeFor(terminal_id)) |runtime| runtime.detachHistory(attachment_id);
        self.releaseProjectionIfIdle(terminal_id);
        return transition;
    }

    pub fn updateTerminalConfiguration(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        terminal_id: core_module.TerminalId,
        configuration: terminal_configuration.Configuration,
    ) ConfigurationUpdateError!TerminalConfigurationUpdate {
        if (!terminal_configuration.valid(configuration)) return error.InvalidConfiguration;
        if (self.core_state.connectionClientId(connection_id) == null) return error.MissingConnection;
        const terminal = self.core_state.findTerminal(terminal_id) orelse return error.MissingResource;
        if (!terminal.acceptsConfigurationUpdate()) return error.InvalidPhase;
        if (terminal.effectiveConfiguration().scrollback_bytes != configuration.scrollback_bytes and
            terminal.configurationRevision() == std.math.maxInt(u64))
        {
            return error.ConfigurationRevisionOverflow;
        }
        const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
        if (runtime.configuration().scrollback_bytes !=
            terminal.effectiveConfiguration().scrollback_bytes)
        {
            return error.RuntimeStateDiverged;
        }
        _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
            return error.RuntimeStateDiverged;
        const runtime_update = runtime.updateConfiguration(configuration) catch |err| {
            return switch (err) {
                error.InvalidConfiguration => error.InvalidConfiguration,
                error.RevisionOverflow => error.TerminalRevisionOverflow,
                error.TerminalResizeFailed => error.TerminalRuntimeUnavailable,
                error.TerminalRuntimeUnavailable => error.TerminalRuntimeUnavailable,
            };
        };

        // All Core checks happen before the runtime mutation. HostService is
        // single-owner, so these commits cannot fail without an invariant
        // violation after the runtime has accepted the update.
        _ = self.core_state.updateTerminalConfiguration(
            connection_id,
            terminal_id,
            configuration,
        ) catch unreachable;
        const runtime_geometry = self.synchronizeRuntimeObservation(
            terminal_id,
            runtime,
        ) catch unreachable;
        std.debug.assert(runtime_geometry.revision == runtime_update.revision);
        const updated_terminal = self.core_state.findTerminal(terminal_id).?;
        return .{
            .configuration = updated_terminal.effectiveConfiguration(),
            .configuration_revision = updated_terminal.configurationRevision(),
            .generation = runtime_update.generation,
            .history_rows_evicted = runtime_update.history_rows_evicted,
            .revision = runtime_update.revision,
            .terminal_id = terminal_id,
        };
    }

    pub fn resizeTerminal(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        geometry: terminal_geometry.Geometry,
    ) ResizeError!TerminalResize {
        if (!terminal_geometry.isValid(geometry)) return error.InvalidGeometry;
        const attachment = self.core_state.resolveAttachment(
            connection_id,
            attachment_id,
            .resize,
        ) catch |err| return err;
        if (!attachment.resize_lease) return error.ResizeLeaseDenied;
        const terminal_id = attachment.terminal_id;
        const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
        _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
            return error.RuntimeStateDiverged;
        _ = try self.core_state.resize(
            connection_id,
            attachment_id,
            geometry.columns,
            geometry.rows,
        );
        const revision = runtime.resizeGeometry(
            geometry.columns,
            geometry.rows,
            geometry.cell_pixel_width,
            geometry.cell_pixel_height,
        ) catch |err| {
            if (err == error.RevisionOverflow) return error.TerminalRevisionOverflow;
            if (err == error.TerminalRuntimeUnavailable) return error.TerminalRuntimeUnavailable;
            if (!runtime.isResizeFenced()) return error.RuntimeResizeFailed;
            const failure = runtime.primaryFailure() orelse .resize;
            _ = self.core_state.runtimeFailed(
                terminal_id,
                runtimeFailureFromEngineKind(failure),
            ) catch unreachable;
            self.maintenance.containRuntimeFailure(
                &self.core_state,
                terminal_id,
                runtime,
            ) catch {};
            return error.UnsafeRuntimeAfterResizeFailure;
        };
        const runtime_geometry = self.synchronizeRuntimeObservation(
            terminal_id,
            runtime,
        ) catch unreachable;
        std.debug.assert(runtime_geometry.columns == geometry.columns);
        std.debug.assert(runtime_geometry.rows == geometry.rows);
        std.debug.assert(runtime_geometry.cell_pixel_width == geometry.cell_pixel_width);
        std.debug.assert(runtime_geometry.cell_pixel_height == geometry.cell_pixel_height);
        std.debug.assert(runtime_geometry.revision == revision);
        return .{
            .attachment_id = attachment_id,
            .geometry = geometry,
            .revision = revision,
            .terminal_id = terminal_id,
        };
    }

    pub fn applyTerminalControl(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        operation: terminal_runtime.TerminalControlOperation,
    ) TerminalControlError!TerminalControl {
        const attachment = try self.core_state.authorizeInput(connection_id, attachment_id);
        const terminal_id = attachment.terminal_id;
        const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
        _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
            return error.RuntimeStateDiverged;
        const update = runtime.applyTerminalControl(operation) catch |err| return switch (err) {
            error.RevisionOverflow => error.RevisionOverflow,
            error.TerminalExited,
            error.TerminalResizeFailed,
            error.TerminalRuntimeUnavailable,
            => error.InvalidPhase,
        };
        const observation = self.synchronizeRuntimeObservation(terminal_id, runtime) catch unreachable;
        std.debug.assert(observation.generation == update.generation);
        std.debug.assert(observation.revision == update.revision);
        return .{
            .attachment_id = attachment_id,
            .generation = update.generation,
            .operation = operation,
            .revision = update.revision,
            .terminal_id = terminal_id,
        };
    }

    pub fn disconnect(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
    ) core_module.TransitionOutcome {
        // Bulk one-pass teardown: detach every attachment exactly once, then
        // release projection resources at most once per affected terminal
        // (when its last attachment is removed). The O(1) active-demand
        // aggregate keeps each release decision constant-cost, so an
        // N-attachment disconnect is linear instead of quadratic.
        while (self.core_state.attachmentsForConnection(connection_id).len > 0) {
            const attachment_ids = self.core_state.attachmentsForConnection(connection_id);
            const attachment_id = attachment_ids[attachment_ids.len - 1];
            const terminal_id = self.core_state.findAttachment(attachment_id).?.terminal_id;
            _ = self.core_state.detach(connection_id, attachment_id) catch unreachable;
            if (self.runtimeFor(terminal_id)) |runtime| {
                runtime.detachHistory(attachment_id);
            }
            if (self.core_state.attachmentCountForTerminal(terminal_id) == 0) {
                self.releaseProjectionIfIdle(terminal_id);
            }
        }
        return self.core_state.disconnect(connection_id);
    }

    pub fn setProjectionDemand(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.AttachmentId,
        demand: core_module.ProjectionDemand,
    ) core_module.CoreError!core_module.TransitionOutcome {
        const attachment = try self.core_state.resolveAttachment(connection_id, attachment_id, .read);
        const terminal_id = attachment.terminal_id;
        const transition = try self.core_state.setProjectionDemand(connection_id, attachment_id, demand);
        self.releaseProjectionIfIdle(terminal_id);
        return transition;
    }

    pub fn synchronizeRuntimes(self: *HostRuntimeAggregate) void {
        self.maintenance.synchronizeRuntimes(&self.core_state);
    }

    /// Synchronizes exactly the bounded inventory slice that is about to be
    /// serialized. This gives one page a coherent Host observation boundary
    /// even when the fair background scan covers fewer terminals per poll.
    pub fn synchronizeTerminalRange(
        self: *HostRuntimeAggregate,
        offset: usize,
        maximum_terminals: usize,
    ) void {
        self.maintenance.synchronizeTerminalRange(
            &self.core_state,
            offset,
            maximum_terminals,
        );
    }

    pub fn pendingOutcomes(
        self: *HostRuntimeAggregate,
        maximum_notices: usize,
    ) PendingOutcomeBatch {
        return self.maintenance.pendingOutcomes(&self.core_state, maximum_notices);
    }

    /// Synchronizes exactly one retained terminal independently of the fair
    /// background cursor. requestTermination and disposeTerminal invoke this
    /// before their Core admission checks so a command target the bounded
    /// background scan has not yet visited still receives target-aware
    /// ordering; the 64-entry fair scan remains solely for background
    /// maintenance.
    pub fn synchronizeTerminal(
        self: *HostRuntimeAggregate,
        terminal_id: core_module.TerminalId,
    ) void {
        self.maintenance.synchronizeTerminal(&self.core_state, terminal_id);
    }

    pub fn disposeTerminal(
        self: *HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        terminal_id: core_module.TerminalId,
    ) DisposeError!DisposeResult {
        // THCR-036: the bounded background scan may not have visited this
        // exact terminal. Synchronize its already-published engine
        // classification and complete or record containment before the
        // disposal phase decision, so a settled target is never rejected as
        // invalid-phase against a stale running/finishing lifecycle.
        self.synchronizeTerminal(terminal_id);
        const admission = try self.core_state.validateTerminalDisposal(
            connection_id,
            terminal_id,
        );
        switch (admission) {
            .idempotent => return .{ .transition = .idempotent },
            // An already-blocked cleanup retains its exact failing phase.
            // Return it directly before any runtime lookup or shutdown; the
            // independent maintenance pass retries cleanup and this request
            // never mutates or retries the blocked phase.
            .cleanup_blocked => |failure| return .{ .cleanup_blocked = failure },
            .containment_stopping => return error.FailureContainmentIncomplete,
            .dispose => {},
        }
        const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;

        // Dispose is the final owner handoff. Join the engine while its
        // TerminalSession still retains it; a failed shutdown leaves the
        // complete session intact so the operation can be retried safely.
        runtime.shutdown() catch |err| {
            return .{ .cleanup_blocked = host_runtime_maintenance.cleanupFailure(err) };
        };
        self.core_state.commitTerminalRemoval(terminal_id);
        runtime.destroyAfterShutdown();
        return .{ .transition = .applied };
    }

    fn releaseProjectionIfIdle(
        self: *HostRuntimeAggregate,
        terminal_id: core_module.TerminalId,
    ) void {
        if (self.core_state.hasActiveProjectionDemand(terminal_id)) return;
        if (target_builtin.is_test) self.projection_release_decisions += 1;
        if (self.runtimeFor(terminal_id)) |runtime| runtime.releaseProjectionResources();
    }
};

const SystemTerminationDispatcher = struct {
    fn dispatch(
        _: *SystemTerminationDispatcher,
        runtime: *terminal_runtime.TerminalRuntime,
        mode: TerminationMode,
    ) !void {
        try dispatchTermination(runtime, mode);
    }
};

const RuntimeFailureFencer = struct {
    fn fence(
        _: *RuntimeFailureFencer,
        runtime: *terminal_runtime.TerminalRuntime,
    ) terminal_runtime.HostFailureFenceResult {
        return runtime.fenceHostFailure();
    }
};

const RuntimeFailureContainment = struct {
    maintenance: *host_runtime_maintenance.Maintenance,

    fn contain(
        self: *RuntimeFailureContainment,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !void {
        try self.maintenance.containRuntimeFailure(core, terminal_id, runtime);
    }
};

fn dispatchTermination(
    runtime: *terminal_runtime.TerminalRuntime,
    mode: TerminationMode,
) !void {
    switch (mode) {
        .graceful => try runtime.terminate(),
        .force => try runtime.forceTerminate(),
    }
}

fn runtimeFailureFromEngineKind(
    failure: terminal_runtime.RuntimeFailureKind,
) core_module.RuntimeFailure {
    return switch (failure) {
        .resize => .apply_resize,
        .process_observation => .observe_process,
        .pty_read => .read_output,
        .pty_write => .write_input,
        .terminal_reply => .admit_terminal_reply,
        .worker_integrity => .synchronize_worker,
    };
}

fn attachmentBinding(attachment: *const core_module.Attachment) AttachmentBinding {
    return .{
        .attachment_id = attachment.attachment_id,
        .capabilities = attachment.capabilities,
        .resize_lease = attachment.resize_lease,
        .terminal_id = attachment.terminal_id,
    };
}

fn shutdownAndReleaseTerminalEngines(owner: anytype) !void {
    var first_error: ?anyerror = null;
    for (0..owner.ownedTerminalCount()) |index| {
        const engine = owner.runtimeAtOwnedTerminal(index) orelse continue;
        engine.shutdown() catch |err| {
            if (first_error == null) first_error = err;
        };
    }
    if (first_error) |err| return err;
    while (owner.ownedTerminalCount() > 0) {
        if (owner.takeLastOwnedRuntime()) |engine| engine.destroyAfterShutdown();
    }
}

fn recordUnsafeResizeFailureForTesting(
    aggregate: *HostRuntimeAggregate,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
) void {
    aggregate.maintenance.recordUnsafeResizeFailure(
        &aggregate.core_state,
        terminal_id,
        runtime,
    );
}

fn takeDueCleanupRetryForTesting(
    aggregate: *HostRuntimeAggregate,
    now_ns: i128,
) ?core_module.TerminalId {
    return aggregate.maintenance.takeDueCleanupRetry(&aggregate.core_state, now_ns);
}

fn retainUnconfirmedCleanupForTesting(aggregate: *HostRuntimeAggregate) void {
    aggregate.retainUnconfirmedCleanup();
}

fn cleanupRetryScanIndexForTesting(aggregate: *const HostRuntimeAggregate) usize {
    return aggregate.maintenance.cleanupRetryScanIndex();
}

fn requestTerminationUsingForTesting(
    aggregate: *HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    mode: TerminationMode,
    dispatcher: anytype,
    failure_fencer: anytype,
    containment: anytype,
) TerminationError!core_module.TransitionOutcome {
    return aggregate.requestTerminationUsing(
        connection_id,
        terminal_id,
        mode,
        dispatcher,
        failure_fencer,
        containment,
    );
}

pub const testing = if (target_builtin.is_test) struct {
    pub const cleanupFailureFn = host_runtime_maintenance.cleanupFailure;
    pub const cleanupRetryScanIndexFn = cleanupRetryScanIndexForTesting;
    pub const CleanupClockType = host_runtime_maintenance.CleanupClock;
    pub const RetainedCleanupResultType = RetainedCleanupResult;
    pub const RetainedCleanupStatusType = RetainedCleanupStatus;
    pub fn finishRetainedCleanup(aggregate: *HostRuntimeAggregate) !void {
        try aggregate.finishRetainedCleanup();
    }
    pub fn markRetainedCleanupExhausted(aggregate: *HostRuntimeAggregate) void {
        aggregate.markRetainedCleanupExhausted();
    }
    pub fn projectionReleaseDecisions(aggregate: *const HostRuntimeAggregate) usize {
        return aggregate.projection_release_decisions;
    }

    pub fn resetProjectionReleaseDecisions(aggregate: *HostRuntimeAggregate) void {
        aggregate.projection_release_decisions = 0;
    }
    pub fn retainedCleanupPending(aggregate: *const HostRuntimeAggregate) bool {
        return aggregate.retainedCleanupPending();
    }
    pub const retainedCleanupStatusFn = HostRuntimeAggregate.retainedCleanupStatus;
    pub const retainUnconfirmedCleanupFn = retainUnconfirmedCleanupForTesting;
    pub fn runRetainedCleanupPass(aggregate: *HostRuntimeAggregate) bool {
        return aggregate.runRetainedCleanupPass();
    }
    pub const runRetainedCleanupWindowFn = HostRuntimeAggregate.runRetainedCleanupWindow;
    pub fn setCleanupClockForTesting(
        aggregate: *HostRuntimeAggregate,
        read_ns: *const fn (*host_runtime_maintenance.CleanupClock) i128,
        context: ?*anyopaque,
    ) void {
        aggregate.maintenance.cleanup_clock.read_ns = read_ns;
        aggregate.maintenance.cleanup_clock.context = context;
    }
    pub const recordUnsafeResizeFailureFn = recordUnsafeResizeFailureForTesting;
    pub const requestTerminationUsingFn = requestTerminationUsingForTesting;
    pub const runtimeFailureFromObservationFn =
        host_runtime_maintenance.runtimeFailureFromObservation;
    pub const shutdownAndReleaseTerminalEnginesFn = shutdownAndReleaseTerminalEngines;
    pub const synchronizeRuntimeCompletionFn =
        host_runtime_maintenance.synchronizeRuntimeCompletion;
    pub const takeDueCleanupRetryFn = takeDueCleanupRetryForTesting;
} else struct {};
