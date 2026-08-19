const std = @import("std");
const termination = @import("termination.zig");
const types = @import("types.zig");

const Phase = types.Phase;
const TransitionOutcome = types.TransitionOutcome;
const TerminationAdmission = types.TerminationAdmission;
const DisposableAdmission = types.DisposableAdmission;
const ExitResult = types.ExitResult;
const CompletionSignal = types.CompletionSignal;
const TerminationRequest = types.TerminationRequest;
const RuntimeFailureKind = types.RuntimeFailureKind;
const RuntimeFailureStage = types.RuntimeFailureStage;
const RuntimeFailure = types.RuntimeFailure;
const RuntimeCleanupFailure = types.RuntimeCleanupFailure;
const ProcessObservationFailure = types.ProcessObservationFailure;
const ProcessObservation = types.ProcessObservation;
const FailureContainment = types.FailureContainment;
const RetainedCleanupBlock = types.RetainedCleanupBlock;
const RetainedCleanup = types.RetainedCleanup;
const OutcomePublication = types.OutcomePublication;
const RunningState = types.RunningState;
const AwaitingStartState = types.AwaitingStartState;
const FinishingState = types.FinishingState;
const ContainingFailureState = types.ContainingFailureState;
const ExitedState = types.ExitedState;
const FailedState = types.FailedState;
const OutcomeKind = types.OutcomeKind;
const PendingOutcome = types.PendingOutcome;
const Error = types.Error;

pub const Lifecycle = union(enum) {
    awaiting_start: AwaitingStartState,
    running: RunningState,
    finishing: FinishingState,
    containing_failure: ContainingFailureState,
    exited: ExitedState,
    failed: FailedState,

    pub fn phase(self: Lifecycle) Phase {
        return switch (self) {
            .awaiting_start => .awaiting_start,
            .running => .running,
            .finishing => .finishing,
            .containing_failure => .containing_failure,
            .exited => .exited,
            .failed => .failed,
        };
    }

    pub fn childExited(self: Lifecycle) bool {
        return switch (self) {
            .awaiting_start => false,
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
            .awaiting_start, .running, .containing_failure, .failed => false,
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
            .awaiting_start, .running, .containing_failure, .failed => null,
        };
    }

    pub fn terminationRequest(self: Lifecycle) TerminationRequest {
        return switch (self) {
            .running => |state| state.termination,
            .finishing => |state| state.termination,
            .awaiting_start => .none,
            .containing_failure, .failed => .force,
            .exited => |state| state.termination,
        };
    }

    pub fn runtimeFailure(self: Lifecycle) ?RuntimeFailure {
        return switch (self) {
            .containing_failure => |state| state.failure,
            .failed => |state| state.failure,
            .awaiting_start, .running, .finishing, .exited => null,
        };
    }

    pub fn failureContainment(self: Lifecycle) ?FailureContainment {
        return switch (self) {
            .containing_failure => |state| state.containment,
            .awaiting_start, .running, .finishing, .exited, .failed => null,
        };
    }

    pub fn failedProcess(self: Lifecycle) ?ProcessObservation {
        return switch (self) {
            .failed => |state| state.process,
            .awaiting_start, .running, .finishing, .containing_failure, .exited => null,
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
            .awaiting_start, .running, .finishing, .containing_failure => null,
        };
    }

    pub fn settledFinalRevision(self: Lifecycle) ?u64 {
        return switch (self) {
            .exited => |state| state.final_revision,
            .failed => |state| state.final_revision,
            .awaiting_start, .running, .finishing, .containing_failure => null,
        };
    }

    pub fn acceptsRuntimeWork(self: Lifecycle) bool {
        return self.phase() == .running;
    }

    pub fn acceptsConfigurationUpdate(self: Lifecycle) bool {
        return switch (self.phase()) {
            .running, .finishing => true,
            .awaiting_start, .containing_failure, .exited, .failed => false,
        };
    }

    pub fn hasReadableRuntimeState(self: Lifecycle) bool {
        return switch (self) {
            .awaiting_start => false,
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
            .awaiting_start => .replay,
            .running => |state| termination.admission(state.termination, request),
            .finishing => |state| termination.admission(state.termination, request),
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
            .awaiting_start => unreachable,
            .running => |*state| termination.commit(&state.termination, request),
            .finishing => |*state| termination.commit(&state.termination, request),
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
            .awaiting_start, .running, .finishing, .exited, .failed => unreachable,
        }
    }

    pub fn start(self: *Lifecycle) Error!TransitionOutcome {
        switch (self.*) {
            .awaiting_start => self.* = .{ .running = .{} },
            .running => return .idempotent,
            .finishing, .containing_failure, .exited, .failed => return error.InvalidPhase,
        }
        return .applied;
    }

    pub fn childExit(
        self: *Lifecycle,
        result: ExitResult,
        final_revision: u64,
    ) Error!TransitionOutcome {
        if (!validExitResult(result)) return error.InvalidProcessObservation;
        switch (self.*) {
            .awaiting_start => return error.InvalidPhase,
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
            .awaiting_start => return .idempotent,
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
            .awaiting_start => return .idempotent,
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
            .awaiting_start, .running, .finishing, .exited => return error.InvalidPhase,
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
            .awaiting_start, .running, .finishing, .exited => return error.InvalidPhase,
        }
    }

    pub fn requireDisposable(self: Lifecycle) Error!DisposableAdmission {
        return switch (self) {
            .awaiting_start => .dispose,
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
            .awaiting_start, .running, .finishing, .exited, .failed => null,
        };
    }

    pub fn needsContainmentWork(self: Lifecycle) bool {
        return switch (self) {
            .containing_failure => |state| switch (state.containment) {
                .stopping => true,
                .blocked => false,
            },
            .awaiting_start, .running, .finishing, .exited, .failed => false,
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
