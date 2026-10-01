const std = @import("std");

pub const Phase = enum {
    awaiting_start,
    running,
    finishing,
    containing_failure,
    exited,
    failed,
};

pub const TransitionOutcome = enum {
    applied,
    idempotent,
};

pub const TerminationAdmission = enum {
    containment_retry,
    dispatch,
    replay,
};

/// Disposal admission for a retained lifecycle: a settled published terminal
/// is disposable; an already-blocked cleanup carries the exact failing phase
/// that must be reported to the caller; stopping containment has no retained
/// failing cleanup phase yet, so it stays incomplete containment.
pub const DisposableAdmission = union(enum) {
    dispose,
    containment_stopping,
    cleanup_blocked: RuntimeCleanupFailure,
};

pub const ExitResult = union(enum) {
    exit_code: u32,
    signal: u32,
    unknown: u32,
};

pub const CompletionSignal = union(enum) {
    child_exited: ExitResult,
    pty_drained,
};

pub const TerminationRequest = enum {
    none,
    graceful,
    force,
};

pub const RuntimeFailureKind = enum {
    editor_document,
    process_control,
    process_observation,
    pty_read,
    pty_write,
    resize,
    terminal_reply,
    worker_integrity,
};

pub const RuntimeFailureStage = enum {
    admit_terminal_reply,
    apply_resize,
    contain_process,
    dispatch_termination,
    observe_process,
    read_output,
    release_output,
    release_process_identity,
    release_editor_document,
    synchronize_worker,
    write_input,
};

pub const RuntimeFailure = enum {
    admit_terminal_reply,
    apply_resize,
    dispatch_termination,
    observe_process,
    read_output,
    synchronize_worker,
    write_input,

    pub fn kind(self: RuntimeFailure) RuntimeFailureKind {
        return switch (self) {
            .admit_terminal_reply => .terminal_reply,
            .apply_resize => .resize,
            .dispatch_termination => .process_control,
            .observe_process => .process_observation,
            .read_output => .pty_read,
            .synchronize_worker => .worker_integrity,
            .write_input => .pty_write,
        };
    }

    pub fn stage(self: RuntimeFailure) RuntimeFailureStage {
        return switch (self) {
            .admit_terminal_reply => .admit_terminal_reply,
            .apply_resize => .apply_resize,
            .dispatch_termination => .dispatch_termination,
            .observe_process => .observe_process,
            .read_output => .read_output,
            .synchronize_worker => .synchronize_worker,
            .write_input => .write_input,
        };
    }
};

pub const RuntimeCleanupFailure = enum {
    contain_process,
    release_output,
    release_process_identity,
    release_editor_document,

    pub fn kind(self: RuntimeCleanupFailure) RuntimeFailureKind {
        return switch (self) {
            .contain_process => .process_control,
            .release_output => .pty_read,
            .release_process_identity => .process_observation,
            .release_editor_document => .editor_document,
        };
    }

    pub fn stage(self: RuntimeCleanupFailure) RuntimeFailureStage {
        return switch (self) {
            .contain_process => .contain_process,
            .release_output => .release_output,
            .release_process_identity => .release_process_identity,
            .release_editor_document => .release_editor_document,
        };
    }
};

pub const ProcessObservationFailure = enum {
    observe_process,

    pub fn kind(_: ProcessObservationFailure) RuntimeFailureKind {
        return .process_observation;
    }

    pub fn stage(_: ProcessObservationFailure) RuntimeFailureStage {
        return .observe_process;
    }
};

pub const ProcessObservation = union(enum) {
    observed: ExitResult,
    unavailable: ProcessObservationFailure,
};

pub const FailureContainment = union(enum) {
    stopping: struct {
        process: ?ProcessObservation,
    },
    blocked: struct {
        cleanup_failure: RuntimeCleanupFailure,
        process: ?ProcessObservation,
        retry_at_ns: i128,
    },
};

pub const RetainedCleanupBlock = struct {
    failure: RuntimeCleanupFailure,
    retry_at_ns: i128,
};

/// Cleanup ownership retained after a settled outcome, or document cleanup of
/// an awaiting-start editor with no child. This state is independent of any
/// immutable normal/failed outcome; retries cannot rewrite a lifecycle result.
pub const RetainedCleanup = union(enum) {
    inactive,
    blocked: RetainedCleanupBlock,
    complete,
    exhausted: RuntimeCleanupFailure,

    pub fn pending(self: RetainedCleanup) bool {
        return switch (self) {
            .blocked, .exhausted => true,
            .inactive, .complete => false,
        };
    }

    pub fn failure(self: RetainedCleanup) ?RuntimeCleanupFailure {
        return switch (self) {
            .blocked => |blocked| blocked.failure,
            .exhausted => |cleanup_failure| cleanup_failure,
            .inactive, .complete => null,
        };
    }

    pub fn retryAt(self: RetainedCleanup) ?i128 {
        return switch (self) {
            .blocked => |blocked| blocked.retry_at_ns,
            .inactive, .complete, .exhausted => null,
        };
    }

    pub fn block(
        self: *RetainedCleanup,
        failure_value: RuntimeCleanupFailure,
        retry_at_ns: i128,
    ) Error!TransitionOutcome {
        switch (self.*) {
            .inactive, .blocked => self.* = .{ .blocked = .{
                .failure = failure_value,
                .retry_at_ns = retry_at_ns,
            } },
            .complete, .exhausted => return error.InvalidPhase,
        }
        return .applied;
    }

    pub fn completeCleanup(self: *RetainedCleanup) Error!TransitionOutcome {
        switch (self.*) {
            .blocked => self.* = .complete,
            .complete => return .idempotent,
            .inactive, .exhausted => return error.InvalidPhase,
        }
        return .applied;
    }

    pub fn exhaust(self: *RetainedCleanup) Error!TransitionOutcome {
        switch (self.*) {
            .blocked => |blocked| self.* = .{ .exhausted = blocked.failure },
            .exhausted => return .idempotent,
            .inactive, .complete => return error.InvalidPhase,
        }
        return .applied;
    }
};

pub const OutcomePublication = enum {
    pending,
    published,
};

pub const RunningState = struct {
    termination: TerminationRequest = .none,
};

pub const AwaitingStartState = struct {};

pub const FinishingState = struct {
    first: CompletionSignal,
    termination: TerminationRequest,
};

pub const ContainingFailureState = struct {
    containment: FailureContainment,
    failure: RuntimeFailure,
};

pub const ExitedState = struct {
    final_revision: u64,
    publication: OutcomePublication = .pending,
    result: ExitResult,
    termination: TerminationRequest,
};

pub const FailedState = struct {
    failure: RuntimeFailure,
    final_revision: u64,
    process: ProcessObservation,
    publication: OutcomePublication = .pending,
};

pub const OutcomeKind = enum {
    exited,
    failed,
};

pub const PendingOutcome = union(enum) {
    exited: struct {
        final_revision: u64,
        result: ExitResult,
    },
    failed: struct {
        failure: RuntimeFailure,
        final_revision: u64,
        process: ProcessObservation,
    },
};

pub const Error = error{
    FailureContainmentIncomplete,
    InvalidPhase,
    InvalidProcessObservation,
    OutcomePublicationPending,
};
