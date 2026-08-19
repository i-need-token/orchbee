//! Independent abstract state used by composition refinement tests.

const std = @import("std");

const core_module = @import("terminal-host-core").state;
const model = @import("terminal-host-model");
const composition = @import("composition_observation.zig");
const semantic_trace = @import("composition_trace.zig");

pub const RefinementError = error{
    ActiveConnectionMismatch,
    AttachmentCountMismatch,
    AttachmentStateMismatch,
    ConflictingCreationReplay,
    ConflictingAttachmentReplay,
    ConnectionCountMismatch,
    ConnectionPhaseMismatch,
    HostEpochMismatch,
    InvalidActionState,
    InvalidAppliedOutcome,
    InvalidReplayOutcome,
    InventoryGenerationMismatch,
    MissingAttachment,
    MissingTerminal,
    ServicePhaseMismatch,
    TerminalCountMismatch,
    TerminalDefaultsMismatch,
    TerminalStateMismatch,
    UnreconciledUnknownOutcome,
};

pub const Connection = union(enum) {
    absent,
    awaiting_hello,
    active: struct {
        client_id: core_module.ClientId,
        connection_id: core_module.ConnectionId,
        limits: semantic_trace.ConnectionLimits,
    },
    closing,
};

pub const FirstCompletion = enum {
    none,
    child_exited,
    pty_drained,
    unknown,
};

pub const Terminal = struct {
    child_exited: bool,
    configuration: model.configuration.Configuration,
    creation_id: core_module.CreationId,
    first_completion: FirstCompletion,
    geometry: model.geometry.Geometry,
    launch_digest: core_module.LaunchDigest,
    model_only: bool,
    pending_outcome: bool,
    phase: core_module.TerminalPhase,
    pty_drained: bool,
    terminal_id: core_module.TerminalId,
    termination_request: core_module.TerminationRequest,
};

pub const Attachment = struct {
    admission_sequence: u64,
    attachment_id: core_module.AttachmentId,
    capabilities: core_module.Capabilities,
    demand: core_module.ProjectionDemand,
    request_resize_lease: bool,
    resize_lease: bool,
    terminal_id: core_module.TerminalId,
};

