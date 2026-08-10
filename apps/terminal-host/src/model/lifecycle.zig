const std = @import("std");

pub const Phase = enum {
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

    pub fn kind(self: RuntimeCleanupFailure) RuntimeFailureKind {
        return switch (self) {
            .contain_process => .process_control,
            .release_output => .pty_read,
            .release_process_identity => .process_observation,
        };
    }

    pub fn stage(self: RuntimeCleanupFailure) RuntimeFailureStage {
        return switch (self) {
            .contain_process => .contain_process,
            .release_output => .release_output,
            .release_process_identity => .release_process_identity,
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

pub const OutcomePublication = enum {
    pending,
    published,
};

pub const RunningState = struct {
    termination: TerminationRequest = .none,
};

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

pub const Lifecycle = union(enum) {
    running: RunningState,
    finishing: FinishingState,
    containing_failure: ContainingFailureState,
    exited: ExitedState,
    failed: FailedState,

    pub fn phase(self: Lifecycle) Phase {
        return switch (self) {
            .running => .running,
            .finishing => .finishing,
            .containing_failure => .containing_failure,
            .exited => .exited,
            .failed => .failed,
        };
    }

    pub fn childExited(self: Lifecycle) bool {
        return switch (self) {
            .running => false,
            .finishing => |state| switch (state.first) {
                .child_exited => true,
                .pty_drained => false,
            },
            .containing_failure => |state| processWasObserved(switch (state.containment) {
                .stopping => |containment| containment.process,
                .blocked => |containment| containment.process,
            }),
            .exited => true,
            .failed => |state| processWasObserved(state.process),
        };
    }

    pub fn ptyDrained(self: Lifecycle) bool {
        return switch (self) {
            .running, .containing_failure, .failed => false,
            .finishing => |state| switch (state.first) {
                .child_exited => false,
                .pty_drained => true,
            },
            .exited => true,
        };
    }

    pub fn exitResult(self: Lifecycle) ?ExitResult {
        return switch (self) {
            .finishing => |state| switch (state.first) {
                .child_exited => |result| result,
                .pty_drained => null,
            },
            .exited => |state| state.result,
            .running, .containing_failure, .failed => null,
        };
    }

    pub fn terminationRequest(self: Lifecycle) TerminationRequest {
        return switch (self) {
            .running => |state| state.termination,
            .finishing => |state| state.termination,
            .containing_failure, .failed => .force,
            .exited => |state| state.termination,
        };
    }

    pub fn runtimeFailure(self: Lifecycle) ?RuntimeFailure {
        return switch (self) {
            .containing_failure => |state| state.failure,
            .failed => |state| state.failure,
            .running, .finishing, .exited => null,
        };
    }

    pub fn failureContainment(self: Lifecycle) ?FailureContainment {
        return switch (self) {
            .containing_failure => |state| state.containment,
            .running, .finishing, .exited, .failed => null,
        };
    }

    pub fn failedProcess(self: Lifecycle) ?ProcessObservation {
        return switch (self) {
            .failed => |state| state.process,
            .running, .finishing, .containing_failure, .exited => null,
        };
    }

    pub fn pendingOutcome(self: Lifecycle) ?PendingOutcome {
        return switch (self) {
            .exited => |state| if (state.publication == .pending)
                .{ .exited = .{
                    .final_revision = state.final_revision,
                    .result = state.result,
                } }
            else
                null,
            .failed => |state| if (state.publication == .pending)
                .{ .failed = .{
                    .failure = state.failure,
                    .final_revision = state.final_revision,
                    .process = state.process,
                } }
            else
                null,
            .running, .finishing, .containing_failure => null,
        };
    }

    pub fn settledFinalRevision(self: Lifecycle) ?u64 {
        return switch (self) {
            .exited => |state| state.final_revision,
            .failed => |state| state.final_revision,
            .running, .finishing, .containing_failure => null,
        };
    }

    pub fn acceptsRuntimeWork(self: Lifecycle) bool {
        return self.phase() == .running;
    }

    pub fn acceptsConfigurationUpdate(self: Lifecycle) bool {
        return switch (self.phase()) {
            .running, .finishing => true,
            .containing_failure, .exited, .failed => false,
        };
    }

    pub fn hasReadableRuntimeState(self: Lifecycle) bool {
        return switch (self) {
            .running, .finishing, .exited => true,
            .failed => |state| state.failure.kind() != .resize,
            .containing_failure => false,
        };
    }

    pub fn prepareTermination(
        self: Lifecycle,
        request: TerminationRequest,
    ) TerminationAdmission {
        return switch (self) {
            .running => |state| terminationAdmission(state.termination, request),
            .finishing => |state| terminationAdmission(state.termination, request),
            .containing_failure => |state| switch (request) {
                .none => unreachable,
                .graceful => .replay,
                .force => switch (state.containment) {
                    .stopping => .replay,
                    .blocked => .containment_retry,
                },
            },
            .exited, .failed => .replay,
        };
    }

    pub fn commitPreparedTermination(
        self: *Lifecycle,
        request: TerminationRequest,
    ) void {
        std.debug.assert(request != .none);
        switch (self.*) {
            .running => |*state| commitTermination(&state.termination, request),
            .finishing => |*state| commitTermination(&state.termination, request),
            .containing_failure, .exited, .failed => unreachable,
        }
    }

    pub fn commitContainmentRetry(self: *Lifecycle) void {
        switch (self.*) {
            .containing_failure => |*state| switch (state.containment) {
                .blocked => |blocked| {
                    state.containment = .{ .stopping = .{ .process = blocked.process } };
                },
                .stopping => unreachable,
            },
            .running, .finishing, .exited, .failed => unreachable,
        }
    }

    pub fn childExit(
        self: *Lifecycle,
        result: ExitResult,
        final_revision: u64,
    ) Error!TransitionOutcome {
        if (!validExitResult(result)) return error.InvalidProcessObservation;
        switch (self.*) {
            .running => |state| self.* = .{
                .finishing = .{
                    .first = .{ .child_exited = result },
                    .termination = state.termination,
                },
            },
            .finishing => |state| switch (state.first) {
                .child_exited => |retained| return if (sameExitResult(retained, result))
                    .idempotent
                else
                    error.InvalidPhase,
                .pty_drained => self.* = .{ .exited = .{
                    .final_revision = final_revision,
                    .result = result,
                    .termination = state.termination,
                } },
            },
            .containing_failure => |*state| switch (state.containment) {
                .stopping => |*containment| return recordContainedChildExit(containment, result),
                .blocked => |*containment| return recordContainedChildExit(containment, result),
            },
            .exited => |state| return if (sameExitResult(state.result, result))
                .idempotent
            else
                error.InvalidPhase,
            .failed => |state| return switch (state.process) {
                .observed => |observed| if (sameExitResult(observed, result))
                    .idempotent
                else
                    error.InvalidPhase,
                .unavailable => .idempotent,
            },
        }
        return .applied;
    }

    pub fn ptyDrain(self: *Lifecycle, final_revision: u64) TransitionOutcome {
        switch (self.*) {
            .running => |state| self.* = .{
                .finishing = .{
                    .first = .pty_drained,
                    .termination = state.termination,
                },
            },
            .finishing => |state| switch (state.first) {
                .child_exited => |result| self.* = .{ .exited = .{
                    .final_revision = final_revision,
                    .result = result,
                    .termination = state.termination,
                } },
                .pty_drained => return .idempotent,
            },
            .containing_failure, .exited, .failed => return .idempotent,
        }
        return .applied;
    }

    pub fn runtimeFailed(
        self: *Lifecycle,
        failure: RuntimeFailure,
    ) TransitionOutcome {
        const process: ?ProcessObservation = switch (self.*) {
            .running => null,
            .finishing => |state| switch (state.first) {
                .child_exited => |result| .{ .observed = result },
                .pty_drained => null,
            },
            .containing_failure, .exited, .failed => return .idempotent,
        };
        self.* = .{ .containing_failure = .{
            .containment = .{ .stopping = .{ .process = process } },
            .failure = failure,
        } };
        return .applied;
    }

    pub fn containmentBlocked(
        self: *Lifecycle,
        cleanup_failure: RuntimeCleanupFailure,
        retry_at_ns: i128,
    ) Error!TransitionOutcome {
        switch (self.*) {
            .containing_failure => |*state| switch (state.containment) {
                .stopping => |stopping| {
                    state.containment = .{ .blocked = .{
                        .cleanup_failure = cleanup_failure,
                        .process = stopping.process,
                        .retry_at_ns = retry_at_ns,
                    } };
                    return .applied;
                },
                .blocked => |*blocked| {
                    blocked.cleanup_failure = cleanup_failure;
                    blocked.retry_at_ns = retry_at_ns;
                    return .applied;
                },
            },
            .failed => return .idempotent,
            .running, .finishing, .exited => return error.InvalidPhase,
        }
    }

    pub fn containmentCompleted(
        self: *Lifecycle,
        observation: ProcessObservation,
        final_revision: u64,
    ) Error!TransitionOutcome {
        if (!validProcessObservation(observation)) return error.InvalidProcessObservation;
        switch (self.*) {
            .containing_failure => |state| {
                const retained_process = switch (state.containment) {
                    .stopping => |containment| containment.process,
                    .blocked => |containment| containment.process,
                };
                self.* = .{ .failed = .{
                    .failure = state.failure,
                    .final_revision = final_revision,
                    .process = strongestProcessObservation(retained_process, observation),
                } };
                return .applied;
            },
            .failed => return .idempotent,
            .running, .finishing, .exited => return error.InvalidPhase,
        }
    }

    pub fn requireDisposable(self: Lifecycle) Error!DisposableAdmission {
        return switch (self) {
            .containing_failure => |state| switch (state.containment) {
                .stopping => .containment_stopping,
                .blocked => |blocked| .{ .cleanup_blocked = blocked.cleanup_failure },
            },
            .exited => |state| if (state.publication == .pending)
                error.OutcomePublicationPending
            else
                .dispose,
            .failed => |state| if (state.publication == .pending)
                error.OutcomePublicationPending
            else
                .dispose,
            .running, .finishing => error.InvalidPhase,
        };
    }

    pub fn cleanupRetryAt(self: Lifecycle) ?i128 {
        return switch (self) {
            .containing_failure => |state| switch (state.containment) {
                .blocked => |blocked| blocked.retry_at_ns,
                .stopping => null,
            },
            .running, .finishing, .exited, .failed => null,
        };
    }

    pub fn needsContainmentWork(self: Lifecycle) bool {
        return switch (self) {
            .containing_failure => |state| switch (state.containment) {
                .stopping => true,
                .blocked => false,
            },
            .running, .finishing, .exited, .failed => false,
        };
    }

    pub fn markOutcomePublished(
        self: *Lifecycle,
        kind: OutcomeKind,
    ) TransitionOutcome {
        switch (kind) {
            .failed => switch (self.*) {
                .failed => |*state| {
                    if (state.publication == .published) return .idempotent;
                    state.publication = .published;
                    return .applied;
                },
                else => return .idempotent,
            },
            .exited => switch (self.*) {
                .exited => |*state| {
                    if (state.publication == .published) return .idempotent;
                    state.publication = .published;
                    return .applied;
                },
                else => return .idempotent,
            },
        }
    }
};

fn terminationAdmission(
    current: TerminationRequest,
    request: TerminationRequest,
) TerminationAdmission {
    return switch (request) {
        .none => unreachable,
        .graceful => if (current == .none) .dispatch else .replay,
        .force => if (current == .force) .replay else .dispatch,
    };
}

fn commitTermination(current: *TerminationRequest, request: TerminationRequest) void {
    switch (request) {
        .none => unreachable,
        .graceful => std.debug.assert(current.* == .none),
        .force => std.debug.assert(current.* != .force),
    }
    current.* = request;
}

fn recordContainedChildExit(
    containment: anytype,
    result: ExitResult,
) Error!TransitionOutcome {
    if (containment.process) |retained| switch (retained) {
        .observed => |observed| {
            if (sameExitResult(observed, result)) return .idempotent;
            return error.InvalidPhase;
        },
        .unavailable => {},
    };
    containment.process = .{ .observed = result };
    return .applied;
}

fn sameExitResult(left: ExitResult, right: ExitResult) bool {
    return switch (left) {
        .exit_code => |left_code| switch (right) {
            .exit_code => |right_code| left_code == right_code,
            .signal, .unknown => false,
        },
        .signal => |left_signal| switch (right) {
            .signal => |right_signal| left_signal == right_signal,
            .exit_code, .unknown => false,
        },
        .unknown => |left_unknown| switch (right) {
            .unknown => |right_unknown| left_unknown == right_unknown,
            .exit_code, .signal => false,
        },
    };
}

fn processWasObserved(observation: ?ProcessObservation) bool {
    const retained = observation orelse return false;
    return switch (retained) {
        .observed => true,
        .unavailable => false,
    };
}

fn validExitResult(result: ExitResult) bool {
    return switch (result) {
        .signal => |signal| signal != 0,
        .exit_code, .unknown => true,
    };
}

fn validProcessObservation(observation: ProcessObservation) bool {
    return switch (observation) {
        .observed => |result| validExitResult(result),
        .unavailable => true,
    };
}

fn strongestProcessObservation(
    retained: ?ProcessObservation,
    next: ProcessObservation,
) ProcessObservation {
    const current = retained orelse return next;
    return switch (current) {
        .observed => current,
        .unavailable => next,
    };
}
