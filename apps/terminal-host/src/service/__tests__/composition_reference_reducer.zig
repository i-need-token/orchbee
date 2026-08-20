//! Test-only reducer for the independent composition reference state.

const std = @import("std");

const core_module = @import("terminal-host-core").state;
const reference_state = @import("composition_reference_state.zig");
const semantic_trace = @import("composition_trace.zig");

const FirstCompletion = reference_state.FirstCompletion;
const Reference = reference_state.Reference;

pub fn apply(reference: *Reference, step: semantic_trace.Step) !void {
    if (reference.uncertain) return error.UnreconciledUnknownOutcome;
    switch (step.outcome) {
        .rejected => return,
        .unknown => {
            reference.uncertain = true;
            return;
        },
        .applied, .replayed => {},
    }
    switch (step.action) {
        .connect => |action| try applyConnect(reference, action, step.outcome),
        .disconnect => |connection_id| try applyDisconnect(reference, connection_id, step.outcome),
        .create => |action| try applyCreate(reference, action, step.outcome),
        .attach => |action| try applyAttach(reference, action, step.outcome),
        .set_projection_demand => |action| try applyDemand(reference, action, step.outcome),
        .accept_input => |action| try applyInput(reference, action, step.outcome),
        .set_focus => |action| try applyFocus(reference, action, step.outcome),
        .resize => |action| try applyResize(reference, action, step.outcome),
        .update_configuration => |action| try applyConfiguration(reference, action, step.outcome),
        .apply_terminal_control => |action| try applyTerminalControl(reference, action, step.outcome),
        .request_termination => |action| try applyTermination(reference, action, step.outcome),
        .observe_child_exit => |terminal_id| try applyCompletion(
            reference,
            terminal_id,
            .child_exited,
            step.outcome,
        ),
        .observe_pty_drain => |terminal_id| try applyCompletion(
            reference,
            terminal_id,
            .pty_drained,
            step.outcome,
        ),
        .observe_runtime_failure => |terminal_id| try applyRuntimeFailure(
            reference,
            terminal_id,
            step.outcome,
        ),
        .confirm_failure_containment => |action| try applyContainment(
            reference,
            action.terminal_id,
            action.process_observed,
            step.outcome,
        ),
        .publish_outcome => |terminal_id| try applyOutcomePublication(
            reference,
            terminal_id,
            step.outcome,
        ),
        .poll => try requireApplied(step.outcome),
        .detach => |attachment_id| try applyDetach(reference, attachment_id, step.outcome),
        .dispose => |terminal_id| try applyDispose(reference, terminal_id, step.outcome),
        .begin_shutdown => try applyBeginShutdown(reference, step.outcome),
        .reconcile => return error.InvalidActionState,
    }
}

fn applyConnect(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    switch (outcome) {
        .applied => switch (reference.connection) {
            .awaiting_hello, .absent => reference.connection = .{ .active = .{
                .client_id = action.client_id,
                .connection_id = action.connection_id,
                .limits = action.limits,
            } },
            .active, .closing => return error.InvalidAppliedOutcome,
        },
        .replayed => switch (reference.connection) {
            .active => |active| if (active.connection_id != action.connection_id or
                !active.client_id.eql(action.client_id)) return error.InvalidReplayOutcome,
            .absent, .awaiting_hello, .closing => return error.InvalidReplayOutcome,
        },
        .rejected, .unknown => unreachable,
    }
}

fn applyDisconnect(
    reference: *Reference,
    connection_id: core_module.ConnectionId,
    outcome: semantic_trace.Outcome,
) !void {
    switch (outcome) {
        .applied => switch (reference.connection) {
            .active => |active| if (active.connection_id != connection_id)
                return error.InvalidAppliedOutcome,
            .absent, .awaiting_hello, .closing => return error.InvalidAppliedOutcome,
        },
        .replayed => switch (reference.connection) {
            .active => return error.InvalidReplayOutcome,
            .absent, .awaiting_hello, .closing => return,
        },
        .rejected, .unknown => unreachable,
    }
    reference.attachments.clearRetainingCapacity();
    reference.connection = .awaiting_hello;
    reference.next_attachment_sequence = 1;
}

fn applyCreate(reference: *Reference, action: semantic_trace.Create, outcome: semantic_trace.Outcome) !void {
    if (reference.findTerminalByCreation(action.creation_id)) |existing| {
        if (outcome != .replayed) return error.InvalidAppliedOutcome;
        if (!existing.terminal_id.eql(action.terminal_id) or
            !std.mem.eql(u8, &existing.launch_digest, &action.launch_digest))
        {
            return error.ConflictingCreationReplay;
        }
        return;
    }
    if (outcome != .applied or !reference.hasActiveConnection() or
        reference.findTerminal(action.terminal_id) != null)
    {
        return error.InvalidAppliedOutcome;
    }
    try reference.terminals.append(reference.allocator, .{
        .child_exited = false,
        .configuration = action.configuration,
        .creation_id = action.creation_id,
        .first_completion = .none,
        .geometry = action.geometry,
        .launch_digest = action.launch_digest,
        .model_only = action.model_only,
        .pending_outcome = false,
        .phase = .running,
        .pty_drained = false,
        .terminal_id = action.terminal_id,
        .termination_request = .none,
    });
    reference.inventory_generation = std.math.add(u64, reference.inventory_generation, 1) catch
        return error.InvalidActionState;
}

