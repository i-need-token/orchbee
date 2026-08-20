//! Test-only comparison between product observations and reference state.

const std = @import("std");

const core_module = @import("terminal-host-core").state;
const composition = @import("composition_observation.zig");
const reference_state = @import("composition_reference_state.zig");

const Attachment = reference_state.Attachment;
const Connection = reference_state.Connection;
const Reference = reference_state.Reference;
const RefinementError = reference_state.RefinementError;
const Terminal = reference_state.Terminal;

pub fn expectObservation(
    reference: *const Reference,
    observation: *const composition.Observation,
) RefinementError!void {
    if (reference.uncertain) return error.UnreconciledUnknownOutcome;
    if (!reference.host_epoch.eql(observation.host_epoch)) return error.HostEpochMismatch;
    if (reference.inventory_generation != observation.inventory_generation) {
        return error.InventoryGenerationMismatch;
    }
    if (!std.meta.eql(reference.terminal_defaults, observation.terminal_defaults)) {
        return error.TerminalDefaultsMismatch;
    }
    if (reference.service_phase != observation.service_phase) return error.ServicePhaseMismatch;
    try expectConnection(reference.connection, observation);
    if (reference.terminals.items.len != observation.terminals.len) {
        return error.TerminalCountMismatch;
    }
    for (reference.terminals.items) |expected| {
        const actual = findObservedTerminal(observation, expected.terminal_id) orelse
            return error.MissingTerminal;
        if (!sameTerminal(expected, actual)) return error.TerminalStateMismatch;
    }
    if (reference.attachments.items.len != observation.attachments.len) {
        return error.AttachmentCountMismatch;
    }
    for (reference.attachments.items) |expected| {
        const actual = findObservedAttachment(observation, expected.attachment_id) orelse
            return error.MissingAttachment;
        if (!sameAttachment(expected, actual)) return error.AttachmentStateMismatch;
    }
}

fn expectConnection(
    expected: Connection,
    observation: *const composition.Observation,
) RefinementError!void {
    switch (expected) {
        .absent => if (observation.connection != .absent) return error.ConnectionPhaseMismatch,
        .awaiting_hello => if (observation.connection != .awaiting_hello)
            return error.ConnectionPhaseMismatch,
        .closing => if (observation.connection != .closing) return error.ConnectionPhaseMismatch,
        .active => |active| switch (observation.connection) {
            .active => |actual| {
                if (actual.connection_id != active.connection_id or
                    !actual.client_id.eql(active.client_id) or
                    actual.frame_bytes != active.limits.frame_bytes or
                    actual.input_bytes != active.limits.input_bytes or
                    actual.row_window_bytes != active.limits.row_window_bytes)
                {
                    return error.ActiveConnectionMismatch;
                }
            },
            .absent, .awaiting_hello, .closing => return error.ConnectionPhaseMismatch,
        },
    }
    const expected_count: usize = switch (expected) {
        .active => 1,
        .absent, .awaiting_hello, .closing => 0,
    };
    if (observation.core_connection_count != expected_count) {
        return error.ConnectionCountMismatch;
    }
}

fn findObservedTerminal(
    observation: *const composition.Observation,
    terminal_id: core_module.TerminalId,
) ?composition.TerminalObservation {
    for (observation.terminals) |terminal| {
        if (terminal.terminal_id.eql(terminal_id)) return terminal;
    }
    return null;
}

fn findObservedAttachment(
    observation: *const composition.Observation,
    attachment_id: core_module.AttachmentId,
) ?composition.AttachmentObservation {
    for (observation.attachments) |attachment| {
        if (attachment.attachment_id.eql(attachment_id)) return attachment;
    }
    return null;
}

fn sameTerminal(expected: Terminal, actual: composition.TerminalObservation) bool {
    return expected.child_exited == actual.child_exited and
        std.meta.eql(expected.configuration, actual.configuration) and
        expected.creation_id.eql(actual.creation_id) and
        actual.geometry != null and std.meta.eql(expected.geometry, actual.geometry.?) and
        std.mem.eql(u8, &expected.launch_digest, &actual.launch_digest) and
        expected.model_only == actual.model_only and
        expected.pending_outcome == actual.pending_outcome and
        expected.phase == actual.phase and
        expected.pty_drained == actual.pty_drained and
        expected.terminal_id.eql(actual.terminal_id) and
        expected.termination_request == actual.termination_request;
}

fn sameAttachment(expected: Attachment, actual: composition.AttachmentObservation) bool {
    return expected.admission_sequence == actual.admission_sequence and
        expected.admitted_capabilities == actual.admitted_capabilities and
        expected.attachment_id.eql(actual.attachment_id) and
        expected.capabilities == actual.capabilities and
        expected.capability_revision == actual.capability_revision and
        expected.demand == actual.projection.demand() and
        expected.request_resize_lease == actual.requested_resize_lease and
        expected.resize_lease == actual.resize_lease and
        expected.terminal_id.eql(actual.terminal_id);
}
