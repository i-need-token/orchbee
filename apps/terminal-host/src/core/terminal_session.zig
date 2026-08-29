const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_lifecycle = model.lifecycle;
const terminal_theme = model.theme;

pub const CreationId = identifiers.CreationId;
pub const TerminalId = identifiers.TerminalId;
pub const LaunchDigest = [32]u8;

pub const OutcomeKind = terminal_lifecycle.OutcomeKind;
pub const OutcomePublicationToken = struct {
    final_revision: u64,
    kind: OutcomeKind,
    terminal_id: TerminalId,
};

pub const PendingOutcome = union(enum) {
    exited: struct {
        creation_id: CreationId,
        final_revision: u64,
        result: terminal_lifecycle.ExitResult,
        terminal_id: TerminalId,
        token: OutcomePublicationToken,
    },
    failed: struct {
        creation_id: CreationId,
        failure: terminal_lifecycle.RuntimeFailure,
        final_revision: u64,
        process: terminal_lifecycle.ProcessObservation,
        terminal_id: TerminalId,
        token: OutcomePublicationToken,
    },
};

/// Protocol-visible configuration committed by the session. A native engine
/// applies the value before this state is committed; its physical scrollback
/// setting is implementation state, not a second semantic owner.
pub const ConfigurationState = struct {
    effective: terminal_configuration.Configuration = .{},
    revision: u64 = 1,
};

/// Last engine state synchronized onto the Host owner thread. This snapshot is
/// monotonic and rebuildable; the engine remains authoritative for live VT
/// state.
pub const EngineObservation = struct {
    geometry: terminal_geometry.Geometry,
    geometry_changed_at_revision: u64 = 0,
    revision: u64 = 0,
};

/// Owns every terminal-scoped semantic fact retained by the Host. Registry
/// membership and creation-identity indexing live in TerminalRegistry; broker
/// capabilities live in AttachmentSession.
pub const Session = struct {
    configuration_state: ConfigurationState = .{},
    creation_id: CreationId,
    engine_observation: ?EngineObservation,
    environment_overlay_digest: LaunchDigest = [_]u8{0} ** 32,
    launch_digest: LaunchDigest,
    lifecycle: terminal_lifecycle.Lifecycle,
    retained_cleanup: terminal_lifecycle.RetainedCleanup = .inactive,
    start_geometry: ?terminal_geometry.Geometry,
    terminal_id: TerminalId,
    theme_state: terminal_theme.State,

    pub fn phase(self: Session) terminal_lifecycle.Phase {
        return self.lifecycle.phase();
    }

    pub fn effectiveConfiguration(self: Session) terminal_configuration.Configuration {
        return self.configuration_state.effective;
    }

    pub fn configurationRevision(self: Session) u64 {
        return self.configuration_state.revision;
    }

    pub fn themeState(self: Session) terminal_theme.State {
        return self.theme_state;
    }

    pub fn observedGeometry(self: Session) terminal_geometry.Geometry {
        return self.engine_observation.?.geometry;
    }

    pub fn maybeObservedGeometry(self: Session) ?terminal_geometry.Geometry {
        return if (self.engine_observation) |observation| observation.geometry else null;
    }

    pub fn startGeometry(self: Session) ?terminal_geometry.Geometry {
        return self.start_geometry;
    }

    pub fn observedGeometryRevision(self: Session) u64 {
        return if (self.engine_observation) |observation|
            observation.geometry_changed_at_revision
        else
            0;
    }

    pub fn observedRevision(self: Session) u64 {
        return if (self.engine_observation) |observation| observation.revision else 0;
    }

    pub fn childExited(self: Session) bool {
        return self.lifecycle.childExited();
    }

    pub fn ptyDrained(self: Session) bool {
        return self.lifecycle.ptyDrained();
    }

    pub fn exitResult(self: Session) ?terminal_lifecycle.ExitResult {
        return self.lifecycle.exitResult();
    }

    pub fn terminateRequested(self: Session) bool {
        return self.lifecycle.terminationRequest() != .none;
    }

    pub fn terminationRequest(self: Session) terminal_lifecycle.TerminationRequest {
        return self.lifecycle.terminationRequest();
    }

    pub fn runtimeFailure(self: Session) ?terminal_lifecycle.RuntimeFailure {
        return self.lifecycle.runtimeFailure();
    }

    pub fn failureContainment(self: Session) ?terminal_lifecycle.FailureContainment {
        return self.lifecycle.failureContainment();
    }

    pub fn retainedCleanup(self: Session) terminal_lifecycle.RetainedCleanup {
        return self.retained_cleanup;
    }

    pub fn retainedCleanupPending(self: Session) bool {
        return self.retained_cleanup.pending();
    }

    pub fn retainedCleanupFailure(self: Session) ?terminal_lifecycle.RuntimeCleanupFailure {
        return self.retained_cleanup.failure();
    }

    pub fn retainedCleanupRetryAt(self: Session) ?i128 {
        return self.retained_cleanup.retryAt();
    }

    pub fn failedProcess(self: Session) ?terminal_lifecycle.ProcessObservation {
        return self.lifecycle.failedProcess();
    }

    pub fn settledFinalRevision(self: Session) ?u64 {
        return self.lifecycle.settledFinalRevision();
    }

    pub fn pendingOutcome(self: Session) ?PendingOutcome {
        const pending = self.lifecycle.pendingOutcome() orelse return null;
        return switch (pending) {
            .exited => |outcome| .{ .exited = .{
                .creation_id = self.creation_id,
                .final_revision = outcome.final_revision,
                .result = outcome.result,
                .terminal_id = self.terminal_id,
                .token = .{
                    .final_revision = outcome.final_revision,
                    .kind = .exited,
                    .terminal_id = self.terminal_id,
                },
            } },
            .failed => |failure| .{ .failed = .{
                .creation_id = self.creation_id,
                .failure = failure.failure,
                .final_revision = failure.final_revision,
                .process = failure.process,
                .terminal_id = self.terminal_id,
                .token = .{
                    .final_revision = failure.final_revision,
                    .kind = .failed,
                    .terminal_id = self.terminal_id,
                },
            } },
        };
    }

    pub fn acceptsRuntimeWork(self: Session) bool {
        return self.lifecycle.acceptsRuntimeWork();
    }

    pub fn acceptsConfigurationUpdate(self: Session) bool {
        return self.lifecycle.acceptsConfigurationUpdate();
    }

    pub fn acceptsThemeUpdate(self: Session) bool {
        return switch (self.phase()) {
            .awaiting_start, .running, .finishing => true,
            .containing_failure, .exited, .failed => false,
        };
    }

    pub fn hasReadableRuntimeState(self: Session) bool {
        return self.lifecycle.hasReadableRuntimeState();
    }
};