fn applyAttach(reference: *Reference, action: semantic_trace.Attach, outcome: semantic_trace.Outcome) !void {
    if (reference.findAttachment(action.attachment_id)) |existing| {
        if (outcome != .replayed) return error.InvalidAppliedOutcome;
        if (!existing.terminal_id.eql(action.terminal_id) or
            existing.capabilities != action.capabilities or
            existing.request_resize_lease != action.request_resize_lease)
        {
            return error.ConflictingAttachmentReplay;
        }
        return;
    }
    const terminal = reference.findTerminal(action.terminal_id) orelse return error.InvalidActionState;
    if (outcome != .applied or !reference.hasActiveConnection() or terminal.phase != .running) {
        return error.InvalidAppliedOutcome;
    }
    if ((!action.capabilities.input and !action.capabilities.read and !action.capabilities.resize) or
        (action.request_resize_lease and !action.capabilities.resize) or
        (action.request_resize_lease and reference.hasResizeLease(action.terminal_id)))
    {
        return error.InvalidAppliedOutcome;
    }
    try reference.attachments.append(reference.allocator, .{
        .admission_sequence = reference.next_attachment_sequence,
        .admitted_capabilities = action.capabilities,
        .attachment_id = action.attachment_id,
        .capabilities = action.capabilities,
        .capability_revision = 1,
        .demand = .paused,
        .request_resize_lease = action.request_resize_lease,
        .resize_lease = action.request_resize_lease,
        .terminal_id = action.terminal_id,
    });
    reference.next_attachment_sequence = std.math.add(
        u64,
        reference.next_attachment_sequence,
        1,
    ) catch return error.InvalidActionState;
}

fn applyDemand(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    const attachment = reference.findAttachment(action.attachment_id) orelse
        return error.InvalidActionState;
    if (!attachment.capabilities.read) return error.InvalidActionState;
    if (outcome == .replayed) {
        if (attachment.demand != action.demand) return error.InvalidReplayOutcome;
        return;
    }
    if (outcome != .applied or attachment.demand == action.demand) {
        return error.InvalidAppliedOutcome;
    }
    attachment.demand = action.demand;
}

fn applyInput(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    if (outcome != .applied) return error.InvalidReplayOutcome;
    const attachment = reference.findAttachment(action.attachment_id) orelse
        return error.InvalidActionState;
    const terminal = reference.findTerminal(attachment.terminal_id) orelse
        return error.InvalidActionState;
    if (!reference.hasActiveConnection() or !attachment.capabilities.input or
        terminal.phase != .running or action.byte_count == 0)
    {
        return error.InvalidAppliedOutcome;
    }
}

fn applyFocus(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    if (outcome != .applied) return error.InvalidReplayOutcome;
    const attachment = reference.findAttachment(action.attachment_id) orelse
        return error.InvalidActionState;
    const terminal = reference.findTerminal(attachment.terminal_id) orelse
        return error.InvalidActionState;
    if (!reference.hasActiveConnection() or !attachment.capabilities.input or
        terminal.phase != .running or (action.focused and attachment.demand != .active))
    {
        return error.InvalidAppliedOutcome;
    }
}

fn applyResize(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    const attachment = reference.findAttachment(action.attachment_id) orelse
        return error.InvalidActionState;
    const terminal = reference.findTerminal(attachment.terminal_id) orelse
        return error.InvalidActionState;
    if (!attachment.capabilities.resize or !attachment.resize_lease or terminal.phase != .running) {
        return error.InvalidActionState;
    }
    if (outcome == .replayed) {
        if (!std.meta.eql(terminal.geometry, action.geometry)) return error.InvalidReplayOutcome;
        return;
    }
    if (outcome != .applied) return error.InvalidAppliedOutcome;
    terminal.geometry = action.geometry;
}

fn applyConfiguration(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    const terminal = reference.findTerminal(action.terminal_id) orelse return error.InvalidActionState;
    if (terminal.phase != .running and terminal.phase != .finishing) {
        return error.InvalidActionState;
    }
    if (outcome == .replayed) {
        if (!std.meta.eql(terminal.configuration, action.configuration)) {
            return error.InvalidReplayOutcome;
        }
        return;
    }
    if (outcome != .applied) return error.InvalidAppliedOutcome;
    terminal.configuration = action.configuration;
}

fn applyTerminalControl(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    _ = action.operation;
    try applyInput(reference, .{
        .attachment_id = action.attachment_id,
        .byte_count = 1,
    }, outcome);
}