/// Abstract state contains only facts determined by semantic actions. It owns
/// no production pointers, runtime resources, queues, revisions, or locks.
pub const Reference = struct {
    allocator: std.mem.Allocator,
    attachments: std.ArrayList(Attachment) = .empty,
    connection: Connection,
    host_epoch: core_module.HostEpoch,
    inventory_generation: u64,
    next_attachment_sequence: u64,
    service_phase: composition.ServicePhase,
    terminal_defaults: model.configuration.Configuration,
    terminals: std.ArrayList(Terminal) = .empty,
    uncertain: bool = false,

    pub fn init(
        allocator: std.mem.Allocator,
        observation: *const composition.Observation,
    ) std.mem.Allocator.Error!Reference {
        var reference: Reference = .{
            .allocator = allocator,
            .connection = connectionFromObservation(observation.connection),
            .host_epoch = observation.host_epoch,
            .inventory_generation = observation.inventory_generation,
            .next_attachment_sequence = 1,
            .service_phase = observation.service_phase,
            .terminal_defaults = observation.terminal_defaults,
        };
        errdefer reference.deinit();
        for (observation.terminals) |terminal| {
            try reference.terminals.append(allocator, terminalFromObservation(terminal));
        }
        for (observation.attachments) |attachment| {
            try reference.attachments.append(allocator, attachmentFromObservation(attachment));
            reference.next_attachment_sequence = @max(
                reference.next_attachment_sequence,
                attachment.admission_sequence +| 1,
            );
        }
        return reference;
    }

    pub fn deinit(self: *Reference) void {
        self.attachments.deinit(self.allocator);
        self.terminals.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn reconcile(
        self: *Reference,
        observation: *const composition.Observation,
    ) std.mem.Allocator.Error!void {
        const replacement = try Reference.init(self.allocator, observation);
        self.deinit();
        self.* = replacement;
    }

    pub fn hasActiveConnection(self: *const Reference) bool {
        return switch (self.connection) {
            .active => true,
            .absent, .awaiting_hello, .closing => false,
        };
    }

    pub fn findTerminal(self: *Reference, terminal_id: core_module.TerminalId) ?*Terminal {
        for (self.terminals.items) |*terminal| {
            if (terminal.terminal_id.eql(terminal_id)) return terminal;
        }
        return null;
    }

    pub fn findTerminalByCreation(
        self: *Reference,
        creation_id: core_module.CreationId,
    ) ?*Terminal {
        for (self.terminals.items) |*terminal| {
            if (terminal.creation_id.eql(creation_id)) return terminal;
        }
        return null;
    }

    pub fn findAttachment(
        self: *Reference,
        attachment_id: core_module.AttachmentId,
    ) ?*Attachment {
        for (self.attachments.items) |*attachment| {
            if (attachment.attachment_id.eql(attachment_id)) return attachment;
        }
        return null;
    }

    pub fn terminalIndex(
        self: *const Reference,
        terminal_id: core_module.TerminalId,
    ) ?usize {
        for (self.terminals.items, 0..) |terminal, index| {
            if (terminal.terminal_id.eql(terminal_id)) return index;
        }
        return null;
    }

    pub fn attachmentIndex(
        self: *const Reference,
        attachment_id: core_module.AttachmentId,
    ) ?usize {
        for (self.attachments.items, 0..) |attachment, index| {
            if (attachment.attachment_id.eql(attachment_id)) return index;
        }
        return null;
    }

    pub fn hasResizeLease(
        self: *const Reference,
        terminal_id: core_module.TerminalId,
    ) bool {
        for (self.attachments.items) |attachment| {
            if (attachment.terminal_id.eql(terminal_id) and attachment.resize_lease) return true;
        }
        return false;
    }

    pub fn promoteResizeLease(self: *Reference, terminal_id: core_module.TerminalId) void {
        var successor: ?*Attachment = null;
        for (self.attachments.items) |*attachment| {
            if (!attachment.terminal_id.eql(terminal_id) or !attachment.capabilities.resize) continue;
            if (successor == null or
                attachment.admission_sequence < successor.?.admission_sequence)
            {
                successor = attachment;
            }
        }
        if (successor) |attachment| attachment.resize_lease = true;
    }

    pub fn removeTerminalAttachments(
        self: *Reference,
        terminal_id: core_module.TerminalId,
    ) void {
        var index: usize = self.attachments.items.len;
        while (index > 0) {
            index -= 1;
            if (self.attachments.items[index].terminal_id.eql(terminal_id)) {
                _ = self.attachments.orderedRemove(index);
            }
        }
    }
};

fn connectionFromObservation(observation: composition.ConnectionObservation) Connection {
    return switch (observation) {
        .absent => .absent,
        .awaiting_hello => .awaiting_hello,
        .closing => .closing,
        .active => |active| .{ .active = .{
            .client_id = active.client_id,
            .connection_id = active.connection_id,
            .limits = .{
                .frame_bytes = active.frame_bytes,
                .input_bytes = active.input_bytes,
                .row_window_bytes = active.row_window_bytes,
            },
        } },
    };
}

fn terminalFromObservation(observation: composition.TerminalObservation) Terminal {
    const first_completion: FirstCompletion = if (observation.phase != .finishing)
        .none
    else if (observation.child_exited)
        .child_exited
    else if (observation.pty_drained)
        .pty_drained
    else
        .unknown;
    return .{
        .child_exited = observation.child_exited,
        .configuration = observation.configuration,
        .creation_id = observation.creation_id,
        .first_completion = first_completion,
        .geometry = observation.geometry.?,
        .launch_digest = observation.launch_digest,
        .model_only = observation.model_only,
        .pending_outcome = observation.pending_outcome,
        .phase = observation.phase,
        .pty_drained = observation.pty_drained,
        .terminal_id = observation.terminal_id,
        .termination_request = observation.termination_request,
    };
}

fn attachmentFromObservation(observation: composition.AttachmentObservation) Attachment {
    return .{
        .admission_sequence = observation.admission_sequence,
        .attachment_id = observation.attachment_id,
        .capabilities = observation.capabilities,
        .demand = observation.projection.demand(),
        .request_resize_lease = observation.requested_resize_lease,
        .resize_lease = observation.resize_lease,
        .terminal_id = observation.terminal_id,
    };
}
