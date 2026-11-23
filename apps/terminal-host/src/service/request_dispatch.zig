const std = @import("std");

const core_module = @import("terminal-host-core").state;
const creation_pipeline = @import("creation_pipeline.zig");
const host_history_requests = @import("history_requests.zig");
const host_input_requests = @import("input_requests.zig");
const host_lifecycle = @import("host_lifecycle.zig");
const host_outgoing = @import("outgoing.zig");
const host_projection_requests = @import("projection_requests.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const host_selection_requests = @import("selection_requests.zig");
const host_terminal_control_requests = @import("terminal_control_requests.zig");
const host_terminal_requests = @import("terminal_requests/root.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");

const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

pub const Limits = struct {
    input_bytes: usize,
    row_window_bytes: usize,
};

/// Routes one authorized post-handshake request to its semantic transaction
/// owner. It has no connection-lifecycle or asynchronous egress authority.
pub const Dispatcher = struct {
    context: host_request_context.Context,
    trace_interactions: bool,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        trace_interactions: bool,
        terminal_admission: host_lifecycle.TerminalAdmission,
        creations: *creation_pipeline.Pipeline,
    ) Dispatcher {
        return .{ .context = .{
            .allocator = allocator,
            .runtime_aggregate = runtime_aggregate,
            .terminal_admission = terminal_admission,
            .creations = creations,
        }, .trace_interactions = trace_interactions };
    }

    pub fn dispatch(
        self: *Dispatcher,
        request_contract: protocol.RequestMessageContract,
        connection_id: core_module.ConnectionId,
        limits: Limits,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        const success = try host_response.resolveSuccessResponse(request_contract);
        switch (request_contract.operation) {
            .hello => unreachable,
            .decode_graphics_png => try @import("graphics_decode.zig").dispatch(&self.context, connection_id, envelope, correlation_id, outgoing),
            .stage_terminal_output => try @import("output_staging.zig").dispatch(&self.context, connection_id, envelope, correlation_id, outgoing),
            .ping,
            .read_process_info,
            .list_terminals,
            .create_terminal,
            .start_terminal,
            .terminate_terminal,
            .force_terminate_terminal,
            .dispose_terminal,
            .attach_terminal,
            .claim_resize_lease,
            .detach_terminal,
            .update_attachment_input_capability,
            .set_terminal_defaults,
            .update_terminal_configuration,
            .apply_terminal_theme,
            => try host_terminal_requests.dispatch(
                &self.context,
                request_contract.operation,
                success,
                connection_id,
                envelope,
                correlation_id,
                outgoing,
            ),
            .write_input,
            .semantic_input,
            => try host_input_requests.dispatch(
                &self.context,
                request_contract.operation,
                success,
                connection_id,
                limits.input_bytes,
                envelope,
                correlation_id,
                outgoing,
            ),
            .resize_terminal,
            .clear_terminal,
            .reset_terminal,
            => try host_terminal_control_requests.dispatch(
                &self.context,
                request_contract.operation,
                success,
                connection_id,
                envelope,
                correlation_id,
                outgoing,
            ),
            .frame_applied,
            .need_snapshot,
            .set_projection_demand,
            => try host_projection_requests.dispatch(
                &self.context,
                request_contract.operation,
                success,
                connection_id,
                envelope,
                correlation_id,
                outgoing,
            ),
            .read_rows,
            .rows_applied,
            .set_history_demand,
            .history_observed,
            => try host_history_requests.dispatch(
                &self.context,
                request_contract.operation,
                success,
                connection_id,
                limits.row_window_bytes,
                envelope,
                correlation_id,
                outgoing,
            ),
            .selection_gesture,
            .selection_command,
            .selection_export,
            .selection_export_chunk_applied,
            .selection_export_cancel,
            => try host_selection_requests.dispatch(
                &self.context,
                request_contract.operation,
                success,
                connection_id,
                self.trace_interactions,
                envelope,
                correlation_id,
                outgoing,
            ),
            .attach_keeper,
            .detach_keeper,
            => if (self.context.keepers) |keepers| {
                try keepers.handle(&self.context, request_contract.operation, connection_id, envelope, correlation_id, outgoing);
            } else try self.notNegotiated(correlation_id, outgoing),
            .quiesce_terminal,
            .read_handover_record,
            .resume_terminal,
            .drop_terminal,
            .import_terminal,
            .write_handover_record,
            .restore_terminal,
            => if (self.context.moves) |moves| if (moves.negotiated()) {
                try moves.handle(&self.context, request_contract.operation, connection_id, envelope, correlation_id, outgoing);
            } else try self.notNegotiated(correlation_id, outgoing) else try self.notNegotiated(correlation_id, outgoing),
            // A restored import's claim and discard serve a move and a recovery alike.
            .claim_terminal,
            .discard_import,
            => if (self.context.moves) |moves| if (moves.negotiated() or moves.recoveryNegotiated()) {
                try moves.handle(&self.context, request_contract.operation, connection_id, envelope, correlation_id, outgoing);
            } else try self.notNegotiated(correlation_id, outgoing) else try self.notNegotiated(correlation_id, outgoing),
            .recover_terminal => if (self.context.moves) |moves| if (moves.recoveryNegotiated()) {
                try moves.handle(&self.context, request_contract.operation, connection_id, envelope, correlation_id, outgoing);
            } else try self.notNegotiated(correlation_id, outgoing) else try self.notNegotiated(correlation_id, outgoing),
            .start_program,
            .cancel_program,
            .read_program_output,
            .dispose_program,
            .list_programs,
            => if (self.context.programs) |programs| {
                try programs.handle(&self.context, request_contract.operation, connection_id, envelope, correlation_id, outgoing);
            } else try self.notNegotiated(correlation_id, outgoing),
            .recover_program => if (self.context.programs) |programs| if (self.context.moves) |moves| if (moves.recoveryNegotiated()) {
                try programs.handle(&self.context, request_contract.operation, connection_id, envelope, correlation_id, outgoing);
            } else try self.notNegotiated(correlation_id, outgoing) else try self.notNegotiated(correlation_id, outgoing) else try self.notNegotiated(correlation_id, outgoing),
        }
    }

    fn notNegotiated(self: *Dispatcher, correlation_id: WireIdentifier, outgoing: *Outgoing) !void {
        try self.context.addFailure(
            outgoing,
            correlation_id,
            "not-negotiated",
            "The connection did not negotiate this request's capability",
            false,
        );
    }
};