fn applyTermination(reference: *Reference, action: anytype, outcome: semantic_trace.Outcome) !void {
    const terminal = reference.findTerminal(action.terminal_id) orelse return error.InvalidActionState;
    if (!reference.hasActiveConnection()) return error.InvalidActionState;
    if (terminal.phase != .running and terminal.phase != .finishing) {
        if (outcome != .replayed and outcome != .applied) return error.InvalidReplayOutcome;
        return;
    }
    const requested: core_module.TerminationRequest = switch (action.mode) {
        .graceful => .graceful,
        .force => .force,
    };
    const should_apply = switch (requested) {
        .none => unreachable,
        .graceful => terminal.termination_request == .none,
        .force => terminal.termination_request != .force,
    };
    if (should_apply != (outcome == .applied)) {
        return if (outcome == .replayed)
            error.InvalidReplayOutcome
        else
            error.InvalidAppliedOutcome;
    }
    if (should_apply) terminal.termination_request = requested;
}

fn applyCompletion(
    reference: *Reference,
    terminal_id: core_module.TerminalId,
    observed: FirstCompletion,
    outcome: semantic_trace.Outcome,
) !void {
    const terminal = reference.findTerminal(terminal_id) orelse return error.InvalidActionState;
    if (terminal.phase == .exited) {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    }
    if (terminal.phase != .running and terminal.phase != .finishing) {
        return error.InvalidActionState;
    }
    if (terminal.first_completion == observed) {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    }
    if (outcome != .applied) return error.InvalidAppliedOutcome;
    if (terminal.first_completion == .none) {
        terminal.first_completion = observed;
        terminal.phase = .finishing;
    } else {
        terminal.phase = .exited;
        terminal.pending_outcome = true;
    }
    if (observed == .child_exited) terminal.child_exited = true;
    if (observed == .pty_drained) terminal.pty_drained = true;
}

fn applyRuntimeFailure(
    reference: *Reference,
    terminal_id: core_module.TerminalId,
    outcome: semantic_trace.Outcome,
) !void {
    const terminal = reference.findTerminal(terminal_id) orelse return error.InvalidActionState;
    if (terminal.phase == .containing_failure or terminal.phase == .failed) {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    }
    if (terminal.phase != .running and terminal.phase != .finishing) {
        return error.InvalidActionState;
    }
    if (outcome != .applied) return error.InvalidAppliedOutcome;
    terminal.phase = .containing_failure;
    terminal.pty_drained = false;
    terminal.termination_request = .force;
}

fn applyContainment(
    reference: *Reference,
    terminal_id: core_module.TerminalId,
    process_observed: bool,
    outcome: semantic_trace.Outcome,
) !void {
    const terminal = reference.findTerminal(terminal_id) orelse return error.InvalidActionState;
    if (terminal.phase == .failed) {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    }
    if (terminal.phase != .containing_failure or outcome != .applied) {
        return error.InvalidAppliedOutcome;
    }
    terminal.phase = .failed;
    terminal.child_exited = process_observed;
    terminal.pty_drained = false;
    terminal.pending_outcome = true;
}

fn applyOutcomePublication(
    reference: *Reference,
    terminal_id: core_module.TerminalId,
    outcome: semantic_trace.Outcome,
) !void {
    const terminal = reference.findTerminal(terminal_id) orelse return error.InvalidActionState;
    if (terminal.phase != .exited and terminal.phase != .failed) return error.InvalidActionState;
    if (terminal.pending_outcome != (outcome == .applied)) {
        return if (outcome == .replayed)
            error.InvalidReplayOutcome
        else
            error.InvalidAppliedOutcome;
    }
    terminal.pending_outcome = false;
}

fn applyDetach(
    reference: *Reference,
    attachment_id: core_module.AttachmentId,
    outcome: semantic_trace.Outcome,
) !void {
    const index = reference.attachmentIndex(attachment_id) orelse {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    };
    if (outcome != .applied) return error.InvalidAppliedOutcome;
    const removed = reference.attachments.orderedRemove(index);
    const terminal = reference.findTerminal(removed.terminal_id);
    if (removed.resize_lease and terminal != null and terminal.?.phase == .running) {
        reference.promoteResizeLease(removed.terminal_id);
    }
}

fn applyDispose(
    reference: *Reference,
    terminal_id: core_module.TerminalId,
    outcome: semantic_trace.Outcome,
) !void {
    const index = reference.terminalIndex(terminal_id) orelse {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    };
    const terminal = reference.terminals.items[index];
    if (outcome != .applied or
        (terminal.phase != .exited and terminal.phase != .failed) or
        terminal.pending_outcome)
    {
        return error.InvalidAppliedOutcome;
    }
    reference.removeTerminalAttachments(terminal_id);
    _ = reference.terminals.orderedRemove(index);
    reference.inventory_generation = std.math.add(u64, reference.inventory_generation, 1) catch
        return error.InvalidActionState;
}

fn applyBeginShutdown(reference: *Reference, outcome: semantic_trace.Outcome) !void {
    if (reference.service_phase == .shutting_down) {
        if (outcome != .replayed) return error.InvalidReplayOutcome;
        return;
    }
    if (outcome != .applied) return error.InvalidAppliedOutcome;
    reference.service_phase = .shutting_down;
}

fn requireApplied(outcome: semantic_trace.Outcome) reference_state.RefinementError!void {
    if (outcome != .applied) return error.InvalidAppliedOutcome;
}
